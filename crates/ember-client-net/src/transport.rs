use std::collections::VecDeque;

use crate::{Keepalive, WireFrame};

// Referenced only by the wasm transport module; the native build has no use site.
#[cfg(target_arch = "wasm32")]
const MAX_BROWSER_BUFFERED_BYTES: u32 = 4 * 1024 * 1024;

/// Stable category for a transport closure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CloseKind {
    /// The peer completed a WebSocket close.
    Remote,
    /// Connection establishment or socket I/O failed.
    Network,
    /// A local protocol conversion failed.
    Protocol,
}

/// Structured closure suitable for a reconnect decision and player message.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConnectionClose {
    /// Stable close category.
    pub kind: CloseKind,
    /// Human-readable diagnostic detail.
    pub detail: String,
    /// Whether opening a fresh connection may reasonably succeed.
    pub reconnectable: bool,
}

/// Current WebSocket lifecycle independent of a game's handshake.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TransportStatus {
    /// The platform backend is establishing the socket.
    Connecting,
    /// The WebSocket is open for data frames.
    Open,
    /// The socket has closed and carries a reconnect-oriented reason.
    Closed(ConnectionClose),
}

/// Bounded transport counters and the newest actionable error.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ConnectionDiagnostics {
    /// Successfully emitted data frames, including inactivity keepalives.
    pub frames_sent: u64,
    /// Data frames accepted from the socket before inbox admission.
    pub frames_received: u64,
    /// Incoming frames rejected because the bounded inbox was full.
    pub inbox_overflows: u64,
    /// Outgoing frames rejected because the bounded outbox was full.
    pub outbox_overflows: u64,
    /// Frames ignored because the browser supplied an unsupported JS value.
    pub unsupported_frames: u64,
    /// Incoming or outgoing data frames rejected at the shared byte ceiling.
    pub oversized_frames: u64,
    /// Newest transport error detail.
    pub last_error: Option<String>,
}

/// Capacity and keepalive policy for one platform transport.
#[derive(Clone, Debug)]
pub struct TransportConfig {
    /// Maximum bytes in one incoming or outgoing data frame.
    pub max_frame_bytes: usize,
    /// Maximum received frames waiting for the game loop.
    pub inbox_capacity: usize,
    /// Maximum outgoing frames waiting for the socket owner.
    pub outbox_capacity: usize,
    /// Optional application-level keepalive after outbound inactivity.
    pub keepalive: Option<Keepalive>,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            max_frame_bytes: ember_net::outer::MAX_OUTER_FRAME_BYTES,
            inbox_capacity: 256,
            outbox_capacity: 256,
            keepalive: None,
        }
    }
}

/// Failure to admit one frame into the bounded outbound path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SendError {
    /// The configured bounded queue has no remaining slot.
    QueueFull,
    /// The exact data frame exceeds the shared canonical byte ceiling.
    FrameTooLarge,
    /// The socket owner has closed or exited.
    Closed,
}

/// Native- and browser-backed WebSocket transport with one shared interface.
pub struct WebSocketTransport {
    imp: imp::Transport,
}

impl WebSocketTransport {
    /// Starts a non-blocking platform connection.
    ///
    /// # Errors
    ///
    /// Returns an immediate URL, browser API, or thread-creation failure.
    pub fn connect(url: &str, config: TransportConfig) -> Result<Self, String> {
        if config
            .keepalive
            .as_ref()
            .is_some_and(|keepalive| keepalive.frame.len() > config.max_frame_bytes.max(1))
        {
            return Err("keepalive frame exceeds the shared byte ceiling".to_string());
        }
        imp::Transport::connect(url, config).map(|imp| Self { imp })
    }

    /// Enqueues or emits one exact WebSocket data frame.
    ///
    /// # Errors
    ///
    /// Returns a bounded-queue or closed-transport failure.
    pub fn send(&self, frame: WireFrame) -> Result<(), SendError> {
        self.imp.send(frame)
    }

