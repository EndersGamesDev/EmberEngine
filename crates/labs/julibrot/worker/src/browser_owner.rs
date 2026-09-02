//! Main-thread Web Worker endpoint over the four transferable buffers.

#![allow(
    clippy::redundant_pub_crate,
    reason = "the private wasm module shares response helpers with its sibling channel module"
)]

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use js_sys::{Array, ArrayBuffer, Object, Reflect, Uint8Array};
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::JsValue;
use web_sys::{MessageEvent, Worker, WorkerOptions, WorkerType};

use crate::browser::TransferBuffer;
use crate::{
    ChannelError, CreditAccount, ErrorCode, JULIBROT_ABI_VERSION, MessageHeader, MessageKind,
    OrbitDisposition, OrbitRequest, OrbitResponseView, Pool, SubmitOutcome, WorkerConfig,
    WorkerFacts, WorkerMode,
};

const WORKER_URL: &str = "./worker.js?v=1";

/// Main-thread endpoint connected to `worker_main` through one browser Worker port.
#[derive(Clone)]
pub struct BrowserOwnerEndpoint {
    core: Rc<RefCell<BrowserOwnerCore>>,
}

impl std::fmt::Debug for BrowserOwnerEndpoint {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BrowserOwnerEndpoint")
            .field("facts", &self.facts())
            .finish_non_exhaustive()
    }
}

impl BrowserOwnerEndpoint {
    /// Creates the module worker, four-buffer pool, and event-driven owner endpoint.
    ///
    /// # Errors
    ///
    /// Returns a typed length or allocation refusal, or `BufferStarved` when the browser refuses
    /// worker construction or message-listener installation.
    pub fn new(config: WorkerConfig) -> Result<Self, ChannelError> {
        Self::from_worker(config, spawn_worker()?)
    }

