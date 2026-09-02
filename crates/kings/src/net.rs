//! A WebSocket channel with two implementations behind one interface:
//! `fire::net`, over `kings_core::proto`.
//!
//! On the web it is `web_sys::WebSocket` driven by JS callbacks pushing into
//! a shared inbox; natively it is a tungstenite socket on a reader thread,
//! with rustls so `kings-app online wss://...` works. The split exists so
//! `online.rs` can be tested natively with a real server instead of only by
//! loading a page and squinting. Duplicating this with fire is a backlog
//! line (lift to a shared crate).

use std::collections::VecDeque;

use kings_core::proto::{C2S, S2C};

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

// ---- web ------------------------------------------------------------------

#[cfg(target_arch = "wasm32")]
mod imp {
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::rc::Rc;

    use kings_core::proto::{C2S, S2C};
    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;

    use super::Status;

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
    }

    impl Net {
        pub fn connect(url: &str) -> Result<Self, String> {
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

            let s = Rc::clone(&shared);
            let on_open = Closure::<dyn FnMut(web_sys::Event)>::new(move |_| {
                s.borrow_mut().status = Status::Open;
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

            Ok(Self {
                ws,
                shared,
                _on_msg: on_msg,
                _on_open: on_open,
                _on_close: on_close,
                _on_err: on_err,
            })
        }

        pub fn send(&self, msg: &C2S) {
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
}

// ---- native ---------------------------------------------------------------

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use std::collections::VecDeque;
    use std::sync::mpsc::{self, Receiver, Sender};
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::Duration;

    use kings_core::proto::{C2S, S2C};
    use tungstenite::Message;
    use tungstenite::stream::MaybeTlsStream;

    use super::Status;

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
        /// Connect in a background reader thread.
        ///
        /// # Errors
        ///
        /// Returns an error if the operating system cannot create that thread.
        pub fn connect(url: &str) -> Result<Self, String> {
            let (in_tx, in_rx) = mpsc::channel::<S2C>();
            let (out_tx, out_rx) = mpsc::channel::<Message>();
            let status = Arc::new(Mutex::new(Status::Connecting));

            // rustls needs an explicitly installed crypto provider (both
            // backends are compiled into the tree). Err = already installed.
            drop(rustls::crypto::ring::default_provider().install_default());

            let st = Arc::clone(&status);
            let url = url.to_string();
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
                    set_status(&st, Status::Open);
                    loop {
                        while let Ok(m) = out_rx.try_recv() {
                            if ws.send(m).is_err() {
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

/// Convenience for the common send.
pub fn hello(net: &Net, handle: &str) {
    net.send(&C2S::Hello {
        proto: kings_core::proto::PROTO_VERSION,
        handle: handle.to_string(),
    });
}