    /// Drains every currently available frame in arrival order.
    pub fn drain(&mut self, output: &mut VecDeque<WireFrame>) {
        self.imp.drain(output);
    }

    /// Returns the current platform lifecycle state.
    #[must_use]
    pub fn status(&self) -> TransportStatus {
        self.imp.status()
    }

    /// Returns a consistent diagnostic snapshot.
    #[must_use]
    pub fn diagnostics(&self) -> ConnectionDiagnostics {
        self.imp.diagnostics()
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use std::collections::VecDeque;
    use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError};
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::{Duration, Instant};

    use tungstenite::Message;
    use tungstenite::stream::MaybeTlsStream;

    use super::{
        CloseKind, ConnectionClose, ConnectionDiagnostics, SendError, TransportConfig,
        TransportStatus, WireFrame,
    };

    fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
        mutex
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn close(
        status: &Mutex<TransportStatus>,
        diagnostics: &Mutex<ConnectionDiagnostics>,
        kind: CloseKind,
        detail: String,
        reconnectable: bool,
    ) {
        lock(diagnostics).last_error = Some(detail.clone());
        *lock(status) = TransportStatus::Closed(ConnectionClose {
            kind,
            detail,
            reconnectable,
        });
    }

    fn message(frame: WireFrame) -> Message {
        match frame {
            WireFrame::Text(text) => Message::text(text),
            WireFrame::Binary(bytes) => Message::binary(bytes),
        }
    }

    pub struct Transport {
        incoming: Receiver<WireFrame>,
        outgoing: SyncSender<WireFrame>,
        status: Arc<Mutex<TransportStatus>>,
        diagnostics: Arc<Mutex<ConnectionDiagnostics>>,
        max_frame_bytes: usize,
    }

