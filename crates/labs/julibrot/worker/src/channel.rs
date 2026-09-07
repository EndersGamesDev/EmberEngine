//! Same-thread lowering of the bounded ownership-transfer channel.

use std::cell::RefCell;
use std::rc::Rc;

#[cfg(not(any(test, target_arch = "wasm32")))]
#[path = "endpoint.rs"]
#[allow(
    dead_code,
    reason = "the shared owner source includes browser-only control and drain entries"
)]
mod native_endpoint;

#[cfg(target_arch = "wasm32")]
use js_sys::{ArrayBuffer, Uint8Array};

#[cfg(target_arch = "wasm32")]
use crate::browser::TransferBuffer;
#[cfg(target_arch = "wasm32")]
use crate::browser_owner::BrowserOwnerEndpoint;
#[cfg(any(test, target_arch = "wasm32"))]
use crate::endpoint::{OwnerCore, OwnerPort, OwnerSlot, OwnerTransport};
use crate::wire::{HEADER_BYTES, ORBIT_RECORD_BYTES, OrbitVerificationFacts, Pool, WireBuffer};
use crate::{
    Admission, ChannelError, ErrorCode, MessageHeader, MessageKind, OrbitDisposition, OrbitRequest,
    ProducerShaper, ReferenceOrbitRecord, ReferenceVerification, WorkerFacts,
};
#[cfg(not(any(test, target_arch = "wasm32")))]
use native_endpoint::{OwnerCore, OwnerPort, OwnerSlot, OwnerTransport};

/// Minimum app-requestable orbit length.
pub const MIN_MAX_ITER: u32 = 64;
/// Fixed owner buffer-return deadline in microseconds.
pub const BUFFER_RETURN_DEADLINE_US: u32 = 4_000_000;
/// Fixed displayed orbit budget in microseconds per second.
pub const ORBIT_BUDGET_US_PER_SECOND: u32 = 250_000;
/// Highest fully implemented worker phase exposed to app integration.
pub const JULIBROT_PHASE_IMPLEMENTED: u32 = 4;

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

#[derive(Debug)]
struct SameThreadPort {
    request_to_producer: BoundedQueue<WireBuffer>,
    orbit_producer: BoundedQueue<WireBuffer>,
    pending_producer_credits: BoundedQueue<ReturnedCredit>,
    shaper: ProducerShaper,
}

impl SameThreadPort {
    const fn new() -> Self {
        Self {
            request_to_producer: BoundedQueue::new(),
            orbit_producer: BoundedQueue::new(),
            pending_producer_credits: BoundedQueue::new(),
            shaper: ProducerShaper::new(),
        }
    }

    fn next_request(&mut self) -> Result<Option<RequestLease>, ChannelError> {
        let Some(buffer) = self.request_to_producer.pop() else {
            return Ok(None);
        };
        let request = OrbitRequest::decode(&buffer)?;
        Ok(Some(RequestLease { request, buffer }))
    }

    fn take_orbit(&mut self) -> Result<WireBuffer, ChannelError> {
        self.orbit_producer
            .pop()
            .ok_or_else(|| ChannelError::new(ErrorCode::BufferStarved, 0, 0, 0))
    }

    fn admit(&mut self, producer_now_us: u64) -> Result<Admission, ChannelError> {
        while let Some(returned) = self.pending_producer_credits.pop() {
            self.shaper
                .observe_return(producer_now_us, returned.credit_us, returned.compute_us)?;
        }
        self.shaper.admit(producer_now_us)
    }
}

impl OwnerPort for SameThreadPort {
    type Slot = WireBuffer;

    fn allocate(&self, pool: Pool, slot: u32, max_iter: u32) -> Result<Self::Slot, ChannelError> {
        let mut buffer = WireBuffer::new(pool, slot, max_iter)?;
        let initial = match pool {
            Pool::Request => MessageKind::RequestReturn,
            Pool::Orbit => MessageKind::CreditStale,
        };
        buffer.write_header(MessageHeader::new(initial, 0))?;
        Ok(buffer)
    }

    fn post(&mut self, slot: Self::Slot) -> Result<(), ChannelError> {
        let (pool, slot_id) = slot.identity()?;
        let header = slot.header()?;
        let kind = slot.validate_message()?;
        match (pool, kind) {
            (Pool::Request, MessageKind::OrbitRequest) => self
                .request_to_producer
                .push(slot)
                .map_err(|_| ChannelError::new(ErrorCode::BufferStarved, slot_id, 0, 0)),
            (Pool::Orbit, MessageKind::CreditApplied | MessageKind::CreditStale) => {
                self.orbit_producer
                    .push(slot)
                    .map_err(|_| ChannelError::new(ErrorCode::BufferStarved, slot_id, 0, 0))?;
                if header.generation == 0 && header.compute_us == 0 && header.credit_us == 0 {
                    Ok(())
                } else {
                    self.pending_producer_credits
                        .push(ReturnedCredit {
                            credit_us: header.credit_us,
                            compute_us: header.compute_us,
                        })
                        .map_err(|_| ChannelError::new(ErrorCode::BufferStarved, slot_id, 0, 0))
                }
            }
            (_, _) => Err(ChannelError::new(ErrorCode::BadKind, header.kind, 0, 0)),
        }
    }

    fn probe_abi(&mut self) -> Result<(), ChannelError> {
        Ok(())
    }

    fn restart_producer(&mut self) -> Result<(), ChannelError> {
        Ok(())
    }

    fn terminate_producer(&mut self) {}

    fn now_us(&self) -> Result<u64, ChannelError> {
        Ok(0)
    }

    fn reclaim_orbit_pool(&mut self) -> Option<[Self::Slot; 2]> {
        if !self.producer_reconciled() {
            return None;
        }
        let first = self.orbit_producer.pop()?;
        let second = self.orbit_producer.pop()?;
        self.pending_producer_credits = BoundedQueue::new();
        self.shaper.reset_for_resize();
        Some([first, second])
    }

    fn producer_reconciled(&self) -> bool {
        self.request_to_producer.is_empty() && self.orbit_producer.len() == 2
    }
}

impl OwnerSlot for WireBuffer {
    fn identity(&self) -> Result<(Pool, u32), ChannelError> {
        Self::identity(self)
    }

    fn header(&self) -> Result<MessageHeader, ChannelError> {
        Self::header(self)
    }

    fn validate_message(&self) -> Result<MessageKind, ChannelError> {
        Self::validate_message(self)
    }

    fn write_header(&mut self, header: MessageHeader) -> Result<(), ChannelError> {
        Self::write_header(self, header)
    }