    /// Attaches the four-buffer owner endpoint to an app-created module Worker.
    ///
    /// The worker must load the version-one bootstrap at `./worker.js?v=1`; no pool buffer moves
    /// until its object handshake acknowledges ABI version one.
    ///
    /// # Errors
    ///
    /// Returns a typed length or allocation refusal, or `BufferStarved` when the browser refuses
    /// message-listener installation.
    #[allow(
        clippy::needless_pass_by_value,
        reason = "the endpoint takes ownership of the app-provided browser port"
    )]
    pub fn from_worker(config: WorkerConfig, worker: Worker) -> Result<Self, ChannelError> {
        crate::buffer_capacity(config.max_iter)?;
        let mut request_owned = Vec::with_capacity(2);
        let mut orbit_owned = Vec::with_capacity(2);
        for slot in 0..=1 {
            request_owned.push(TransferBuffer::allocate(
                Pool::Request,
                slot,
                config.max_iter,
            )?);
            orbit_owned.push(TransferBuffer::allocate(
                Pool::Orbit,
                slot,
                config.max_iter,
            )?);
        }
        let endpoint = Self {
            core: Rc::new(RefCell::new(BrowserOwnerCore {
                worker: worker.clone(),
                listener: None,
                config,
                request_owned,
                orbit_owned,
                arrivals: Vec::with_capacity(2),
                pending_request: None,
                latest_generation: 0,
                latest_centre_revision: 0,
                last_error: None,
                credit: CreditAccount::new(),
                facts: WorkerFacts::new(WorkerMode::WebWorker),
                orbit_leases: 0,
                ready: false,
                closed: false,
                shutdown_requested: false,
                shutdown_sent: false,
                shutdown_acknowledged: false,
                pending_resize: None,
            })),
        };
        let weak = Rc::downgrade(&endpoint.core);
        let listener = Closure::<dyn FnMut(MessageEvent)>::new(move |event| {
            receive_event(&weak, &event);
        });
        worker
            .add_event_listener_with_callback("message", listener.as_ref().unchecked_ref())
            .map_err(|_| ChannelError::new(ErrorCode::BufferStarved, 0, 0, 0))?;
        endpoint.core.borrow_mut().listener = Some(listener);
        endpoint.core.borrow().post_abi_probe()?;
        endpoint.refresh_facts();
        Ok(endpoint)
    }

    /// Accepts every newer edit and keeps at most one request pending while the port is busy.
    #[must_use]
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
        let requested_cap = request.max_iter();
        core.pending_request = Some(request);
        if requested_cap != core.config.max_iter {
            if requested_cap < crate::MIN_MAX_ITER {
                core.last_error = Some(ChannelError::new(
                    ErrorCode::BadLength,
                    requested_cap,
                    crate::MIN_MAX_ITER,
                    requested_cap,
                ));
            } else if !core.ready && core.request_owned.len() == 2 && core.orbit_owned.len() == 2 {
                if let Err(error) = core.replace_pool(requested_cap) {
                    core.last_error = Some(error);
                }
            } else {
                core.pending_resize = Some(requested_cap);
                core.shutdown_requested = true;
                if let Err(error) = core.return_queued_arrivals() {
                    core.last_error = Some(error);
                }
                if let Err(error) = core.drive_shutdown() {
                    core.last_error = Some(error);
                }
            }
        } else if core.shutdown_requested && !core.closed {
            core.pending_resize = Some(requested_cap);
        }
        let transferred = core.pump_request();
        core.bump_facts();
        core.refresh_facts();
        if transferred {
            SubmitOutcome::Transferred
        } else {
            SubmitOutcome::Coalesced
        }
    }

    /// Returns the next validated response without blocking.
    #[must_use]
    pub fn next_arrival(&self) -> Option<OrbitResponseView> {
        let mut core = self.core.borrow_mut();
        if core.arrivals.is_empty() {
            return None;
        }
        let buffer = core.arrivals.remove(0);
        let header = match buffer.header() {
            Ok(header) => header,
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
        core.orbit_leases = core.orbit_leases.saturating_add(1);
        core.bump_facts();
        core.refresh_facts();
        drop(core);
        match OrbitResponseView::from_browser_transfer(buffer, self.clone(), centre_revision) {
            Ok(response) => Some(response),
            Err(error) => {
                self.core.borrow_mut().last_error = Some(error);
                None
            }
        }
    }

    /// Returns one response buffer through the browser port with owner credit accounting.
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
        if !response.records.belongs_to_browser(self) {
            return Err(ChannelError::new(ErrorCode::BufferStarved, 0, 0, 0));
        }
        response.records.return_credit(disposition, owner_now_us)
    }

    /// Returns and clears the latest typed browser-channel refusal.
    #[must_use]
    pub fn take_error(&self) -> Option<ChannelError> {
        self.core.borrow_mut().last_error.take()
    }

    /// Reports the latest accepted request generation.
    #[must_use]
    pub fn latest_generation(&self) -> u32 {
        self.core.borrow().latest_generation
    }

    /// Reports the single latest-wins pending request.
    #[must_use]
    pub fn pending_request_depth(&self) -> u32 {
        u32::from(self.core.borrow().pending_request.is_some())
    }

    /// Returns one coherent page-visible accounting snapshot.
    #[must_use]
    pub fn facts(&self) -> WorkerFacts {
        self.refresh_facts();
        self.core.borrow().facts
    }

    /// Begins event-driven shutdown; completion is reported by `shutdown_acknowledged`.
    ///
    /// # Errors
    ///
    /// Returns the latest typed port refusal if a shutdown buffer cannot be transferred.
    pub fn shutdown(&self) -> Result<(), ChannelError> {
        let mut core = self.core.borrow_mut();
        core.closed = true;
        core.shutdown_requested = true;
        core.pending_resize = None;
        core.pending_request = None;
        core.return_queued_arrivals()?;
        core.drive_shutdown()?;
        core.bump_facts();
        core.refresh_facts();
        Ok(())
    }

    /// Reports whether the worker returned all four slots and acknowledged shutdown.
    #[must_use]
    pub fn shutdown_acknowledged(&self) -> bool {
        self.core.borrow().shutdown_acknowledged
    }

    pub(crate) fn return_transfer(
        &self,
        buffer: &mut Option<TransferBuffer>,
        disposition: OrbitDisposition,
        owner_now_us: u64,
    ) -> Result<(), ChannelError> {
        let buffer_ref = buffer
            .as_ref()
            .ok_or_else(|| ChannelError::new(ErrorCode::BufferStarved, 0, 0, 0))?;
        let old = buffer_ref.header()?;
        let kind = match disposition {
            OrbitDisposition::Applied => MessageKind::CreditApplied,
            OrbitDisposition::Stale => MessageKind::CreditStale,
        };
        let mut core = self.core.borrow_mut();
        let charge = core.credit.charge(owner_now_us, old.compute_us)?;
        let mut header = MessageHeader::new(kind, old.generation);
        header.precision_bits = old.precision_bits;
        header.compute_us = old.compute_us;
        header.credit_us = charge.credit_us;
        buffer_ref.write_header(header)?;
        core.post_ref(buffer_ref)?;
        drop(buffer.take());
        core.record_credit(old, disposition, charge.overfeed_us);
        core.orbit_leases = core.orbit_leases.saturating_sub(1);
        core.bump_facts();
        core.refresh_facts();
        Ok(())
    }

    fn refresh_facts(&self) {
        self.core.borrow_mut().refresh_facts();
    }

    pub(crate) fn same_channel(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.core, &other.core)
    }
}