    impl Transport {
        // Native socket ownership and its bounded channel loop are one lifecycle unit.
        #[allow(clippy::too_many_lines)]
        pub fn connect(url: &str, config: TransportConfig) -> Result<Self, String> {
            drop(rustls::crypto::ring::default_provider().install_default());
            let inbox_capacity = config.inbox_capacity.max(1);
            let outbox_capacity = config.outbox_capacity.max(1);
            let max_frame_bytes = config.max_frame_bytes.max(1);
            let (incoming_tx, incoming) = mpsc::sync_channel::<WireFrame>(inbox_capacity);
            let (outgoing, outgoing_rx) = mpsc::sync_channel::<WireFrame>(outbox_capacity);
            let status = Arc::new(Mutex::new(TransportStatus::Connecting));
            let diagnostics = Arc::new(Mutex::new(ConnectionDiagnostics::default()));
            let thread_status = Arc::clone(&status);
            let thread_diagnostics = Arc::clone(&diagnostics);
            let url = url.to_string();
            thread::Builder::new()
                .name("ember-client-net".to_string())
                .spawn(move || {
                    let mut socket = match tungstenite::connect(&url) {
                        Ok((socket, _)) => socket,
                        Err(error) => {
                            close(
                                &thread_status,
                                &thread_diagnostics,
                                CloseKind::Network,
                                format!("could not connect to {url}: {error}"),
                                true,
                            );
                            return;
                        }
                    };
                    match socket.get_ref() {
                        MaybeTlsStream::Plain(stream) => {
                            drop(stream.set_read_timeout(Some(Duration::from_millis(5))));
                        }
                        MaybeTlsStream::Rustls(stream) => {
                            drop(
                                stream
                                    .get_ref()
                                    .set_read_timeout(Some(Duration::from_millis(5))),
                            );
                        }
                        _ => {}
                    }
                    *lock(&thread_status) = TransportStatus::Open;
                    let mut last_outbound = Instant::now();
                    loop {
                        loop {
                            match outgoing_rx.try_recv() {
                                Ok(frame) => {
                                    if let Err(error) = socket.send(message(frame)) {
                                        close(
                                            &thread_status,
                                            &thread_diagnostics,
                                            CloseKind::Network,
                                            format!("WebSocket send failed: {error}"),
                                            true,
                                        );
                                        return;
                                    }
                                    last_outbound = Instant::now();
                                    lock(&thread_diagnostics).frames_sent += 1;
                                }
                                Err(TryRecvError::Empty) => break,
                                Err(TryRecvError::Disconnected) => {
                                    drop(socket.close(None));
                                    return;
                                }
                            }
                        }
                        if let Some(keepalive) = &config.keepalive
                            && last_outbound.elapsed() >= keepalive.interval
                        {
                            if let Err(error) = socket.send(message(keepalive.frame.clone())) {
                                close(
                                    &thread_status,
                                    &thread_diagnostics,
                                    CloseKind::Network,
                                    format!("WebSocket keepalive failed: {error}"),
                                    true,
                                );
                                return;
                            }
                            last_outbound = Instant::now();
                            lock(&thread_diagnostics).frames_sent += 1;
                        }
                        match socket.read() {
                            Ok(Message::Text(text)) => {
                                lock(&thread_diagnostics).frames_received += 1;
                                let frame = WireFrame::Text(text.to_string());
                                if frame.len() > max_frame_bytes {
                                    lock(&thread_diagnostics).oversized_frames += 1;
                                    close(
                                        &thread_status,
                                        &thread_diagnostics,
                                        CloseKind::Protocol,
                                        "incoming WebSocket text frame is too large".to_string(),
                                        false,
                                    );
                                    return;
                                }
                                match incoming_tx.try_send(frame) {
                                    Ok(()) => {}
                                    Err(TrySendError::Full(_)) => {
                                        lock(&thread_diagnostics).inbox_overflows += 1;
                                    }
                                    Err(TrySendError::Disconnected(_)) => return,
                                }
                            }
                            Ok(Message::Binary(bytes)) => {
                                lock(&thread_diagnostics).frames_received += 1;
                                let frame = WireFrame::Binary(bytes.to_vec());
                                if frame.len() > max_frame_bytes {
                                    lock(&thread_diagnostics).oversized_frames += 1;
                                    close(
                                        &thread_status,
                                        &thread_diagnostics,
                                        CloseKind::Protocol,
                                        "incoming WebSocket binary frame is too large".to_string(),
                                        false,
                                    );
                                    return;
                                }
                                match incoming_tx.try_send(frame) {
                                    Ok(()) => {}
                                    Err(TrySendError::Full(_)) => {
                                        lock(&thread_diagnostics).inbox_overflows += 1;
                                    }
                                    Err(TrySendError::Disconnected(_)) => return,
                                }
                            }
                            Ok(Message::Close(frame)) => {
                                let detail = frame.map_or_else(
                                    || "server closed the connection".to_string(),
                                    |close_frame| {
                                        if close_frame.reason.is_empty() {
                                            format!(
                                                "server closed the connection ({:?})",
                                                close_frame.code
                                            )
                                        } else {
                                            close_frame.reason.to_string()
                                        }
                                    },
                                );
                                close(
                                    &thread_status,
                                    &thread_diagnostics,
                                    CloseKind::Remote,
                                    detail,
                                    true,
                                );
                                return;
                            }
                            Ok(Message::Ping(_) | Message::Pong(_) | Message::Frame(_)) => {
                                drop(socket.flush());
                            }
                            Err(tungstenite::Error::Io(error))
                                if matches!(
                                    error.kind(),
                                    std::io::ErrorKind::WouldBlock
                                        | std::io::ErrorKind::TimedOut
                                        | std::io::ErrorKind::Interrupted
                                ) || error.raw_os_error() == Some(997) => {}
                            Err(error) => {
                                close(
                                    &thread_status,
                                    &thread_diagnostics,
                                    CloseKind::Network,
                                    format!("WebSocket read failed: {error}"),
                                    true,
                                );
                                return;
                            }
                        }
                    }
                })
                .map_err(|error| format!("could not start WebSocket worker: {error}"))?;
            Ok(Self {
                incoming,
                outgoing,
                status,
                diagnostics,
                max_frame_bytes,
            })
        }

