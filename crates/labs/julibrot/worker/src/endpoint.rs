//! Transport-agnostic main-thread owner over the four pool buffers.
//!
//! The browser lowering supplies a Web Worker port whose slots are transferable `ArrayBuffer`
//! objects; the native tests supply an in-process port over the same wire layout. Both drive the
//! identical resize handshake, four-slot reconciliation, credit accounting, and drain deadline, so
//! a max-iteration change is proved without a browser.
//!
//! The endpoint owns no timer. Every owner-side interaction — submit, arrival, credit return,
//! facts, and each delivered message — advances an armed drain, which is how the deadline is
//! observed in an event-driven page.

#![allow(
    clippy::redundant_pub_crate,
    reason = "the private module publishes its port seam to the browser and channel modules"
)]

use crate::{
    BUFFER_RETURN_DEADLINE_US, ChannelError, CreditAccount, ErrorCode, JULIBROT_ABI_VERSION,
    MIN_MAX_ITER, MessageHeader, MessageKind, OrbitDisposition, OrbitRequest, Pool, SubmitOutcome,
    WorkerConfig, WorkerFacts, WorkerMode,
};

/// A fixed-capacity FIFO whose `clear` drops occupied values in physical slot order.
struct TwoSlotQueue<T> {
    slots: [Option<T>; 2],
    head: usize,
    len: usize,
}

impl<T> TwoSlotQueue<T> {
    const fn new() -> Self {
        Self {
            slots: [None, None],
            head: 0,
            len: 0,
        }
    }

    const fn len(&self) -> usize {
        self.len
    }

    const fn is_empty(&self) -> bool {
        self.len == 0
    }

    fn push_back(&mut self, value: T) -> Result<(), T> {
        if self.len == 2 {
            return Err(value);
        }
        let tail = (self.head + self.len) & 1;
        self.slots[tail] = Some(value);
        self.len += 1;
        Ok(())
    }

    const fn pop_front(&mut self) -> Option<T> {
        if self.is_empty() {
            return None;
        }
        let value = self.slots[self.head].take();
        self.head = (self.head + 1) & 1;
        self.len -= 1;
        value
    }

    fn clear(&mut self) {
        self.slots = [None, None];
        self.head = 0;
        self.len = 0;
    }
}

/// One owner-held pool buffer with the exact wire operations the owner performs.
pub(crate) trait OwnerSlot: Sized {
    /// Reads the immutable pool and slot identity.
    fn identity(&self) -> Result<(Pool, u32), ChannelError>;

    /// Reads and validates the message header.
    fn header(&self) -> Result<MessageHeader, ChannelError>;

    /// Validates pool, kind, count, and kind-owned unused capacity.
    fn validate_message(&self) -> Result<MessageKind, ChannelError>;

    /// Writes one canonical header under the message kind's payload-ownership rule.
    fn write_header(&self, header: MessageHeader) -> Result<(), ChannelError>;

    /// Writes one canonical request body into a request-pool slot.
    fn encode_request(&self, request: &OrbitRequest) -> Result<(), ChannelError>;

    /// Reads the typed body of a validated channel-error message.
    fn channel_error(&self) -> Result<ChannelError, ChannelError>;

    /// Writes a payload-free message of the named kind.
    fn write_empty(
        &self,
        kind: MessageKind,
        generation: u32,
        compute_us: u32,
        credit_us: u32,
    ) -> Result<(), ChannelError> {
        let mut header = MessageHeader::new(kind, generation);
        header.compute_us = compute_us;
        header.credit_us = credit_us;
        self.write_header(header)
    }
}

/// The owner's transport: allocation, ownership transfer, producer restart, and monotonic time.
pub(crate) trait OwnerPort {
    /// Transferred pool buffer.
    type Slot: OwnerSlot;

    /// Allocates one trailer-bearing pool buffer sized for the orbit and minimum request wall.
    fn allocate(&self, pool: Pool, slot: u32, max_iter: u32) -> Result<Self::Slot, ChannelError>;

    /// Transfers one slot to the producer, detaching this side.
    fn post(&mut self, slot: Self::Slot) -> Result<(), ChannelError>;

    /// Sends the object-handshake ABI probe.
    fn probe_abi(&mut self) -> Result<(), ChannelError>;

    /// Stops the current producer and starts a fresh one from the cached module artifact.
    fn restart_producer(&mut self) -> Result<(), ChannelError>;

    /// Stops the producer without replacing it after a closing drain becomes terminal.
    fn terminate_producer(&mut self);

    /// Reads the owner-side monotonic clock in microseconds.
    fn now_us(&self) -> Result<u64, ChannelError>;
}

/// One decoded producer object-handshake message.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ControlMessage {
    /// The producer bootstrap loaded and reported its ABI version.
    ProducerReady(u32),
    /// The producer accepted the owner's ABI probe.
    AbiAccepted(u32),
    /// The producer refused the ABI or reported a transport failure.
    Refused,
}

/// One armed reconciliation of all four slots.
#[derive(Clone, Copy, Debug)]
struct Drain {
    /// Capacity to install after reconciliation, or `None` for a closing drain.
    resize: Option<u32>,
    /// Owner time at which this drain was armed.
    armed_us: u64,
    /// Whether the `Shutdown` slot was transferred.
    sent: bool,
    /// Whether `ShutdownAck` was received.
    acknowledged: bool,
}

/// Main-thread endpoint state independent of transport lowering.
pub(crate) struct OwnerCore<P: OwnerPort> {
    port: P,
    config: WorkerConfig,
    request_owned: Vec<P::Slot>,
    orbit_owned: Vec<P::Slot>,
    arrivals: TwoSlotQueue<P::Slot>,
    pending_request: Option<OrbitRequest>,
    latest_generation: u32,
    latest_centre_revision: u32,
    last_error: Option<ChannelError>,
    credit: CreditAccount,
    facts: WorkerFacts,
    orbit_leases: u32,
    pool_epoch: u32,
    ready: bool,
    closed: bool,
    reconciled: bool,
    drain: Option<Drain>,
}

impl<P: OwnerPort> OwnerCore<P> {
    /// Allocates the startup pool and returns an endpoint that has not yet probed the producer.
    pub(crate) fn new(port: P, config: WorkerConfig) -> Result<Self, ChannelError> {
        if config.max_iter < MIN_MAX_ITER {
            return Err(ChannelError::new(
                ErrorCode::BadLength,
                config.max_iter,
                MIN_MAX_ITER,
                config.max_iter,
            ));
        }
        crate::buffer_capacity(config.max_iter)?;
        let mut core = Self {
            port,
            config,
            request_owned: Vec::with_capacity(2),
            orbit_owned: Vec::with_capacity(2),
            arrivals: TwoSlotQueue::new(),
            pending_request: None,
            latest_generation: 0,
            latest_centre_revision: 0,
            last_error: None,
            credit: CreditAccount::new(),
            facts: WorkerFacts::new(WorkerMode::WebWorker),
            orbit_leases: 0,
            pool_epoch: 0,
            ready: false,
            closed: false,
            reconciled: false,
            drain: None,
        };
        core.allocate_pool(config.max_iter)?;
        core.refresh_facts();
        Ok(core)
    }