    fn encode_request(&mut self, request: &OrbitRequest) -> Result<(), ChannelError> {
        request.encode_into(self)
    }
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
    #[allow(
        clippy::new_ret_no_self,
        reason = "the reviewed API returns its paired endpoints"
    )]
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
        #[cfg(target_arch = "wasm32")]
        if mode == WorkerMode::WebWorker {
            let browser = BrowserOwnerEndpoint::new(config)?;
            return Ok((
                OwnerEndpoint {
                    backend: OwnerBackend::Browser(browser.clone()),
                },
                ProducerEndpoint {
                    backend: ProducerBackend::Browser(browser),
                },
            ));
        }
        let core = Rc::new(RefCell::new(OwnerCore::new(
            SameThreadPort::new(),
            config,
            mode,
            OwnerTransport::SameThread,
        )?));
        Ok((
            OwnerEndpoint {
                backend: OwnerBackend::Core(Rc::clone(&core)),
            },
            ProducerEndpoint {
                backend: ProducerBackend::Core(core),
            },
        ))
    }
}

/// Selects the same-thread test lowering only for the exact page flag.
#[must_use]
pub fn worker_mode_from_search(search: &str) -> WorkerMode {
    let query = search.strip_prefix('?').unwrap_or(search);
    if query.split('&').any(|field| field == "worker=same-thread") {
        WorkerMode::SameThread
    } else {
        WorkerMode::WebWorker
    }
}

/// Main-thread side of the channel.
#[derive(Debug)]
pub struct OwnerEndpoint {
    backend: OwnerBackend,
}

#[derive(Debug)]
enum OwnerBackend {
    Core(Rc<RefCell<OwnerCore<SameThreadPort>>>),
    #[cfg(target_arch = "wasm32")]
    Browser(BrowserOwnerEndpoint),
}

impl OwnerEndpoint {
    /// Accepts every newer edit immediately and keeps at most one untransferred request.
    #[must_use]
    pub fn submit(&self, request: OrbitRequest) -> SubmitOutcome {
        match &self.backend {
            OwnerBackend::Core(core) => core.borrow_mut().submit(request),
            #[cfg(target_arch = "wasm32")]
            OwnerBackend::Browser(browser) => browser.submit(request),
        }
    }

    /// Returns the next completed response without blocking.
    #[must_use]
    pub fn next_arrival(&self) -> Option<OrbitResponseView> {
        match &self.backend {
            OwnerBackend::Core(core) => {
                let (buffer, centre_revision, pool_epoch) = {
                    let mut state = core.borrow_mut();
                    let (buffer, centre_revision) = state.take_arrival()?;
                    (buffer, centre_revision, state.pool_epoch())
                };
                match OrbitResponseView::from_same_thread_parts(
                    buffer,
                    Rc::clone(core),
                    centre_revision,
                    pool_epoch,
                ) {
                    Ok(response) => Some(response),
                    Err(error) => {
                        core.borrow_mut().publish_error(error);
                        None
                    }
                }
            }
            #[cfg(target_arch = "wasm32")]
            OwnerBackend::Browser(browser) => browser.next_arrival(),
        }
    }

    /// Returns one response buffer with applied or stale credit accounting.
    ///
    /// # Errors
    ///
    /// Returns `BufferStarved` after an earlier return, `TimingOverflow` for a regressing owner
    /// clock, or a typed wire or browser-port refusal.
    pub fn return_credit(
        &self,
        response: &mut OrbitResponseView,
        disposition: OrbitDisposition,
        owner_now_us: u64,
    ) -> Result<(), ChannelError> {
        let belongs = match &self.backend {
            OwnerBackend::Core(owner) => matches!(
                &response.records.backend,
                OrbitLeaseBackend::Core { core: response, .. } if Rc::ptr_eq(owner, response)
            ),
            #[cfg(target_arch = "wasm32")]
            OwnerBackend::Browser(owner) => response.records.belongs_to_browser(owner),
        };
        if !belongs {
            return Err(ChannelError::new(ErrorCode::BufferStarved, 0, 0, 0));
        }
        response.records.return_credit(disposition, owner_now_us)
    }

    /// Returns and clears the latest typed internal channel refusal.
    #[must_use]
    pub fn take_error(&self) -> Option<ChannelError> {
        match &self.backend {
            OwnerBackend::Core(core) => core.borrow_mut().take_error(),
            #[cfg(target_arch = "wasm32")]
            OwnerBackend::Browser(browser) => browser.take_error(),
        }
    }

    /// Reports the latest submitted generation.
    #[must_use]
    pub fn latest_generation(&self) -> u32 {
        match &self.backend {
            OwnerBackend::Core(core) => core.borrow().latest_generation(),
            #[cfg(target_arch = "wasm32")]
            OwnerBackend::Browser(browser) => browser.latest_generation(),
        }
    }

    /// Reports one coalesced request when producer delivery is saturated.
    #[must_use]
    pub fn pending_request_depth(&self) -> u32 {
        match &self.backend {
            OwnerBackend::Core(core) => core.borrow_mut().pending_request_depth(),
            #[cfg(target_arch = "wasm32")]
            OwnerBackend::Browser(browser) => browser.pending_request_depth(),
        }
    }

    /// Returns one coherent copy of the page-visible channel accounting.
    #[must_use]
    pub fn facts(&self) -> WorkerFacts {
        match &self.backend {
            OwnerBackend::Core(core) => core.borrow_mut().facts(),
            #[cfg(target_arch = "wasm32")]
            OwnerBackend::Browser(browser) => browser.facts(),
        }
    }

    /// Closes a reconciled logical channel without waiting or spinning.
    ///
    /// Browser ownership waits are driven by `worker_main` and bounded by the app's four-second
    /// deadline; this same-thread lowering reports the first outstanding pool immediately.
    ///
    /// # Errors
    ///
    /// Returns `BufferStarved` while any request or orbit slot remains away from its startup owner.
    pub fn shutdown(&self) -> Result<(), ChannelError> {
        match &self.backend {
            OwnerBackend::Core(core) => core.borrow_mut().shutdown(),
            #[cfg(target_arch = "wasm32")]
            OwnerBackend::Browser(browser) => browser.shutdown(),
        }
    }

    /// Reports completed same-thread closure or browser four-slot acknowledgement.
    #[must_use]
    pub fn shutdown_acknowledged(&self) -> bool {
        match &self.backend {
            OwnerBackend::Core(core) => core.borrow().shutdown_acknowledged(),
            #[cfg(target_arch = "wasm32")]
            OwnerBackend::Browser(browser) => browser.shutdown_acknowledged(),
        }
    }
}

/// Producer side of the channel.
#[derive(Debug)]
pub struct ProducerEndpoint {
    backend: ProducerBackend,
}