        pub fn send(&self, frame: WireFrame) -> Result<(), SendError> {
            if frame.len() > self.max_frame_bytes {
                lock(&self.diagnostics).oversized_frames += 1;
                return Err(SendError::FrameTooLarge);
            }
            match self.outgoing.try_send(frame) {
                Ok(()) => Ok(()),
                Err(TrySendError::Full(_)) => {
                    lock(&self.diagnostics).outbox_overflows += 1;
                    Err(SendError::QueueFull)
                }
                Err(TrySendError::Disconnected(_)) => Err(SendError::Closed),
            }
        }

        pub fn drain(&mut self, output: &mut VecDeque<WireFrame>) {
            while let Ok(frame) = self.incoming.try_recv() {
                output.push_back(frame);
            }
        }

        pub fn status(&self) -> TransportStatus {
            lock(&self.status).clone()
        }

        pub fn diagnostics(&self) -> ConnectionDiagnostics {
            lock(&self.diagnostics).clone()
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod imp {
    use std::cell::{Cell, RefCell};
    use std::collections::VecDeque;
    use std::rc::Rc;

    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;

    use super::{
        CloseKind, ConnectionClose, ConnectionDiagnostics, MAX_BROWSER_BUFFERED_BYTES,
        SendError, TransportConfig, TransportStatus, WireFrame,
    };

    struct Shared {
        inbox: VecDeque<WireFrame>,
        status: TransportStatus,
        diagnostics: ConnectionDiagnostics,
        inbox_capacity: usize,
        max_frame_bytes: usize,
    }

    fn close(shared: &RefCell<Shared>, kind: CloseKind, detail: String, reconnectable: bool) {
        let mut shared = shared.borrow_mut();
        shared.diagnostics.last_error = Some(detail.clone());
        shared.status = TransportStatus::Closed(ConnectionClose {
            kind,
            detail,
            reconnectable,
        });
    }

    fn send_socket(socket: &web_sys::WebSocket, frame: &WireFrame) -> bool {
        match frame {
            WireFrame::Text(text) => socket.send_with_str(text).is_ok(),
            WireFrame::Binary(bytes) => socket.send_with_u8_array(bytes).is_ok(),
        }
    }

    fn has_buffer_capacity(socket: &web_sys::WebSocket, frame: &WireFrame) -> bool {
        let frame_bytes = u32::try_from(frame.len()).unwrap_or(u32::MAX);
        socket.buffered_amount().saturating_add(frame_bytes) <= MAX_BROWSER_BUFFERED_BYTES
    }

    pub struct Transport {
        socket: web_sys::WebSocket,
        shared: Rc<RefCell<Shared>>,
        pending: Rc<RefCell<VecDeque<WireFrame>>>,
        outbox_capacity: usize,
        last_outbound_ms: Rc<Cell<f64>>,
        _on_message: Closure<dyn FnMut(web_sys::MessageEvent)>,
        _on_open: Closure<dyn FnMut(web_sys::Event)>,
        _on_close: Closure<dyn FnMut(web_sys::CloseEvent)>,
        _on_error: Closure<dyn FnMut(web_sys::Event)>,
        keepalive_id: Option<i32>,
        _keepalive: Option<Closure<dyn FnMut()>>,
    }

    impl Transport {
        // Browser callback installation stays together so every closure lifetime is visible.
        #[allow(clippy::too_many_lines)]
        pub fn connect(url: &str, config: TransportConfig) -> Result<Self, String> {
            let socket =
                web_sys::WebSocket::new(url).map_err(|_| format!("invalid WebSocket URL: {url}"))?;
            socket.set_binary_type(web_sys::BinaryType::Arraybuffer);
            let shared = Rc::new(RefCell::new(Shared {
                inbox: VecDeque::new(),
                status: TransportStatus::Connecting,
                diagnostics: ConnectionDiagnostics::default(),
                inbox_capacity: config.inbox_capacity.max(1),
                max_frame_bytes: config.max_frame_bytes.max(1),
            }));
            let pending = Rc::new(RefCell::new(VecDeque::new()));
            let last_outbound_ms = Rc::new(Cell::new(js_sys::Date::now()));

            let message_shared = Rc::clone(&shared);
            let on_message = Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |event| {
                let data = event.data();
                let frame = if let Some(text) = data.as_string() {
                    Some(WireFrame::Text(text))
                } else if let Ok(buffer) = data.dyn_into::<js_sys::ArrayBuffer>() {
                    Some(WireFrame::Binary(js_sys::Uint8Array::new(&buffer).to_vec()))
                } else {
                    None
                };
                let mut shared = message_shared.borrow_mut();
                if matches!(&shared.status, TransportStatus::Closed(_)) {
                    return;
                }
                let Some(frame) = frame else {
                    shared.diagnostics.unsupported_frames += 1;
                    return;
                };
                shared.diagnostics.frames_received += 1;
                if frame.len() > shared.max_frame_bytes {
                    shared.diagnostics.oversized_frames += 1;
                    shared.diagnostics.last_error =
                        Some("incoming WebSocket data frame is too large".to_string());
                    shared.status = TransportStatus::Closed(ConnectionClose {
                        kind: CloseKind::Protocol,
                        detail: "incoming WebSocket data frame is too large".to_string(),
                        reconnectable: false,
                    });
                } else if shared.inbox.len() >= shared.inbox_capacity {
                    shared.diagnostics.inbox_overflows += 1;
                } else {
                    shared.inbox.push_back(frame);
                }
            });
            socket.set_onmessage(Some(on_message.as_ref().unchecked_ref()));

            let open_shared = Rc::clone(&shared);
            let open_pending = Rc::clone(&pending);
            let open_socket = socket.clone();
            let open_last = Rc::clone(&last_outbound_ms);
            let on_open = Closure::<dyn FnMut(web_sys::Event)>::new(move |_| {
                open_shared.borrow_mut().status = TransportStatus::Open;
                while let Some(frame) = open_pending.borrow_mut().pop_front() {
                    if !has_buffer_capacity(&open_socket, &frame) {
                        open_shared.borrow_mut().diagnostics.outbox_overflows += 1;
                        continue;
                    }
                    if !send_socket(&open_socket, &frame) {
                        close(
                            &open_shared,
                            CloseKind::Network,
                            "WebSocket send failed while opening".to_string(),
                            true,
                        );
                        break;
                    }
                    open_shared.borrow_mut().diagnostics.frames_sent += 1;
                    open_last.set(js_sys::Date::now());
                }
            });
            socket.set_onopen(Some(on_open.as_ref().unchecked_ref()));

            let close_shared = Rc::clone(&shared);
            let on_close =
                Closure::<dyn FnMut(web_sys::CloseEvent)>::new(move |event| {
                    let detail = if event.reason().is_empty() {
                        format!("server closed the connection ({})", event.code())
                    } else {
                        event.reason()
                    };
                    close(&close_shared, CloseKind::Remote, detail, true);
                });
            socket.set_onclose(Some(on_close.as_ref().unchecked_ref()));

            let error_shared = Rc::clone(&shared);
            let on_error = Closure::<dyn FnMut(web_sys::Event)>::new(move |_| {
                close(
                    &error_shared,
                    CloseKind::Network,
                    "could not reach the WebSocket server".to_string(),
                    true,
                );
            });
            socket.set_onerror(Some(on_error.as_ref().unchecked_ref()));

            let mut keepalive_id = None;
            let mut keepalive_callback = None;
            if let (Some(window), Some(keepalive)) = (web_sys::window(), config.keepalive.clone()) {
                let keepalive_socket = socket.clone();
                let keepalive_shared = Rc::clone(&shared);
                let keepalive_last = Rc::clone(&last_outbound_ms);
                let interval_ms = keepalive.interval.as_secs_f64() * 1_000.0;
                let callback = Closure::<dyn FnMut()>::new(move || {
                    if keepalive_shared.borrow().status != TransportStatus::Open
                        || js_sys::Date::now() - keepalive_last.get() < interval_ms
                    {
                        return;
                    }
                    if !has_buffer_capacity(&keepalive_socket, &keepalive.frame) {
                        keepalive_shared.borrow_mut().diagnostics.outbox_overflows += 1;
                    } else if !send_socket(&keepalive_socket, &keepalive.frame) {
                        close(
                            &keepalive_shared,
                            CloseKind::Network,
                            "WebSocket keepalive failed".to_string(),
                            true,
                        );
                    } else {
                        keepalive_shared.borrow_mut().diagnostics.frames_sent += 1;
                        keepalive_last.set(js_sys::Date::now());
                    }
                });
                let timer_ms = i32::try_from(keepalive.interval.as_millis())
                    .unwrap_or(i32::MAX)
                    .max(1);
                keepalive_id = window
                    .set_interval_with_callback_and_timeout_and_arguments_0(
                        callback.as_ref().unchecked_ref(),
                        timer_ms,
                    )
                    .ok();
                keepalive_callback = Some(callback);
            }

            Ok(Self {
                socket,
                shared,
                pending,
                outbox_capacity: config.outbox_capacity.max(1),
                last_outbound_ms,
                _on_message: on_message,
                _on_open: on_open,
                _on_close: on_close,
                _on_error: on_error,
                keepalive_id,
                _keepalive: keepalive_callback,
            })
        }

        pub fn send(&self, frame: WireFrame) -> Result<(), SendError> {
            if frame.len() > self.shared.borrow().max_frame_bytes {
                self.shared.borrow_mut().diagnostics.oversized_frames += 1;
                return Err(SendError::FrameTooLarge);
            }
            match self.shared.borrow().status.clone() {
                TransportStatus::Connecting => {
                    let mut pending = self.pending.borrow_mut();
                    if pending.len() >= self.outbox_capacity {
                        self.shared.borrow_mut().diagnostics.outbox_overflows += 1;
                        Err(SendError::QueueFull)
                    } else {
                        pending.push_back(frame);
                        Ok(())
                    }
                }
                TransportStatus::Open => {
                    if !has_buffer_capacity(&self.socket, &frame) {
                        self.shared.borrow_mut().diagnostics.outbox_overflows += 1;
                        Err(SendError::QueueFull)
                    } else if !send_socket(&self.socket, &frame) {
                        close(
                            &self.shared,
                            CloseKind::Network,
                            "WebSocket send failed".to_string(),
                            true,
                        );
                        Err(SendError::Closed)
                    } else {
                        self.shared.borrow_mut().diagnostics.frames_sent += 1;
                        self.last_outbound_ms.set(js_sys::Date::now());
                        Ok(())
                    }
                }
                TransportStatus::Closed(_) => Err(SendError::Closed),
            }
        }

        pub fn drain(&mut self, output: &mut VecDeque<WireFrame>) {
            output.append(&mut self.shared.borrow_mut().inbox);
        }

        pub fn status(&self) -> TransportStatus {
            self.shared.borrow().status.clone()
        }

        pub fn diagnostics(&self) -> ConnectionDiagnostics {
            self.shared.borrow().diagnostics.clone()
        }
    }

    impl Drop for Transport {
        fn drop(&mut self) {
            if let (Some(window), Some(timer)) = (web_sys::window(), self.keepalive_id) {
                window.clear_interval_with_handle(timer);
            }
        }
    }
}
