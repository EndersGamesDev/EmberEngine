//! A WebSocket channel with two implementations behind one interface.
//!
//! On the web it is `web_sys::WebSocket` driven by JS callbacks pushing into
//! a shared inbox; natively it is a tungstenite socket on a reader thread.
//! The split exists so the interesting half — reconciling prediction against
//! the server, in `online.rs` — can be tested natively with a real server
//! instead of only by loading a page and squinting.

use std::collections::VecDeque;

use fire_core::proto::{C2S, S2C};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Connecting,
    Open,
    Closed(String),
}

// ---- web ------------------------------------------------------------------

#[cfg(target_arch = "wasm32")]
mod imp {
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::rc::Rc;

    use fire_core::proto::{C2S, S2C};
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::JsCast;

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
            let on_close = Closure::<dyn FnMut(web_sys::CloseEvent)>::new(
                move |e: web_sys::CloseEvent| {
                    let why = e.reason();
                    s.borrow_mut().status = Status::Closed(if why.is_empty() {
                        format!("connection closed ({})", e.code())
                    } else {
                        why
                    });
                },
            );
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
            if let Ok(t) = serde_json::to_string(msg) {
                let _ = self.ws.send_with_str(&t);
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

    use fire_core::proto::{C2S, S2C};
    use tungstenite::Message;

    use super::Status;

    pub struct Net {
        rx: Receiver<S2C>,
        tx: Sender<Message>,
        status: Arc<Mutex<Status>>,
    }

    impl Net {
        pub fn connect(url: &str) -> Result<Self, String> {
            let (in_tx, in_rx) = mpsc::channel::<S2C>();
            let (out_tx, out_rx) = mpsc::channel::<Message>();
            let status = Arc::new(Mutex::new(Status::Connecting));

            let st = Arc::clone(&status);
            let url = url.to_string();
            thread::spawn(move || {
                let mut ws = match tungstenite::connect(&url) {
                    Ok((ws, _)) => ws,
                    Err(e) => {
                        *st.lock().unwrap() = Status::Closed(e.to_string());
                        return;
                    }
                };
                if let tungstenite::stream::MaybeTlsStream::Plain(s) = ws.get_ref() {
                    let _ = s.set_read_timeout(Some(Duration::from_millis(5)));
                }
                *st.lock().unwrap() = Status::Open;
                loop {
                    while let Ok(m) = out_rx.try_recv() {
                        if ws.send(m).is_err() {
                            *st.lock().unwrap() = Status::Closed("send failed".into());
                            return;
                        }
                    }
                    match ws.read() {
                        Ok(Message::Text(t)) => {
                            if let Ok(m) = serde_json::from_str::<S2C>(&t) {
                                if in_tx.send(m).is_err() {
                                    return;
                                }
                            }
                        }
                        Ok(Message::Close(_)) => {
                            *st.lock().unwrap() = Status::Closed("server closed".into());
                            return;
                        }
                        Ok(_) => {}
                        Err(tungstenite::Error::Io(e))
                            if matches!(
                                e.kind(),
                                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                            ) => {}
                        Err(e) => {
                            *st.lock().unwrap() = Status::Closed(e.to_string());
                            return;
                        }
                    }
                }
            });

            Ok(Self { rx: in_rx, tx: out_tx, status })
        }

        pub fn send(&self, msg: &C2S) {
            if let Ok(t) = serde_json::to_string(msg) {
                let _ = self.tx.send(Message::text(t));
            }
        }

        pub fn drain(&mut self, out: &mut VecDeque<S2C>) {
            while let Ok(m) = self.rx.try_recv() {
                out.push_back(m);
            }
        }

        pub fn status(&self) -> Status {
            self.status.lock().unwrap().clone()
        }
    }
}

pub use imp::Net;

/// A queue of messages received but not yet consumed by the game loop.
#[derive(Default)]
pub struct Inbox(pub VecDeque<S2C>);

impl Inbox {
    pub fn pump(&mut self, net: &mut Net) {
        net.drain(&mut self.0);
    }
    pub fn pop(&mut self) -> Option<S2C> {
        self.0.pop_front()
    }
}

/// Convenience for the common send.
pub fn hello(net: &Net, handle: &str) {
    net.send(&C2S::Hello {
        proto: fire_core::proto::PROTO_VERSION,
        handle: handle.to_string(),
    });
}
