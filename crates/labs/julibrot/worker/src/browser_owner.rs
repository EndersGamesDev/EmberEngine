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
use crate::endpoint::{ControlMessage, OwnerCore, OwnerPort, OwnerSlot};
use crate::{
    ChannelError, ErrorCode, MessageHeader, MessageKind, OrbitDisposition, OrbitRequest,
    OrbitResponseView, Pool, SubmitOutcome, WorkerConfig, WorkerFacts,
};

const WORKER_URL_GLOBAL: &str = "JULIBROT_WORKER_URL";
const WORKER_URL_FALLBACK: &str = "./worker.js?v=1";

/// Main-thread endpoint connected to `worker_main` through one browser Worker port.
#[derive(Clone)]
pub struct BrowserOwnerEndpoint {
    core: Rc<RefCell<OwnerCore<BrowserPort>>>,
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
    /// The worker was created from the page-published deployment URL; no pool buffer moves until
    /// its object handshake acknowledges the active ABI version.
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
        let port = BrowserPort {
            worker,
            listener: None,
        };
        let endpoint = Self {
            core: Rc::new(RefCell::new(OwnerCore::new(port, config)?)),
        };
        let weak = Rc::downgrade(&endpoint.core);
        let listener = Closure::<dyn FnMut(MessageEvent)>::new(move |event| {
            receive_event(&weak, &event);
        });
        let mut core = endpoint.core.borrow_mut();
        core.port_mut().install_listener(listener)?;
        core.probe_abi()?;
        drop(core);
        Ok(endpoint)
    }

    /// Accepts every newer edit and keeps at most one request pending while the port is busy.
    #[must_use]
    pub fn submit(&self, request: OrbitRequest) -> SubmitOutcome {
        self.core.borrow_mut().submit(request)
    }

    /// Returns the next validated response without blocking.
    #[must_use]
    pub fn next_arrival(&self) -> Option<OrbitResponseView> {
        let (buffer, centre_revision, epoch) = {
            let mut core = self.core.borrow_mut();
            let (buffer, centre_revision) = core.take_arrival()?;
            (buffer, centre_revision, core.pool_epoch())
        };
        match OrbitResponseView::from_browser_transfer(buffer, self.clone(), centre_revision, epoch)
        {
            Ok(response) => Some(response),
            Err(error) => {
                self.core.borrow_mut().publish_error(error);
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
        self.core.borrow_mut().take_error()
    }

    /// Reports the latest accepted request generation.
    #[must_use]
    pub fn latest_generation(&self) -> u32 {
        self.core.borrow().latest_generation()
    }

    /// Reports the single latest-wins pending request.
    #[must_use]
    pub fn pending_request_depth(&self) -> u32 {
        self.core.borrow_mut().pending_request_depth()
    }

    /// Returns one coherent page-visible accounting snapshot.
    #[must_use]
    pub fn facts(&self) -> WorkerFacts {
        self.core.borrow_mut().facts()
    }

    /// Begins event-driven shutdown; completion is reported by `shutdown_acknowledged`.
    ///
    /// # Errors
    ///
    /// Returns the latest typed port refusal if a shutdown buffer cannot be transferred.
    pub fn shutdown(&self) -> Result<(), ChannelError> {
        self.core.borrow_mut().shutdown()
    }

    /// Reports whether the worker returned all four slots and acknowledged shutdown.
    #[must_use]
    pub fn shutdown_acknowledged(&self) -> bool {
        self.core.borrow().shutdown_acknowledged()
    }

    pub(crate) fn return_transfer(
        &self,
        buffer: &mut Option<TransferBuffer>,
        lease_epoch: u32,
        disposition: OrbitDisposition,
        owner_now_us: u64,
    ) -> Result<(), ChannelError> {
        let buffer = buffer
            .take()
            .ok_or_else(|| ChannelError::new(ErrorCode::BufferStarved, 0, 0, 0))?;
        self.core
            .borrow_mut()
            .return_slot(buffer, lease_epoch, disposition, owner_now_us)
    }

    pub(crate) fn same_channel(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.core, &other.core)
    }
}

/// Browser lowering of the owner transport: one module Worker and its transferable buffers.
struct BrowserPort {
    worker: Worker,
    listener: Option<Closure<dyn FnMut(MessageEvent)>>,
}

impl BrowserPort {
    fn install_listener(
        &mut self,
        listener: Closure<dyn FnMut(MessageEvent)>,
    ) -> Result<(), ChannelError> {
        self.listen(&listener)?;
        self.listener = Some(listener);
        Ok(())
    }

