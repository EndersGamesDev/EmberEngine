//! Online mode: a platform-split WebSocket channel plus the client-side
//! game that renders the server-authoritative match with interpolation.

use ember_engine::{EmberGame, Frame, InputState, KeyCode};
use pong_core::proto::{C2S, S2C, PROTO_VERSION, STATE_EVERY_TICKS};
use serde::Deserialize;

use crate::{build_scene, SceneParams};

#[derive(Deserialize, Clone, Debug)]
pub struct OnlineConfig {
    pub url: String,
    /// "create" or "join"
    pub action: String,
    pub lobby: String,
    #[serde(default)]
    pub password: Option<String>,
    pub handle: String,
}

impl OnlineConfig {
    fn opening_msgs(&self) -> Result<Vec<C2S>, String> {
        let action = match self.action.as_str() {
            "create" => C2S::CreateLobby {
                name: self.lobby.clone(),
                password: self.password.clone().filter(|p| !p.is_empty()),
            },
            "join" => C2S::JoinLobby {
                name: self.lobby.clone(),
                password: self.password.clone().filter(|p| !p.is_empty()),
            },
            other => return Err(format!("unknown action \"{other}\"")),
        };
        Ok(vec![
            C2S::Hello { proto: PROTO_VERSION, handle: self.handle.clone() },
            action,
        ])
    }
}

/// Show progress where the player can see it: the page's #status element on
/// the web, the log on native.
fn set_status(text: &str) {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(el) = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id("status"))
        {
            el.set_text_content(Some(text));
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    tracing::info!(status = %text);
}

#[derive(Clone, Copy, Default)]
struct NetSnap {
    ball: [f32; 2],
    paddles: [f32; 2],
}

#[derive(PartialEq)]
enum OnlinePhase {
    Waiting,
    Playing,
    Lost,
}

pub struct OnlineGame {
    chan: net::NetChan,
    handle: String,
    phase: OnlinePhase,
    role: u8,
    opponent: String,
    from: NetSnap,
    to: NetSnap,
    t: f32,
    scores: [u32; 2],
    serving: bool,
    last_axis: f32,
    since_input: f32,
    since_ping: f32,
    anim_t: f32,
}

impl OnlineGame {
    pub fn connect(cfg: &OnlineConfig) -> Result<Self, String> {
        let chan = net::NetChan::connect(&cfg.url, cfg.opening_msgs()?)?;
        set_status("connecting…");
        Ok(Self {
            chan,
            handle: cfg.handle.clone(),
            phase: OnlinePhase::Waiting,
            role: 0,
            opponent: String::new(),
            from: NetSnap::default(),
            to: NetSnap::default(),
            t: 1.0,
            scores: [0, 0],
            serving: true,
            last_axis: 0.0,
            since_input: 0.0,
            since_ping: 0.0,
            anim_t: 0.0,
        })
    }

    fn render_snap(&self) -> NetSnap {
        let a = self.t.clamp(0.0, 1.0);
        let lerp = |x: f32, y: f32| x + (y - x) * a;
        NetSnap {
            ball: [
                lerp(self.from.ball[0], self.to.ball[0]),
                lerp(self.from.ball[1], self.to.ball[1]),
            ],
            paddles: [
                lerp(self.from.paddles[0], self.to.paddles[0]),
                lerp(self.from.paddles[1], self.to.paddles[1]),
            ],
        }
    }

    fn me_vs_them(&self) -> String {
        // scores[0] is always the near/blue player (role 0).
        let (me, them) = if self.role == 0 {
            (self.scores[0], self.scores[1])
        } else {
            (self.scores[1], self.scores[0])
        };
        format!("{} {me} : {them} {}", self.handle, self.opponent)
    }
}

