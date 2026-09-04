//! Browser Web Worker lowering over transferable standalone array buffers.

#![allow(
    clippy::redundant_pub_crate,
    reason = "the private wasm module exposes transfer storage to its sibling owner module"
)]

use std::cell::RefCell;

use js_sys::{Array, ArrayBuffer, Object, Promise, Reflect, Uint8Array};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{DedicatedWorkerGlobalScope, MessageEvent, WorkerGlobalScope};

use crate::codec::{
    REQUEST_FIXED_END, REQUEST_LIMB_COUNT_OFFSET, visit_transfer_request_body_words,
};
use crate::wire::{
    BUFFER_OVERHEAD_BYTES, ORBIT_FACT_BYTES, OrbitVerificationFacts, WireBuffer,
    retains_orbit_payload, validate_message_layout, write_words,
};
use crate::{
    Admission, ChannelError, CreditAccount, CreditCharge, ErrorCode, HEADER_BYTES,
    JULIBROT_ABI_VERSION, MessageHeader, MessageKind, MonotonicClock, ORBIT_RECORD_BYTES,
    OrbitDisposition, OrbitRequest, OrbitTaskPoll, POOL_TRAILER_BYTES, Pool, ProducerShaper,
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
    shaper: ProducerShaper,
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
            shaper: ProducerShaper::new(),
        }
    }

    fn receive(&mut self, buffer: TransferBuffer) -> Result<bool, ChannelError> {
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
                if header.generation != 0 {
                    self.shaper.observe_return(
                        browser_clock()?.now_us(),
                        header.credit_us,
                        header.compute_us,
                    )?;
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
        while let Some(orbit) = self.orbit_buffers.pop() {
            orbit.write_empty(MessageKind::CreditStale, 0, 0, 0)?;
            self.post(orbit)?;
        }
        let Some(buffer) = self.shutdown_buffer.take() else {
            return Ok(());
        };
        buffer.write_empty(MessageKind::ShutdownAck, 0, 0, 0)?;
        self.post(buffer)
    }
}

pub(crate) struct TransferBuffer {
    array: ArrayBuffer,
    bytes: Uint8Array,
}