    /// Borrows the transport mutably for lowering-specific work.
    pub(crate) const fn port_mut(&mut self) -> &mut P {
        &mut self.port
    }

    /// Sends the startup ABI probe.
    pub(crate) fn probe_abi(&mut self) -> Result<(), ChannelError> {
        self.port.probe_abi()
    }

    /// Accepts every newer edit and keeps at most one request pending while the port is busy.
    pub(crate) fn submit(&mut self, request: OrbitRequest) -> SubmitOutcome {
        self.advance();
        if self.closed || self.latest_generation == u32::MAX {
            return SubmitOutcome::GenerationExhausted;
        }
        if request.generation() <= self.latest_generation {
            return SubmitOutcome::Coalesced;
        }
        let requested_cap = request.max_iter();
        self.latest_generation = request.generation();
        self.latest_centre_revision = request.centre().revision;
        if requested_cap < MIN_MAX_ITER {
            self.last_error = Some(ChannelError::new(
                ErrorCode::BadLength,
                requested_cap,
                MIN_MAX_ITER,
                requested_cap,
            ));
            self.bump_facts();
            return SubmitOutcome::Coalesced;
        }
        self.pending_request = Some(request);
        if requested_cap > self.config.max_iter {
            self.arm_resize(requested_cap);
        }
        let transferred = self.pump_request();
        self.bump_facts();
        self.refresh_facts();
        if transferred {
            SubmitOutcome::Transferred
        } else {
            SubmitOutcome::Coalesced
        }
    }

    /// Takes the next arrival together with the centre revision it belongs to.
    pub(crate) fn take_arrival(&mut self) -> Option<(P::Slot, u32)> {
        self.advance();
        let slot = self.arrivals.pop_front()?;
        let header = match slot.header() {
            Ok(header) => header,
            Err(error) => {
                self.last_error = Some(error);
                return None;
            }
        };
        let centre_revision = if header.generation == self.latest_generation {
            self.latest_centre_revision
        } else {
            0
        };
        self.orbit_leases = self.orbit_leases.saturating_add(1);
        self.bump_facts();
        self.refresh_facts();
        Some((slot, centre_revision))
    }

    /// Returns one leased orbit slot with owner credit accounting.
    ///
    /// A slot leased before a pool replacement belongs to no live pool: it is charged and dropped
    /// instead of transferred, so a superseded buffer never reaches the restarted producer.
    pub(crate) fn return_slot(
        &mut self,
        slot: P::Slot,
        lease_epoch: u32,
        disposition: OrbitDisposition,
        owner_now_us: u64,
    ) -> Result<(), ChannelError> {
        let old = slot.header()?;
        let kind = match disposition {
            OrbitDisposition::Applied => MessageKind::CreditApplied,
            OrbitDisposition::Stale => MessageKind::CreditStale,
        };
        let charge = self.credit.charge(owner_now_us, old.compute_us)?;
        let mut header = MessageHeader::new(kind, old.generation);
        header.precision_bits = old.precision_bits;
        header.compute_us = old.compute_us;
        header.credit_us = charge.credit_us;
        slot.write_header(header)?;
        if lease_epoch == self.pool_epoch {
            self.port.post(slot)?;
        } else {
            drop(slot);
        }
        self.record_credit(old, disposition, charge.overfeed_us);
        self.orbit_leases = self.orbit_leases.saturating_sub(1);
        self.advance();
        self.bump_facts();
        self.refresh_facts();
        Ok(())
    }

    /// Accepts one transferred pool buffer from the producer.
    pub(crate) fn receive_slot(&mut self, slot: P::Slot) -> Result<(), ChannelError> {
        let kind = slot.validate_message()?;
        let (pool, _) = slot.identity()?;
        match (pool, kind) {
            (Pool::Request, MessageKind::RequestReturn) => {
                push_unique(&mut self.request_owned, slot)?;
            }
            (Pool::Orbit, MessageKind::OrbitResponse | MessageKind::OrbitCancelled) => {
                if self.closed {
                    self.return_stale_slot(slot)?;
                } else if self.arrivals.len() == 2 {
                    return Err(ChannelError::new(ErrorCode::BufferStarved, 2, 0, 0));
                } else {
                    self.arrivals
                        .push_back(slot)
                        .map_err(|_| ChannelError::new(ErrorCode::BufferStarved, 2, 0, 0))?;
                }
            }
            (Pool::Orbit, MessageKind::CreditStale) => {
                push_unique(&mut self.orbit_owned, slot)?;
            }
            (Pool::Request, MessageKind::ShutdownAck) => {
                push_unique(&mut self.request_owned, slot)?;
                if let Some(drain) = self.drain.as_mut() {
                    drain.acknowledged = true;
                }
            }
            (Pool::Orbit, MessageKind::ChannelError) => {
                let error = slot.channel_error()?;
                self.return_stale_slot(slot)?;
                self.last_error = Some(error);
            }
            (_, _) => {
                return Err(ChannelError::new(
                    ErrorCode::BadKind,
                    slot.header()?.kind,
                    0,
                    0,
                ));
            }
        }
        self.advance();
        self.pump_request();
        self.bump_facts();
        self.refresh_facts();
        Ok(())
    }

    /// Accepts one producer object-handshake message.
    pub(crate) fn receive_control(&mut self, message: ControlMessage) -> Result<(), ChannelError> {
        match message {
            ControlMessage::ProducerReady(version) => {
                if version != JULIBROT_ABI_VERSION {
                    return Err(ChannelError::new(ErrorCode::BadVersion, version, 0, 0));
                }
                self.port.probe_abi()
            }
            ControlMessage::AbiAccepted(version) => {
                if version != JULIBROT_ABI_VERSION {
                    return Err(ChannelError::new(ErrorCode::BadVersion, version, 0, 0));
                }
                self.ready = true;
                while let Some(slot) = self.orbit_owned.pop() {
                    self.port.post(slot)?;
                }
                self.advance();
                self.pump_request();
                self.bump_facts();
                self.refresh_facts();
                Ok(())
            }
            ControlMessage::Refused => Err(ChannelError::new(ErrorCode::BadVersion, 0, 0, 0)),
        }
    }

