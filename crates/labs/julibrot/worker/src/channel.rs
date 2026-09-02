//! Same-thread lowering of the bounded ownership-transfer channel.

use std::cell::RefCell;
use std::rc::Rc;

use crate::slots::{FourSlotModel, SlotId};
use crate::wire::{HEADER_BYTES, ORBIT_RECORD_BYTES, Pool, WireBuffer};
use crate::{
    ChannelError, ErrorCode, MessageHeader, MessageKind, OrbitDisposition, OrbitRequest,
    ReferenceOrbitRecord,
};

/// Minimum app-requestable orbit length.
pub const MIN_MAX_ITER: u32 = 64;
/// Fixed owner buffer-return deadline in microseconds.
pub const BUFFER_RETURN_DEADLINE_US: u32 = 4_000_000;
/// Fixed displayed orbit budget in microseconds per second.
pub const ORBIT_BUDGET_US_PER_SECOND: u32 = 250_000;

/// Startup transport lowering.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum WorkerMode {
    /// Browser Web Worker with transferable standalone buffers.
    WebWorker = 0,
    /// Same-thread bounded queues with identical logical ownership.
    SameThread = 1,
}

/// Fixed channel allocation configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerConfig {
    /// Current maximum orbit length and per-buffer record capacity.
    pub max_iter: u32,
}

/// Immediate disposition of a submitted latest-wins request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SubmitOutcome {
    /// A request buffer moved immediately to the producer queue.
    Transferred,
    /// The single pending request was installed or replaced.
    Coalesced,
    /// No later generation can be represented without wrapping.
    GenerationExhausted,
}

/// Factory for paired owner and producer endpoints.
#[derive(Clone, Copy, Debug, Default)]
pub struct WorkerChannel;

impl WorkerChannel {
    /// Allocates exactly two buffers per direction and returns paired endpoints.
    ///
    /// # Errors
    ///
    /// Returns `BadLength` for a cap below 64 or unrepresentable capacity, or a typed allocation
    /// identity refusal if one of the four initial trailers cannot be made.
    pub fn new(
        config: WorkerConfig,
        mode: WorkerMode,
    ) -> Result<(OwnerEndpoint, ProducerEndpoint), ChannelError> {
        if config.max_iter < MIN_MAX_ITER {
            return Err(ChannelError::new(
                ErrorCode::BadLength,
                config.max_iter,
                MIN_MAX_ITER,
                config.max_iter,
            ));
        }
        let mut request_main = BoundedQueue::new();
        let mut orbit_producer = BoundedQueue::new();
        for slot in 0..=1 {
            request_main
                .push(WireBuffer::new(Pool::Request, slot, config.max_iter)?)
                .map_err(|_| ChannelError::new(ErrorCode::BufferStarved, slot, 0, 0))?;
            orbit_producer
                .push(WireBuffer::new(Pool::Orbit, slot, config.max_iter)?)
                .map_err(|_| ChannelError::new(ErrorCode::BufferStarved, slot, 0, 0))?;
        }
        let core = Rc::new(RefCell::new(ChannelCore {
            config,
            mode,
            slots: FourSlotModel::new(),
            request_main,
            request_to_producer: BoundedQueue::new(),
            orbit_producer,
            orbit_to_main: BoundedQueue::new(),
            pending_request: None,
            latest_generation: 0,
            latest_centre_revision: 0,
            last_error: None,
            closed: false,
        }));
        Ok((
            OwnerEndpoint {
                core: Rc::clone(&core),
            },
            ProducerEndpoint { core },
        ))
    }
}

/// Main-thread side of the channel.
#[derive(Debug)]
pub struct OwnerEndpoint {
    core: Rc<RefCell<ChannelCore>>,
}