impl EmberGame for OnlineGame {
    fn update(&mut self, input: &InputState, dt: f32) -> Frame {
        self.anim_t += dt;
        self.since_input += dt;
        self.since_ping += dt;

        while let Some(msg) = self.chan.poll() {
            match msg {
                S2C::Welcome { .. } => set_status("connected — waiting in lobby…"),
                S2C::LobbyCreated { name } => {
                    set_status(&format!("lobby \"{name}\" created — waiting for an opponent…"));
                }
                S2C::MatchStart { role, opponent } => {
                    self.role = role;
                    self.opponent = opponent;
                    self.phase = OnlinePhase::Playing;
                    self.scores = [0, 0];
                    self.from = NetSnap::default();
                    self.to = NetSnap::default();
                    set_status(&format!("match vs {} — {}", self.opponent, self.me_vs_them()));
                }
                S2C::State { ball, paddles, scores, serving, .. } => {
                    self.from = self.render_snap();
                    self.to = NetSnap { ball, paddles };
                    self.t = 0.0;
                    self.scores = scores;
                    self.serving = serving;
                }
                S2C::MatchEvent { scorer, won, scores } => {
                    self.scores = scores;
                    let i_scored = scorer == self.role;
                    let line = if won {
                        if i_scored { "🏆 YOU WIN the game!" } else { "opponent wins the game" }
                    } else if i_scored {
                        "you score!"
                    } else {
                        "opponent scores"
                    };
                    set_status(&format!("{line}  ·  {}", self.me_vs_them()));
                }
                S2C::OpponentLeft => {
                    self.phase = OnlinePhase::Waiting;
                    set_status("opponent left — lobby is open again, waiting…");
                }
                S2C::Error { message } => {
                    self.phase = OnlinePhase::Lost;
                    set_status(&format!("server error: {message}"));
                }
                S2C::Pong { .. } | S2C::LobbyList { .. } => {}
            }
        }
        if self.chan.is_dead() && self.phase != OnlinePhase::Lost {
            self.phase = OnlinePhase::Lost;
            set_status("connection lost — reload to play again");
        }

        // Input: either key set steers YOUR paddle. With the flipped camera
        // (role 1) screen-left is +x, so invert to keep controls natural.
        if self.phase == OnlinePhase::Playing {
            let mut axis = (input.axis(KeyCode::KeyA, KeyCode::KeyD)
                + input.axis(KeyCode::ArrowLeft, KeyCode::ArrowRight))
            .clamp(-1.0, 1.0);
            if self.role == 1 {
                axis = -axis;
            }
            if axis != self.last_axis || self.since_input > 0.1 {
                self.last_axis = axis;
                self.since_input = 0.0;
                self.chan.send(&C2S::Input { axis });
            }
        }
        if self.since_ping > 4.0 {
            self.since_ping = 0.0;
            self.chan.send(&C2S::Ping { nonce: 1 });
        }

        // Advance interpolation: one state interval covers from->to.
        self.t += dt * (60.0 / STATE_EVERY_TICKS as f32);
        let snap = self.render_snap();
        let ball_y = if self.serving {
            0.5 + (self.anim_t * 6.0).sin().abs() * 0.4
        } else {
            0.5
        };
        build_scene(&SceneParams {
            p1_x: snap.paddles[0],
            p2_x: snap.paddles[1],
            ball: snap.ball,
            ball_y,
            scores: self.scores,
            flip: self.role == 1,
        })
    }
}

// ---- platform-split WebSocket channel ----

#[cfg(not(target_arch = "wasm32"))]
mod net {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc::{self, Receiver, Sender};
    use std::sync::Arc;
    use std::time::Duration;

    use pong_core::proto::{C2S, S2C};
    use tungstenite::stream::MaybeTlsStream;
    use tungstenite::Message;

    pub struct NetChan {
        out_tx: Sender<C2S>,
        in_rx: Receiver<S2C>,
        dead: Arc<AtomicBool>,
    }

    impl NetChan {
        pub fn connect(url: &str, initial: Vec<C2S>) -> Result<NetChan, String> {
            // rustls needs an explicitly installed crypto provider (both
            // backends are compiled into the tree). Err = already installed.
            let _ = rustls::crypto::ring::default_provider().install_default();
            let (mut ws, _) = tungstenite::connect(url).map_err(|e| format!("connect: {e}"))?;
            match ws.get_ref() {
                MaybeTlsStream::Plain(s) => {
                    let _ = s.set_read_timeout(Some(Duration::from_millis(20)));
                }
                MaybeTlsStream::Rustls(s) => {
                    let _ = s.get_ref().set_read_timeout(Some(Duration::from_millis(20)));
                }
                _ => {}
            }
            for msg in &initial {
                let text = serde_json::to_string(msg).map_err(|e| e.to_string())?;
                ws.send(Message::text(text)).map_err(|e| format!("send: {e}"))?;
            }

            let (out_tx, out_rx) = mpsc::channel::<C2S>();
            let (in_tx, in_rx) = mpsc::channel::<S2C>();
            let dead = Arc::new(AtomicBool::new(false));
            {
                let dead = Arc::clone(&dead);
                std::thread::spawn(move || {
                    loop {
                        // Outbound first…
                        loop {
                            match out_rx.try_recv() {
                                Ok(msg) => {
                                    let Ok(text) = serde_json::to_string(&msg) else { continue };
                                    if ws.send(Message::text(text)).is_err() {
                                        dead.store(true, Ordering::Relaxed);
                                        return;
                                    }
                                }
                                Err(mpsc::TryRecvError::Empty) => break,
                                Err(mpsc::TryRecvError::Disconnected) => {
                                    let _ = ws.close(None);
                                    return;
                                }
                            }
                        }
                        // …then poll one inbound frame (20 ms max).
                        match ws.read() {
                            Ok(Message::Text(t)) => {
                                if let Ok(msg) = serde_json::from_str::<S2C>(t.as_str()) {
                                    if in_tx.send(msg).is_err() {
                                        return;
                                    }
                                }
                            }
                            Ok(Message::Close(_)) => {
                                dead.store(true, Ordering::Relaxed);
                                return;
                            }
                            Ok(_) => {}
                            Err(tungstenite::Error::Io(e))
                                if e.kind() == std::io::ErrorKind::WouldBlock
                                    || e.kind() == std::io::ErrorKind::TimedOut => {}
                            Err(_) => {
                                dead.store(true, Ordering::Relaxed);
                                return;
                            }
                        }
                    }
                });
            }
            Ok(NetChan { out_tx, in_rx, dead })
        }