    /// Begins the closing drain; completion is reported by `shutdown_acknowledged`.
    pub(crate) fn shutdown(&mut self) -> Result<(), ChannelError> {
        self.closed = true;
        self.pending_request = None;
        self.drain = Some(Drain {
            resize: None,
            armed_us: self.armed_now(),
            sent: self.drain.is_some_and(|drain| drain.sent),
            acknowledged: self.drain.is_some_and(|drain| drain.acknowledged),
        });
        self.return_queued_arrivals()?;
        self.advance();
        self.bump_facts();
        self.refresh_facts();
        Ok(())
    }

    /// Reports whether the producer returned all four slots and acknowledged the closing drain.
    pub(crate) const fn shutdown_acknowledged(&self) -> bool {
        self.reconciled
    }

    /// Returns and clears the latest typed refusal.
    pub(crate) const fn take_error(&mut self) -> Option<ChannelError> {
        self.last_error.take()
    }

    /// Records one typed refusal raised by the transport lowering.
    pub(crate) const fn publish_error(&mut self, error: ChannelError) {
        self.last_error = Some(error);
    }

    /// Reports the latest accepted request generation.
    pub(crate) const fn latest_generation(&self) -> u32 {
        self.latest_generation
    }

    /// Reports the single latest-wins pending request.
    pub(crate) fn pending_request_depth(&mut self) -> u32 {
        self.advance();
        u32::from(self.pending_request.is_some())
    }

    /// Reports the current pool generation carried by every live lease.
    pub(crate) const fn pool_epoch(&self) -> u32 {
        self.pool_epoch
    }

    /// Returns one coherent page-visible accounting snapshot.
    pub(crate) fn facts(&mut self) -> WorkerFacts {
        self.advance();
        self.refresh_facts();
        self.facts
    }

    /// Advances an armed drain, restarting the producer once all four slots are home.
    fn advance(&mut self) {
        if let Err(error) = self.advance_drain() {
            self.last_error = Some(error);
        }
    }

    fn advance_drain(&mut self) -> Result<(), ChannelError> {
        let Some(drain) = self.drain else {
            return Ok(());
        };
        if !drain.sent {
            self.send_shutdown()?;
        }
        let home = self.four_slots_home();
        let Some(drain) = self.drain.as_mut() else {
            return Ok(());
        };
        let acknowledged = drain.acknowledged && home;
        self.reconciled = acknowledged;
        let resize = drain.resize;
        if acknowledged {
            return match resize {
                Some(max_iter) => self.restart_pool(max_iter),
                None => {
                    self.finish_close();
                    Ok(())
                }
            };
        }
        let armed_us = drain.armed_us;
        let now_us = self.port.now_us()?;
        if now_us.saturating_sub(armed_us) < u64::from(BUFFER_RETURN_DEADLINE_US) {
            return Ok(());
        }
        let missing = self.first_missing_slot();
        match resize {
            Some(max_iter) => self.restart_pool(max_iter)?,
            None => self.finish_close(),
        }
        Err(ChannelError::new(
            ErrorCode::BufferStarved,
            missing.0 as u32,
            missing.1,
            0,
        ))
    }

    fn send_shutdown(&mut self) -> Result<(), ChannelError> {
        if !self.ready {
            return Ok(());
        }
        let Some(slot) = self.request_owned.pop() else {
            return Ok(());
        };
        slot.write_empty(MessageKind::Shutdown, self.latest_generation, 0, 0)?;
        self.port.post(slot)?;
        if let Some(drain) = self.drain.as_mut() {
            drain.sent = true;
        }
        Ok(())
    }

    fn arm_resize(&mut self, max_iter: u32) {
        if !self.ready && self.drain.is_none() && self.four_slots_home() {
            if let Err(error) = self.replace_pool(max_iter) {
                self.last_error = Some(error);
            }
            return;
        }
        match self.drain.as_mut() {
            Some(drain) => {
                let target = drain.resize.unwrap_or(self.config.max_iter);
                drain.resize = Some(target.max(max_iter));
            }
            None => {
                self.drain = Some(Drain {
                    resize: Some(max_iter),
                    armed_us: self.armed_now(),
                    sent: false,
                    acknowledged: false,
                });
            }
        }
        self.advance();
    }

    fn armed_now(&mut self) -> u64 {
        match self.port.now_us() {
            Ok(now_us) => now_us,
            Err(error) => {
                self.last_error = Some(error);
                0
            }
        }
    }

    const fn four_slots_home(&self) -> bool {
        self.request_owned.len() == 2
            && self.orbit_owned.len() == 2
            && self.arrivals.is_empty()
            && self.orbit_leases == 0
    }

    fn first_missing_slot(&self) -> (Pool, u32) {
        let present = |owned: &Vec<P::Slot>, slot: u32| {
            owned
                .iter()
                .any(|held| held.identity().is_ok_and(|(_, held)| held == slot))
        };
        for slot in 0..=1 {
            if !present(&self.orbit_owned, slot) {
                return (Pool::Orbit, slot);
            }
        }
        for slot in 0..=1 {
            if !present(&self.request_owned, slot) {
                return (Pool::Request, slot);
            }
        }
        (Pool::Request, 0)
    }

    fn restart_pool(&mut self, max_iter: u32) -> Result<(), ChannelError> {
        self.port.restart_producer()?;
        self.replace_pool(max_iter)?;
        self.ready = false;
        self.reconciled = false;
        self.drain = None;
        self.port.probe_abi()
    }

    fn finish_close(&mut self) {
        self.port.terminate_producer();
        self.request_owned.clear();
        self.orbit_owned.clear();
        self.arrivals.clear();
        self.orbit_leases = 0;
        self.pool_epoch = self.pool_epoch.wrapping_add(1);
        self.ready = false;
        self.reconciled = true;
        self.drain = None;
        self.refresh_facts();
    }

    fn replace_pool(&mut self, max_iter: u32) -> Result<(), ChannelError> {
        self.allocate_pool(max_iter)?;
        self.arrivals.clear();
        self.orbit_leases = 0;
        self.config.max_iter = max_iter;
        self.pool_epoch = self.pool_epoch.wrapping_add(1);
        self.facts.allocation_events = self.facts.allocation_events.saturating_add(1);
        self.bump_facts();
        self.refresh_facts();
        Ok(())
    }

    fn allocate_pool(&mut self, max_iter: u32) -> Result<(), ChannelError> {
        let mut request_owned = Vec::with_capacity(2);
        let mut orbit_owned = Vec::with_capacity(2);
        for slot in 0..=1 {
            request_owned.push(self.port.allocate(Pool::Request, slot, max_iter)?);
            orbit_owned.push(self.port.allocate(Pool::Orbit, slot, max_iter)?);
        }
        self.request_owned = request_owned;
        self.orbit_owned = orbit_owned;
        Ok(())
    }