impl OwnerEndpoint {
    /// Accepts every newer edit immediately and keeps at most one untransferred request.
    pub fn submit(&self, request: OrbitRequest) -> SubmitOutcome {
        let mut core = self.core.borrow_mut();
        if core.closed || core.latest_generation == u32::MAX {
            return SubmitOutcome::GenerationExhausted;
        }
        if request.generation() <= core.latest_generation {
            return SubmitOutcome::Coalesced;
        }
        core.latest_generation = request.generation();
        core.latest_centre_revision = request.centre().revision;
        match core.try_dispatch(request) {
            Ok(None) => SubmitOutcome::Transferred,
            Ok(Some(pending)) => {
                core.pending_request = Some(pending);
                SubmitOutcome::Coalesced
            }
            Err(error) => {
                core.last_error = Some(error);
                SubmitOutcome::Coalesced
            }
        }
    }

    /// Returns the next completed response without blocking.
    #[must_use]
    pub fn next_arrival(&self) -> Option<OrbitResponseView> {
        let mut core = self.core.borrow_mut();
        let buffer = core.orbit_to_main.pop()?;
        let header = match buffer.header() {
            Ok(header) if header.validate() == Ok(MessageKind::OrbitResponse) => header,
            Ok(header) => {
                core.last_error = Some(ChannelError::new(
                    ErrorCode::BadKind,
                    header.kind,
                    0,
                    0,
                ));
                return None;
            }
            Err(error) => {
                core.last_error = Some(error);
                return None;
            }
        };
        let centre_revision = if header.generation == core.latest_generation {
            core.latest_centre_revision
        } else {
            0
        };
        Some(OrbitResponseView {
            generation: header.generation,
            centre_revision,
            length: header.length,
            compute_us: header.compute_us,
            precision_bits: header.precision_bits,
            admission_credit_us: header.credit_us,
            records: OrbitLease {
                core: Rc::clone(&self.core),
                buffer: Some(buffer),
            },
        })
    }

    /// Returns and clears the latest typed internal channel refusal.
    #[must_use]
    pub fn take_error(&self) -> Option<ChannelError> {
        self.core.borrow_mut().last_error.take()
    }

    /// Reports the latest submitted generation.
    #[must_use]
    pub fn latest_generation(&self) -> u32 {
        self.core.borrow().latest_generation
    }

    /// Reports one coalesced request when producer delivery is saturated.
    #[must_use]
    pub fn pending_request_depth(&self) -> u32 {
        u32::from(self.core.borrow().pending_request.is_some())
    }
}

/// Producer side of the channel.
#[derive(Debug)]
pub struct ProducerEndpoint {
    core: Rc<RefCell<ChannelCore>>,
}

impl ProducerEndpoint {
    /// Takes the next delivered request without blocking.
    ///
    /// # Errors
    ///
    /// Returns a typed wire refusal if the delivered request was corrupted.
    pub fn next_request(&self) -> Result<Option<RequestLease>, ChannelError> {
        let mut core = self.core.borrow_mut();
        let Some(buffer) = core.request_to_producer.pop() else {
            return Ok(None);
        };
        let request = OrbitRequest::decode(&buffer)?;
        Ok(Some(RequestLease { request, buffer }))
    }

    /// Returns the request slot and transfers one completed orbit buffer to main.
    ///
    /// # Errors
    ///
    /// Returns a typed ownership, capacity, or queue refusal; the request slot is returned before
    /// an absent orbit buffer is reported.
    pub fn complete(
        &self,
        lease: RequestLease,
        records: &[ReferenceOrbitRecord],
        delivered_precision_bits: u32,
        compute_us: u32,
        admission_credit_us: u32,
    ) -> Result<(), ChannelError> {
        let generation = lease.request.generation();
        let mut core = self.core.borrow_mut();
        core.return_request(lease.buffer, generation, MessageKind::RequestReturn)?;
        let mut orbit = core
            .orbit_producer
            .pop()
            .ok_or_else(|| ChannelError::new(ErrorCode::BufferStarved, 0, 0, 0))?;
        orbit.write_orbit(
            generation,
            delivered_precision_bits,
            compute_us,
            admission_credit_us,
            records,
        )?;
        core.send_to_main(orbit, MessageKind::OrbitResponse)
    }

    /// Reports this endpoint's configured lowering.
    #[must_use]
    pub fn mode(&self) -> WorkerMode {
        self.core.borrow().mode
    }
}