#[allow(
    clippy::struct_excessive_bools,
    reason = "each boolean is a separately reported wire lifecycle fact"
)]
struct BrowserOwnerCore {
    worker: Worker,
    listener: Option<Closure<dyn FnMut(MessageEvent)>>,
    config: WorkerConfig,
    request_owned: Vec<TransferBuffer>,
    orbit_owned: Vec<TransferBuffer>,
    arrivals: Vec<TransferBuffer>,
    pending_request: Option<OrbitRequest>,
    latest_generation: u32,
    latest_centre_revision: u32,
    last_error: Option<ChannelError>,
    credit: CreditAccount,
    facts: WorkerFacts,
    orbit_leases: u32,
    ready: bool,
    closed: bool,
    shutdown_requested: bool,
    shutdown_sent: bool,
    shutdown_acknowledged: bool,
    pending_resize: Option<u32>,
}

impl BrowserOwnerCore {
    fn receive(&mut self, event: &MessageEvent) -> Result<(), ChannelError> {
        let data = event.data();
        if let Ok(array) = data.clone().dyn_into::<ArrayBuffer>() {
            return self.receive_buffer(TransferBuffer::from_array(array)?);
        }
        let Some(kind) = object_string(&data, "kind") else {
            return Ok(());
        };
        match kind.as_str() {
            "WorkerReady" => {
                if object_u32(&data, "version") != Some(JULIBROT_ABI_VERSION) {
                    return Err(ChannelError::new(ErrorCode::BadVersion, 0, 0, 0));
                }
                self.post_abi_probe()
            }
            "AbiAccepted" => {
                let version = object_u32(&data, "version").unwrap_or(0);
                if version != JULIBROT_ABI_VERSION {
                    return Err(ChannelError::new(ErrorCode::BadVersion, version, 0, 0));
                }
                self.ready = true;
                while let Some(buffer) = self.orbit_owned.pop() {
                    self.post(buffer)?;
                }
                self.pump_request();
                self.drive_shutdown()?;
                self.bump_facts();
                self.refresh_facts();
                Ok(())
            }
            "VersionSkew" | "ChannelError" => {
                Err(ChannelError::new(ErrorCode::BadVersion, 0, 0, 0))
            }
            _ => Ok(()),
        }
    }

    fn receive_buffer(&mut self, buffer: TransferBuffer) -> Result<(), ChannelError> {
        let kind = buffer.validate_message()?;
        let (pool, _) = buffer.identity()?;
        match (pool, kind) {
            (Pool::Request, MessageKind::RequestReturn) => {
                push_unique(&mut self.request_owned, buffer)?;
                if self.shutdown_requested {
                    self.drive_shutdown()?;
                } else {
                    self.pump_request();
                }
            }
            (Pool::Orbit, MessageKind::OrbitResponse | MessageKind::OrbitCancelled) => {
                if self.shutdown_requested {
                    self.return_stale_buffer(buffer)?;
                } else {
                    if self.arrivals.len() == 2 {
                        return Err(ChannelError::new(ErrorCode::BufferStarved, 2, 0, 0));
                    }
                    self.arrivals.push(buffer);
                }
            }
            (Pool::Orbit, MessageKind::CreditStale) if self.shutdown_requested => {
                push_unique(&mut self.orbit_owned, buffer)?;
            }
            (Pool::Request, MessageKind::ShutdownAck) if self.shutdown_requested => {
                push_unique(&mut self.request_owned, buffer)?;
                self.shutdown_acknowledged = self.request_owned.len() == 2
                    && self.orbit_owned.len() == 2
                    && self.arrivals.is_empty()
                    && self.orbit_leases == 0;
                self.restart_after_resize()?;
            }
            (Pool::Orbit, MessageKind::ChannelError) => {
                self.return_error_buffer(buffer)?;
            }
            (_, _) => {
                return Err(ChannelError::new(
                    ErrorCode::BadKind,
                    buffer.header()?.kind,
                    0,
                    0,
                ));
            }
        }
        self.bump_facts();
        self.refresh_facts();
        Ok(())
    }

