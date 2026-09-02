//! A WebSocket channel with two implementations behind one interface:
//! `fire::net`, over `kings_core::proto`.
//!
//! On the web it is `web_sys::WebSocket` driven by JS callbacks pushing into
//! a shared inbox; natively it is a tungstenite socket on a reader thread,
//! with rustls so `kings-app online wss://...` works. The split exists so
//! `online.rs` can be tested natively with a real server instead of only by
//! loading a page and squinting. Duplicating this with fire is a backlog
//! line (lift to a shared crate).
//!
//! The socket owns the two messages that keep a seat: it sends `Hello` the
//! moment it opens and a `Ping` every `CLIENT_PING_SECS` while it is open.
//! Neither may depend on the game loop. On the web that loop runs on
//! `requestAnimationFrame`, and a hidden tab gets no frames, so a client that
//! greeted and pinged from `update` fell silent the moment the player looked
//! away and was dropped after `CLIENT_TIMEOUT_SECS`. A JS interval keeps
//! running when frames stop; natively the reader thread does the same.

use std::collections::VecDeque;
use std::time::Duration;

use kings_core::proto::{C2S, CLIENT_PING_SECS, PROTO_VERSION, S2C};

/// Where the socket is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    /// Not open yet.
    Connecting,
    /// Open.
    Open,
    /// Closed, with why.
    Closed(String),
}

/// How often the socket pings, from the protocol.
#[must_use]
pub const fn ping_period() -> Duration {
    Duration::from_secs(CLIENT_PING_SECS)
}

/// The one `Hello` a connection sends, already serialized.
fn hello_text(handle: &str) -> String {
    serde_json::to_string(&C2S::Hello {
        proto: PROTO_VERSION,
        handle: handle.to_string(),
    })
    .unwrap_or_default()
}

/// The keepalive, already serialized.
fn ping_text(nonce: u32) -> String {
    serde_json::to_string(&C2S::Ping { nonce }).unwrap_or_default()
}

/// Whether the game may send this. `Hello` is the socket's: the server
/// closes a connection on a second one, so it never goes through `send`.
fn sendable(msg: &C2S) -> bool {
    if matches!(msg, C2S::Hello { .. }) {
        tracing::warn!("kings net: Hello is sent by the socket itself; ignoring the game's");
        return false;
    }
    true
}

// ---- web ------------------------------------------------------------------

#[cfg(target_arch = "wasm32")]
mod imp {
    use std::cell::{Cell, RefCell};
    use std::collections::VecDeque;
    use std::rc::Rc;

    use kings_core::proto::{C2S, S2C};
    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;

    use super::{Status, hello_text, ping_period, ping_text, sendable};

    struct Shared {
        inbox: VecDeque<S2C>,
        status: Status,
    }

    pub struct Net {
        ws: web_sys::WebSocket,
        shared: Rc<RefCell<Shared>>,
        // Closures must outlive the socket or the callbacks are dropped and
        // messages silently stop arriving.
        _on_msg: Closure<dyn FnMut(web_sys::MessageEvent)>,
        _on_open: Closure<dyn FnMut(web_sys::Event)>,
        _on_close: Closure<dyn FnMut(web_sys::CloseEvent)>,
        _on_err: Closure<dyn FnMut(web_sys::Event)>,
        /// The keepalive timer, cleared when the channel is dropped.
        keepalive_id: Option<i32>,
        _keepalive: Option<Closure<dyn FnMut()>>,
    }

