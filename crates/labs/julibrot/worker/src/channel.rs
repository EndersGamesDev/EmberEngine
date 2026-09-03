//! Same-thread lowering of the bounded ownership-transfer channel.

use std::cell::RefCell;
use std::rc::Rc;

#[cfg(target_arch = "wasm32")]
use js_sys::{ArrayBuffer, Uint8Array};

#[cfg(target_arch = "wasm32")]
use crate::browser::TransferBuffer;
#[cfg(target_arch = "wasm32")]
use crate::browser_owner::BrowserOwnerEndpoint;
use crate::slots::{FourSlotModel, SlotId};
use crate::wire::{HEADER_BYTES, ORBIT_RECORD_BYTES, OrbitVerificationFacts, Pool, WireBuffer};
use crate::{
    Admission, ChannelError, CreditAccount, ErrorCode, MessageHeader, MessageKind,
    OrbitDisposition, OrbitRequest, ProducerShaper, ReferenceOrbitRecord, ReferenceVerification,
    WorkerFacts,
};

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
            credit: CreditAccount::new(),
            shaper: ProducerShaper::new(),
            pending_producer_credits: BoundedQueue::new(),
            facts: WorkerFacts::new(mode),
            orbit_leases: 0,
        }));
        Ok((
            OwnerEndpoint {
                backend: OwnerBackend::Queue(Rc::clone(&core)),
            },
            ProducerEndpoint {
                backend: ProducerBackend::Queue(core),
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
    Queue(Rc<RefCell<ChannelCore>>),
    #[cfg(target_arch = "wasm32")]
    Browser(BrowserOwnerEndpoint),
}

impl OwnerEndpoint {
    /// Accepts every newer edit immediately and keeps at most one untransferred request.
    #[must_use]
    pub fn submit(&self, request: OrbitRequest) -> SubmitOutcome {
        #[cfg(target_arch = "wasm32")]
        if let OwnerBackend::Browser(browser) = &self.backend {
            return browser.submit(request);
        }
        let Some(queue) = self.queue_core() else {
            return SubmitOutcome::GenerationExhausted;
        };
        let mut core = queue.borrow_mut();
        if core.closed || core.latest_generation == u32::MAX {
            return SubmitOutcome::GenerationExhausted;
        }
        if request.generation() <= core.latest_generation {
            return SubmitOutcome::Coalesced;
        }
        core.latest_generation = request.generation();
        core.latest_centre_revision = request.centre().revision;
        let outcome = match core.try_dispatch(request) {
            Ok(None) => SubmitOutcome::Transferred,
            Ok(Some(pending)) => {
                core.pending_request = Some(pending);
                SubmitOutcome::Coalesced
            }
            Err(error) => {
                core.last_error = Some(error);
                SubmitOutcome::Coalesced
            }
        };
        core.bump_facts();
        core.refresh_facts();
        outcome
    }

    /// Returns the next completed response without blocking.
    #[must_use]
    pub fn next_arrival(&self) -> Option<OrbitResponseView> {
        #[cfg(target_arch = "wasm32")]
        if let OwnerBackend::Browser(browser) = &self.backend {
            return browser.next_arrival();
        }
        let queue = self.queue_core()?;
        let mut core = queue.borrow_mut();
        let buffer = core.orbit_to_main.pop()?;
        let (header, cancelled) = match buffer.header() {
            Ok(header) if header.validate() == Ok(MessageKind::OrbitResponse) => (header, false),
            Ok(header) if header.validate() == Ok(MessageKind::OrbitCancelled) => (header, true),
            Ok(header) => {
                core.last_error = Some(ChannelError::new(ErrorCode::BadKind, header.kind, 0, 0));
                return None;
            }
            Err(error) => {
                core.last_error = Some(error);
                return None;
            }
        };
        core.orbit_leases += 1;
        core.bump_facts();
        core.refresh_facts();
        let centre_revision = if header.generation == core.latest_generation {
            core.latest_centre_revision
        } else {
            0
        };
        let verification_facts = if cancelled {
            OrbitVerificationFacts::deferred()
        } else {
            match buffer.orbit_facts() {
                Ok(facts) => facts,
                Err(error) => {
                    core.last_error = Some(error);
                    return None;
                }
            }
        };
        Some(OrbitResponseView {
            generation: header.generation,
            centre_revision,
            length: header.length,
            compute_us: header.compute_us,
            precision_bits: header.precision_bits,
            admission_credit_us: header.credit_us,
            verification_facts,
            cancelled,
            records: OrbitLease {
                backend: OrbitLeaseBackend::Queue {
                    core: Rc::clone(queue),
                    buffer: Some(buffer),
                },
            },
        })
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
        let belongs = match (&self.backend, &response.records.backend) {
            (OwnerBackend::Queue(owner), OrbitLeaseBackend::Queue { core: response, .. }) => {
                Rc::ptr_eq(owner, response)
            }
            #[cfg(target_arch = "wasm32")]
            (
                OwnerBackend::Browser(owner),
                OrbitLeaseBackend::Browser {
                    endpoint: Some(response),
                    ..
                },
            ) => owner.same_channel(response),
            #[cfg(target_arch = "wasm32")]
            _ => false,
        };
        if !belongs {
            return Err(ChannelError::new(ErrorCode::BufferStarved, 0, 0, 0));
        }
        response.records.return_credit(disposition, owner_now_us)
    }

    /// Returns and clears the latest typed internal channel refusal.
    #[must_use]
    pub fn take_error(&self) -> Option<ChannelError> {
        #[cfg(target_arch = "wasm32")]
        if let OwnerBackend::Browser(browser) = &self.backend {
            return browser.take_error();
        }
        self.queue_core()?.borrow_mut().last_error.take()
    }

    /// Reports the latest submitted generation.
    #[must_use]
    pub fn latest_generation(&self) -> u32 {
        #[cfg(target_arch = "wasm32")]
        if let OwnerBackend::Browser(browser) = &self.backend {
            return browser.latest_generation();
        }
        self.queue_core()
            .map_or(u32::MAX, |queue| queue.borrow().latest_generation)
    }

    /// Reports one coalesced request when producer delivery is saturated.
    #[must_use]
    pub fn pending_request_depth(&self) -> u32 {
        #[cfg(target_arch = "wasm32")]
        if let OwnerBackend::Browser(browser) = &self.backend {
            return browser.pending_request_depth();
        }
        self.queue_core().map_or(0, |queue| {
            u32::from(queue.borrow().pending_request.is_some())
        })
    }

    /// Returns one coherent copy of the page-visible channel accounting.
    #[must_use]
    pub fn facts(&self) -> WorkerFacts {
        #[cfg(target_arch = "wasm32")]
        if let OwnerBackend::Browser(browser) = &self.backend {
            return browser.facts();
        }
        let Some(queue) = self.queue_core() else {
            return WorkerFacts::new(WorkerMode::WebWorker);
        };
        let mut core = queue.borrow_mut();
        core.refresh_facts();
        core.facts
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
        #[cfg(target_arch = "wasm32")]
        if let OwnerBackend::Browser(browser) = &self.backend {
            return browser.shutdown();
        }
        let Some(queue) = self.queue_core() else {
            return Err(ChannelError::new(
                ErrorCode::UnexpectedWork,
                WorkerMode::WebWorker as u32,
                0,
                0,
            ));
        };
        let mut core = queue.borrow_mut();
        if !core.is_reconciled() {
            return Err(ChannelError::new(ErrorCode::BufferStarved, 0, 0, 0));
        }
        core.closed = true;
        Ok(())
    }

    /// Reports completed same-thread closure or browser four-slot acknowledgement.
    #[must_use]
    pub fn shutdown_acknowledged(&self) -> bool {
        #[cfg(target_arch = "wasm32")]
        if let OwnerBackend::Browser(browser) = &self.backend {
            return browser.shutdown_acknowledged();
        }
        self.queue_core().is_some_and(|queue| {
            let core = queue.borrow();
            core.closed && core.is_reconciled()
        })
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[allow(
        clippy::unnecessary_wraps,
        reason = "the matching wasm accessor can refuse a browser backend without trapping"
    )]
    const fn queue_core(&self) -> Option<&Rc<RefCell<ChannelCore>>> {
        match &self.backend {
            OwnerBackend::Queue(core) => Some(core),
        }
    }

    #[cfg(target_arch = "wasm32")]
    const fn queue_core(&self) -> Option<&Rc<RefCell<ChannelCore>>> {
        match &self.backend {
            OwnerBackend::Queue(core) => Some(core),
            OwnerBackend::Browser(_) => None,
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
    Queue(Rc<RefCell<ChannelCore>>),
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
        let Some(queue) = self.queue_core() else {
            return Err(browser_producer_refusal());
        };
        let mut core = queue.borrow_mut();
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
        let Some(queue) = self.queue_core() else {
            return Err(browser_producer_refusal());
        };
        let mut core = queue.borrow_mut();
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
            facts,
        )?;
        core.send_to_main(orbit, MessageKind::OrbitResponse)?;
        core.pump_pending();
        Ok(())
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
        let Some(queue) = self.queue_core() else {
            return Err(browser_producer_refusal());
        };
        let mut core = queue.borrow_mut();
        core.return_request(lease.buffer, generation, MessageKind::RequestReturn)?;
        let mut orbit = core
            .orbit_producer
            .pop()
            .ok_or_else(|| ChannelError::new(ErrorCode::BufferStarved, 0, 0, 0))?;
        let mut header = MessageHeader::new(MessageKind::OrbitCancelled, generation);
        header.compute_us = compute_us;
        header.credit_us = admission_credit_us;
        orbit.write_header(header)?;
        core.send_to_main(orbit, MessageKind::OrbitCancelled)?;
        core.pump_pending();
        Ok(())
    }

    /// Applies producer-side admission shaping at a monotonic producer timestamp.
    ///
    /// # Errors
    ///
    /// Returns `TimingOverflow` if producer time moves backwards.
    pub fn admit(&self, producer_now_us: u64) -> Result<Admission, ChannelError> {
        let Some(queue) = self.queue_core() else {
            return Err(browser_producer_refusal());
        };
        let mut core = queue.borrow_mut();
        while let Some(returned) = core.pending_producer_credits.pop() {
            core.shaper
                .observe_return(producer_now_us, returned.credit_us, returned.compute_us)?;
        }
        core.shaper.admit(producer_now_us)
    }

    /// Returns the shared page-visible accounting from the producer endpoint.
    #[must_use]
    pub fn facts(&self) -> WorkerFacts {
        #[cfg(target_arch = "wasm32")]
        if let ProducerBackend::Browser(browser) = &self.backend {
            return browser.facts();
        }
        let Some(queue) = self.queue_core() else {
            return WorkerFacts::new(WorkerMode::WebWorker);
        };
        let mut core = queue.borrow_mut();
        core.refresh_facts();
        core.facts
    }

    /// Reports this endpoint's configured lowering.
    #[must_use]
    pub fn mode(&self) -> WorkerMode {
        match &self.backend {
            ProducerBackend::Queue(core) => core.borrow().mode,
            #[cfg(target_arch = "wasm32")]
            ProducerBackend::Browser(_) => WorkerMode::WebWorker,
        }
    }

    #[allow(
        clippy::unnecessary_wraps,
        reason = "wasm32 has a browser backend with no in-process producer core"
    )]
    const fn queue_core(&self) -> Option<&Rc<RefCell<ChannelCore>>> {
        match &self.backend {
            ProducerBackend::Queue(core) => Some(core),
            #[cfg(target_arch = "wasm32")]
            ProducerBackend::Browser(_) => None,
        }
    }
}

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
    /// Adopts and validates one browser-transferred orbit buffer.
    ///
    /// The standalone view has no owner port; use `BrowserOwnerEndpoint::next_arrival` when the
    /// buffer must later be returned as credit.
    ///
    /// # Errors
    ///
    /// Returns the same trailer, header, pool, kind, length, and unused-byte refusals as the
    /// same-thread path.
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
    Queue {
        core: Rc<RefCell<ChannelCore>>,
        buffer: Option<WireBuffer>,
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
            OrbitLeaseBackend::Queue { buffer, .. } => buffer,
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
        let (core, buffer) = match &mut self.backend {
            OrbitLeaseBackend::Queue { core, buffer } => (core, buffer),
            #[cfg(target_arch = "wasm32")]
            OrbitLeaseBackend::Browser {
                endpoint,
                buffer,
                pool_epoch,
            } => {
                let endpoint = endpoint.as_ref().ok_or_else(|| {
                    ChannelError::new(ErrorCode::BufferStarved, Pool::Orbit as u32, 0, 0)
                })?;
                return endpoint.return_transfer(buffer, *pool_epoch, disposition, owner_now_us);
            }
        };
        let old = buffer
            .as_ref()
            .ok_or_else(|| ChannelError::new(ErrorCode::BufferStarved, 0, 0, 0))?
            .header()?;
        let kind = match disposition {
            OrbitDisposition::Applied => MessageKind::CreditApplied,
            OrbitDisposition::Stale => MessageKind::CreditStale,
        };
        let mut core = core.borrow_mut();
        let charge = core.credit.charge(owner_now_us, old.compute_us)?;
        let mut buffer = buffer
            .take()
            .ok_or_else(|| ChannelError::new(ErrorCode::BufferStarved, 0, 0, 0))?;
        let mut header = MessageHeader::new(kind, old.generation);
        header.precision_bits = old.precision_bits;
        header.compute_us = old.compute_us;
        header.credit_us = charge.credit_us;
        buffer.write_header(header)?;
        core.record_credit(old, disposition, charge.overfeed_us);
        core.return_to_producer(buffer, kind)
    }
}

