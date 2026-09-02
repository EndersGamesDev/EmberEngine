//! Browser Web Worker lowering over transferable standalone array buffers.

use std::cell::RefCell;

use js_sys::{Array, ArrayBuffer, Promise, Uint8Array};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{DedicatedWorkerGlobalScope, MessageEvent, WorkerGlobalScope};

use crate::wire::{BUFFER_OVERHEAD_BYTES, WireBuffer};
use crate::{
    ChannelError, ErrorCode, HEADER_BYTES, JULIBROT_ABI_VERSION, MessageHeader, MessageKind,
    ORBIT_RECORD_BYTES, OrbitRequest, OrbitTaskPoll, POOL_TRAILER_BYTES, Pool,
    ReferenceOrbitRecord, TRAILER_MAGIC,
};

thread_local! {
    static PRODUCER: RefCell<Option<BrowserProducer>> = const { RefCell::new(None) };
}

struct BrowserProducer {
    scope: DedicatedWorkerGlobalScope,
    latest_generation: u32,
    pending: Option<OrbitRequest>,
    orbit_buffers: Vec<TransferBuffer>,
    shutdown_buffer: Option<TransferBuffer>,
    running: bool,
    closed: bool,
}

impl BrowserProducer {
    fn new(scope: DedicatedWorkerGlobalScope) -> Self {
        Self {
            scope,
            latest_generation: 0,
            pending: None,
            orbit_buffers: Vec::with_capacity(2),
            shutdown_buffer: None,
            running: false,
            closed: false,
        }
    }

    fn receive(&mut self, mut buffer: TransferBuffer) -> Result<bool, ChannelError> {
        let header = buffer.header()?;
        let kind = header.validate()?;
        match (buffer.pool()?, kind) {
            (Pool::Request, MessageKind::OrbitRequest) => {
                let request = buffer.decode_request()?;
                let generation = request.generation();
                buffer.write_empty(MessageKind::RequestReturn, generation, 0, 0)?;
                self.post(buffer)?;
                if !self.closed && generation > self.latest_generation {
                    self.latest_generation = generation;
                    self.pending = Some(request);
                }
            }
            (Pool::Orbit, MessageKind::CreditApplied | MessageKind::CreditStale) => {
                if self.orbit_buffers.len() == 2 {
                    return Err(ChannelError::new(ErrorCode::BufferStarved, 2, 0, 0));
                }
                self.orbit_buffers.push(buffer);
            }
            (Pool::Request, MessageKind::Shutdown) => {
                if self.shutdown_buffer.is_some() {
                    return Err(ChannelError::new(ErrorCode::BadKind, header.kind, 0, 0));
                }
                self.closed = true;
                self.latest_generation = 0;
                self.pending = None;
                self.shutdown_buffer = Some(buffer);
            }
            (_, _) => return Err(ChannelError::new(ErrorCode::BadKind, header.kind, 0, 0)),
        }
        let should_start = !self.running && self.pending.is_some() && !self.closed;
        if should_start {
            self.running = true;
        }
        self.try_shutdown_ack()?;
        Ok(should_start)
    }

    fn post(&self, buffer: TransferBuffer) -> Result<(), ChannelError> {
        let array = buffer.into_array();
        let transfer = Array::new();
        transfer.push(array.as_ref());
        self.scope
            .post_message_with_transfer(array.as_ref(), transfer.as_ref())
            .map_err(|_| ChannelError::new(ErrorCode::BufferStarved, 0, 0, 0))?;
        debug_assert_eq!(array.byte_length(), 0);
        Ok(())
    }

    fn try_shutdown_ack(&mut self) -> Result<(), ChannelError> {
        if !self.closed || self.running || self.orbit_buffers.len() != 2 {
            return Ok(());
        }
        let Some(mut buffer) = self.shutdown_buffer.take() else {
            return Ok(());
        };
        buffer.write_empty(MessageKind::ShutdownAck, 0, 0, 0)?;
        self.post(buffer)
    }
}

struct TransferBuffer {
    array: ArrayBuffer,
    bytes: Uint8Array,
}

impl TransferBuffer {
    fn allocate(pool: Pool, slot: u32, max_iter: u32) -> Result<Self, ChannelError> {
        if slot > 1 {
            return Err(ChannelError::new(ErrorCode::BadTrailer, slot, 0, 0));
        }
        let capacity = crate::buffer_capacity(max_iter)?;
        let capacity_u32 = u32::try_from(capacity)
            .map_err(|_| ChannelError::new(ErrorCode::BadLength, max_iter, u32::MAX, 0))?;
        let array = ArrayBuffer::new(capacity_u32);
        let mut buffer = Self::from_array(array)?;
        write_words_at(
            &buffer.bytes,
            capacity_u32 - u32::try_from(POOL_TRAILER_BYTES).unwrap_or(16),
            &[pool as u32, slot, capacity_u32, TRAILER_MAGIC],
        );
        let initial = match pool {
            Pool::Request => MessageKind::RequestReturn,
            Pool::Orbit => MessageKind::CreditStale,
        };
        buffer.write_empty(initial, 0, 0, 0)?;
        Ok(buffer)
    }