    fn pump_request(&mut self) -> bool {
        if !self.ready || self.closed || self.pending_request.is_none() {
            return false;
        }
        let Some(buffer) = self.request_owned.pop() else {
            return false;
        };
        let Some(request) = self.pending_request.take() else {
            self.request_owned.push(buffer);
            return false;
        };
        if request.max_iter() != self.config.max_iter {
            self.last_error = Some(ChannelError::new(
                ErrorCode::BadLength,
                request.max_iter(),
                self.config.max_iter,
                self.config.max_iter,
            ));
            self.pending_request = Some(request);
            self.request_owned.push(buffer);
            return false;
        }
        if let Err(error) = crate::encode_transfer_request(buffer.array(), &request) {
            self.last_error = Some(error);
            self.pending_request = Some(request);
            self.request_owned.push(buffer);
            return false;
        }
        if let Err(error) = self.post(buffer) {
            self.last_error = Some(error);
            return false;
        }
        true
    }

    fn return_error_buffer(&mut self, buffer: TransferBuffer) -> Result<(), ChannelError> {
        let error = buffer.channel_error()?;
        self.return_stale_buffer(buffer)?;
        self.last_error = Some(error);
        Ok(())
    }

    fn return_stale_buffer(&mut self, buffer: TransferBuffer) -> Result<(), ChannelError> {
        let old = buffer.header()?;
        let charge = self.credit.charge(browser_now_us()?, old.compute_us)?;
        let mut header = MessageHeader::new(MessageKind::CreditStale, old.generation);
        header.precision_bits = old.precision_bits;
        header.compute_us = old.compute_us;
        header.credit_us = charge.credit_us;
        buffer.write_header(header)?;
        self.post(buffer)?;
        self.record_credit(old, OrbitDisposition::Stale, charge.overfeed_us);
        Ok(())
    }

    fn return_queued_arrivals(&mut self) -> Result<(), ChannelError> {
        while !self.arrivals.is_empty() {
            let buffer = self.arrivals.remove(0);
            self.return_stale_buffer(buffer)?;
        }
        Ok(())
    }

    fn drive_shutdown(&mut self) -> Result<(), ChannelError> {
        if !self.ready
            || !self.shutdown_requested
            || self.shutdown_sent
            || self.shutdown_acknowledged
        {
            return Ok(());
        }
        let Some(buffer) = self.request_owned.pop() else {
            return Ok(());
        };
        buffer.write_empty(MessageKind::Shutdown, self.latest_generation, 0, 0)?;
        self.post(buffer)?;
        self.shutdown_sent = true;
        Ok(())
    }

    fn restart_after_resize(&mut self) -> Result<(), ChannelError> {
        let Some(max_iter) = self.pending_resize else {
            return Ok(());
        };
        if !self.shutdown_acknowledged {
            return Ok(());
        }
        self.worker.terminate();
        let worker = spawn_worker()?;
        let listener = self
            .listener
            .as_ref()
            .ok_or_else(|| ChannelError::new(ErrorCode::BufferStarved, 0, 0, 0))?;
        worker
            .add_event_listener_with_callback("message", listener.as_ref().unchecked_ref())
            .map_err(|_| ChannelError::new(ErrorCode::BufferStarved, 0, 0, 0))?;
        self.worker = worker;
        self.replace_pool(max_iter)?;
        self.ready = false;
        self.shutdown_requested = false;
        self.shutdown_sent = false;
        self.shutdown_acknowledged = false;
        self.pending_resize = None;
        self.post_abi_probe()
    }

    fn replace_pool(&mut self, max_iter: u32) -> Result<(), ChannelError> {
        let mut request_owned = Vec::with_capacity(2);
        let mut orbit_owned = Vec::with_capacity(2);
        for slot in 0..=1 {
            request_owned.push(TransferBuffer::allocate(Pool::Request, slot, max_iter)?);
            orbit_owned.push(TransferBuffer::allocate(Pool::Orbit, slot, max_iter)?);
        }
        self.request_owned = request_owned;
        self.orbit_owned = orbit_owned;
        self.config.max_iter = max_iter;
        self.facts.allocation_events = self.facts.allocation_events.saturating_add(1);
        self.bump_facts();
        self.refresh_facts();
        Ok(())
    }