impl Drop for OrbitLease {
    fn drop(&mut self) {
        let returned = match &self.backend {
            OrbitLeaseBackend::Queue { buffer, .. } => buffer.is_none(),
            #[cfg(target_arch = "wasm32")]
            OrbitLeaseBackend::Browser {
                endpoint, buffer, ..
            } => endpoint.is_none() || buffer.is_none(),
        };
        debug_assert!(returned, "orbit lease dropped without credit return");
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
    credit: CreditAccount,
    shaper: ProducerShaper,
    pending_producer_credits: BoundedQueue<ReturnedCredit>,
    facts: WorkerFacts,
    orbit_leases: u32,
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
        self.bump_facts();
        self.refresh_facts();
        Ok(None)
    }

    /// Returns one request slot without dispatching: the caller still owes main its orbit, and a
    /// coalesced cap change must not replace the orbit pool underneath that write.
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
        self.bump_facts();
        self.refresh_facts();
        Ok(())
    }

    fn send_to_main(&mut self, buffer: WireBuffer, kind: MessageKind) -> Result<(), ChannelError> {
        let id = id_for(&buffer)?;
        self.slots.begin(id, kind)?;
        self.slots.deliver(id)?;
        self.orbit_to_main
            .push(buffer)
            .map_err(|_| ChannelError::new(ErrorCode::BufferStarved, id.slot, 0, 0))?;
        self.bump_facts();
        self.refresh_facts();
        Ok(())
    }

    fn return_to_producer(
        &mut self,
        buffer: WireBuffer,
        kind: MessageKind,
    ) -> Result<(), ChannelError> {
        let id = id_for(&buffer)?;
        let header = buffer.header()?;
        self.slots.begin(id, kind)?;
        self.slots.deliver(id)?;
        self.orbit_producer
            .push(buffer)
            .map_err(|_| ChannelError::new(ErrorCode::BufferStarved, id.slot, 0, 0))?;
        self.orbit_leases = self.orbit_leases.saturating_sub(1);
        self.pending_producer_credits
            .push(ReturnedCredit {
                credit_us: header.credit_us,
                compute_us: header.compute_us,
            })
            .map_err(|_| ChannelError::new(ErrorCode::BufferStarved, id.slot, 0, 0))?;
        self.pump_pending();
        self.bump_facts();
        self.refresh_facts();
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
        self.shaper.reset_for_resize();
        self.pending_producer_credits = BoundedQueue::new();
        self.facts.allocation_events = self.facts.allocation_events.saturating_add(1);
        self.bump_facts();
        self.refresh_facts();
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
        self.bump_facts();
    }

    fn refresh_facts(&mut self) {
        self.facts.orbit_queue_depth = u32::try_from(self.orbit_to_main.len()).unwrap_or(u32::MAX);
        self.facts.request_buffers_owned_main =
            u32::try_from(self.request_main.len()).unwrap_or(u32::MAX);
        self.facts.orbit_buffers_owned_main = u32::try_from(self.orbit_to_main.len())
            .unwrap_or(u32::MAX)
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

#[derive(Clone, Copy, Debug)]
struct ReturnedCredit {
    credit_us: u32,
    compute_us: u32,
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
    use ember_julibrot_math::PrecisionMode;

    use super::{
        Admission, SubmitOutcome, WorkerChannel, WorkerConfig, WorkerMode, worker_mode_from_search,
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

    #[test]
    fn logical_trace_and_browser_binding_are_mode_equivalent_contracts() {
        fn trace(mode: WorkerMode) -> ((u32, u32, u32, bool), crate::WorkerFacts) {
            let (owner, producer) =
                WorkerChannel::new(WorkerConfig { max_iter: 64 }, mode).unwrap();
            assert!(matches!(
                producer.admit(100).unwrap(),
                Admission::Ready { warm_up: true, .. }
            ));
            assert_eq!(owner.submit(request(11, 12)), SubmitOutcome::Transferred);
            let lease = producer.next_request().unwrap().unwrap();
            producer
                .complete(lease, &[zero_record()], 96, 1_000, 250_000)
                .unwrap();
            let mut response = owner.next_arrival().unwrap();
            let observed = (
                response.generation(),
                response.length(),
                response.precision_bits(),
                response.cancelled(),
            );
            response
                .records
                .return_credit(OrbitDisposition::Applied, 200)
                .unwrap();
            (observed, owner.facts())
        }

        let (same_observed, mut same_facts) = trace(WorkerMode::SameThread);
        let (web_observed, mut web_facts) = trace(WorkerMode::WebWorker);
        assert_eq!(same_observed, web_observed);
        same_facts.mode = 0;
        web_facts.mode = 0;
        assert_eq!(same_facts, web_facts);

        let channel_source = include_str!("channel.rs");
        let browser_source = include_str!("browser_owner.rs");
        let endpoint_source = include_str!("endpoint.rs");
        assert!(channel_source.contains("BrowserOwnerEndpoint::new(config)?"));
        assert!(channel_source.contains("endpoint.return_transfer"));
        assert!(browser_source.contains("OrbitResponseView::from_browser_transfer"));
        assert!(browser_source.contains("impl OwnerPort for BrowserPort"));
        assert!(browser_source.contains("OwnerCore<BrowserPort>"));
        assert!(endpoint_source.contains("fn restart_pool"));
    }

    const fn zero_record() -> ReferenceOrbitRecord {
        ReferenceOrbitRecord {
            re: 0.0,
            im: 0.0,
        }
    }
}