    impl Net {
        /// Open the socket. It greets the server with `handle` as soon as it
        /// is open and pings on a timer from then on; the game sends nothing
        /// until `Welcome` arrives.
        ///
        /// # Errors
        ///
        /// Returns an error if the URL is not one a `WebSocket` accepts.
        pub fn connect(url: &str, handle: &str) -> Result<Self, String> {
            let ws = web_sys::WebSocket::new(url).map_err(|_| format!("bad url: {url}"))?;
            let shared = Rc::new(RefCell::new(Shared {
                inbox: VecDeque::new(),
                status: Status::Connecting,
            }));

            let s = Rc::clone(&shared);
            let on_msg = Closure::<dyn FnMut(web_sys::MessageEvent)>::new(
                move |e: web_sys::MessageEvent| {
                    if let Some(txt) = e.data().as_string() {
                        // A frame this build cannot parse is the server being
                        // newer, not a reason to tear the connection down.
                        if let Ok(m) = serde_json::from_str::<S2C>(&txt) {
                            s.borrow_mut().inbox.push_back(m);
                        }
                    }
                },
            );
            ws.set_onmessage(Some(on_msg.as_ref().unchecked_ref()));

            // Hello goes out from the open event, not from a frame: a tab
            // hidden at connect time gets no frames, and `open` fires exactly
            // once per socket, which is the one Hello the server accepts.
            let s = Rc::clone(&shared);
            let hello = hello_text(handle);
            let ws_open = ws.clone();
            let on_open = Closure::<dyn FnMut(web_sys::Event)>::new(move |_| {
                s.borrow_mut().status = Status::Open;
                if ws_open.send_with_str(&hello).is_err() {
                    tracing::warn!("kings net: Hello failed to send on open");
                }
            });
            ws.set_onopen(Some(on_open.as_ref().unchecked_ref()));

            let s = Rc::clone(&shared);
            let on_close =
                Closure::<dyn FnMut(web_sys::CloseEvent)>::new(move |e: web_sys::CloseEvent| {
                    let why = e.reason();
                    s.borrow_mut().status = Status::Closed(if why.is_empty() {
                        format!("connection closed ({})", e.code())
                    } else {
                        why
                    });
                });
            ws.set_onclose(Some(on_close.as_ref().unchecked_ref()));

            let s = Rc::clone(&shared);
            let on_err = Closure::<dyn FnMut(web_sys::Event)>::new(move |_| {
                let mut b = s.borrow_mut();
                if b.status != Status::Open {
                    b.status = Status::Closed("could not reach the server".into());
                }
            });
            ws.set_onerror(Some(on_err.as_ref().unchecked_ref()));

            // The keepalive runs on a JS interval, NOT on the frame loop, so
            // it keeps going in a hidden tab. It sends only while the socket
            // is open: a Ping before Hello is a protocol violation, and one
            // after close is noise.
            let mut keepalive = None;
            let mut keepalive_id = None;
            if let Some(win) = web_sys::window() {
                let ws_ping = ws.clone();
                let nonce = Cell::new(0u32);
                let cb = Closure::<dyn FnMut()>::new(move || {
                    if ws_ping.ready_state() != web_sys::WebSocket::OPEN {
                        return;
                    }
                    let n = nonce.get().wrapping_add(1);
                    nonce.set(n);
                    if ws_ping.send_with_str(&ping_text(n)).is_err() {
                        tracing::warn!("kings net: Ping failed to send");
                    }
                });
                let period_ms = i32::try_from(ping_period().as_millis()).unwrap_or(5000);
                match win.set_interval_with_callback_and_timeout_and_arguments_0(
                    cb.as_ref().unchecked_ref(),
                    period_ms,
                ) {
                    Ok(id) => keepalive_id = Some(id),
                    Err(_) => {
                        tracing::warn!("kings net: no keepalive timer; the seat will time out")
                    }
                }
                keepalive = Some(cb);
            } else {
                tracing::warn!("kings net: no window; no keepalive timer");
            }

            Ok(Self {
                ws,
                shared,
                _on_msg: on_msg,
                _on_open: on_open,
                _on_close: on_close,
                _on_err: on_err,
                keepalive_id,
                _keepalive: keepalive,
            })
        }

        pub fn send(&self, msg: &C2S) {
            if !sendable(msg) {
                return;
            }
            if let Ok(t) = serde_json::to_string(msg)
                && self.ws.send_with_str(&t).is_err()
            {
                // The socket is closing or closed; `status()` reports that
                // on the next frame, so a warning is all this needs to be.
                tracing::warn!("kings net: send failed, the socket is not open");
            }
        }

        pub fn drain(&mut self, out: &mut VecDeque<S2C>) {
            let mut b = self.shared.borrow_mut();
            while let Some(m) = b.inbox.pop_front() {
                out.push_back(m);
            }
        }

        pub fn status(&self) -> Status {
            self.shared.borrow().status.clone()
        }
    }

    impl Drop for Net {
        fn drop(&mut self) {
            if let (Some(win), Some(id)) = (web_sys::window(), self.keepalive_id) {
                win.clear_interval_with_handle(id);
            }
        }
    }
}