        pub fn send(&mut self, msg: &C2S) {
            let _ = self.out_tx.send(clone_c2s(msg));
        }

        pub fn poll(&mut self) -> Option<S2C> {
            self.in_rx.try_recv().ok()
        }

        pub fn is_dead(&self) -> bool {
            self.dead.load(Ordering::Relaxed)
        }
    }

    /// C2S is small and only built from owned data; re-serialize instead of
    /// deriving Clone on the protocol type.
    fn clone_c2s(msg: &C2S) -> C2S {
        serde_json::from_str(&serde_json::to_string(msg).unwrap()).unwrap()
    }
}

#[cfg(target_arch = "wasm32")]
mod net {
    use std::cell::{Cell, RefCell};
    use std::collections::VecDeque;
    use std::rc::Rc;

    use pong_core::proto::{C2S, S2C};
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::JsCast;

    pub struct NetChan {
        ws: web_sys::WebSocket,
        inbox: Rc<RefCell<VecDeque<S2C>>>,
        open: Rc<Cell<bool>>,
        dead: Rc<Cell<bool>>,
        /// Messages queued until the socket opens.
        pending: Rc<RefCell<Vec<String>>>,
        _callbacks: Vec<Closure<dyn FnMut(web_sys::Event)>>,
        _on_msg: Closure<dyn FnMut(web_sys::MessageEvent)>,
    }

    impl NetChan {
        pub fn connect(url: &str, initial: Vec<C2S>) -> Result<NetChan, String> {
            let ws = web_sys::WebSocket::new(url).map_err(|_| format!("bad url: {url}"))?;
            let inbox = Rc::new(RefCell::new(VecDeque::new()));
            let open = Rc::new(Cell::new(false));
            let dead = Rc::new(Cell::new(false));
            let pending: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(
                initial
                    .iter()
                    .map(|m| serde_json::to_string(m).unwrap())
                    .collect(),
            ));

            let on_msg = {
                let inbox = Rc::clone(&inbox);
                Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |e: web_sys::MessageEvent| {
                    if let Some(text) = e.data().as_string() {
                        if let Ok(msg) = serde_json::from_str::<S2C>(&text) {
                            inbox.borrow_mut().push_back(msg);
                        }
                    }
                })
            };
            ws.set_onmessage(Some(on_msg.as_ref().unchecked_ref()));

            let mut callbacks = Vec::new();
            {
                let open = Rc::clone(&open);
                let pending = Rc::clone(&pending);
                let ws2 = ws.clone();
                let cb = Closure::<dyn FnMut(web_sys::Event)>::new(move |_| {
                    open.set(true);
                    for text in pending.borrow_mut().drain(..) {
                        let _ = ws2.send_with_str(&text);
                    }
                });
                ws.set_onopen(Some(cb.as_ref().unchecked_ref()));
                callbacks.push(cb);
            }
            for setter in ["error", "close"] {
                let dead = Rc::clone(&dead);
                let cb = Closure::<dyn FnMut(web_sys::Event)>::new(move |_| {
                    dead.set(true);
                });
                match setter {
                    "error" => ws.set_onerror(Some(cb.as_ref().unchecked_ref())),
                    _ => ws.set_onclose(Some(cb.as_ref().unchecked_ref())),
                }
                callbacks.push(cb);
            }

            Ok(NetChan { ws, inbox, open, dead, pending, _callbacks: callbacks, _on_msg: on_msg })
        }

        pub fn send(&mut self, msg: &C2S) {
            let Ok(text) = serde_json::to_string(msg) else { return };
            if self.open.get() {
                if self.ws.send_with_str(&text).is_err() {
                    self.dead.set(true);
                }
            } else if !self.dead.get() {
                self.pending.borrow_mut().push(text);
            }
        }

        pub fn poll(&mut self) -> Option<S2C> {
            self.inbox.borrow_mut().pop_front()
        }

        pub fn is_dead(&self) -> bool {
            self.dead.get()
        }
    }
}