    fn from_array(array: ArrayBuffer) -> Result<Self, ChannelError> {
        let capacity = usize::try_from(array.byte_length())
            .map_err(|_| ChannelError::new(ErrorCode::BadLength, 0, u32::MAX, 0))?;
        if capacity < BUFFER_OVERHEAD_BYTES {
            return Err(ChannelError::new(
                ErrorCode::BadLength,
                0,
                u32::try_from(BUFFER_OVERHEAD_BYTES).unwrap_or(u32::MAX),
                u32::try_from(capacity).unwrap_or(u32::MAX),
            ));
        }
        let bytes = Uint8Array::new(&array);
        Ok(Self { array, bytes })
    }

    fn into_array(self) -> ArrayBuffer {
        self.array
    }

    fn header(&self) -> Result<MessageHeader, ChannelError> {
        self.validate_trailer()?;
        let header = MessageHeader {
            magic: self.word(0),
            version: self.word(4),
            generation: self.word(8),
            kind: self.word(12),
            length: self.word(16),
            precision_bits: self.word(20),
            compute_us: self.word(24),
            credit_us: self.word(28),
        };
        header.validate()?;
        Ok(header)
    }

    fn pool(&self) -> Result<Pool, ChannelError> {
        self.validate_trailer()
    }

    fn validate_trailer(&self) -> Result<Pool, ChannelError> {
        let capacity = self.bytes.length();
        let offset = capacity - u32::try_from(POOL_TRAILER_BYTES).unwrap_or(16);
        let pool = self.word(offset);
        let slot = self.word(offset + 4);
        let recorded_capacity = self.word(offset + 8);
        let magic = self.word(offset + 12);
        if slot > 1 || recorded_capacity != capacity || magic != TRAILER_MAGIC {
            return Err(ChannelError::new(
                ErrorCode::BadTrailer,
                slot,
                recorded_capacity,
                capacity,
            ));
        }
        Pool::try_from(pool)
    }

    fn decode_request(&self) -> Result<OrbitRequest, ChannelError> {
        let copied = self.bytes.to_vec().into_boxed_slice();
        let buffer = WireBuffer::from_transferred(copied)?;
        OrbitRequest::decode(&buffer)
    }