#[derive(Debug)]
enum ProducerBackend {
    Core(Rc<RefCell<OwnerCore<SameThreadPort>>>),
    #[cfg(target_arch = "wasm32")]
    Browser(BrowserOwnerEndpoint),
}

impl ProducerEndpoint {
    /// Takes the next delivered request without blocking.
    ///
    /// # Errors
    ///
    /// Returns a typed wire refusal if the delivered request was corrupted.
    pub fn next_request(&self) -> Result<Option<RequestLease>, ChannelError> {
        match &self.backend {
            ProducerBackend::Core(core) => core.borrow_mut().port_mut().next_request(),
            #[cfg(target_arch = "wasm32")]
            ProducerBackend::Browser(_) => Err(browser_producer_refusal()),
        }
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
        self.complete_with_facts(
            lease,
            records,
            delivered_precision_bits,
            compute_us,
            admission_credit_us,
            OrbitVerificationFacts::stable(0, 0),
        )
    }

    /// Returns a completed orbit together with its explicit verification facts.
    ///
    /// # Errors
    ///
    /// Returns the same typed ownership, capacity, or queue refusal as [`Self::complete`].
    pub fn complete_with_facts(
        &self,
        lease: RequestLease,
        records: &[ReferenceOrbitRecord],
        delivered_precision_bits: u32,
        compute_us: u32,
        admission_credit_us: u32,
        facts: OrbitVerificationFacts,
    ) -> Result<(), ChannelError> {
        let generation = lease.request.generation();
        match &self.backend {
            ProducerBackend::Core(core) => {
                let mut state = core.borrow_mut();
                let mut request_buffer = lease.buffer;
                request_buffer
                    .write_header(MessageHeader::new(MessageKind::RequestReturn, generation))?;
                state.receive_slot(request_buffer)?;
                let mut orbit = state.port_mut().take_orbit()?;
                orbit.write_orbit(
                    generation,
                    delivered_precision_bits,
                    compute_us,
                    admission_credit_us,
                    records,
                    facts,
                )?;
                state.receive_slot(orbit)
            }
            #[cfg(target_arch = "wasm32")]
            ProducerBackend::Browser(_) => Err(browser_producer_refusal()),
        }
    }

    /// Returns the request slot and reports measured stale work without an orbit payload.
    ///
    /// # Errors
    ///
    /// Returns a typed ownership or queue refusal.
    pub fn cancel(
        &self,
        lease: RequestLease,
        compute_us: u32,
        admission_credit_us: u32,
    ) -> Result<(), ChannelError> {
        let generation = lease.request.generation();
        match &self.backend {
            ProducerBackend::Core(core) => {
                let mut state = core.borrow_mut();
                let mut request_buffer = lease.buffer;
                request_buffer
                    .write_header(MessageHeader::new(MessageKind::RequestReturn, generation))?;
                state.receive_slot(request_buffer)?;
                let mut orbit = state.port_mut().take_orbit()?;
                let mut header = MessageHeader::new(MessageKind::OrbitCancelled, generation);
                header.compute_us = compute_us;
                header.credit_us = admission_credit_us;
                orbit.write_header(header)?;
                state.receive_slot(orbit)
            }
            #[cfg(target_arch = "wasm32")]
            ProducerBackend::Browser(_) => Err(browser_producer_refusal()),
        }
    }

    /// Applies producer-side admission shaping at a monotonic producer timestamp.
    ///
    /// # Errors
    ///
    /// Returns `TimingOverflow` if producer time moves backwards.
    pub fn admit(&self, producer_now_us: u64) -> Result<Admission, ChannelError> {
        match &self.backend {
            ProducerBackend::Core(core) => core.borrow_mut().port_mut().admit(producer_now_us),
            #[cfg(target_arch = "wasm32")]
            ProducerBackend::Browser(_) => Err(browser_producer_refusal()),
        }
    }

    /// Returns the shared page-visible accounting from the producer endpoint.
    #[must_use]
    pub fn facts(&self) -> WorkerFacts {
        match &self.backend {
            ProducerBackend::Core(core) => core.borrow_mut().facts(),
            #[cfg(target_arch = "wasm32")]
            ProducerBackend::Browser(browser) => browser.facts(),
        }
    }

    /// Reports this endpoint's configured lowering.
    #[must_use]
    pub fn mode(&self) -> WorkerMode {
        match &self.backend {
            ProducerBackend::Core(core) => core.borrow().mode(),
            #[cfg(target_arch = "wasm32")]
            ProducerBackend::Browser(_) => WorkerMode::WebWorker,
        }
    }
}

#[cfg(target_arch = "wasm32")]
const fn browser_producer_refusal() -> ChannelError {
    ChannelError::new(ErrorCode::BadKind, WorkerMode::WebWorker as u32, 0, 0)
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
    verification_facts: OrbitVerificationFacts,
    cancelled: bool,
    /// Exclusive ownership of transferred record bytes until credit return.
    pub records: OrbitLease,
}

impl OrbitResponseView {
    fn from_same_thread_parts(
        buffer: WireBuffer,
        core: Rc<RefCell<OwnerCore<SameThreadPort>>>,
        centre_revision: u32,
        pool_epoch: u32,
    ) -> Result<Self, ChannelError> {
        let kind = buffer.validate_message()?;
        if !matches!(
            kind,
            MessageKind::OrbitResponse | MessageKind::OrbitCancelled
        ) {
            return Err(ChannelError::new(
                ErrorCode::BadKind,
                buffer.header()?.kind,
                0,
                0,
            ));
        }
        let header = buffer.header()?;
        let verification_facts = if kind == MessageKind::OrbitResponse {
            buffer.orbit_facts()?
        } else {
            OrbitVerificationFacts::deferred()
        };
        Ok(Self {
            generation: header.generation,
            centre_revision,
            length: header.length,
            compute_us: header.compute_us,
            precision_bits: header.precision_bits,
            admission_credit_us: header.credit_us,
            verification_facts,
            cancelled: kind == MessageKind::OrbitCancelled,
            records: OrbitLease {
                backend: OrbitLeaseBackend::Core {
                    core,
                    buffer: Some(buffer),
                    pool_epoch,
                },
            },
        })
    }