impl TransferBuffer {
    pub(crate) fn allocate(pool: Pool, slot: u32, max_iter: u32) -> Result<Self, ChannelError> {
        if slot > 1 {
            return Err(ChannelError::new(ErrorCode::BadTrailer, slot, 0, 0));
        }
        let capacity = crate::buffer_capacity(max_iter)?;
        let capacity_u32 = u32::try_from(capacity)
            .map_err(|_| ChannelError::new(ErrorCode::BadLength, max_iter, u32::MAX, 0))?;
        let array = ArrayBuffer::new(capacity_u32);
        let buffer = Self::from_array(array)?;
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

    pub(crate) fn from_array(array: ArrayBuffer) -> Result<Self, ChannelError> {
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

    pub(crate) fn into_array(self) -> ArrayBuffer {
        self.array
    }

    pub(crate) const fn array(&self) -> &ArrayBuffer {
        &self.array
    }

    pub(crate) fn header(&self) -> Result<MessageHeader, ChannelError> {
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

    pub(crate) fn pool(&self) -> Result<Pool, ChannelError> {
        self.validate_trailer()
    }

    pub(crate) fn identity(&self) -> Result<(Pool, u32), ChannelError> {
        let pool = self.validate_trailer()?;
        let capacity = self.bytes.length();
        let offset = capacity - u32::try_from(POOL_TRAILER_BYTES).unwrap_or(16);
        Ok((pool, self.word(offset + 4)))
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

    pub(crate) fn validate_message(&self) -> Result<MessageKind, ChannelError> {
        let pool = self.validate_trailer()?;
        let header = self.header()?;
        let capacity = usize::try_from(self.bytes.length())
            .map_err(|_| ChannelError::new(ErrorCode::BadLength, 0, u32::MAX, 0))?;
        let kind = validate_message_layout(pool, capacity, header, |used, message_end| {
            let Ok(used) = u32::try_from(used) else {
                return false;
            };
            let Ok(message_end) = u32::try_from(message_end) else {
                return false;
            };
            (used..message_end).all(|offset| self.bytes.get_index(offset) == 0)
        })?;
        if kind == MessageKind::OrbitResponse {
            self.orbit_facts()?.status()?;
        }
        Ok(kind)
    }

    pub(crate) fn orbit_facts(&self) -> Result<OrbitVerificationFacts, ChannelError> {
        let offset = self
            .bytes
            .length()
            .checked_sub(u32::try_from(POOL_TRAILER_BYTES + ORBIT_FACT_BYTES).unwrap_or(32))
            .ok_or_else(|| ChannelError::new(ErrorCode::BadLength, 0, 0, 0))?;
        let facts = OrbitVerificationFacts {
            verification: self.word(offset),
            max_consumed_word_error_ulps: self.word(offset + 4),
            precision_escalations: self.word(offset + 8),
            reserved: self.word(offset + 12),
        };
        facts.status()?;
        Ok(facts)
    }

    pub(crate) fn record_bytes(&self) -> Result<Uint8Array, ChannelError> {
        let kind = self.validate_message()?;
        let header = self.header()?;
        if kind == MessageKind::OrbitCancelled {
            return Ok(self.bytes.subarray(
                u32::try_from(HEADER_BYTES).unwrap_or(32),
                u32::try_from(HEADER_BYTES).unwrap_or(32),
            ));
        }
        if kind != MessageKind::OrbitResponse {
            return Err(ChannelError::new(ErrorCode::BadKind, header.kind, 0, 0));
        }
        let end = u32::try_from(HEADER_BYTES)
            .unwrap_or(32)
            .checked_add(
                header
                    .length
                    .checked_mul(u32::try_from(ORBIT_RECORD_BYTES).unwrap_or(8))
                    .ok_or_else(|| ChannelError::new(ErrorCode::BadLength, header.length, 0, 0))?,
            )
            .ok_or_else(|| ChannelError::new(ErrorCode::BadLength, header.length, 0, 0))?;
        Ok(self
            .bytes
            .subarray(u32::try_from(HEADER_BYTES).unwrap_or(32), end))
    }

    pub(crate) fn channel_error(&self) -> Result<ChannelError, ChannelError> {
        if self.validate_message()? != MessageKind::ChannelError {
            return Err(ChannelError::new(
                ErrorCode::BadKind,
                self.header()?.kind,
                0,
                0,
            ));
        }
        let offset = u32::try_from(HEADER_BYTES).unwrap_or(32);
        Ok(ChannelError::new(
            ErrorCode::try_from(self.word(offset))?,
            self.word(offset + 4),
            self.word(offset + 8),
            self.word(offset + 12),
        ))
    }

    fn decode_request(&self) -> Result<OrbitRequest, ChannelError> {
        let (pool, slot) = self.identity()?;
        let header = self.header()?;
        if pool != Pool::Request || header.validate()? != MessageKind::OrbitRequest {
            return Err(ChannelError::new(ErrorCode::BadKind, header.kind, 0, 0));
        }
        let limb_count_offset = u32::try_from(REQUEST_LIMB_COUNT_OFFSET)
            .map_err(|_| ChannelError::new(ErrorCode::BadLength, 0, u32::MAX, 0))?;
        let fixed_end = u32::try_from(REQUEST_FIXED_END)
            .map_err(|_| ChannelError::new(ErrorCode::BadLength, 0, u32::MAX, 0))?;
        let limb_count = self.word(limb_count_offset);
        let used = limb_count
            .checked_mul(4)
            .and_then(|limb_bytes| fixed_end.checked_add(limb_bytes))
            .ok_or_else(|| ChannelError::new(ErrorCode::BadLength, limb_count, u32::MAX, 0))?;
        let available = self.bytes.length() - u32::try_from(POOL_TRAILER_BYTES).unwrap_or(16);
        if used > available {
            return Err(ChannelError::new(
                ErrorCode::BadLength,
                limb_count,
                used,
                available,
            ));
        }
        let compact_capacity = used
            .checked_add(u32::try_from(POOL_TRAILER_BYTES).unwrap_or(16))
            .ok_or_else(|| ChannelError::new(ErrorCode::BadLength, limb_count, u32::MAX, 0))?;
        let mut copied = vec![
            0;
            usize::try_from(compact_capacity).map_err(|_| {
                ChannelError::new(ErrorCode::BadLength, limb_count, u32::MAX, available)
            })?
        ];
        self.bytes
            .subarray(0, used)
            .copy_to(&mut copied[..usize::try_from(used).unwrap_or(0)]);
        write_words(
            &mut copied[usize::try_from(used).unwrap_or(0)..],
            &[pool as u32, slot, compact_capacity, TRAILER_MAGIC],
        );
        let copied = copied.into_boxed_slice();
        let buffer = WireBuffer::from_transferred(copied)?;
        OrbitRequest::decode(&buffer)
    }

    pub(crate) fn write_empty(
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

    fn write_orbit(
        &self,
        generation: u32,
        precision_bits: u32,
        compute_us: u32,
        credit_us: u32,
        records: &[ReferenceOrbitRecord],
        facts: OrbitVerificationFacts,
    ) -> Result<(), ChannelError> {
        facts.status()?;
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
        let available = self.bytes.length()
            - u32::try_from(POOL_TRAILER_BYTES + ORBIT_FACT_BYTES).unwrap_or(32);
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
                + index * u32::try_from(ORBIT_RECORD_BYTES).unwrap_or(8);
            write_words_at(
                &self.bytes,
                offset,
                &[record.re.to_bits(), record.im.to_bits()],
            );
        }
        let facts_offset = self.bytes.length()
            - u32::try_from(POOL_TRAILER_BYTES + ORBIT_FACT_BYTES).unwrap_or(32);
        write_words_at(
            &self.bytes,
            facts_offset,
            &[
                facts.verification,
                facts.max_consumed_word_error_ulps,
                facts.precision_escalations,
                facts.reserved,
            ],
        );
        Ok(())
    }

    pub(crate) fn write_header(&self, header: MessageHeader) -> Result<(), ChannelError> {
        self.validate_trailer()?;
        let kind = header.validate()?;
        if !retains_orbit_payload(kind) {
            let message_end = self.bytes.length() - u32::try_from(POOL_TRAILER_BYTES).unwrap_or(16);
            drop(self.bytes.fill(0, 0, message_end));
        }
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

    fn write_error(
        &self,
        generation: u32,
        error: ChannelError,
        credit_us: u32,
    ) -> Result<(), ChannelError> {
        let mut header = MessageHeader::new(MessageKind::ChannelError, generation);
        header.length = 4;
        header.credit_us = credit_us;
        self.write_header(header)?;
        write_words_at(
            &self.bytes,
            u32::try_from(HEADER_BYTES).unwrap_or(32),
            &[
                error.code as u32,
                error.detail,
                error.requested_bytes,
                error.available_bytes,
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

/// Allocates one standalone version-three transfer buffer and initializes its immutable trailer.
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

/// Writes one canonical request directly into a request-pool standalone buffer.
///
/// This function creates only a JavaScript view over the supplied allocation and does not copy the
/// buffer or allocate another transport buffer.
///
/// # Errors
///
/// Returns a typed trailer, pool, canonical-centre, or capacity refusal.
pub fn encode_transfer_request(
    array: &ArrayBuffer,
    request: &OrbitRequest,
) -> Result<(), ChannelError> {
    let buffer = TransferBuffer::from_array(array.clone())?;
    if buffer.pool()? != Pool::Request {
        return Err(ChannelError::new(
            ErrorCode::BadKind,
            Pool::Orbit as u32,
            0,
            0,
        ));
    }
    let requested = request.centre().request_bytes()?;
    let available = usize::try_from(buffer.bytes.length())
        .unwrap_or(0)
        .saturating_sub(POOL_TRAILER_BYTES);
    if requested > available {
        return Err(ChannelError::new(
            ErrorCode::CentreEncodingWall,
            u32::try_from(request.centre().limbs.len()).unwrap_or(u32::MAX),
            u32::try_from(requested).unwrap_or(u32::MAX),
            u32::try_from(available).unwrap_or(u32::MAX),
        ));
    }
    let mut header = MessageHeader::new(MessageKind::OrbitRequest, request.generation());
    header.length = request.max_iter();
    header.precision_bits = request.precision_bits();
    buffer.write_header(header)?;
    visit_transfer_request_body_words(request, |offset, words| {
        let offset = u32::try_from(offset)
            .map_err(|_| ChannelError::new(ErrorCode::BadLength, 0, u32::MAX, 0))?;
        write_words_at(&buffer.bytes, offset, words);
        Ok(())
    })
}

/// Reads and validates one standalone transfer header and immutable trailer.
///
/// # Errors
///
/// Returns the stable wire refusal for a detached, short, corrupt, or version-skewed buffer.
pub fn read_transfer_header(array: &ArrayBuffer) -> Result<MessageHeader, ChannelError> {
    TransferBuffer::from_array(array.clone())?.header()
}

/// Returns a zero-copy JavaScript view over initialized orbit record bytes.
///
/// A cancelled response returns an empty view; all other non-response kinds are refused.
///
/// # Errors
///
/// Returns a typed pool, kind, count, or capacity refusal.
pub fn transfer_record_bytes(array: &ArrayBuffer) -> Result<Uint8Array, ChannelError> {
    let buffer = TransferBuffer::from_array(array.clone())?;
    buffer.record_bytes()
}

/// Charges an orbit or cancellation and rewrites it as a returned CREDIT message in place.
///
/// # Errors
///
/// Returns a typed wire refusal or `TimingOverflow` for a regressing owner clock.
pub fn write_transfer_credit(
    array: &ArrayBuffer,
    disposition: OrbitDisposition,
    account: &mut CreditAccount,
    owner_now_us: u64,
) -> Result<CreditCharge, ChannelError> {
    let buffer = TransferBuffer::from_array(array.clone())?;
    if buffer.pool()? != Pool::Orbit {
        return Err(ChannelError::new(
            ErrorCode::BadKind,
            Pool::Request as u32,
            0,
            0,
        ));
    }
    let old = buffer.header()?;
    let old_kind = old.validate()?;
    if !matches!(
        old_kind,
        MessageKind::OrbitResponse | MessageKind::OrbitCancelled | MessageKind::ChannelError
    ) {
        return Err(ChannelError::new(ErrorCode::BadKind, old.kind, 0, 0));
    }
    let charge = account.charge(owner_now_us, old.compute_us)?;
    let kind = match disposition {
        OrbitDisposition::Applied => MessageKind::CreditApplied,
        OrbitDisposition::Stale => MessageKind::CreditStale,
    };
    let mut header = MessageHeader::new(kind, old.generation);
    header.precision_bits = old.precision_bits;
    header.compute_us = old.compute_us;
    header.credit_us = charge.credit_us;
    buffer.write_header(header)?;
    Ok(charge)
}

/// Rewrites one owned request buffer as a shutdown request without allocation.
///
/// # Errors
///
/// Returns a typed trailer or wrong-pool refusal.
pub fn write_transfer_shutdown(array: &ArrayBuffer, generation: u32) -> Result<(), ChannelError> {
    let buffer = TransferBuffer::from_array(array.clone())?;
    if buffer.pool()? != Pool::Request {
        return Err(ChannelError::new(
            ErrorCode::BadKind,
            Pool::Orbit as u32,
            0,
            0,
        ));
    }
    buffer.write_empty(MessageKind::Shutdown, generation, 0, 0)
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
        if let Err(error) = receive_message(&event) {
            ember_lab_heap::publish_browser_error(&format!("Julibrot worker: {error}"));
        }
    });
    scope
        .add_event_listener_with_callback("message", onmessage.as_ref().unchecked_ref())
        .map_err(|_| JsValue::from_str("worker message listener could not be installed"))?;
    onmessage.forget();
    Ok(JULIBROT_ABI_VERSION)
}

fn receive_message(event: &MessageEvent) -> Result<(), ChannelError> {
    let data = event.data();
    let Ok(array) = data.clone().dyn_into::<ArrayBuffer>() else {
        return receive_handshake(&data);
    };
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

fn receive_handshake(data: &JsValue) -> Result<(), ChannelError> {
    let kind = Reflect::get(data, &JsValue::from_str("kind"))
        .ok()
        .and_then(|field| field.as_string());
    if kind.as_deref() != Some("AbiProbe") {
        return Ok(());
    }
    let version = Reflect::get(data, &JsValue::from_str("version"))
        .ok()
        .and_then(|field| field.as_f64());
    if version != Some(f64::from(JULIBROT_ABI_VERSION)) {
        return Err(ChannelError::new(ErrorCode::BadVersion, 0, 0, 0));
    }
    let scope: DedicatedWorkerGlobalScope = js_sys::global().unchecked_into();
    scope.set_onmessage(None);
    let accepted = Object::new();
    Reflect::set(
        accepted.as_ref(),
        &JsValue::from_str("kind"),
        &JsValue::from_str("AbiAccepted"),
    )
    .map_err(|_| ChannelError::new(ErrorCode::BadVersion, 0, 0, 0))?;
    Reflect::set(
        accepted.as_ref(),
        &JsValue::from_str("version"),
        &JsValue::from_f64(f64::from(JULIBROT_ABI_VERSION)),
    )
    .map_err(|_| ChannelError::new(ErrorCode::BadVersion, 0, 0, 0))?;
    scope
        .post_message(accepted.as_ref())
        .map_err(|_| ChannelError::new(ErrorCode::BufferStarved, 0, 0, 0))
}

#[allow(
    clippy::future_not_send,
    reason = "wasm worker tasks are intentionally local to one browser event loop"
)]
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

#[allow(
    clippy::future_not_send,
    clippy::too_many_lines,
    reason = "the local browser task owns one explicit protocol loop"
)]
async fn run_producer_inner() -> Result<(), ChannelError> {
    let clock = browser_clock()?;
    loop {
        let work = PRODUCER.with(|slot| -> Result<Option<BrowserWork>, ChannelError> {
            let mut slot = slot
                .try_borrow_mut()
                .map_err(|_| ChannelError::new(ErrorCode::BufferStarved, 0, 0, 0))?;
            let Some(producer) = slot.as_mut() else {
                return Ok(None);
            };
            if producer.closed || producer.pending.is_none() || producer.orbit_buffers.is_empty() {
                return Ok(None);
            }
            let admission = producer.shaper.admit(clock.now_us())?;
            match admission {
                Admission::Ready { credit_us, .. } => {
                    let request = producer
                        .pending
                        .take()
                        .ok_or_else(|| ChannelError::new(ErrorCode::BufferStarved, 0, 0, 0))?;
                    let transfer = producer
                        .orbit_buffers
                        .pop()
                        .ok_or_else(|| ChannelError::new(ErrorCode::BufferStarved, 0, 0, 0))?;
                    Ok(Some(BrowserWork::Run {
                        request,
                        transfer,
                        credit_us,
                    }))
                }
                Admission::Delay { wait_us } => Ok(Some(BrowserWork::Delay(wait_us))),
                Admission::TimingUnavailable => {
                    let request = producer
                        .pending
                        .take()
                        .ok_or_else(|| ChannelError::new(ErrorCode::BufferStarved, 0, 0, 0))?;
                    let transfer = producer
                        .orbit_buffers
                        .pop()
                        .ok_or_else(|| ChannelError::new(ErrorCode::BufferStarved, 0, 0, 0))?;
                    Ok(Some(BrowserWork::TimingUnavailable { request, transfer }))
                }
            }
        })?;
        let Some(work) = work else {
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
        if let BrowserWork::Delay(wait_us) = work {
            yield_worker_task_after(wait_us).await?;
            continue;
        }
        if let BrowserWork::TimingUnavailable { request, transfer } = work {
            transfer.write_error(
                request.generation(),
                ChannelError::new(ErrorCode::TimingOverflow, 0, 0, 0),
                0,
            )?;
            post_from_producer(transfer)?;
            continue;
        }
        let (request, transfer, admission_credit) = match work {
            BrowserWork::Run {
                request,
                transfer,
                credit_us,
            } => Ok((request, transfer, credit_us)),
            BrowserWork::Delay(_) => Err(ChannelError::new(ErrorCode::UnexpectedWork, 1, 0, 0)),
            BrowserWork::TimingUnavailable { .. } => {
                Err(ChannelError::new(ErrorCode::UnexpectedWork, 2, 0, 0))
            }
        }?;
        let mut task = match crate::ReferenceOrbitTask::start(&request, &clock) {
            Ok(task) => task,
            Err(error) => {
                transfer.write_error(request.generation(), error, admission_credit)?;
                post_from_producer(transfer)?;
                continue;
            }
        };
        loop {
            let latest = PRODUCER.with(|slot| {
                slot.try_borrow()
                    .ok()
                    .and_then(|slot| slot.as_ref().map(|producer| producer.latest_generation))
                    .unwrap_or(0)
            });
            let poll = match task.poll(latest, &clock) {
                Ok(poll) => poll,
                Err(error) => {
                    transfer.write_error(request.generation(), error, admission_credit)?;
                    post_from_producer(transfer)?;
                    break;
                }
            };
            match poll {
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
                    let facts = OrbitVerificationFacts::from_orbit(&orbit);
                    transfer.write_orbit(
                        request.generation(),
                        orbit.precision_bits,
                        compute_us,
                        admission_credit,
                        &orbit.records,
                        facts,
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

enum BrowserWork {
    Run {
        request: OrbitRequest,
        transfer: TransferBuffer,
        credit_us: u32,
    },
    Delay(u64),
    TimingUnavailable {
        request: OrbitRequest,
        transfer: TransferBuffer,
    },
}

impl TransferBuffer {
    fn set_compute_us(&self, compute_us: u32) -> Result<(), ChannelError> {
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

#[allow(
    clippy::future_not_send,
    reason = "wasm timer futures are intentionally local to the worker event loop"
)]
async fn yield_worker_task() -> Result<(), ChannelError> {
    yield_worker_task_after(0).await
}

#[allow(
    clippy::future_not_send,
    reason = "wasm timer futures are intentionally local to the worker event loop"
)]
async fn yield_worker_task_after(wait_us: u64) -> Result<(), ChannelError> {
    let scope: WorkerGlobalScope = js_sys::global().unchecked_into();
    let delay_millis =
        i32::try_from(wait_us.div_ceil(1_000).min(2_147_483_647)).unwrap_or(i32::MAX);
    let promise = Promise::new(&mut |resolve, reject| {
        if let Err(error) =
            scope.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, delay_millis)
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