    fn pump_request(&mut self) -> bool {
        if !self.ready || self.closed || self.drain.is_some() || self.pending_request.is_none() {
            return false;
        }
        let Some(slot) = self.request_owned.pop() else {
            return false;
        };
        let Some(request) = self.pending_request.take() else {
            self.request_owned.push(slot);
            return false;
        };
        if request.max_iter() > self.config.max_iter {
            let requested_cap = request.max_iter();
            self.pending_request = Some(request);
            self.request_owned.push(slot);
            self.arm_resize(requested_cap);
            return false;
        }
        if let Err(error) = slot.encode_request(&request) {
            self.last_error = Some(error);
            self.pending_request = Some(request);
            self.request_owned.push(slot);
            return false;
        }
        if let Err(error) = self.port.post(slot) {
            self.last_error = Some(error);
            self.pending_request = Some(request);
            return false;
        }
        true
    }

    fn return_queued_arrivals(&mut self) -> Result<(), ChannelError> {
        while let Some(slot) = self.arrivals.pop_front() {
            self.return_stale_slot(slot)?;
        }
        Ok(())
    }

    fn return_stale_slot(&mut self, slot: P::Slot) -> Result<(), ChannelError> {
        let old = slot.header()?;
        let charge = self.credit.charge(self.port.now_us()?, old.compute_us)?;
        let mut header = MessageHeader::new(MessageKind::CreditStale, old.generation);
        header.precision_bits = old.precision_bits;
        header.compute_us = old.compute_us;
        header.credit_us = charge.credit_us;
        slot.write_header(header)?;
        self.port.post(slot)?;
        self.record_credit(old, OrbitDisposition::Stale, charge.overfeed_us);
        Ok(())
    }

    const fn record_credit(
        &mut self,
        header: MessageHeader,
        disposition: OrbitDisposition,
        overfeed_us: u32,
    ) {
        self.facts.last_ack_generation = header.generation;
        self.facts.last_compute_us = header.compute_us;
        self.facts.last_overfeed_us = overfeed_us;
        self.facts.credit_us = self.credit.credit_us();
        if header.kind == MessageKind::OrbitCancelled as u32 {
            self.facts.cancelled_count = self.facts.cancelled_count.saturating_add(1);
        } else {
            match disposition {
                OrbitDisposition::Applied => {
                    self.facts.last_applied_generation = header.generation;
                    self.facts.applied_count = self.facts.applied_count.saturating_add(1);
                }
                OrbitDisposition::Stale => {
                    self.facts.stale_count = self.facts.stale_count.saturating_add(1);
                }
            }
        }
    }

    fn refresh_facts(&mut self) {
        self.facts.orbit_queue_depth = u32::try_from(self.arrivals.len()).unwrap_or(u32::MAX);
        self.facts.shutdown_queue_depth = u32::from(self.drain.is_some());
        self.facts.request_buffers_owned_main =
            u32::try_from(self.request_owned.len()).unwrap_or(u32::MAX);
        self.facts.orbit_buffers_owned_main = u32::try_from(self.orbit_owned.len())
            .unwrap_or(u32::MAX)
            .saturating_add(u32::try_from(self.arrivals.len()).unwrap_or(u32::MAX))
            .saturating_add(self.orbit_leases);
    }

    const fn bump_facts(&mut self) {
        if let Some(epoch) = self.facts.epoch.checked_add(1) {
            self.facts.epoch = epoch;
        } else {
            self.last_error = Some(ChannelError::new(ErrorCode::EpochExhausted, 0, 0, 0));
        }
    }
}