    #[allow(
        clippy::needless_pass_by_value,
        reason = "posting consumes logical ownership and drops the detached wrapper"
    )]
    fn post(&self, buffer: TransferBuffer) -> Result<(), ChannelError> {
        self.post_ref(&buffer)
    }

    fn post_ref(&self, buffer: &TransferBuffer) -> Result<(), ChannelError> {
        let array = buffer.array();
        let transfer = Array::new();
        transfer.push(array.as_ref());
        self.worker
            .post_message_with_transfer(array.as_ref(), transfer.as_ref())
            .map_err(|_| ChannelError::new(ErrorCode::BufferStarved, 0, 0, 0))?;
        debug_assert_eq!(array.byte_length(), 0);
        Ok(())
    }

    fn post_abi_probe(&self) -> Result<(), ChannelError> {
        let probe = Object::new();
        Reflect::set(
            probe.as_ref(),
            &JsValue::from_str("kind"),
            &JsValue::from_str("AbiProbe"),
        )
        .map_err(|_| ChannelError::new(ErrorCode::BadVersion, 0, 0, 0))?;
        Reflect::set(
            probe.as_ref(),
            &JsValue::from_str("version"),
            &JsValue::from_f64(f64::from(JULIBROT_ABI_VERSION)),
        )
        .map_err(|_| ChannelError::new(ErrorCode::BadVersion, 0, 0, 0))?;
        self.worker
            .post_message(probe.as_ref())
            .map_err(|_| ChannelError::new(ErrorCode::BufferStarved, 0, 0, 0))
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
        self.facts.shutdown_queue_depth =
            u32::from(self.shutdown_requested && !self.shutdown_acknowledged);
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

fn receive_event(core: &Weak<RefCell<BrowserOwnerCore>>, event: &MessageEvent) {
    let Some(core) = core.upgrade() else {
        return;
    };
    let result = core.borrow_mut().receive(event);
    if let Err(error) = result {
        core.borrow_mut().last_error = Some(error);
    }
}

fn push_unique(
    buffers: &mut Vec<TransferBuffer>,
    buffer: TransferBuffer,
) -> Result<(), ChannelError> {
    let identity = buffer.identity()?;
    if buffers
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
    buffers.push(buffer);
    Ok(())
}

fn object_string(value: &JsValue, name: &str) -> Option<String> {
    Reflect::get(value, &JsValue::from_str(name))
        .ok()
        .and_then(|field| field.as_string())
}

fn object_u32(value: &JsValue, name: &str) -> Option<u32> {
    let number = Reflect::get(value, &JsValue::from_str(name))
        .ok()?
        .as_f64()?;
    if number.fract() != 0.0 || !(0.0..=f64::from(u32::MAX)).contains(&number) {
        return None;
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Some(number as u32)
}

fn spawn_worker() -> Result<Worker, ChannelError> {
    let options = WorkerOptions::new();
    options.set_type(WorkerType::Module);
    options.set_name("julibrot-orbit");
    Worker::new_with_options(WORKER_URL, &options)
        .map_err(|_| ChannelError::new(ErrorCode::BufferStarved, 0, 0, 0))
}

fn browser_now_us() -> Result<u64, ChannelError> {
    let performance = Reflect::get(&js_sys::global(), &JsValue::from_str("performance"))
        .map_err(|_| ChannelError::new(ErrorCode::TimingOverflow, 0, 0, 0))?;
    let now = Reflect::get(&performance, &JsValue::from_str("now"))
        .ok()
        .and_then(|method| method.dyn_into::<js_sys::Function>().ok())
        .ok_or_else(|| ChannelError::new(ErrorCode::TimingOverflow, 0, 0, 0))?;
    let milliseconds = now
        .call0(&performance)
        .ok()
        .and_then(|value| value.as_f64())
        .ok_or_else(|| ChannelError::new(ErrorCode::TimingOverflow, 0, 0, 0))?;
    if !milliseconds.is_finite() || milliseconds < 0.0 {
        return Err(ChannelError::new(ErrorCode::TimingOverflow, 0, 0, 0));
    }
    let microseconds = (milliseconds * 1_000.0).ceil();
    if microseconds > 18_446_744_073_709_551_615.0 {
        return Err(ChannelError::new(ErrorCode::TimingOverflow, 0, 0, 0));
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Ok(microseconds as u64)
}

/// Returns a zero-copy view of a browser response's initialized record bytes.
///
/// # Errors
///
/// Returns a typed wire refusal for a detached, corrupt, or non-response transfer.
pub(crate) fn response_record_bytes(buffer: &TransferBuffer) -> Result<Uint8Array, ChannelError> {
    buffer.record_bytes()
}