    fn write_empty(
        &mut self,
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

    fn write_orbit(
        &mut self,
        generation: u32,
        precision_bits: u32,
        compute_us: u32,
        credit_us: u32,
        records: &[ReferenceOrbitRecord],
    ) -> Result<(), ChannelError> {
        let record_count = u32::try_from(records.len())
            .map_err(|_| ChannelError::new(ErrorCode::BadLength, u32::MAX, 0, 0))?;
        let used = u32::try_from(HEADER_BYTES)
            .ok()
            .and_then(|header| {
                record_count
                    .checked_mul(u32::try_from(ORBIT_RECORD_BYTES).ok()?)
                    .and_then(|bytes| header.checked_add(bytes))
            })
            .ok_or_else(|| ChannelError::new(ErrorCode::BadLength, record_count, u32::MAX, 0))?;
        let available = self.bytes.length() - u32::try_from(POOL_TRAILER_BYTES).unwrap_or(16);
        if records.is_empty() || used > available || self.pool()? != Pool::Orbit {
            return Err(ChannelError::new(
                ErrorCode::BadLength,
                record_count,
                used,
                available,
            ));
        }
        let mut header = MessageHeader::new(MessageKind::OrbitResponse, generation);
        header.length = record_count;
        header.precision_bits = precision_bits;
        header.compute_us = compute_us;
        header.credit_us = credit_us;
        self.write_header(header)?;
        for (index, record) in records.iter().enumerate() {
            let index = u32::try_from(index)
                .map_err(|_| ChannelError::new(ErrorCode::BadLength, record_count, 0, 0))?;
            let offset = u32::try_from(HEADER_BYTES).unwrap_or(32)
                + index * u32::try_from(ORBIT_RECORD_BYTES).unwrap_or(16);
            write_words_at(
                &self.bytes,
                offset,
                &[
                    record.re_hi.to_bits(),
                    record.im_hi.to_bits(),
                    record.re_lo.to_bits(),
                    record.im_lo.to_bits(),
                ],
            );
        }
        Ok(())
    }

    fn write_header(&mut self, header: MessageHeader) -> Result<(), ChannelError> {
        self.validate_trailer()?;
        let message_end = self.bytes.length() - u32::try_from(POOL_TRAILER_BYTES).unwrap_or(16);
        drop(self.bytes.fill(0, 0, message_end));
        write_words_at(
            &self.bytes,
            0,
            &[
                header.magic,
                header.version,
                header.generation,
                header.kind,
                header.length,
                header.precision_bits,
                header.compute_us,
                header.credit_us,
            ],
        );
        Ok(())
    }

    fn word(&self, offset: u32) -> u32 {
        u32::from_le_bytes([
            self.bytes.get_index(offset),
            self.bytes.get_index(offset + 1),
            self.bytes.get_index(offset + 2),
            self.bytes.get_index(offset + 3),
        ])
    }
}

struct BrowserClock {
    performance: web_sys::Performance,
}

impl crate::MonotonicClock for BrowserClock {
    fn now_us(&self) -> u64 {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        {
            (self.performance.now() * 1_000.0).ceil() as u64
        }
    }
}

/// Allocates one standalone version-one transfer buffer and initializes its immutable trailer.
///
/// # Errors
///
/// Returns a typed JavaScript refusal for an invalid pool, slot, or capacity.
#[wasm_bindgen]
pub fn allocate_transfer_buffer(
    pool: u32,
    slot: u32,
    max_iter: u32,
) -> Result<ArrayBuffer, JsValue> {
    let pool = Pool::try_from(pool).map_err(channel_js)?;
    TransferBuffer::allocate(pool, slot, max_iter)
        .map(TransferBuffer::into_array)
        .map_err(channel_js)
}

/// Installs the transferable producer endpoint in a dedicated worker instance.
///
/// # Errors
///
/// Returns version skew, wrong-global, duplicate-start, or missing-performance refusals.
#[wasm_bindgen]
pub fn worker_main(expected_abi: u32) -> Result<u32, JsValue> {
    ember_lab_heap::install_heap_lattice_panic_hook();
    if expected_abi != JULIBROT_ABI_VERSION {
        return Err(JsValue::from_str(&format!(
            "VersionSkew: worker expected {expected_abi}, wasm provides {JULIBROT_ABI_VERSION}"
        )));
    }
    let scope = js_sys::global()
        .dyn_into::<DedicatedWorkerGlobalScope>()
        .map_err(|_| JsValue::from_str("worker_main requires DedicatedWorkerGlobalScope"))?;
    if scope.performance().is_none() {
        return Err(JsValue::from_str("worker performance clock is unavailable"));
    }
    PRODUCER.with(|slot| {
        let mut slot = slot
            .try_borrow_mut()
            .map_err(|_| JsValue::from_str("worker endpoint is already borrowed"))?;
        if slot.is_some() {
            return Err(JsValue::from_str("worker endpoint is already installed"));
        }
        *slot = Some(BrowserProducer::new(scope.clone()));
        Ok(())
    })?;
    let onmessage = Closure::<dyn FnMut(MessageEvent)>::new(move |event| {
        if let Err(error) = receive_message(event) {
            ember_lab_heap::publish_browser_error(&format!("Julibrot worker: {error}"));
        }
    });
    scope.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
    onmessage.forget();
    Ok(JULIBROT_ABI_VERSION)
}

fn receive_message(event: MessageEvent) -> Result<(), ChannelError> {
    let array = event
        .data()
        .dyn_into::<ArrayBuffer>()
        .map_err(|_| ChannelError::new(ErrorCode::BadLength, 0, 0, 0))?;
    let buffer = TransferBuffer::from_array(array)?;
    let should_start = PRODUCER.with(|slot| {
        let mut slot = slot
            .try_borrow_mut()
            .map_err(|_| ChannelError::new(ErrorCode::BufferStarved, 0, 0, 0))?;
        slot.as_mut()
            .ok_or_else(|| ChannelError::new(ErrorCode::BadVersion, 0, 0, 0))?
            .receive(buffer)
    })?;
    if should_start {
        spawn_local(run_producer());
    }
    Ok(())
}

async fn run_producer() {
    if let Err(error) = run_producer_inner().await {
        ember_lab_heap::publish_browser_error(&format!("Julibrot worker: {error}"));
    }
    let result = PRODUCER.with(|slot| {
        let mut slot = slot
            .try_borrow_mut()
            .map_err(|_| ChannelError::new(ErrorCode::BufferStarved, 0, 0, 0))?;
        let producer = slot
            .as_mut()
            .ok_or_else(|| ChannelError::new(ErrorCode::BadVersion, 0, 0, 0))?;
        producer.running = false;
        producer.try_shutdown_ack()
    });
    if let Err(error) = result {
        ember_lab_heap::publish_browser_error(&format!("Julibrot worker shutdown: {error}"));
    }
}

async fn run_producer_inner() -> Result<(), ChannelError> {
    let clock = browser_clock()?;
    loop {
        let work = PRODUCER.with(|slot| {
            let mut slot = slot.try_borrow_mut().ok()?;
            let producer = slot.as_mut()?;
            if producer.closed || producer.pending.is_none() || producer.orbit_buffers.is_empty() {
                return None;
            }
            Some((producer.pending.take()?, producer.orbit_buffers.pop()?))
        });
        let Some((request, mut transfer)) = work else {
            let done = PRODUCER.with(|slot| {
                slot.try_borrow()
                    .ok()
                    .and_then(|slot| {
                        slot.as_ref()
                            .map(|producer| producer.closed || producer.pending.is_none())
                    })
                    .unwrap_or(true)
            });
            if done {
                return Ok(());
            }
            yield_worker_task().await?;
            continue;
        };
        let admission_credit = transfer.header()?.credit_us;
        let mut task = crate::ReferenceOrbitTask::start(&request, &clock)?;
        loop {
            let latest = PRODUCER.with(|slot| {
                slot.try_borrow()
                    .ok()
                    .and_then(|slot| slot.as_ref().map(|producer| producer.latest_generation))
                    .unwrap_or(0)
            });
            match task.poll(latest, &clock)? {
                OrbitTaskPoll::Pending { .. } => yield_worker_task().await?,
                OrbitTaskPoll::Cancelled {
                    generation,
                    compute_us,
                } => {
                    transfer.write_empty(
                        MessageKind::OrbitCancelled,
                        generation,
                        compute_us,
                        admission_credit,
                    )?;
                    post_from_producer(transfer)?;
                    break;
                }
                OrbitTaskPoll::Complete { orbit, compute_us } => {
                    let copy_started = clock.now_us();
                    transfer.write_orbit(
                        request.generation(),
                        orbit.precision_bits,
                        compute_us,
                        admission_credit,
                        &orbit.records,
                    )?;
                    let copy_us = clock
                        .now_us()
                        .checked_sub(copy_started)
                        .ok_or_else(|| ChannelError::new(ErrorCode::TimingOverflow, 0, 0, 0))?;
                    let compute_us = u64::from(compute_us)
                        .checked_add(copy_us)
                        .and_then(|value| u32::try_from(value).ok())
                        .ok_or_else(|| ChannelError::new(ErrorCode::TimingOverflow, 0, 0, 0))?;
                    transfer.set_compute_us(compute_us)?;
                    post_from_producer(transfer)?;
                    break;
                }
            }
        }
    }
}

impl TransferBuffer {
    fn set_compute_us(&mut self, compute_us: u32) -> Result<(), ChannelError> {
        self.header()?;
        write_words_at(&self.bytes, 24, &[compute_us]);
        Ok(())
    }
}

fn post_from_producer(buffer: TransferBuffer) -> Result<(), ChannelError> {
    PRODUCER.with(|slot| {
        let slot = slot
            .try_borrow()
            .map_err(|_| ChannelError::new(ErrorCode::BufferStarved, 0, 0, 0))?;
        slot.as_ref()
            .ok_or_else(|| ChannelError::new(ErrorCode::BadVersion, 0, 0, 0))?
            .post(buffer)
    })
}

fn browser_clock() -> Result<BrowserClock, ChannelError> {
    let scope: WorkerGlobalScope = js_sys::global().unchecked_into();
    let performance = scope
        .performance()
        .ok_or_else(|| ChannelError::new(ErrorCode::TimingOverflow, 0, 0, 0))?;
    Ok(BrowserClock { performance })
}

async fn yield_worker_task() -> Result<(), ChannelError> {
    let scope: WorkerGlobalScope = js_sys::global().unchecked_into();
    let promise = Promise::new(&mut |resolve, reject| {
        if let Err(error) = scope.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, 0)
        {
            drop(reject.call1(&JsValue::UNDEFINED, &error));
        }
    });
    JsFuture::from(promise)
        .await
        .map(|_| ())
        .map_err(|_| ChannelError::new(ErrorCode::BufferStarved, 0, 0, 0))
}

fn write_words_at(bytes: &Uint8Array, offset: u32, words: &[u32]) {
    for (word_index, word) in words.iter().enumerate() {
        let word_offset = offset + u32::try_from(word_index).unwrap_or(0) * 4;
        for (byte_index, byte) in word.to_le_bytes().into_iter().enumerate() {
            bytes.set_index(word_offset + u32::try_from(byte_index).unwrap_or(0), byte);
        }
    }
}

fn channel_js(error: ChannelError) -> JsValue {
    JsValue::from_str(&error.to_string())
}