/// Producer-owned request buffer paired with its decoded semantic request.
#[derive(Debug)]
pub struct RequestLease {
    request: OrbitRequest,
    buffer: WireBuffer,
}

impl RequestLease {
    /// Borrows the validated semantic request.
    #[must_use]
    pub const fn request(&self) -> &OrbitRequest {
        &self.request
    }
}

/// Main-thread view of a completed reference response.
#[derive(Debug)]
pub struct OrbitResponseView {
    generation: u32,
    centre_revision: u32,
    length: u32,
    compute_us: u32,
    precision_bits: u32,
    admission_credit_us: u32,
    /// Exclusive ownership of transferred record bytes until credit return.
    pub records: OrbitLease,
}

impl OrbitResponseView {
    /// Returns the orbit generation.
    #[must_use]
    pub const fn generation(&self) -> u32 {
        self.generation
    }

    /// Returns the authoritative-centre revision associated with a latest response.
    #[must_use]
    pub const fn centre_revision(&self) -> u32 {
        self.centre_revision
    }

    /// Returns stored orbit-entry count.
    #[must_use]
    pub const fn length(&self) -> u32 {
        self.length
    }

    /// Returns measured worker compute wall in microseconds.
    #[must_use]
    pub const fn compute_us(&self) -> u32 {
        self.compute_us
    }

    /// Returns measured worker compute wall converted to milliseconds for display.
    #[must_use]
    pub fn compute_ms(&self) -> f64 {
        f64::from(self.compute_us) / 1_000.0
    }

    /// Returns delivered bignum precision.
    #[must_use]
    pub const fn precision_bits(&self) -> u32 {
        self.precision_bits
    }

    /// Returns producer-projected admission credit.
    #[must_use]
    pub const fn admission_credit_us(&self) -> u32 {
        self.admission_credit_us
    }
}

/// Exclusive main-side ownership of transferred orbit bytes.
#[derive(Debug)]
pub struct OrbitLease {
    core: Rc<RefCell<ChannelCore>>,
    buffer: Option<WireBuffer>,
}

impl OrbitLease {
    /// Borrows exactly the initialized reference-record payload bytes.
    ///
    /// # Errors
    ///
    /// Returns `BufferStarved` after credit was already returned, or a typed wire refusal if the
    /// owned buffer no longer contains a valid orbit response.
    pub fn record_bytes(&self) -> Result<&[u8], ChannelError> {
        let buffer = self
            .buffer
            .as_ref()
            .ok_or_else(|| ChannelError::new(ErrorCode::BufferStarved, 0, 0, 0))?;
        let header = buffer.header()?;
        buffer.validate_message()?;
        let length = usize::try_from(header.length)
            .map_err(|_| ChannelError::new(ErrorCode::BadLength, header.length, 0, 0))?;
        let end = HEADER_BYTES + length * ORBIT_RECORD_BYTES;
        Ok(&buffer.as_bytes()[HEADER_BYTES..end])
    }

    /// Rewrites the CREDIT header and returns this buffer exactly once.
    ///
    /// # Errors
    ///
    /// Returns `BufferStarved` on a second return or a typed ownership refusal if pool state does
    /// not name main as the current owner.
    pub fn return_credit(
        mut self,
        disposition: OrbitDisposition,
        credit_us: u32,
    ) -> Result<(), ChannelError> {
        let mut buffer = self
            .buffer
            .take()
            .ok_or_else(|| ChannelError::new(ErrorCode::BufferStarved, 0, 0, 0))?;
        let old = buffer.header()?;
        let kind = match disposition {
            OrbitDisposition::Applied => MessageKind::CreditApplied,
            OrbitDisposition::Stale => MessageKind::CreditStale,
        };
        let mut header = MessageHeader::new(kind, old.generation);
        header.precision_bits = old.precision_bits;
        header.compute_us = old.compute_us;
        header.credit_us = credit_us;
        buffer.write_header(header)?;
        self.core.borrow_mut().return_to_producer(buffer, kind)
    }
}