    /// Adopts and validates one browser-transferred orbit buffer.
    ///
    /// The standalone view has no owner port; use `BrowserOwnerEndpoint::next_arrival` when the
    /// buffer must later be returned as credit.
    ///
    /// # Errors
    ///
    /// Returns the same trailer, header, pool, kind, length, and kind-owned unused-byte refusals as
    /// the same-thread path.
    #[cfg(target_arch = "wasm32")]
    pub fn from_transfer(array: ArrayBuffer) -> Result<Self, ChannelError> {
        Self::from_browser_parts(TransferBuffer::from_array(array)?, None, 0, 0)
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn from_browser_transfer(
        buffer: TransferBuffer,
        endpoint: BrowserOwnerEndpoint,
        centre_revision: u32,
        pool_epoch: u32,
    ) -> Result<Self, ChannelError> {
        Self::from_browser_parts(buffer, Some(endpoint), centre_revision, pool_epoch)
    }

    #[cfg(target_arch = "wasm32")]
    fn from_browser_parts(
        buffer: TransferBuffer,
        endpoint: Option<BrowserOwnerEndpoint>,
        centre_revision: u32,
        pool_epoch: u32,
    ) -> Result<Self, ChannelError> {
        let kind = buffer.validate_message()?;
        if !matches!(
            kind,
            MessageKind::OrbitResponse | MessageKind::OrbitCancelled
        ) {
            return Err(ChannelError::new(
                ErrorCode::BadKind,
                buffer.header()?.kind,
                0,
                0,
            ));
        }
        let header = buffer.header()?;
        let verification_facts = if kind == MessageKind::OrbitResponse {
            buffer.orbit_facts()?
        } else {
            OrbitVerificationFacts::deferred()
        };
        Ok(Self {
            generation: header.generation,
            centre_revision,
            length: header.length,
            compute_us: header.compute_us,
            precision_bits: header.precision_bits,
            admission_credit_us: header.credit_us,
            verification_facts,
            cancelled: kind == MessageKind::OrbitCancelled,
            records: OrbitLease {
                backend: OrbitLeaseBackend::Browser {
                    endpoint,
                    buffer: Some(buffer),
                    pool_epoch,
                },
            },
        })
    }

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

    /// Returns whether Final/Measure word verification ran for this orbit.
    #[must_use]
    pub const fn reference_verification(&self) -> ReferenceVerification {
        if self.verification_facts.verification == ReferenceVerification::Stable as u32 {
            ReferenceVerification::Stable
        } else {
            ReferenceVerification::Deferred
        }
    }

    /// Returns the maximum ULP error over consumed reference words, or `None` for Preview.
    #[must_use]
    pub const fn max_consumed_word_error_ulps(&self) -> Option<u32> {
        self.verification_facts.max_consumed_word_error_ulps()
    }

    /// Returns the number of sixteen-digit precision escalations before publication.
    #[must_use]
    pub const fn precision_escalations(&self) -> u32 {
        self.verification_facts.precision_escalations
    }

    /// Reports whether this arrival is measured stale work without an orbit payload.
    #[must_use]
    pub const fn cancelled(&self) -> bool {
        self.cancelled
    }
}

/// Exclusive main-side ownership of transferred orbit bytes.
pub struct OrbitLease {
    backend: OrbitLeaseBackend,
}

enum OrbitLeaseBackend {
    Core {
        core: Rc<RefCell<OwnerCore<SameThreadPort>>>,
        buffer: Option<WireBuffer>,
        pool_epoch: u32,
    },
    #[cfg(target_arch = "wasm32")]
    Browser {
        endpoint: Option<BrowserOwnerEndpoint>,
        buffer: Option<TransferBuffer>,
        /// Pool generation this lease was taken under; a superseded lease is never transferred.
        pool_epoch: u32,
    },
}

impl std::fmt::Debug for OrbitLease {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("OrbitLease").finish_non_exhaustive()
    }
}

impl OrbitLease {
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn belongs_to_browser(&self, owner: &BrowserOwnerEndpoint) -> bool {
        matches!(
            &self.backend,
            OrbitLeaseBackend::Browser {
                endpoint: Some(endpoint),
                ..
            } if owner.same_channel(endpoint)
        )
    }

    /// Borrows exactly the initialized reference-record payload bytes.
    ///
    /// # Errors
    ///
    /// Returns `BufferStarved` after credit was already returned, or a typed wire refusal if the
    /// owned buffer no longer contains a valid orbit response.
    pub fn record_bytes(&self) -> Result<&[u8], ChannelError> {
        let buffer = match &self.backend {
            OrbitLeaseBackend::Core { buffer, .. } => buffer,
            #[cfg(target_arch = "wasm32")]
            OrbitLeaseBackend::Browser { .. } => {
                return Err(ChannelError::new(
                    ErrorCode::BadKind,
                    WorkerMode::WebWorker as u32,
                    0,
                    0,
                ));
            }
        };
        let buffer = buffer
            .as_ref()
            .ok_or_else(|| ChannelError::new(ErrorCode::BufferStarved, 0, 0, 0))?;
        let header = buffer.header()?;
        buffer.validate_message()?;
        let length = usize::try_from(header.length)
            .map_err(|_| ChannelError::new(ErrorCode::BadLength, header.length, 0, 0))?;
        let end = HEADER_BYTES + length * ORBIT_RECORD_BYTES;
        Ok(&buffer.as_bytes()[HEADER_BYTES..end])
    }

    /// Returns a zero-copy JavaScript view over browser-transferred record bytes.
    ///
    /// # Errors
    ///
    /// Returns `BadKind` for the same-thread lowering, `BufferStarved` after return, or a typed
    /// wire refusal for a corrupt response.
    #[cfg(target_arch = "wasm32")]
    pub fn transfer_record_bytes(&self) -> Result<Uint8Array, ChannelError> {
        let OrbitLeaseBackend::Browser { buffer, .. } = &self.backend else {
            return Err(ChannelError::new(
                ErrorCode::BadKind,
                WorkerMode::SameThread as u32,
                0,
                0,
            ));
        };
        let buffer = buffer
            .as_ref()
            .ok_or_else(|| ChannelError::new(ErrorCode::BufferStarved, 0, 0, 0))?;
        crate::browser_owner::response_record_bytes(buffer)
    }

    /// Rewrites the CREDIT header and returns this buffer exactly once.
    ///
    /// # Errors
    ///
    /// Returns `BufferStarved` on a second return or a typed ownership refusal if pool state does
    /// not name main as the current owner.
    pub fn return_credit(
        &mut self,
        disposition: OrbitDisposition,
        owner_now_us: u64,
    ) -> Result<(), ChannelError> {
        match &mut self.backend {
            OrbitLeaseBackend::Core {
                core,
                buffer,
                pool_epoch,
            } => {
                core.borrow_mut()
                    .return_lease_slot(buffer, *pool_epoch, disposition, owner_now_us)
            }
            #[cfg(target_arch = "wasm32")]
            OrbitLeaseBackend::Browser {
                endpoint,
                buffer,
                pool_epoch,
            } => {
                let endpoint = endpoint.as_ref().ok_or_else(|| {
                    ChannelError::new(ErrorCode::BufferStarved, Pool::Orbit as u32, 0, 0)
                })?;
                endpoint.return_transfer(buffer, *pool_epoch, disposition, owner_now_us)
            }
        }
    }
}