    fn listen(&self, listener: &Closure<dyn FnMut(MessageEvent)>) -> Result<(), ChannelError> {
        self.worker
            .add_event_listener_with_callback("message", listener.as_ref().unchecked_ref())
            .map_err(|_| ChannelError::new(ErrorCode::BufferStarved, 0, 0, 0))
    }
}

impl OwnerPort for BrowserPort {
    type Slot = TransferBuffer;

    fn allocate(&self, pool: Pool, slot: u32, max_iter: u32) -> Result<Self::Slot, ChannelError> {
        TransferBuffer::allocate(pool, slot, max_iter)
    }

    #[allow(
        clippy::needless_pass_by_value,
        reason = "posting consumes logical ownership and drops the detached wrapper"
    )]
    fn post(&mut self, slot: Self::Slot) -> Result<(), ChannelError> {
        let array = slot.array();
        let transfer = Array::new();
        transfer.push(array.as_ref());
        self.worker
            .post_message_with_transfer(array.as_ref(), transfer.as_ref())
            .map_err(|_| ChannelError::new(ErrorCode::BufferStarved, 0, 0, 0))?;
        debug_assert_eq!(array.byte_length(), 0);
        Ok(())
    }

    fn probe_abi(&mut self) -> Result<(), ChannelError> {
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
            &JsValue::from_f64(f64::from(crate::JULIBROT_ABI_VERSION)),
        )
        .map_err(|_| ChannelError::new(ErrorCode::BadVersion, 0, 0, 0))?;
        self.worker
            .post_message(probe.as_ref())
            .map_err(|_| ChannelError::new(ErrorCode::BufferStarved, 0, 0, 0))
    }

    fn restart_producer(&mut self) -> Result<(), ChannelError> {
        self.worker.terminate();
        self.worker = spawn_worker()?;
        let listener = self
            .listener
            .take()
            .ok_or_else(|| ChannelError::new(ErrorCode::BufferStarved, 0, 0, 0))?;
        let installed = self.listen(&listener);
        self.listener = Some(listener);
        installed
    }

    fn now_us(&self) -> Result<u64, ChannelError> {
        browser_now_us()
    }
}

impl OwnerSlot for TransferBuffer {
    fn identity(&self) -> Result<(Pool, u32), ChannelError> {
        Self::identity(self)
    }

    fn header(&self) -> Result<MessageHeader, ChannelError> {
        Self::header(self)
    }

    fn validate_message(&self) -> Result<MessageKind, ChannelError> {
        Self::validate_message(self)
    }

    fn write_header(&self, header: MessageHeader) -> Result<(), ChannelError> {
        Self::write_header(self, header)
    }

    fn encode_request(&self, request: &OrbitRequest) -> Result<(), ChannelError> {
        crate::encode_transfer_request(self.array(), request)
    }

    fn channel_error(&self) -> Result<ChannelError, ChannelError> {
        Self::channel_error(self)
    }

    fn write_empty(
        &self,
        kind: MessageKind,
        generation: u32,
        compute_us: u32,
        credit_us: u32,
    ) -> Result<(), ChannelError> {
        Self::write_empty(self, kind, generation, compute_us, credit_us)
    }
}

fn receive_event(core: &Weak<RefCell<OwnerCore<BrowserPort>>>, event: &MessageEvent) {
    let Some(core) = core.upgrade() else {
        return;
    };
    let result = receive(&mut core.borrow_mut(), event);
    if let Err(error) = result {
        core.borrow_mut().publish_error(error);
    }
}

fn receive(core: &mut OwnerCore<BrowserPort>, event: &MessageEvent) -> Result<(), ChannelError> {
    let data = event.data();
    if let Ok(array) = data.clone().dyn_into::<ArrayBuffer>() {
        return core.receive_slot(TransferBuffer::from_array(array)?);
    }
    let Some(kind) = object_string(&data, "kind") else {
        return Ok(());
    };
    let message = match kind.as_str() {
        "WorkerReady" => ControlMessage::ProducerReady(object_u32(&data, "version").unwrap_or(0)),
        "AbiAccepted" => ControlMessage::AbiAccepted(object_u32(&data, "version").unwrap_or(0)),
        "VersionSkew" | "ChannelError" => ControlMessage::Refused,
        _ => return Ok(()),
    };
    core.receive_control(message)
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
    let url = Reflect::get(&js_sys::global(), &JsValue::from_str(WORKER_URL_GLOBAL))
        .ok()
        .and_then(|value| value.as_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| WORKER_URL_FALLBACK.to_owned());
    Worker::new_with_options(&url, &options)
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