#[derive(Debug)]
struct ChannelCore {
    config: WorkerConfig,
    mode: WorkerMode,
    slots: FourSlotModel,
    request_main: BoundedQueue<WireBuffer>,
    request_to_producer: BoundedQueue<WireBuffer>,
    orbit_producer: BoundedQueue<WireBuffer>,
    orbit_to_main: BoundedQueue<WireBuffer>,
    pending_request: Option<OrbitRequest>,
    latest_generation: u32,
    latest_centre_revision: u32,
    last_error: Option<ChannelError>,
    closed: bool,
}

impl ChannelCore {
    fn try_dispatch(
        &mut self,
        request: OrbitRequest,
    ) -> Result<Option<OrbitRequest>, ChannelError> {
        if request.max_iter() != self.config.max_iter {
            if !self.is_reconciled() {
                return Ok(Some(request));
            }
            self.resize(request.max_iter())?;
        }
        if self.request_to_producer.is_full() {
            return Ok(Some(request));
        }
        let Some(mut buffer) = self.request_main.pop() else {
            return Ok(Some(request));
        };
        request.encode_into(&mut buffer)?;
        let id = id_for(&buffer)?;
        self.slots.begin(id, MessageKind::OrbitRequest)?;
        self.slots.deliver(id)?;
        self.request_to_producer
            .push(buffer)
            .map_err(|_| ChannelError::new(ErrorCode::BufferStarved, id.slot, 0, 0))?;
        Ok(None)
    }

    fn return_request(
        &mut self,
        mut buffer: WireBuffer,
        generation: u32,
        kind: MessageKind,
    ) -> Result<(), ChannelError> {
        buffer.write_header(MessageHeader::new(kind, generation))?;
        let id = id_for(&buffer)?;
        self.slots.begin(id, kind)?;
        self.slots.deliver(id)?;
        self.request_main
            .push(buffer)
            .map_err(|_| ChannelError::new(ErrorCode::BufferStarved, id.slot, 0, 0))?;
        self.pump_pending();
        Ok(())
    }

    fn send_to_main(
        &mut self,
        buffer: WireBuffer,
        kind: MessageKind,
    ) -> Result<(), ChannelError> {
        let id = id_for(&buffer)?;
        self.slots.begin(id, kind)?;
        self.slots.deliver(id)?;
        self.orbit_to_main
            .push(buffer)
            .map_err(|_| ChannelError::new(ErrorCode::BufferStarved, id.slot, 0, 0))
    }

    fn return_to_producer(
        &mut self,
        buffer: WireBuffer,
        kind: MessageKind,
    ) -> Result<(), ChannelError> {
        let id = id_for(&buffer)?;
        self.slots.begin(id, kind)?;
        self.slots.deliver(id)?;
        self.orbit_producer
            .push(buffer)
            .map_err(|_| ChannelError::new(ErrorCode::BufferStarved, id.slot, 0, 0))?;
        self.pump_pending();
        Ok(())
    }

    fn pump_pending(&mut self) {
        let Some(request) = self.pending_request.take() else {
            return;
        };
        match self.try_dispatch(request) {
            Ok(None) => {}
            Ok(Some(request)) => self.pending_request = Some(request),
            Err(error) => self.last_error = Some(error),
        }
    }

    fn is_reconciled(&self) -> bool {
        self.slots.is_reconciled()
            && self.request_main.len() == 2
            && self.orbit_producer.len() == 2
            && self.request_to_producer.is_empty()
            && self.orbit_to_main.is_empty()
    }

    fn resize(&mut self, max_iter: u32) -> Result<(), ChannelError> {
        if max_iter < MIN_MAX_ITER {
            return Err(ChannelError::new(
                ErrorCode::BadLength,
                max_iter,
                MIN_MAX_ITER,
                max_iter,
            ));
        }
        let mut request_main = BoundedQueue::new();
        let mut orbit_producer = BoundedQueue::new();
        for slot in 0..=1 {
            request_main
                .push(WireBuffer::new(Pool::Request, slot, max_iter)?)
                .map_err(|_| ChannelError::new(ErrorCode::BufferStarved, slot, 0, 0))?;
            orbit_producer
                .push(WireBuffer::new(Pool::Orbit, slot, max_iter)?)
                .map_err(|_| ChannelError::new(ErrorCode::BufferStarved, slot, 0, 0))?;
        }
        self.request_main = request_main;
        self.orbit_producer = orbit_producer;
        self.config.max_iter = max_iter;
        Ok(())
    }
}