// ---- native ---------------------------------------------------------------

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use std::collections::VecDeque;
    use std::sync::mpsc::{self, Receiver, Sender};
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::{Duration, Instant};

    use kings_core::proto::{C2S, S2C};
    use tungstenite::Message;
    use tungstenite::stream::MaybeTlsStream;

    use super::{Status, hello_text, ping_period, ping_text, sendable};

    fn set_status(status: &Mutex<Status>, next: Status) {
        *status
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = next;
    }

    /// The reader polls with a short timeout so it can also send.
    fn set_read_timeout(ws: &tungstenite::WebSocket<MaybeTlsStream<std::net::TcpStream>>) {
        let timeout = Some(Duration::from_millis(5));
        match ws.get_ref() {
            MaybeTlsStream::Plain(s) => drop(s.set_read_timeout(timeout)),
            MaybeTlsStream::Rustls(s) => drop(s.get_ref().set_read_timeout(timeout)),
            _ => {}
        }
    }

    pub struct Net {
        rx: Receiver<S2C>,
        tx: Sender<Message>,
        status: Arc<Mutex<Status>>,
    }

    impl Net {
        /// Connect in a background reader thread. The thread greets the
        /// server with `handle` as soon as the socket is open and pings on
        /// its own clock from then on; the game sends nothing until
        /// `Welcome` arrives.
        ///
        /// # Errors
        ///
        /// Returns an error if the operating system cannot create that thread.
        pub fn connect(url: &str, handle: &str) -> Result<Self, String> {
            let (in_tx, in_rx) = mpsc::channel::<S2C>();
            let (out_tx, out_rx) = mpsc::channel::<Message>();
            let status = Arc::new(Mutex::new(Status::Connecting));

            // rustls needs an explicitly installed crypto provider (both
            // backends are compiled into the tree). Err = already installed.
            drop(rustls::crypto::ring::default_provider().install_default());

            let st = Arc::clone(&status);
            let url = url.to_string();
            let hello = hello_text(handle);
            thread::Builder::new()
                .name("kings-net".into())
                .spawn(move || {
                    let mut ws = match tungstenite::connect(&url) {
                        Ok((ws, _)) => ws,
                        Err(e) => {
                            set_status(&st, Status::Closed(e.to_string()));
                            return;
                        }
                    };
                    set_read_timeout(&ws);
                    // The one Hello, before anything the game queued: the
                    // server closes on any other message first.
                    if ws.send(Message::text(hello)).is_err() {
                        set_status(&st, Status::Closed("Hello failed to send".into()));
                        return;
                    }
                    set_status(&st, Status::Open);
                    let period = ping_period();
                    let mut last_ping = Instant::now();
                    let mut nonce: u32 = 0;
                    loop {
                        while let Ok(m) = out_rx.try_recv() {
                            if ws.send(m).is_err() {
                                set_status(&st, Status::Closed("send failed".into()));
                                return;
                            }
                        }
                        // The keepalive lives here, not in the game loop, so
                        // a game that stops calling `update` keeps its seat.
                        if last_ping.elapsed() >= period {
                            last_ping = Instant::now();
                            nonce = nonce.wrapping_add(1);
                            if ws.send(Message::text(ping_text(nonce))).is_err() {
                                set_status(&st, Status::Closed("send failed".into()));
                                return;
                            }
                        }
                        match ws.read() {
                            Ok(Message::Text(t)) => match serde_json::from_str::<S2C>(&t) {
                                Ok(m) => {
                                    if in_tx.send(m).is_err() {
                                        return;
                                    }
                                }
                                // A frame this build cannot parse is the server
                                // being newer. Say so: silently discarding it
                                // makes a lost message indistinguishable from one
                                // that was never sent.
                                Err(e) => {
                                    tracing::warn!("kings net: undecodable frame ({e}): {t}");
                                }
                            },
                            Ok(Message::Close(_)) => {
                                set_status(&st, Status::Closed("server closed".into()));
                                return;
                            }
                            Ok(_) => {}
                            Err(tungstenite::Error::Io(e))
                                if kings_core::proto::is_transient_read(&e) => {}
                            Err(e) => {
                                // The reader owns the only path messages arrive
                                // by, so its death is total and silent from the
                                // game's point of view: `drain` simply returns
                                // nothing for ever. Record why.
                                tracing::warn!("kings net: reader thread exiting: {e}");
                                set_status(&st, Status::Closed(e.to_string()));
                                return;
                            }
                        }
                    }
                })
                .map_err(|e| e.to_string())?;

            Ok(Self {
                rx: in_rx,
                tx: out_tx,
                status,
            })
        }

        pub fn send(&self, msg: &C2S) {
            if !sendable(msg) {
                return;
            }
            if let Ok(t) = serde_json::to_string(msg) {
                drop(self.tx.send(Message::text(t)));
            }
        }

        pub fn drain(&mut self, out: &mut VecDeque<S2C>) {
            while let Ok(m) = self.rx.try_recv() {
                out.push_back(m);
            }
        }

        #[must_use]
        pub fn status(&self) -> Status {
            self.status
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone()
        }
    }
}

pub use imp::Net;

/// A queue of messages received but not yet consumed by the game loop.
#[derive(Default)]
pub struct Inbox(pub VecDeque<S2C>);

impl Inbox {
    /// Move everything the socket has received into the queue.
    pub fn pump(&mut self, net: &mut Net) {
        net.drain(&mut self.0);
    }

    /// The oldest unconsumed message.
    pub fn pop(&mut self) -> Option<S2C> {
        self.0.pop_front()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ping_period_is_the_protocols() {
        assert_eq!(ping_period(), Duration::from_secs(CLIENT_PING_SECS));
        assert_eq!(ping_period().as_secs(), 5);
    }

    #[test]
    fn the_socket_greets_with_this_builds_protocol() {
        let hello: C2S = serde_json::from_str(&hello_text("ada")).unwrap();
        assert_eq!(
            hello,
            C2S::Hello {
                proto: PROTO_VERSION,
                handle: "ada".into(),
            }
        );
        assert_eq!(
            serde_json::from_str::<C2S>(&ping_text(7)).unwrap(),
            C2S::Ping { nonce: 7 }
        );
    }

    /// A second Hello closes the connection server-side, so the game's
    /// `send` must never let one through.
    #[test]
    fn the_game_cannot_send_a_hello() {
        assert!(!sendable(&C2S::Hello {
            proto: PROTO_VERSION,
            handle: "x".into(),
        }));
        assert!(sendable(&C2S::Ping { nonce: 1 }));
        assert!(sendable(&C2S::Start));
    }
}