impl Drop for OrbitLease {
    fn drop(&mut self) {
        let returned = match &self.backend {
            OrbitLeaseBackend::Core { buffer, .. } => buffer.is_none(),
            #[cfg(target_arch = "wasm32")]
            OrbitLeaseBackend::Browser {
                endpoint, buffer, ..
            } => endpoint.is_none() || buffer.is_none(),
        };
        debug_assert!(returned, "orbit lease dropped without credit return");
    }
}

#[derive(Clone, Copy, Debug)]
struct ReturnedCredit {
    credit_us: u32,
    compute_us: u32,
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
}

#[cfg(test)]
mod tests {
    use ember_julibrot_math::PrecisionMode;

    use super::{
        Admission, SubmitOutcome, WorkerChannel, WorkerConfig, WorkerMode, worker_mode_from_search,
    };
    use crate::endpoint::tests::{
        OWNERSHIP_SCENARIO_GENERATION, OWNERSHIP_SCENARIO_MAX_ITER,
        OWNERSHIP_SCENARIO_OWNER_NOW_US, OWNERSHIP_SCENARIO_RECORDS,
    };
    use crate::endpoint::{
        NormalizedWorkerFacts, OwnershipPhase, OwnershipTrace, begin_ownership_trace,
        finish_ownership_trace,
    };
    use crate::{
        CoordinateDescriptor, EncodedCentre, OrbitDisposition, OrbitReason, OrbitRequest,
        OrbitVerificationFacts, ReferenceOrbitRecord, ReferenceVerification,
    };

    fn request(generation: u32, revision: u32) -> OrbitRequest {
        request_with_cap(generation, revision, 64)
    }