fn id_for(buffer: &WireBuffer) -> Result<SlotId, ChannelError> {
    let (pool, slot) = buffer.identity()?;
    SlotId::new(pool, slot)
}

#[derive(Debug)]
struct BoundedQueue<T> {
    entries: [Option<T>; 2],
}

impl<T> BoundedQueue<T> {
    const fn new() -> Self {
        Self {
            entries: [None, None],
        }
    }

    fn push(&mut self, value: T) -> Result<(), T> {
        let Some(slot) = self.entries.iter_mut().find(|entry| entry.is_none()) else {
            return Err(value);
        };
        *slot = Some(value);
        Ok(())
    }

    fn pop(&mut self) -> Option<T> {
        let value = self.entries[0].take();
        if value.is_some() {
            self.entries[0] = self.entries[1].take();
        }
        value
    }

    fn len(&self) -> usize {
        self.entries.iter().filter(|entry| entry.is_some()).count()
    }

    fn is_empty(&self) -> bool {
        self.entries.iter().all(Option::is_none)
    }

    fn is_full(&self) -> bool {
        self.entries.iter().all(Option::is_some)
    }
}

#[cfg(test)]
mod tests {
    use super::{SubmitOutcome, WorkerChannel, WorkerConfig, WorkerMode};
    use crate::{
        CoordinateDescriptor, EncodedCentre, OrbitDisposition, OrbitReason, OrbitRequest,
        ReferenceOrbitRecord,
    };

    fn request(generation: u32, revision: u32) -> OrbitRequest {
        OrbitRequest::new(
            generation,
            EncodedCentre {
                revision,
                coordinates: [CoordinateDescriptor::default(); 4],
                limbs: Vec::new(),
            },
            0,
            64,
            64,
            OrbitReason::INITIAL,
        )
        .unwrap()
    }

    #[test]
    fn two_transfers_then_one_latest_pending_request() {
        let (owner, producer) = WorkerChannel::new(
            WorkerConfig { max_iter: 64 },
            WorkerMode::SameThread,
        )
        .unwrap();
        assert_eq!(owner.submit(request(1, 1)), SubmitOutcome::Transferred);
        assert_eq!(owner.submit(request(2, 2)), SubmitOutcome::Transferred);
        assert_eq!(owner.submit(request(3, 3)), SubmitOutcome::Coalesced);
        assert_eq!(owner.submit(request(4, 4)), SubmitOutcome::Coalesced);
        assert_eq!(owner.pending_request_depth(), 1);

        let lease = producer.next_request().unwrap().unwrap();
        assert_eq!(lease.request().generation(), 1);
        producer
            .complete(
                lease,
                &[ReferenceOrbitRecord::default()],
                64,
                10,
                250_000,
            )
            .unwrap();
        let response = owner.next_arrival().unwrap();
        assert_eq!(response.generation(), 1);
        assert_eq!(response.records.record_bytes().unwrap().len(), 16);
        response
            .records
            .return_credit(OrbitDisposition::Stale, 249_990)
            .unwrap();

        let second = producer.next_request().unwrap().unwrap();
        assert_eq!(second.request().generation(), 2);
        producer
            .complete(
                second,
                &[ReferenceOrbitRecord::default()],
                64,
                10,
                249_990,
            )
            .unwrap();
        assert_eq!(producer.next_request().unwrap().unwrap().request().generation(), 4);
    }

    #[test]
    fn stale_generation_never_replaces_latest_generation() {
        let (owner, producer) = WorkerChannel::new(
            WorkerConfig { max_iter: 64 },
            WorkerMode::SameThread,
        )
        .unwrap();
        assert_eq!(owner.submit(request(9, 1)), SubmitOutcome::Transferred);
        assert_eq!(owner.submit(request(8, 2)), SubmitOutcome::Coalesced);
        assert_eq!(owner.latest_generation(), 9);
        assert_eq!(producer.mode(), WorkerMode::SameThread);
    }
}