fn push_unique<S: OwnerSlot>(owned: &mut Vec<S>, slot: S) -> Result<(), ChannelError> {
    let identity = slot.identity()?;
    if owned
        .iter()
        .any(|present| present.identity().ok() == Some(identity))
    {
        return Err(ChannelError::new(
            ErrorCode::BufferStarved,
            identity.1,
            0,
            0,
        ));
    }
    owned.push(slot);
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::rc::Rc;
    use std::time::Instant;

    use ember_julibrot_math::PrecisionMode;

    use super::{ControlMessage, OwnerCore, OwnerPort, OwnerSlot, TwoSlotQueue};
    use crate::wire::WireBuffer;
    use crate::{
        ChannelError, CoordinateDescriptor, EncodedCentre, ErrorCode, JULIBROT_ABI_VERSION,
        MessageHeader, MessageKind, OrbitDisposition, OrbitReason, OrbitRequest, Pool,
        ReferenceOrbitRecord, SubmitOutcome, WorkerChannel, WorkerConfig, WorkerFacts, WorkerMode,
        buffer_capacity,
    };

    /// One in-process transfer slot with the browser slot's shared-mutation shape.
    struct FakeSlot {
        buffer: RefCell<WireBuffer>,
    }

    impl FakeSlot {
        fn allocate(pool: Pool, slot: u32, max_iter: u32) -> Result<Self, ChannelError> {
            let allocated = Self {
                buffer: RefCell::new(WireBuffer::new(pool, slot, max_iter)?),
            };
            let initial = match pool {
                Pool::Request => MessageKind::RequestReturn,
                Pool::Orbit => MessageKind::CreditStale,
            };
            allocated.write_empty(initial, 0, 0, 0)?;
            Ok(allocated)
        }

        fn capacity(&self) -> usize {
            self.buffer.borrow().as_bytes().len()
        }

        fn pool(&self) -> Result<Pool, ChannelError> {
            Ok(self.buffer.borrow().identity()?.0)
        }

        fn decode_request(&self) -> Result<OrbitRequest, ChannelError> {
            OrbitRequest::decode(&self.buffer.borrow())
        }

        fn write_orbit(
            &self,
            generation: u32,
            compute_us: u32,
            records: &[ReferenceOrbitRecord],
        ) -> Result<(), ChannelError> {
            self.buffer.borrow_mut().write_orbit(
                generation,
                64,
                compute_us,
                250_000,
                records,
                crate::wire::OrbitVerificationFacts::stable(0, 0),
            )
        }
    }

    impl OwnerSlot for FakeSlot {
        fn identity(&self) -> Result<(Pool, u32), ChannelError> {
            self.buffer.borrow().identity()
        }

        fn header(&self) -> Result<MessageHeader, ChannelError> {
            self.buffer.borrow().header()
        }

        fn validate_message(&self) -> Result<MessageKind, ChannelError> {
            self.buffer.borrow().validate_message()
        }

        fn write_header(&self, header: MessageHeader) -> Result<(), ChannelError> {
            self.buffer.borrow_mut().write_header(header)
        }

        fn encode_request(&self, request: &OrbitRequest) -> Result<(), ChannelError> {
            request.encode_into(&mut self.buffer.borrow_mut())
        }

        fn channel_error(&self) -> Result<ChannelError, ChannelError> {
            self.buffer.borrow().error()
        }
    }

    /// One message the producer delivered to the owner.
    enum FakeMessage {
        Slot(FakeSlot),
        Control(ControlMessage),
    }

    /// Native model of `worker_main`: the same acceptance rules and shutdown acknowledgement.
    #[derive(Default)]
    struct FakeProducer {
        latest_generation: u32,
        pending: Option<OrbitRequest>,
        orbit_slots: Vec<FakeSlot>,
        shutdown_slot: Option<FakeSlot>,
        running: bool,
        closed: bool,
    }

    /// One producer instance, its delivery queue, and the owner-visible clock.
    struct FakeWire {
        producer: FakeProducer,
        to_owner: VecDeque<FakeMessage>,
        delivered: Vec<(u32, u32)>,
        now_us: u64,
        restarts: u32,
    }

    impl FakeWire {
        fn new() -> Self {
            Self {
                producer: FakeProducer::default(),
                to_owner: VecDeque::new(),
                delivered: Vec::new(),
                now_us: 0,
                restarts: 0,
            }
        }

        fn receive(&mut self, slot: FakeSlot) -> Result<(), ChannelError> {
            let header = slot.header()?;
            let kind = header.validate()?;
            match (slot.pool()?, kind) {
                (Pool::Request, MessageKind::OrbitRequest) => {
                    let request = slot.decode_request()?;
                    let generation = request.generation();
                    self.delivered.push((generation, request.max_iter()));
                    slot.write_empty(MessageKind::RequestReturn, generation, 0, 0)?;
                    self.to_owner.push_back(FakeMessage::Slot(slot));
                    if !self.producer.closed && generation > self.producer.latest_generation {
                        self.producer.latest_generation = generation;
                        self.producer.pending = Some(request);
                    }
                }
                (Pool::Orbit, MessageKind::CreditApplied | MessageKind::CreditStale) => {
                    if self.producer.orbit_slots.len() == 2 {
                        return Err(ChannelError::new(ErrorCode::BufferStarved, 2, 0, 0));
                    }
                    self.producer.orbit_slots.push(slot);
                }
                (Pool::Request, MessageKind::Shutdown) => {
                    if self.producer.shutdown_slot.is_some() {
                        return Err(ChannelError::new(ErrorCode::BadKind, header.kind, 0, 0));
                    }
                    self.producer.closed = true;
                    self.producer.latest_generation = 0;
                    self.producer.pending = None;
                    self.producer.shutdown_slot = Some(slot);
                }
                (_, _) => return Err(ChannelError::new(ErrorCode::BadKind, header.kind, 0, 0)),
            }
            self.try_shutdown_ack()
        }

        fn try_shutdown_ack(&mut self) -> Result<(), ChannelError> {
            if !self.producer.closed
                || self.producer.running
                || self.producer.orbit_slots.len() != 2
            {
                return Ok(());
            }
            while let Some(orbit) = self.producer.orbit_slots.pop() {
                orbit.write_empty(MessageKind::CreditStale, 0, 0, 0)?;
                self.to_owner.push_back(FakeMessage::Slot(orbit));
            }
            let Some(slot) = self.producer.shutdown_slot.take() else {
                return Ok(());
            };
            slot.write_empty(MessageKind::ShutdownAck, 0, 0, 0)?;
            self.to_owner.push_back(FakeMessage::Slot(slot));
            Ok(())
        }

        fn begin_work(&mut self) -> Option<(u32, FakeSlot)> {
            if self.producer.running || self.producer.closed {
                return None;
            }
            let request = self.producer.pending.take()?;
            let slot = self.producer.orbit_slots.pop()?;
            self.producer.running = true;
            Some((request.generation(), slot))
        }

        fn finish_work(
            &mut self,
            generation: u32,
            slot: FakeSlot,
            records: &[ReferenceOrbitRecord],
        ) -> Result<(), ChannelError> {
            slot.write_orbit(generation, 1_000, records)?;
            self.to_owner.push_back(FakeMessage::Slot(slot));
            self.producer.running = false;
            self.try_shutdown_ack()
        }
    }

    /// In-process lowering of the owner transport.
    struct FakePort {
        wire: Rc<RefCell<FakeWire>>,
    }

    impl OwnerPort for FakePort {
        type Slot = FakeSlot;

        fn allocate(
            &self,
            pool: Pool,
            slot: u32,
            max_iter: u32,
        ) -> Result<Self::Slot, ChannelError> {
            FakeSlot::allocate(pool, slot, max_iter)
        }

        fn post(&mut self, slot: Self::Slot) -> Result<(), ChannelError> {
            self.wire.borrow_mut().receive(slot)
        }

        fn probe_abi(&mut self) -> Result<(), ChannelError> {
            self.wire
                .borrow_mut()
                .to_owner
                .push_back(FakeMessage::Control(ControlMessage::AbiAccepted(
                    JULIBROT_ABI_VERSION,
                )));
            Ok(())
        }

        fn restart_producer(&mut self) -> Result<(), ChannelError> {
            let mut wire = self.wire.borrow_mut();
            wire.producer = FakeProducer::default();
            wire.restarts += 1;
            wire.to_owner.clear();
            wire.to_owner
                .push_back(FakeMessage::Control(ControlMessage::ProducerReady(
                    JULIBROT_ABI_VERSION,
                )));
            Ok(())
        }

        fn terminate_producer(&mut self) {
            let mut wire = self.wire.borrow_mut();
            wire.producer = FakeProducer {
                closed: true,
                ..FakeProducer::default()
            };
            wire.to_owner.clear();
        }

        fn now_us(&self) -> Result<u64, ChannelError> {
            Ok(self.wire.borrow().now_us)
        }
    }

    /// One booted owner over the in-process producer.
    struct Harness {
        core: OwnerCore<FakePort>,
        wire: Rc<RefCell<FakeWire>>,
    }

    impl Harness {
        fn boot(max_iter: u32) -> Self {
            let wire = Rc::new(RefCell::new(FakeWire::new()));
            let port = FakePort {
                wire: Rc::clone(&wire),
            };
            let mut harness = Self {
                core: OwnerCore::new(port, WorkerConfig { max_iter }).unwrap(),
                wire,
            };
            harness.core.probe_abi().unwrap();
            harness.pump();
            harness
        }

        fn pump(&mut self) {
            loop {
                let message = self.wire.borrow_mut().to_owner.pop_front();
                let Some(message) = message else {
                    return;
                };
                match message {
                    FakeMessage::Slot(slot) => self.core.receive_slot(slot).unwrap(),
                    FakeMessage::Control(control) => self.core.receive_control(control).unwrap(),
                }
            }
        }

        fn submit(&mut self, generation: u32, max_iter: u32) -> SubmitOutcome {
            let outcome = self.core.submit(request(generation, max_iter));
            self.pump();
            outcome
        }

        fn produce(&mut self, records: usize) {
            let work = self.wire.borrow_mut().begin_work();
            let (generation, slot) = work.expect("the producer holds a request and an orbit slot");
            self.wire
                .borrow_mut()
                .finish_work(generation, slot, &orbit_records(records))
                .unwrap();
            self.pump();
        }

        /// Takes one arrival, returns its credit, and reports generation, length, and capacity.
        fn drain_arrival(&mut self, disposition: OrbitDisposition) -> (u32, u32, usize) {
            let epoch = self.core.pool_epoch();
            let (slot, _) = self.core.take_arrival().expect("one queued arrival");
            let header = slot.header().unwrap();
            let capacity = slot.capacity();
            let now_us = self.wire.borrow().now_us;
            self.core
                .return_slot(slot, epoch, disposition, now_us)
                .unwrap();
            self.pump();
            (header.generation, header.length, capacity)
        }
    }

    fn request(generation: u32, max_iter: u32) -> OrbitRequest {
        OrbitRequest::new(
            generation,
            EncodedCentre {
                revision: generation,
                coordinates: [CoordinateDescriptor::default(); 4],
                limbs: Vec::new(),
            },
            0,
            64,
            max_iter,
            PrecisionMode::Deterministic,
            OrbitReason::INITIAL,
        )
        .unwrap()
    }

    fn largest_canonical_request(generation: u32) -> OrbitRequest {
        let limbs = vec![1_u32; 128];
        let coordinates = core::array::from_fn(|index| CoordinateDescriptor {
            sign: u32::try_from(index % 2).unwrap_or(0),
            exponent_twos_complement: 0,
            limb_start: u32::try_from(index * 32).unwrap_or(0),
            limb_count: 32,
        });
        OrbitRequest::new(
            generation,
            EncodedCentre {
                revision: generation,
                coordinates,
                limbs,
            },
            0,
            4_096,
            4_096,
            PrecisionMode::Deterministic,
            OrbitReason::INITIAL,
        )
        .unwrap()
    }

    const fn zero_record() -> ReferenceOrbitRecord {
        ReferenceOrbitRecord { re: 0.0, im: 0.0 }
    }

    fn orbit_records(count: usize) -> Vec<ReferenceOrbitRecord> {
        vec![zero_record(); count]
    }

    #[test]
    fn a_lower_cap_reuses_the_stable_pool_without_refusing_a_length() {
        let mut harness = Harness::boot(512);
        assert_eq!(harness.submit(1, 512), SubmitOutcome::Transferred);
        let work = harness.wire.borrow_mut().begin_work().expect("in flight");

        assert_eq!(harness.submit(2, 64), SubmitOutcome::Transferred);
        assert_eq!(harness.core.pending_request_depth(), 0);
        assert_eq!(
            harness.core.take_error(),
            None,
            "a cap change is a resize, never a length refusal against the old pool"
        );

        harness
            .wire
            .borrow_mut()
            .finish_work(work.0, work.1, &orbit_records(8))
            .unwrap();
        harness.pump();
        assert_eq!(
            harness.drain_arrival(OrbitDisposition::Stale),
            (1, 8, buffer_capacity(512).unwrap()),
            "the in-flight arrival still reaches the app, so its coalescing is released"
        );

        assert_eq!(harness.core.pending_request_depth(), 0);
        assert_eq!(harness.core.facts().allocation_events, 1);
        assert_eq!(harness.wire.borrow().restarts, 0);
        assert_eq!(harness.wire.borrow().delivered, vec![(1, 512), (2, 64)]);
        assert_eq!(harness.core.take_error(), None);

        harness.produce(64);
        assert_eq!(
            harness.drain_arrival(OrbitDisposition::Applied),
            (2, 64, buffer_capacity(512).unwrap()),
            "the lower-cap orbit arrives in the unchanged stable pool"
        );
        assert_eq!(harness.core.facts().applied_count, 1);
    }

    #[test]
    fn the_cap_sequence_grows_the_stable_pool_only_once() {
        let mut harness = Harness::boot(512);
        let mut generation = 1;
        assert_eq!(harness.submit(generation, 512), SubmitOutcome::Transferred);
        for (cap, expected) in [
            (64_u32, SubmitOutcome::Transferred),
            (4_096, SubmitOutcome::Coalesced),
            (512, SubmitOutcome::Transferred),
        ] {
            let work = harness.wire.borrow_mut().begin_work().expect("in flight");
            generation += 1;
            assert_eq!(harness.submit(generation, cap), expected);
            harness
                .wire
                .borrow_mut()
                .finish_work(work.0, work.1, &orbit_records(8))
                .unwrap();
            harness.pump();
            let (arrived, _, _) = harness.drain_arrival(OrbitDisposition::Stale);
            assert_eq!(arrived, generation - 1);
            assert_eq!(harness.core.pending_request_depth(), 0);
            assert_eq!(harness.core.take_error(), None);
            assert_eq!(
                harness.wire.borrow().delivered.last(),
                Some(&(generation, cap))
            );
        }
        let facts = harness.core.facts();
        assert_eq!(facts.allocation_events, 2);
        assert_eq!(facts.stale_count, 3);
        assert_eq!(harness.wire.borrow().restarts, 1);
    }

    #[test]
    fn ten_cap_changes_collapse_into_one_growth_drain_without_starvation() {
        let mut harness = Harness::boot(64);
        assert_eq!(harness.submit(1, 64), SubmitOutcome::Transferred);
        let work = harness.wire.borrow_mut().begin_work().expect("in flight");
        let caps = [128_u32, 256, 512, 1_024, 2_048, 4_096, 2_048, 1_024, 256, 512];
        let mut generation = 1;
        let mut maximum_drain_depth = 0;
        for cap in caps {
            generation += 1;
            assert_eq!(harness.submit(generation, cap), SubmitOutcome::Coalesced);
            maximum_drain_depth = maximum_drain_depth.max(harness.core.facts().shutdown_queue_depth);
            assert_eq!(harness.core.take_error(), None);
        }
        assert_eq!(maximum_drain_depth, 1, "the burst arms one bounded drain");

        harness
            .wire
            .borrow_mut()
            .finish_work(work.0, work.1, &orbit_records(8))
            .unwrap();
        harness.pump();
        assert_eq!(
            harness.drain_arrival(OrbitDisposition::Stale).0,
            1,
            "the old arrival is returned instead of starving the pool"
        );
        assert_eq!(harness.core.pending_request_depth(), 0);
        assert_eq!(harness.core.facts().allocation_events, 2);
        assert_eq!(harness.wire.borrow().restarts, 1);
        assert_eq!(harness.wire.borrow().delivered.last(), Some(&(generation, 512)));
        assert_eq!(harness.core.take_error(), None);
    }

    #[test]
    fn an_unreturned_lease_cannot_hold_a_resize_past_the_return_deadline() {
        let mut harness = Harness::boot(512);
        assert_eq!(harness.submit(1, 512), SubmitOutcome::Transferred);
        harness.produce(8);
        let held_epoch = harness.core.pool_epoch();
        let (held, _) = harness.core.take_arrival().expect("one queued arrival");

        assert_eq!(harness.submit(2, 4_096), SubmitOutcome::Coalesced);
        assert_eq!(harness.core.facts().allocation_events, 1);
        assert_eq!(harness.core.pending_request_depth(), 1);

        harness.core.port_mut().wire.borrow_mut().now_us =
            u64::from(crate::BUFFER_RETURN_DEADLINE_US);
        let facts = harness.core.facts();
        assert_eq!(facts.allocation_events, 2);
        assert_eq!(
            harness.core.take_error(),
            Some(ChannelError::new(
                ErrorCode::BufferStarved,
                Pool::Orbit as u32,
                0,
                0
            )),
            "the expired drain names the pool and slot that never came home"
        );
        harness.pump();
        assert_eq!(harness.core.pending_request_depth(), 0);
        assert_eq!(harness.wire.borrow().delivered.last(), Some(&(2, 4_096)));

        let now_us = harness.wire.borrow().now_us;
        harness
            .core
            .return_slot(held, held_epoch, OrbitDisposition::Stale, now_us)
            .unwrap();
        assert_eq!(
            harness.wire.borrow().producer.orbit_slots.len(),
            2,
            "a lease from the superseded pool is charged and dropped, never transferred"
        );
        harness.produce(4_096);
        assert_eq!(
            harness.drain_arrival(OrbitDisposition::Applied),
            (2, 4_096, buffer_capacity(4_096).unwrap())
        );
    }

    #[test]
    fn a_closing_drain_stale_credits_a_queued_arrival_and_reconciles() {
        let mut harness = Harness::boot(64);
        assert_eq!(harness.submit(1, 64), SubmitOutcome::Transferred);
        assert_eq!(harness.core.latest_generation(), 1);
        harness.produce(4);
        assert_eq!(harness.core.facts().orbit_queue_depth, 1);

        harness.core.shutdown().unwrap();
        harness.pump();
        assert!(harness.core.shutdown_acknowledged());
        assert_eq!(harness.core.facts().stale_count, 1);
        assert_eq!(
            harness.core.submit(request(2, 64)),
            SubmitOutcome::GenerationExhausted
        );
        assert_eq!(harness.core.take_error(), None);
    }

    #[test]
    fn a_closing_drain_with_a_missing_slot_terminates_at_the_deadline() {
        let mut harness = Harness::boot(64);
        assert_eq!(harness.submit(1, 64), SubmitOutcome::Transferred);
        harness.produce(4);
        let held_epoch = harness.core.pool_epoch();
        let (held, _) = harness.core.take_arrival().expect("one leased slot");

        harness.core.shutdown().unwrap();
        harness.pump();
        assert!(!harness.core.shutdown_acknowledged());
        harness.core.port_mut().wire.borrow_mut().now_us =
            u64::from(crate::BUFFER_RETURN_DEADLINE_US);
        let facts = harness.core.facts();
        assert!(harness.core.shutdown_acknowledged());
        assert_eq!(facts.shutdown_queue_depth, 0);
        assert_eq!(facts.request_buffers_owned_main, 0);
        assert_eq!(facts.orbit_buffers_owned_main, 0);
        assert_eq!(
            harness.core.take_error(),
            Some(ChannelError::new(
                ErrorCode::BufferStarved,
                Pool::Orbit as u32,
                0,
                0
            )),
            "the terminal refusal names the slot that never returned"
        );

        let now_us = harness.wire.borrow().now_us;
        harness
            .core
            .return_slot(held, held_epoch, OrbitDisposition::Stale, now_us)
            .unwrap();
        assert_eq!(harness.core.facts().orbit_buffers_owned_main, 0);
    }

    #[test]
    fn two_queued_arrivals_remain_fifo() {
        let mut harness = Harness::boot(64);
        assert_eq!(harness.submit(1, 64), SubmitOutcome::Transferred);
        harness.produce(4);
        assert_eq!(harness.submit(2, 64), SubmitOutcome::Transferred);
        harness.produce(4);
        assert_eq!(harness.core.facts().orbit_queue_depth, 2);

        assert_eq!(
            harness.drain_arrival(OrbitDisposition::Stale).0,
            1,
            "the oldest queued arrival drains first"
        );
        assert_eq!(
            harness.drain_arrival(OrbitDisposition::Applied).0,
            2,
            "the newest queued arrival drains second"
        );
        assert_eq!(harness.core.facts().orbit_queue_depth, 0);
    }

    #[test]
    fn a_full_two_slot_queue_refuses_without_overwriting() {
        let mut queue = TwoSlotQueue::new();
        assert_eq!(queue.push_back(10), Ok(()));
        assert_eq!(queue.push_back(20), Ok(()));
        assert_eq!(queue.push_back(30), Err(30));
        assert_eq!(queue.len(), 2);
        assert_eq!(queue.pop_front(), Some(10));
        assert_eq!(queue.pop_front(), Some(20));
    }

    #[test]
    #[ignore = "measurement harness"]
    #[allow(
        clippy::print_stderr,
        reason = "the explicitly selected performance harness reports its wall measurement"
    )]
    fn measures_two_slot_arrival_drain_wall_before_after() {
        const DRAINS: u32 = 20_000_000;
        let mut shifted = vec![0_u32, 1];
        let shifted_start = Instant::now();
        let mut shifted_sum = 0_u64;
        for next in 2..DRAINS + 2 {
            shifted_sum = shifted_sum.wrapping_add(u64::from(shifted.remove(0)));
            shifted.push(next);
        }
        let shifted_wall = shifted_start.elapsed();

        let mut fixed = TwoSlotQueue::new();
        fixed.push_back(0_u32).expect("first fixed slot");
        fixed.push_back(1).expect("second fixed slot");
        let fixed_start = Instant::now();
        let mut fixed_sum = 0_u64;
        for next in 2..DRAINS + 2 {
            fixed_sum = fixed_sum.wrapping_add(u64::from(
                fixed
                    .pop_front()
                    .expect("the two-slot queue stays populated"),
            ));
            fixed.push_back(next).expect("one fixed slot is free");
        }
        let fixed_wall = fixed_start.elapsed();

        assert_eq!(shifted_sum, fixed_sum);
        assert_eq!(fixed.pop_front(), Some(shifted[0]));
        assert_eq!(fixed.pop_front(), Some(shifted[1]));
        eprintln!(
            "PF-V5 drains={DRAINS} shifted_vec_us={} fixed_queue_us={}",
            shifted_wall.as_micros(),
            fixed_wall.as_micros()
        );
    }

    #[test]
    fn the_largest_canonical_request_uses_only_its_declared_prefix() {
        let request = largest_canonical_request(1);
        let prefix_bytes = request.centre().request_bytes().unwrap();
        let mut buffer = WireBuffer::new(Pool::Request, 0, 4_096).unwrap();
        request.encode_into(&mut buffer).unwrap();

        assert_eq!(prefix_bytes, 628);
        assert_eq!(buffer.capacity(), 32_832);
        assert_eq!(
            prefix_bytes,
            crate::codec::REQUEST_FIXED_END + request.centre().limbs.len() * 4
        );
        assert_eq!(OrbitRequest::decode(&buffer).unwrap(), request);
    }

    #[test]
    fn browser_request_decode_uses_one_prefix_bulk_copy() {
        let source = include_str!("browser.rs");
        let body = source
            .split_once("fn decode_request")
            .and_then(|(_, suffix)| suffix.split_once("pub(crate) fn write_empty"))
            .map(|(body, _)| body)
            .expect("browser request decoder body");
        assert!(body.contains("if used > available"));
        assert!(body.contains(".subarray(0, used)"));
        assert!(body.contains(".copy_to("));
        assert!(!body.contains("get_index"));
        assert!(!body.contains("used..available"));
    }

    #[test]
    #[ignore = "measurement harness"]
    #[allow(
        clippy::print_stderr,
        reason = "the explicitly selected performance harness reports copied bytes and wall"
    )]
    fn measures_browser_request_decode_copy_bytes() {
        const COPIES: u32 = 20_000;
        let request = largest_canonical_request(1);
        let prefix_bytes = request.centre().request_bytes().unwrap();
        let mut buffer = WireBuffer::new(Pool::Request, 0, 4_096).unwrap();
        request.encode_into(&mut buffer).unwrap();

        let whole_start = Instant::now();
        let mut whole_total = 0_usize;
        for _ in 0..COPIES {
            let copied = std::hint::black_box(buffer.as_bytes().to_vec());
            whole_total = whole_total.saturating_add(copied.len());
        }
        let whole_wall = whole_start.elapsed();

        let prefix_start = Instant::now();
        let mut prefix_total = 0_usize;
        for _ in 0..COPIES {
            let copied = std::hint::black_box(buffer.as_bytes()[..prefix_bytes].to_vec());
            prefix_total = prefix_total.saturating_add(copied.len());
        }
        let prefix_wall = prefix_start.elapsed();

        assert_eq!(whole_total, buffer.capacity() * COPIES as usize);
        assert_eq!(prefix_total, prefix_bytes * COPIES as usize);
        eprintln!(
            "PF-R7 copies={COPIES} whole_bytes_per_decode={} prefix_bytes_per_decode={prefix_bytes} native_whole_copy_us={} native_prefix_copy_us={}",
            buffer.capacity(),
            whole_wall.as_micros(),
            prefix_wall.as_micros()
        );
    }

    #[test]
    fn a_refused_or_skewed_handshake_is_a_typed_version_refusal() {
        let mut harness = Harness::boot(64);
        assert_eq!(
            harness.core.receive_control(ControlMessage::Refused),
            Err(ChannelError::new(ErrorCode::BadVersion, 0, 0, 0))
        );
        assert_eq!(
            harness
                .core
                .receive_control(ControlMessage::ProducerReady(JULIBROT_ABI_VERSION + 1)),
            Err(ChannelError::new(
                ErrorCode::BadVersion,
                JULIBROT_ABI_VERSION + 1,
                0,
                0
            ))
        );
        let refusal = ChannelError::new(ErrorCode::TimingOverflow, 7, 0, 0);
        harness.core.publish_error(refusal);
        assert_eq!(harness.core.take_error(), Some(refusal));
    }

    #[test]
    fn a_cap_change_with_work_in_flight_is_mode_equivalent() {
        let mut harness = Harness::boot(512);
        assert_eq!(harness.submit(1, 512), SubmitOutcome::Transferred);
        let work = harness.wire.borrow_mut().begin_work().expect("in flight");
        assert_eq!(harness.submit(2, 64), SubmitOutcome::Transferred);
        harness
            .wire
            .borrow_mut()
            .finish_work(work.0, work.1, &orbit_records(8))
            .unwrap();
        harness.pump();
        let browser_arrival = harness.drain_arrival(OrbitDisposition::Stale);
        let browser_facts = harness.core.facts();

        let (owner, producer) =
            WorkerChannel::new(WorkerConfig { max_iter: 512 }, WorkerMode::SameThread).unwrap();
        assert_eq!(owner.submit(request(1, 512)), SubmitOutcome::Transferred);
        let lease = producer.next_request().unwrap().unwrap();
        assert_eq!(owner.submit(request(2, 64)), SubmitOutcome::Transferred);
        producer
            .complete(lease, &orbit_records(8), 64, 1_000, 250_000)
            .unwrap();
        let mut arrival = owner.next_arrival().unwrap();
        let same_arrival = (
            arrival.generation(),
            arrival.length(),
            buffer_capacity(512).unwrap(),
        );
        assert_eq!(arrival.records.record_bytes().unwrap().len(), 8 * 8);
        owner
            .return_credit(&mut arrival, OrbitDisposition::Stale, 0)
            .unwrap();
        let same_facts = owner.facts();
        assert_eq!(owner.pending_request_depth(), 0);
        assert_eq!(
            producer
                .next_request()
                .unwrap()
                .unwrap()
                .request()
                .max_iter(),
            64
        );

        assert_eq!(browser_arrival, same_arrival);
        assert_eq!(comparable(browser_facts), comparable(same_facts));
    }

    /// Facts both lowerings must agree on; buffer ownership counts and the epoch are lowering
    /// facts, because the browser producer returns a request slot before it starts computing while
    /// the same-thread producer holds it for the life of its lease.
    const fn comparable(facts: WorkerFacts) -> [u32; 9] {
        [
            facts.allocation_events,
            facts.applied_count,
            facts.stale_count,
            facts.cancelled_count,
            facts.last_ack_generation,
            facts.last_applied_generation,
            facts.last_compute_us,
            facts.credit_us,
            facts.orbit_queue_depth,
        ]
    }
}