    fn request_with_cap(generation: u32, revision: u32, max_iter: u32) -> OrbitRequest {
        OrbitRequest::new(
            generation,
            EncodedCentre {
                revision,
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

    #[test]
    fn same_thread_transfers_a_request_below_the_browser_minimum() {
        let (owner, producer) =
            WorkerChannel::new(WorkerConfig { max_iter: 64 }, WorkerMode::SameThread).unwrap();

        assert_eq!(
            owner.submit(request_with_cap(1, 1, 16)),
            SubmitOutcome::Transferred
        );
        let receipt = producer.next_request().unwrap().unwrap();
        assert_eq!(receipt.request().generation(), 1);
        assert_eq!(receipt.request().max_iter(), 16);
    }

    #[test]
    fn two_transfers_then_one_latest_pending_request() {
        let (owner, producer) =
            WorkerChannel::new(WorkerConfig { max_iter: 64 }, WorkerMode::SameThread).unwrap();
        assert_eq!(owner.submit(request(1, 1)), SubmitOutcome::Transferred);
        assert_eq!(owner.submit(request(2, 2)), SubmitOutcome::Transferred);
        assert_eq!(owner.submit(request(3, 3)), SubmitOutcome::Coalesced);
        assert_eq!(owner.submit(request(4, 4)), SubmitOutcome::Coalesced);
        assert_eq!(owner.pending_request_depth(), 1);

        let lease = producer.next_request().unwrap().unwrap();
        assert_eq!(lease.request().generation(), 1);
        producer
            .complete_with_facts(
                lease,
                &[zero_record()],
                64,
                10,
                250_000,
                OrbitVerificationFacts::stable(2, 1),
            )
            .unwrap();
        let mut response = owner.next_arrival().unwrap();
        assert_eq!(response.generation(), 1);
        assert_eq!(response.records.record_bytes().unwrap().len(), 8);
        assert_eq!(
            response.reference_verification(),
            ReferenceVerification::Stable
        );
        assert_eq!(response.max_consumed_word_error_ulps(), Some(2));
        assert_eq!(response.precision_escalations(), 1);
        response
            .records
            .return_credit(OrbitDisposition::Stale, 10)
            .unwrap();

        let second = producer.next_request().unwrap().unwrap();
        assert_eq!(second.request().generation(), 2);
        producer
            .complete(second, &[zero_record()], 64, 10, 249_990)
            .unwrap();
        assert_eq!(
            producer
                .next_request()
                .unwrap()
                .unwrap()
                .request()
                .generation(),
            4
        );
    }

    #[test]
    fn stale_generation_never_replaces_latest_generation() {
        let (owner, producer) =
            WorkerChannel::new(WorkerConfig { max_iter: 64 }, WorkerMode::SameThread).unwrap();
        assert_eq!(owner.submit(request(9, 1)), SubmitOutcome::Transferred);
        assert_eq!(owner.submit(request(8, 2)), SubmitOutcome::Coalesced);
        assert_eq!(owner.latest_generation(), 9);
        assert_eq!(producer.mode(), WorkerMode::SameThread);
    }

    #[test]
    fn web_mode_and_exact_same_thread_page_flag_are_accepted() {
        let (owner, producer) =
            WorkerChannel::new(WorkerConfig { max_iter: 64 }, WorkerMode::WebWorker).unwrap();
        assert_eq!(producer.mode(), WorkerMode::WebWorker);
        assert_eq!(
            worker_mode_from_search("?worker=same-thread"),
            WorkerMode::SameThread
        );
        assert_eq!(
            worker_mode_from_search("?worker=web"),
            WorkerMode::WebWorker
        );
        assert_eq!(owner.shutdown(), Ok(()));
    }

    #[test]
    fn cancelled_work_is_charged_and_all_facts_are_coherent() {
        let (owner, producer) =
            WorkerChannel::new(WorkerConfig { max_iter: 64 }, WorkerMode::SameThread).unwrap();
        assert!(matches!(
            producer.admit(0).unwrap(),
            Admission::Ready { warm_up: true, .. }
        ));
        assert_eq!(owner.submit(request(5, 8)), SubmitOutcome::Transferred);
        let lease = producer.next_request().unwrap().unwrap();
        producer.cancel(lease, 300_000, 250_000).unwrap();
        let queued = owner.facts();
        assert_eq!(queued.orbit_queue_depth, 1);
        let mut arrival = owner.next_arrival().unwrap();
        assert!(owner.facts().epoch > queued.epoch);
        assert!(arrival.cancelled());
        assert_eq!(arrival.length(), 0);
        assert_eq!(arrival.records.record_bytes().unwrap(), []);
        arrival
            .records
            .return_credit(OrbitDisposition::Stale, 10)
            .unwrap();
        let facts = owner.facts();
        assert_eq!(facts.last_ack_generation, 5);
        assert_eq!(facts.last_applied_generation, 0);
        assert_eq!(facts.cancelled_count, 1);
        assert_eq!(facts.last_compute_us, 300_000);
        assert_eq!(facts.last_overfeed_us, 50_000);
        assert_eq!(facts.credit_us, 0);
        assert_eq!(facts.orbit_queue_depth, 0);
        assert_eq!(facts.orbit_buffers_owned_main, 0);
        assert!(facts.epoch > 0);
        assert_eq!(producer.facts(), facts);
    }

    #[test]
    fn reconciled_resize_counts_one_allocation_event_and_resets_warm_up() {
        let (owner, producer) =
            WorkerChannel::new(WorkerConfig { max_iter: 64 }, WorkerMode::SameThread).unwrap();
        assert_eq!(owner.facts().allocation_events, 1);
        assert_eq!(
            owner.submit(request_with_cap(1, 1, 128)),
            SubmitOutcome::Transferred
        );
        assert_eq!(owner.facts().allocation_events, 2);
        assert!(matches!(
            producer.admit(5).unwrap(),
            Admission::Ready { warm_up: true, .. }
        ));
    }

    #[test]
    fn owner_and_producer_clock_origins_are_independent() {
        let (owner, producer) =
            WorkerChannel::new(WorkerConfig { max_iter: 64 }, WorkerMode::SameThread).unwrap();
        assert!(matches!(
            producer.admit(5_000_000).unwrap(),
            Admission::Ready { warm_up: true, .. }
        ));
        assert_eq!(owner.submit(request(3, 4)), SubmitOutcome::Transferred);
        let lease = producer.next_request().unwrap().unwrap();
        producer
            .complete(lease, &[zero_record()], 64, 20_000, 250_000)
            .unwrap();
        let mut response = owner.next_arrival().unwrap();
        response
            .records
            .return_credit(OrbitDisposition::Applied, 10)
            .unwrap();
        assert!(matches!(
            producer.admit(5_000_100).unwrap(),
            Admission::Ready { warm_up: false, .. }
        ));
    }

    #[test]
    fn an_orbit_beyond_the_budget_still_admits_the_next_request() {
        let (owner, producer) =
            WorkerChannel::new(WorkerConfig { max_iter: 64 }, WorkerMode::SameThread).unwrap();
        assert!(matches!(
            producer.admit(0).unwrap(),
            Admission::Ready { warm_up: true, .. }
        ));
        assert_eq!(owner.submit(request(1, 1)), SubmitOutcome::Transferred);
        let lease = producer.next_request().unwrap().unwrap();
        producer
            .complete(lease, &[zero_record()], 64, 852_293, 250_000)
            .unwrap();
        let mut response = owner.next_arrival().unwrap();
        response
            .records
            .return_credit(OrbitDisposition::Applied, 1)
            .unwrap();
        let overfed = owner.facts();
        assert_eq!(overfed.last_compute_us, 852_293);
        assert_eq!(overfed.last_overfeed_us, 602_293);
        assert_eq!(overfed.credit_us, 0);

        assert_eq!(
            producer.admit(1).unwrap(),
            Admission::Delay { wait_us: 1_000_000 }
        );
        assert!(matches!(
            producer.admit(1_000_001).unwrap(),
            Admission::Ready { warm_up: false, .. }
        ));

        assert_eq!(owner.submit(request(2, 2)), SubmitOutcome::Transferred);
        let second = producer.next_request().unwrap().unwrap();
        producer
            .complete(second, &[zero_record()], 64, 1_000, 250_000)
            .unwrap();
        let mut cheap = owner.next_arrival().unwrap();
        cheap
            .records
            .return_credit(OrbitDisposition::Applied, 2_000_001)
            .unwrap();
        let refilled = owner.facts();
        assert_eq!(refilled.last_compute_us, 1_000);
        assert_eq!(refilled.last_overfeed_us, 0);
        assert_eq!(refilled.credit_us, 249_000);
        assert_eq!(
            producer.admit(2_000_002).unwrap(),
            Admission::Delay { wait_us: 4_000 }
        );
    }

    #[test]
    fn endpoint_credit_return_rejects_a_foreign_lease() {
        let (owner, producer) =
            WorkerChannel::new(WorkerConfig { max_iter: 64 }, WorkerMode::SameThread).unwrap();
        let (foreign, _) =
            WorkerChannel::new(WorkerConfig { max_iter: 64 }, WorkerMode::SameThread).unwrap();
        assert_eq!(owner.submit(request(1, 1)), SubmitOutcome::Transferred);
        let lease = producer.next_request().unwrap().unwrap();
        producer
            .complete(lease, &[zero_record()], 64, 10, 250_000)
            .unwrap();
        let mut response = owner.next_arrival().unwrap();
        assert_eq!(
            foreign
                .return_credit(&mut response, OrbitDisposition::Stale, 10)
                .unwrap_err()
                .code,
            crate::ErrorCode::BufferStarved
        );
        owner
            .return_credit(&mut response, OrbitDisposition::Applied, 10)
            .unwrap();
    }

    fn same_thread_ownership_trace() -> OwnershipTrace {
        let (owner, producer) = WorkerChannel::new(
            WorkerConfig {
                max_iter: OWNERSHIP_SCENARIO_MAX_ITER,
            },
            WorkerMode::SameThread,
        )
        .unwrap();
        begin_ownership_trace();
        assert_eq!(
            owner.submit(request_with_cap(
                OWNERSHIP_SCENARIO_GENERATION,
                OWNERSHIP_SCENARIO_GENERATION,
                OWNERSHIP_SCENARIO_MAX_ITER,
            )),
            SubmitOutcome::Transferred
        );
        let lease = producer.next_request().unwrap().unwrap();
        let records = vec![zero_record(); OWNERSHIP_SCENARIO_RECORDS];
        producer
            .complete(lease, &records, OWNERSHIP_SCENARIO_MAX_ITER, 1_000, 250_000)
            .unwrap();
        let mut response = owner.next_arrival().unwrap();
        assert_eq!(response.generation(), OWNERSHIP_SCENARIO_GENERATION);
        owner
            .return_credit(
                &mut response,
                OrbitDisposition::Applied,
                OWNERSHIP_SCENARIO_OWNER_NOW_US,
            )
            .unwrap();
        finish_ownership_trace(owner.facts())
    }

    #[derive(Debug, Eq, PartialEq)]
    enum OwnershipModeDifference {
        EventSlot {
            index: usize,
            phase: OwnershipPhase,
            same_thread: u32,
            browser: u32,
        },
        EventFactEpoch {
            index: usize,
            phase: OwnershipPhase,
            same_thread: u64,
            browser: u64,
        },
        FinalFactEpoch {
            same_thread: u64,
            browser: u64,
        },
    }

    fn ownership_mode_differences(
        same_thread: &OwnershipTrace,
        browser: &OwnershipTrace,
    ) -> Vec<OwnershipModeDifference> {
        assert_eq!(same_thread.events.len(), browser.events.len());
        let mut differences = Vec::new();
        for (index, (same_event, browser_event)) in
            same_thread.events.iter().zip(&browser.events).enumerate()
        {
            assert_eq!(same_event.phase, browser_event.phase, "event {index}");
            assert_eq!(same_event.result, browser_event.result, "event {index}");
            assert_eq!(same_event.pool, browser_event.pool, "event {index}");
            assert_eq!(
                same_event.logical_owner, browser_event.logical_owner,
                "event {index}"
            );
            assert_eq!(
                same_event.generation, browser_event.generation,
                "event {index}"
            );
            assert_eq!(
                same_event.pool_epoch, browser_event.pool_epoch,
                "event {index}"
            );
            assert_eq!(
                same_event.credit_us, browser_event.credit_us,
                "event {index}"
            );
            assert_eq!(
                same_event.orbit_queue_depth, browser_event.orbit_queue_depth,
                "event {index}"
            );
            assert_eq!(
                same_event.shutdown_queue_depth, browser_event.shutdown_queue_depth,
                "event {index}"
            );
            assert_eq!(
                same_event.allocation_events, browser_event.allocation_events,
                "event {index}"
            );
            assert_eq!(
                same_event.request_buffers_owned_main, browser_event.request_buffers_owned_main,
                "event {index}"
            );
            assert_eq!(
                same_event.orbit_buffers_owned_main, browser_event.orbit_buffers_owned_main,
                "event {index}"
            );
            if same_event.slot != browser_event.slot {
                differences.push(OwnershipModeDifference::EventSlot {
                    index,
                    phase: same_event.phase,
                    same_thread: same_event.slot,
                    browser: browser_event.slot,
                });
            }
            if same_event.fact_epoch != browser_event.fact_epoch {
                differences.push(OwnershipModeDifference::EventFactEpoch {
                    index,
                    phase: same_event.phase,
                    same_thread: same_event.fact_epoch,
                    browser: browser_event.fact_epoch,
                });
            }
        }

        assert_normalized_facts_equal_except_epoch(same_thread.facts, browser.facts);
        if same_thread.facts.epoch != browser.facts.epoch {
            differences.push(OwnershipModeDifference::FinalFactEpoch {
                same_thread: same_thread.facts.epoch,
                browser: browser.facts.epoch,
            });
        }
        differences
    }

    fn assert_normalized_facts_equal_except_epoch(
        same_thread: NormalizedWorkerFacts,
        browser: NormalizedWorkerFacts,
    ) {
        assert_eq!(
            same_thread.last_applied_generation,
            browser.last_applied_generation
        );
        assert_eq!(same_thread.last_ack_generation, browser.last_ack_generation);
        assert_eq!(same_thread.orbit_queue_depth, browser.orbit_queue_depth);
        assert_eq!(
            same_thread.shutdown_queue_depth,
            browser.shutdown_queue_depth
        );
        assert_eq!(same_thread.credit_us, browser.credit_us);
        assert_eq!(same_thread.applied_count, browser.applied_count);
        assert_eq!(same_thread.stale_count, browser.stale_count);
        assert_eq!(same_thread.cancelled_count, browser.cancelled_count);
        assert_eq!(same_thread.allocation_events, browser.allocation_events);
        assert_eq!(
            same_thread.request_buffers_owned_main,
            browser.request_buffers_owned_main
        );
        assert_eq!(
            same_thread.orbit_buffers_owned_main,
            browser.orbit_buffers_owned_main
        );
    }

    const SAME_THREAD_OWNERSHIP_FIXTURE: &str = "OwnershipTrace {\n    events: [\n        OwnershipEvent {\n            phase: RequestDispatched,\n            result: Transferred,\n            pool: Request,\n            slot: 0,\n            logical_owner: Producer,\n            generation: 11,\n            pool_epoch: 0,\n            fact_epoch: 1,\n            credit_us: 250000,\n            orbit_queue_depth: 0,\n            shutdown_queue_depth: 0,\n            allocation_events: 1,\n            request_buffers_owned_main: 1,\n            orbit_buffers_owned_main: 0,\n        },\n        OwnershipEvent {\n            phase: RequestReturned,\n            result: Transferred,\n            pool: Request,\n            slot: 0,\n            logical_owner: Main,\n            generation: 11,\n            pool_epoch: 0,\n            fact_epoch: 3,\n            credit_us: 250000,\n            orbit_queue_depth: 0,\n            shutdown_queue_depth: 0,\n            allocation_events: 1,\n            request_buffers_owned_main: 2,\n            orbit_buffers_owned_main: 0,\n        },\n        OwnershipEvent {\n            phase: ResponseQueued,\n            result: Transferred,\n            pool: Orbit,\n            slot: 0,\n            logical_owner: Main,\n            generation: 11,\n            pool_epoch: 0,\n            fact_epoch: 4,\n            credit_us: 250000,\n            orbit_queue_depth: 1,\n            shutdown_queue_depth: 0,\n            allocation_events: 1,\n            request_buffers_owned_main: 2,\n            orbit_buffers_owned_main: 1,\n        },\n        OwnershipEvent {\n            phase: ResponseLeased,\n            result: Leased,\n            pool: Orbit,\n            slot: 0,\n            logical_owner: Main,\n            generation: 11,\n            pool_epoch: 0,\n            fact_epoch: 5,\n            credit_us: 250000,\n            orbit_queue_depth: 0,\n            shutdown_queue_depth: 0,\n            allocation_events: 1,\n            request_buffers_owned_main: 2,\n            orbit_buffers_owned_main: 1,\n        },\n        OwnershipEvent {\n            phase: CreditReturned,\n            result: Credited(\n                Applied,\n            ),\n            pool: Orbit,\n            slot: 0,\n            logical_owner: Producer,\n            generation: 11,\n            pool_epoch: 0,\n            fact_epoch: 7,\n            credit_us: 249000,\n            orbit_queue_depth: 0,\n            shutdown_queue_depth: 0,\n            allocation_events: 1,\n            request_buffers_owned_main: 2,\n            orbit_buffers_owned_main: 0,\n        },\n    ],\n    facts: NormalizedWorkerFacts {\n        epoch: 7,\n        last_applied_generation: 11,\n        last_ack_generation: 11,\n        orbit_queue_depth: 0,\n        shutdown_queue_depth: 0,\n        credit_us: 249000,\n        applied_count: 1,\n        stale_count: 0,\n        cancelled_count: 0,\n        allocation_events: 1,\n        request_buffers_owned_main: 2,\n        orbit_buffers_owned_main: 0,\n    },\n}";
    const BROWSER_OWNERSHIP_FIXTURE: &str = "OwnershipTrace {\n    events: [\n        OwnershipEvent {\n            phase: RequestDispatched,\n            result: Transferred,\n            pool: Request,\n            slot: 1,\n            logical_owner: Producer,\n            generation: 11,\n            pool_epoch: 0,\n            fact_epoch: 1,\n            credit_us: 250000,\n            orbit_queue_depth: 0,\n            shutdown_queue_depth: 0,\n            allocation_events: 1,\n            request_buffers_owned_main: 1,\n            orbit_buffers_owned_main: 0,\n        },\n        OwnershipEvent {\n            phase: RequestReturned,\n            result: Transferred,\n            pool: Request,\n            slot: 1,\n            logical_owner: Main,\n            generation: 11,\n            pool_epoch: 0,\n            fact_epoch: 3,\n            credit_us: 250000,\n            orbit_queue_depth: 0,\n            shutdown_queue_depth: 0,\n            allocation_events: 1,\n            request_buffers_owned_main: 2,\n            orbit_buffers_owned_main: 0,\n        },\n        OwnershipEvent {\n            phase: ResponseQueued,\n            result: Transferred,\n            pool: Orbit,\n            slot: 0,\n            logical_owner: Main,\n            generation: 11,\n            pool_epoch: 0,\n            fact_epoch: 4,\n            credit_us: 250000,\n            orbit_queue_depth: 1,\n            shutdown_queue_depth: 0,\n            allocation_events: 1,\n            request_buffers_owned_main: 2,\n            orbit_buffers_owned_main: 1,\n        },\n        OwnershipEvent {\n            phase: ResponseLeased,\n            result: Leased,\n            pool: Orbit,\n            slot: 0,\n            logical_owner: Main,\n            generation: 11,\n            pool_epoch: 0,\n            fact_epoch: 5,\n            credit_us: 250000,\n            orbit_queue_depth: 0,\n            shutdown_queue_depth: 0,\n            allocation_events: 1,\n            request_buffers_owned_main: 2,\n            orbit_buffers_owned_main: 1,\n        },\n        OwnershipEvent {\n            phase: CreditReturned,\n            result: Credited(\n                Applied,\n            ),\n            pool: Orbit,\n            slot: 0,\n            logical_owner: Producer,\n            generation: 11,\n            pool_epoch: 0,\n            fact_epoch: 6,\n            credit_us: 249000,\n            orbit_queue_depth: 0,\n            shutdown_queue_depth: 0,\n            allocation_events: 1,\n            request_buffers_owned_main: 2,\n            orbit_buffers_owned_main: 0,\n        },\n    ],\n    facts: NormalizedWorkerFacts {\n        epoch: 6,\n        last_applied_generation: 11,\n        last_ack_generation: 11,\n        orbit_queue_depth: 0,\n        shutdown_queue_depth: 0,\n        credit_us: 249000,\n        applied_count: 1,\n        stale_count: 0,\n        cancelled_count: 0,\n        allocation_events: 1,\n        request_buffers_owned_main: 2,\n        orbit_buffers_owned_main: 0,\n    },\n}";

    #[test]
    fn ownership_modes_match_their_frozen_transition_sequences() {
        let same_thread = format!("{:#?}", same_thread_ownership_trace());
        let browser = format!("{:#?}", crate::endpoint::tests::browser_ownership_trace());
        assert_eq!(same_thread, SAME_THREAD_OWNERSHIP_FIXTURE);
        assert_eq!(browser, BROWSER_OWNERSHIP_FIXTURE);
    }

    #[test]
    fn ownership_modes_freeze_their_transport_differences() {
        let same_thread = same_thread_ownership_trace();
        let browser = crate::endpoint::tests::browser_ownership_trace();
        assert_eq!(
            ownership_mode_differences(&same_thread, &browser),
            vec![
                OwnershipModeDifference::EventSlot {
                    index: 0,
                    phase: OwnershipPhase::RequestDispatched,
                    same_thread: 0,
                    browser: 1,
                },
                OwnershipModeDifference::EventSlot {
                    index: 1,
                    phase: OwnershipPhase::RequestReturned,
                    same_thread: 0,
                    browser: 1,
                },
                OwnershipModeDifference::EventFactEpoch {
                    index: 4,
                    phase: OwnershipPhase::CreditReturned,
                    same_thread: 7,
                    browser: 6,
                },
                OwnershipModeDifference::FinalFactEpoch {
                    same_thread: 7,
                    browser: 6,
                },
            ]
        );
    }

    #[test]
    #[ignore = "prints the transition-site worker fixtures for review and verbatim commit"]
    #[allow(
        clippy::print_stdout,
        reason = "the ignored generator emits its reviewed fixtures"
    )]
    fn print_worker_ownership_trace_fixtures() {
        let same_thread = same_thread_ownership_trace();
        let browser = crate::endpoint::tests::browser_ownership_trace();
        let same_thread_fixture = format!("{same_thread:#?}");
        let browser_fixture = format!("{browser:#?}");
        println!("const SAME_THREAD_OWNERSHIP_FIXTURE: &str = {same_thread_fixture:?};");
        println!("const BROWSER_OWNERSHIP_FIXTURE: &str = {browser_fixture:?};");
    }

    #[test]
    fn both_transports_bind_the_shared_owner_core() {
        let channel_source = include_str!("channel.rs");
        let browser_source = include_str!("browser_owner.rs");
        let endpoint_source = include_str!("endpoint.rs");
        assert!(channel_source.contains("BrowserOwnerEndpoint::new(config)?"));
        assert!(channel_source.contains("OwnerCore<SameThreadPort>"));
        assert!(channel_source.contains("endpoint.return_transfer"));
        assert!(browser_source.contains("OrbitResponseView::from_browser_transfer"));
        assert!(browser_source.contains("impl OwnerPort for BrowserPort"));
        assert!(browser_source.contains("OwnerCore<BrowserPort>"));
        assert!(endpoint_source.contains("fn restart_pool"));
    }

    const fn zero_record() -> ReferenceOrbitRecord {
        ReferenceOrbitRecord { re: 0.0, im: 0.0 }
    }
}
