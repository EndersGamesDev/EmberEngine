// Winit supplies physical positions as f64/u32, while engine input and camera math use f32.
#![allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]

use std::sync::Arc;
use std::time::Duration;

use web_time::Instant;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
#[cfg(not(target_arch = "wasm32"))]
use winit::keyboard::KeyCode;
use winit::keyboard::PhysicalKey;
use winit::window::{Window, WindowId};

use crate::EmberGame;
use crate::feedback::Rumble;
use crate::input::{InputState, PadButton, PadState};
use crate::renderer::Renderer;

#[cfg(target_arch = "wasm32")]
use std::{cell::RefCell, rc::Rc};

/// Install the native tracing pipeline.
///
/// It provides `RUST_LOG`-style filtering via `EnvFilter`, plus a bridge so
/// `log` records from wgpu/winit land in the
/// same output. Idempotent — game code may call it before `run()` to get
/// tracing during its own startup (e.g. connecting), and `run()` calls it
/// again harmlessly.
#[cfg(not(target_arch = "wasm32"))]
pub fn init_diagnostics() {
    use std::io::IsTerminal;
    use tracing_subscriber::EnvFilter;
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,wgpu_core=warn,wgpu_hal=warn,naga=warn"));
    let _ = tracing_log::LogTracer::init();
    drop(
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            // No color codes when output is redirected to a file.
            .with_ansi(std::io::stdout().is_terminal())
            .try_init(),
    );
}

/// A frame gap above this is reported as a stall.
const FRAME_STALL_THRESHOLD_MS: u128 = 100;

pub struct EngineConfig {
    pub title: String,
    /// FPS-style mouse capture: clicking the window grabs the cursor
    /// (pointer lock on the web) and mouse-look deltas start flowing.
    pub capture_mouse: bool,
    /// Extra meshes registered at startup; instances reference them by id
    /// (1..=N, in order — 0 is the built-in cube).
    pub meshes: Vec<crate::renderer::MeshData>,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            title: "ember".to_string(),
            capture_mouse: false,
            meshes: Vec::new(),
        }
    }
}

struct App<G: EmberGame> {
    config: EngineConfig,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    /// On the web the renderer is created by an async task (no blocking on
    /// wasm); it lands here and is picked up on the next redraw.
    #[cfg(target_arch = "wasm32")]
    pending_renderer: Rc<RefCell<Option<Renderer>>>,
    game: G,
    input: InputState,
    /// Gamepads and rumble; the platform side of `EmberGame::feedback`.
    haptics: Haptics,
    /// Whether the window has keyboard focus. Keys arrive only while it
    /// does, but a pad is polled, not delivered, so the platform has to
    /// remember focus itself to give the pad the same rule: an alt-tabbed
    /// game neither reads a held trigger nor buzzes the pad.
    focused: bool,
    last_frame: Instant,
    /// Rate limit for stall warnings so one bad stretch doesn't spam.
    last_stall_warn: Option<Instant>,
    /// Debug overlay / ATW rig (F3), native only.
    #[cfg(not(target_arch = "wasm32"))]
    overlay: Option<crate::overlay::Overlay>,
}

impl<G: EmberGame> ApplicationHandler for App<G> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // On desktop `resumed` fires once at startup; guard so a second call
        // (possible on some platforms) doesn't rebuild the GPU context.
        if self.window.is_some() {
            return;
        }
        let window = Arc::new(
            event_loop
                .create_window(Window::default_attributes().with_title(&self.config.title))
                .expect("failed to create window"),
        );

        let meshes = std::mem::take(&mut self.config.meshes);
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.renderer = Some(pollster::block_on(Renderer::new(window.clone(), meshes)));
        }
        #[cfg(target_arch = "wasm32")]
        {
            use winit::platform::web::WindowExtWebSys;
            let canvas = window.canvas().expect("no canvas");
            let document = web_sys::window().unwrap().document().unwrap();
            let root = document
                .get_element_by_id("ember-root")
                .unwrap_or_else(|| document.body().expect("no body").into());
            root.append_child(&canvas).expect("append canvas");
            drop(canvas.focus()); // keyboard events go to the canvas
            // NOTE: no request_inner_size here — winit would pin an inline
            // CSS size that overrides the page's responsive width rule. CSS
            // owns layout; the per-frame sync below owns the backing store.

            let pending = Rc::clone(&self.pending_renderer);
            let win = window.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let renderer = Renderer::new(win.clone(), meshes).await;
                *pending.borrow_mut() = Some(renderer);
                win.request_redraw();
            });
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            self.overlay = Some(crate::overlay::Overlay::new(&window));
        }
        window.request_redraw();
        self.window = Some(window);
        self.last_frame = Instant::now();
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
        if let winit::event::DeviceEvent::MouseMotion { delta } = event {
            self.input.add_mouse_delta(delta.0 as f32, delta.1 as f32);
        }
    }

    // Keeping the winit event dispatch linear makes input ordering and early returns explicit.
    #[allow(clippy::too_many_lines)]
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        // Overlay first: F3 toggles it; a visible overlay may consume input.
        #[cfg(not(target_arch = "wasm32"))]
        if let (Some(overlay), Some(window)) = (self.overlay.as_mut(), self.window.as_ref()) {
            if let WindowEvent::KeyboardInput { event: key, .. } = &event
                && key.state == ElementState::Pressed
                && key.physical_key == PhysicalKey::Code(KeyCode::F3)
            {
                overlay.visible = !overlay.visible;
                return;
            }
            if overlay.on_window_event(window, &event) {
                return;
            }
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Focused(false) => {
                self.focused = false;
                self.input.clear();
                self.haptics.stop();
            }
            WindowEvent::Focused(true) => self.focused = true,
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    match event.state {
                        ElementState::Pressed => self.input.press(code),
                        ElementState::Released => self.input.release(code),
                    }
                    #[cfg(not(target_arch = "wasm32"))]
                    if code == KeyCode::Escape {
                        event_loop.exit();
                    }
                    // F11 toggles borderless fullscreen on the native
                    // window. It lives here in the platform layer rather
                    // than in any game because the game never sees the
                    // window (one-way layering), and every game wants the
                    // same thing from the key. Borderless, not exclusive:
                    // no mode switch, alt-tab keeps working, and the
                    // swapchain simply follows the Resized event.
                    //
                    // Native only. winit's web backend would call
                    // requestFullscreen on the bare canvas, which throws
                    // the page's own overlays (crosshair, scoreboard) off
                    // screen; the page fullscreens its stage element itself,
                    // and the per-frame canvas sync below picks up the new
                    // size. The browser's F11 stays the browser's.
                    #[cfg(not(target_arch = "wasm32"))]
                    if code == KeyCode::F11
                        && event.state == ElementState::Pressed
                        && !event.repeat
                        && let Some(window) = self.window.as_ref()
                    {
                        use winit::window::Fullscreen;
                        let next = if window.fullscreen().is_some() {
                            None
                        } else {
                            Some(Fullscreen::Borderless(None))
                        };
                        window.set_fullscreen(next);
                    }
                }
            }
            WindowEvent::Resized(size) => {
                tracing::debug!(width = size.width, height = size.height, "window resized");
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.resize(size.width, size.height);
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                if let Some(window) = self.window.as_ref() {
                    let size = window.inner_size();
                    if size.width > 0 && size.height > 0 {
                        self.input.set_cursor_ndc(Some([
                            (position.x as f32 / size.width as f32).mul_add(2.0, -1.0),
                            (position.y as f32 / size.height as f32).mul_add(-2.0, 1.0),
                        ]));
                    }
                }
            }
            WindowEvent::CursorLeft { .. } => self.input.set_cursor_ndc(None),
            WindowEvent::MouseInput { state, button, .. } => {
                match state {
                    ElementState::Pressed => self.input.mouse_press(button),
                    ElementState::Released => self.input.mouse_release(button),
                }
                // Clicking (re)captures the mouse for FPS look. Cheap to
                // re-request; also restores capture after Esc on the web.
                if self.config.capture_mouse
                    && state == ElementState::Pressed
                    && let Some(window) = self.window.as_ref()
                {
                    use winit::window::CursorGrabMode;
                    drop(
                        window
                            .set_cursor_grab(CursorGrabMode::Locked)
                            .or_else(|_| window.set_cursor_grab(CursorGrabMode::Confined)),
                    );
                    window.set_cursor_visible(false);
                }
            }
            WindowEvent::RedrawRequested => {
                #[cfg(target_arch = "wasm32")]
                {
                    if self.renderer.is_none() {
                        if let Some(r) = self.pending_renderer.borrow_mut().take() {
                            self.renderer = Some(r);
                            self.last_frame = Instant::now();
                            // The GPU is actually up: only now drop the
                            // page's "loading" placeholder.
                            if let Some(el) = web_sys::window()
                                .and_then(|w| w.document())
                                .and_then(|d| d.get_element_by_id("loading"))
                            {
                                el.remove();
                            }
                        }
                    }
                    // winit's web backend doesn't track the canvas CSS size,
                    // so sync the backing store to layout ourselves (wgpu
                    // sets canvas width/height on surface configure).
                    if let (Some(window), Some(renderer)) =
                        (self.window.as_ref(), self.renderer.as_mut())
                    {
                        use winit::platform::web::WindowExtWebSys;
                        if let Some(canvas) = window.canvas() {
                            let dpr = web_sys::window()
                                .map(|w| w.device_pixel_ratio())
                                .unwrap_or(1.0);
                            let w = (canvas.client_width() as f64 * dpr) as u32;
                            let h = (canvas.client_height() as f64 * dpr) as u32;
                            if w > 0 && h > 0 {
                                renderer.resize_if_changed(w, h);
                            }
                        }
                    }
                }

                // The sim must not advance while the renderer is still
                // initializing (async on wasm): the game would play out
                // invisibly against a blank canvas.
                if self.renderer.is_none() {
                    if let Some(window) = self.window.as_ref() {
                        window.request_redraw();
                    }
                    return;
                }

                if let Some(window) = self.window.as_ref() {
                    // NOTE (checked, not yet a bug): this and `set_cursor_ndc`
                    // below both measure `window.inner_size()`, while the
                    // renderer's projection uses its own surface size. On
                    // native those are the same quantity — `Resized` hands
                    // winit's new inner size straight to `renderer.resize`
                    // — so a picking ray and the image agree.
                    //
                    // On the web they are NOT: the branch above resizes the
                    // surface from `canvas.client_width() * dpr`, which
                    // winit's `inner_size` need not equal. Only the editor
                    // reads `aspect()`/`cursor_ndc()`, and the editor is
                    // native-only today, so nothing is wrong on screen
                    // anywhere yet. It becomes wrong the moment the editor
                    // has a web shell, which is why the fix belongs with
                    // that work rather than ahead of it.
                    let size = window.inner_size();
                    self.input
                        .set_aspect(size.width as f32 / size.height.max(1) as f32);
                }

                let now = Instant::now();
                let raw_gap = now.duration_since(self.last_frame);
                // Clamp dt so a debugger pause or long stall doesn't teleport
                // everything on the next frame.
                let dt = raw_gap.as_secs_f32().min(0.1);
                self.last_frame = now;

                // Stall detection: a gap well above any vsync interval means
                // the loop was starved (GC, OS hitch, hidden tab, GPU stall).
                if self.renderer.is_some() && raw_gap.as_millis() > FRAME_STALL_THRESHOLD_MS {
                    let ok_to_warn = self
                        .last_stall_warn
                        .is_none_or(|t| now.duration_since(t).as_secs() >= 1);
                    if ok_to_warn {
                        self.last_stall_warn = Some(now);
                        let stall_ms = u64::try_from(raw_gap.as_millis()).unwrap_or(u64::MAX);
                        tracing::warn!(
                            stall_ms,
                            "frame stall: gap since previous frame exceeded {FRAME_STALL_THRESHOLD_MS} ms"
                        );
                    }
                }

                // The pad is polled, not delivered: read it here, on the
                // main thread, right before the game sees the frame's
                // input, so a stick and a key pressed in the same frame
                // arrive together.
                let pad = self.haptics.poll_pad();
                self.input.set_pad(if self.focused { pad } else { None });
                self.input.set_pad_status(self.haptics.status());

                let frame = self.game.update(&self.input, dt);
                let feedback = self.game.feedback();
                self.input.end_frame();
                self.haptics.tick(now);
                if self.focused {
                    for r in feedback.rumbles {
                        self.haptics.request(r, now);
                    }
                }
                #[cfg(not(target_arch = "wasm32"))]
                if let Some(renderer) = self.renderer.as_mut() {
                    let draw = match (self.overlay.as_mut(), self.window.as_ref()) {
                        (Some(overlay), Some(window)) => {
                            renderer.set_scene_hz_cap(overlay.scene_hz_cap);
                            if overlay.visible {
                                let age = renderer.scene_age_ms();
                                let size = renderer.surface_size();
                                Some(overlay.run(window, dt, age, size))
                            } else {
                                None
                            }
                        }
                        _ => None,
                    };
                    renderer.render_with_overlay(&frame, draw);
                }
                #[cfg(target_arch = "wasm32")]
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.render(&frame);
                }
                // Continuous rendering: immediately schedule the next frame.
                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
            }
            _ => {}
        }
    }
}

/// Start the engine event loop and hand each frame to `game`.
///
/// # Panics
///
/// Panics if the platform cannot create the event loop or the native event
/// loop terminates with an error.
pub fn run<G: EmberGame + 'static>(config: EngineConfig, game: G) {
    #[cfg(not(target_arch = "wasm32"))]
    init_diagnostics();
    #[cfg(target_arch = "wasm32")]
    {
        console_error_panic_hook::set_once();
        let _ = console_log::init_with_level(log::Level::Info);
    }

    let event_loop = EventLoop::new().expect("failed to create event loop");
    #[cfg(not(target_arch = "wasm32"))]
    event_loop.set_control_flow(ControlFlow::Poll);
    // On the web the redraw loop is rAF-driven and self-sustaining; Poll
    // would busy-spin the event loop between frames.
    #[cfg(target_arch = "wasm32")]
    event_loop.set_control_flow(ControlFlow::Wait);
    let app = App {
        config,
        window: None,
        renderer: None,
        #[cfg(target_arch = "wasm32")]
        pending_renderer: Rc::new(RefCell::new(None)),
        game,
        input: InputState::default(),
        haptics: Haptics::new(),
        focused: true,
        last_frame: Instant::now(),
        last_stall_warn: None,
        #[cfg(not(target_arch = "wasm32"))]
        overlay: None,
    };

    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut app = app;
        event_loop.run_app(&mut app).expect("event loop error");
        // Every native exit (Escape, the close button) returns here, and
        // the motors may still be on: gilrs's force-feedback server never
        // resets a pad on its own, WGI's `SetVibration` is persistent, and
        // the server thread simply dies with the process. So the stop is
        // sent from this one place rather than from each exit path, and
        // the process waits one server tick for it to be delivered.
        if app.haptics.is_playing() {
            app.haptics.stop();
            std::thread::sleep(PLAY_TO_STOP_GRACE);
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        use winit::platform::web::EventLoopExtWebSys;
        event_loop.spawn_app(app); // returns immediately; runs on rAF
    }
}

// ---------------------------------------------------------------------------
// Haptics: the platform side of `EmberGame::feedback`, and the gamepad poll.
// ---------------------------------------------------------------------------

/// The least time between a play and the stop that ends it.
///
/// gilrs's force-feedback server runs on its own thread at 50 ms ticks and
/// drains its command queue once per tick, so a play and a stop that land
/// in the same tick collapse to the stop and the motors never move. A stop
/// is therefore never sent sooner than this after the last play, whatever
/// the request's own length: a 30 ms hitmarker becomes one server tick of
/// rumble rather than none. The web actuator is handed a duration and ends
/// the effect itself, so it needs no spacing.
#[cfg(not(target_arch = "wasm32"))]
const PLAY_TO_STOP_GRACE: Duration = Duration::from_millis(60);
#[cfg(target_arch = "wasm32")]
const PLAY_TO_STOP_GRACE: Duration = Duration::ZERO;

/// Gamepads and rumble. Owns whatever the platform has (gilrs on native,
/// the Gamepad API and the page's `emberRumble` shim on the web), and the
/// one piece of state both share: what the motors are doing right now.
///
/// Requests merge by `max` per motor while an earlier request is still
/// playing, and the one `ends_at` is the later of the two ends. A short
/// request never cuts a long one short, and its magnitude stays in the
/// merge until that single end passes: there is no per-motor end (see
/// [`Rumble`] for why). A request that arrives after `ends_at` starts from
/// zero, so a finished death rumble never leaks its magnitude into the
/// next tick.
pub struct Haptics {
    /// The merged strong-motor magnitude on the pad now (0 while idle).
    strong: f32,
    /// The merged weak-motor magnitude on the pad now (0 while idle).
    weak: f32,
    /// When the motors stop; `None` while idle.
    ends_at: Option<Instant>,
    /// When the motors were last (re)started, so `tick` can hold the stop
    /// back until [`PLAY_TO_STOP_GRACE`] has passed; `None` while idle.
    last_play: Option<Instant>,
    /// What the probe found; see `InputState::pad_status`. Logged on change.
    status: &'static str,
    #[cfg(not(target_arch = "wasm32"))]
    pads: Option<NativePads>,
    #[cfg(target_arch = "wasm32")]
    pads: WebPads,
}

impl Haptics {
    /// Open the platform's pad backend. Never fails: a platform without pads
    /// is a game with keys, which every game already is.
    pub fn new() -> Self {
        Self {
            strong: 0.0,
            weak: 0.0,
            ends_at: None,
            last_play: None,
            status: "none",
            #[cfg(not(target_arch = "wasm32"))]
            pads: NativePads::open(),
            #[cfg(target_arch = "wasm32")]
            pads: WebPads::open(),
        }
    }

    /// A `Haptics` with no backend, so the merge rule can be tested without
    /// a pad or a window.
    #[cfg(all(test, not(target_arch = "wasm32")))]
    const fn detached() -> Self {
        Self {
            strong: 0.0,
            weak: 0.0,
            ends_at: None,
            last_play: None,
            status: "none",
            pads: None,
        }
    }

    /// Read the first connected pad. Called once per frame on the main
    /// thread before the game's update, which is the only place gilrs may
    /// be polled from.
    pub fn poll_pad(&mut self) -> Option<PadState> {
        #[cfg(not(target_arch = "wasm32"))]
        let (pad, rumble) = self.pads.as_mut().map_or((None, false), NativePads::poll);
        #[cfg(target_arch = "wasm32")]
        let (pad, rumble) = self.pads.poll();
        if pad.is_some() {
            let status = if rumble { "input+rumble" } else { "input-only" };
            if status != self.status {
                tracing::info!(gamepad = status, "gamepad probe");
                self.status = status;
            }
        }
        pad
    }

    /// `none | input-only | input+rumble`: what the probe found the last
    /// time a pad was seen.
    pub const fn status(&self) -> &'static str {
        self.status
    }

    /// Merge one request into what is playing and drive the motors.
    pub fn request(&mut self, r: Rumble, now: Instant) {
        if r.ms == 0 || (r.strong <= 0.0 && r.weak <= 0.0) {
            return;
        }
        let running = self.ends_at.is_some_and(|t| now < t);
        let new_end = now + std::time::Duration::from_millis(u64::from(r.ms));
        if running {
            self.strong = self.strong.max(r.strong);
            self.weak = self.weak.max(r.weak);
            self.ends_at = Some(self.ends_at.map_or(new_end, |t| t.max(new_end)));
        } else {
            self.strong = r.strong;
            self.weak = r.weak;
            self.ends_at = Some(new_end);
        }
        let remaining_ms = self
            .ends_at
            .map_or(0, |t| t.saturating_duration_since(now).as_millis());
        self.play(remaining_ms);
        self.last_play = Some(now);
    }

    /// Stop the motors once the merged request has run its course, and not
    /// before [`PLAY_TO_STOP_GRACE`] has passed since the last play.
    pub fn tick(&mut self, now: Instant) {
        let Some(end) = self.ends_at else {
            return;
        };
        let earliest = self
            .last_play
            .map_or(end, |p| end.max(p + PLAY_TO_STOP_GRACE));
        if now >= earliest {
            self.stop();
        }
    }

    /// Whether a request is on the motors, or waiting for its stop. Only
    /// the native exit path asks; the web page has no exit of its own.
    #[cfg(not(target_arch = "wasm32"))]
    pub const fn is_playing(&self) -> bool {
        self.ends_at.is_some()
    }

    /// Stop both motors now and forget the request: focus loss, expiry and
    /// exit. This ignores the grace period on purpose: a stop that lands
    /// in the same server tick as its play leaves the motors silent, which
    /// is the right result on every path that calls this directly.
    pub fn stop(&mut self) {
        let was_playing = self.ends_at.is_some();
        self.strong = 0.0;
        self.weak = 0.0;
        self.ends_at = None;
        self.last_play = None;
        if was_playing {
            #[cfg(not(target_arch = "wasm32"))]
            if let Some(p) = self.pads.as_ref() {
                p.stop();
            }
            #[cfg(target_arch = "wasm32")]
            self.pads.stop();
        }
    }

    /// What is on the motors: `(strong, weak, ends_at)`, or `None` while idle.
    #[cfg(all(test, not(target_arch = "wasm32")))]
    fn playing(&self) -> Option<(f32, f32, Instant)> {
        self.ends_at.map(|t| (self.strong, self.weak, t))
    }

    // Only the web path uses the remaining time (the actuator wants a
    // duration) and mutates (the shim is looked up lazily); gilrs needs
    // neither, so the native build sees both as unneeded.
    #[cfg_attr(
        not(target_arch = "wasm32"),
        allow(unused_variables, clippy::needless_pass_by_ref_mut)
    )]
    fn play(&mut self, remaining_ms: u128) {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(p) = self.pads.as_ref() {
            p.play(self.strong, self.weak);
        }
        #[cfg(target_arch = "wasm32")]
        self.pads.play(self.strong, self.weak, remaining_ms);
    }
}

/// gilrs on the main thread: the pad snapshot and two persistent effects.
#[cfg(not(target_arch = "wasm32"))]
struct NativePads {
    gilrs: gilrs::Gilrs,
    /// The pad the two effects were built for. They are rebuilt when the
    /// first connected pad changes, and never per request: a pad has a
    /// small number of effect slots, and building a fresh `ff::Effect` for
    /// every hitmarker exhausts them within a minute.
    ff_pad: Option<gilrs::GamepadId>,
    strong_fx: Option<gilrs::ff::Effect>,
    weak_fx: Option<gilrs::ff::Effect>,
}

/// gilrs's button names in W3C standard-mapping order, so
/// `GILRS_BUTTONS[i]` is the button whose bit is `i`. The pairing with
/// `PadButton` is spelled out rather than implied by position so a test can
/// check both halves of the table against each other.
#[cfg(not(target_arch = "wasm32"))]
const GILRS_BUTTONS: [(gilrs::Button, PadButton); 16] = [
    (gilrs::Button::South, PadButton::South),
    (gilrs::Button::East, PadButton::East),
    (gilrs::Button::West, PadButton::West),
    (gilrs::Button::North, PadButton::North),
    (gilrs::Button::LeftTrigger, PadButton::LB),
    (gilrs::Button::RightTrigger, PadButton::RB),
    (gilrs::Button::LeftTrigger2, PadButton::LT),
    (gilrs::Button::RightTrigger2, PadButton::RT),
    (gilrs::Button::Select, PadButton::Back),
    (gilrs::Button::Start, PadButton::Start),
    (gilrs::Button::LeftThumb, PadButton::L3),
    (gilrs::Button::RightThumb, PadButton::R3),
    (gilrs::Button::DPadUp, PadButton::Up),
    (gilrs::Button::DPadDown, PadButton::Down),
    (gilrs::Button::DPadLeft, PadButton::Left),
    (gilrs::Button::DPadRight, PadButton::Right),
];

#[cfg(not(target_arch = "wasm32"))]
impl NativePads {
    /// Open gilrs, or log why not and go without. `Gilrs::new` fails on a
    /// platform gilrs does not implement; on Windows it is
    /// Windows.Gaming.Input (WGI, the crate's default backend there) and
    /// succeeds with zero pads attached, which is the path this workstation
    /// exercises.
    fn open() -> Option<Self> {
        match gilrs::Gilrs::new() {
            Ok(gilrs) => Some(Self {
                gilrs,
                ff_pad: None,
                strong_fx: None,
                weak_fx: None,
            }),
            Err(e) => {
                tracing::debug!(error = %e, "gamepads unavailable: gilrs failed to open");
                None
            }
        }
    }

    /// Drain gilrs's queue (that is what refreshes its cached state) and
    /// snapshot the first connected pad. Returns the snapshot and whether
    /// the pad can rumble.
    fn poll(&mut self) -> (Option<PadState>, bool) {
        while let Some(ev) = self.gilrs.next_event() {
            match ev.event {
                gilrs::EventType::Connected => {
                    let name = self.gilrs.gamepad(ev.id).name().to_owned();
                    tracing::info!(pad = %name, "gamepad connected");
                }
                gilrs::EventType::Disconnected => tracing::info!("gamepad disconnected"),
                _ => {}
            }
        }
        let Some((id, pad)) = self.gilrs.gamepads().next() else {
            return (None, false);
        };
        let ff = pad.is_ff_supported();
        let state = Self::snapshot(&pad);
        if self.ff_pad != Some(id) {
            self.build_effects(id, ff);
        }
        (Some(state), self.strong_fx.is_some())
    }

    fn snapshot(pad: &gilrs::Gamepad<'_>) -> PadState {
        use gilrs::Axis;
        // gilrs reports stick Y up-positive on every backend, so no negation here.
        let left = PadState::stick([pad.value(Axis::LeftStickX), pad.value(Axis::LeftStickY)]);
        let right = PadState::stick([pad.value(Axis::RightStickX), pad.value(Axis::RightStickY)]);
        let trigger = |b: gilrs::Button| {
            pad.button_data(b)
                .map_or(0.0, gilrs::ev::state::ButtonData::value)
        };
        let mut buttons = 0u16;
        for (g, b) in GILRS_BUTTONS {
            if pad.is_pressed(g) {
                buttons |= b.mask();
            }
        }
        PadState {
            left,
            right,
            lt: trigger(gilrs::Button::LeftTrigger2),
            rt: trigger(gilrs::Button::RightTrigger2),
            buttons,
        }
    }

    /// Build the strong and weak effects for `id`, once. Dropping the old
    /// handles frees their slots on the pad they were built for.
    fn build_effects(&mut self, id: gilrs::GamepadId, ff: bool) {
        self.strong_fx = None;
        self.weak_fx = None;
        self.ff_pad = Some(id);
        if !ff {
            tracing::debug!("gamepad has no force feedback; input only");
            return;
        }
        let build = |gilrs: &mut gilrs::Gilrs, kind: gilrs::ff::BaseEffectType| {
            use gilrs::ff::{BaseEffect, EffectBuilder, Replay, Ticks};
            EffectBuilder::new()
                .add_effect(BaseEffect {
                    kind,
                    scheduling: Replay {
                        play_for: Ticks::from_ms(10_000),
                        ..Replay::default()
                    },
                    ..BaseEffect::default()
                })
                .gamepads(&[id])
                .finish(gilrs)
        };
        let strong = build(
            &mut self.gilrs,
            gilrs::ff::BaseEffectType::Strong {
                magnitude: u16::MAX,
            },
        );
        let weak = build(
            &mut self.gilrs,
            gilrs::ff::BaseEffectType::Weak {
                magnitude: u16::MAX,
            },
        );
        match (strong, weak) {
            (Ok(s), Ok(w)) => {
                self.strong_fx = Some(s);
                self.weak_fx = Some(w);
            }
            (Err(e), _) | (_, Err(e)) => {
                tracing::debug!(error = %e, "gamepad rumble effect failed to build");
            }
        }
    }

    /// Set each channel's gain and (re)start it. `play` restarts the
    /// effect's 10 s window, so a request that arrives mid-rumble does not
    /// inherit the remainder of an older one.
    fn play(&self, strong: f32, weak: f32) {
        for (fx, gain) in [(&self.strong_fx, strong), (&self.weak_fx, weak)] {
            if let Some(fx) = fx
                && let Err(e) = fx.set_gain(gain).and_then(|()| fx.play())
            {
                tracing::debug!(error = %e, "gamepad rumble play failed");
            }
        }
    }

    fn stop(&self) {
        for fx in [&self.strong_fx, &self.weak_fx].into_iter().flatten() {
            if let Err(e) = fx.stop() {
                tracing::debug!(error = %e, "gamepad rumble stop failed");
            }
        }
    }
}

/// The browser's Gamepad API for input and the page's `emberRumble` shim
/// for rumble.
///
/// The shim exists because `GamepadHapticActuator::play_effect` sits behind
/// web-sys's unstable-APIs flag, and the deploy copies only `arena.js` and
/// the wasm to the live page, so a wasm-bindgen `inline_js` snippet would
/// never arrive. A page without the shim, or a browser without the actuator
/// (Firefox, Safari), is a silent no-op: every intent has a key.
#[cfg(target_arch = "wasm32")]
struct WebPads {
    /// `window.emberRumble`, looked up once: on the first frame a pad is
    /// seen, not at startup, because a browser surfaces a pad only after a
    /// button press, by which time every script on the page has run. A
    /// lookup at startup would race the page's own `<script>` order.
    shim: Option<js_sys::Function>,
    /// Whether `shim` has been looked up yet.
    shim_probed: bool,
    /// The shim's last answer: whether it found a pad with a
    /// `vibrationActuator`. `None` until it has been called once. The
    /// status line is derived from this rather than from the shim's mere
    /// presence, because the page carries the shim on every browser and
    /// only Chromium has the actuator behind it.
    actuator: Option<bool>,
    /// A pad with a non-standard mapping is logged once, not every frame.
    warned_mapping: bool,
}

#[cfg(target_arch = "wasm32")]
impl WebPads {
    const fn open() -> Self {
        Self {
            shim: None,
            shim_probed: false,
            actuator: None,
            warned_mapping: false,
        }
    }

    /// The page's shim, looked up on first use and cached, absent or not.
    fn shim(&mut self) -> Option<&js_sys::Function> {
        use wasm_bindgen::JsCast;
        if !self.shim_probed {
            self.shim_probed = true;
            self.shim = web_sys::window()
                .and_then(|w| {
                    js_sys::Reflect::get(&w, &wasm_bindgen::JsValue::from_str("emberRumble")).ok()
                })
                .and_then(|v| v.dyn_into::<js_sys::Function>().ok());
            if self.shim.is_none() {
                tracing::debug!("no window.emberRumble on this page; rumble is a no-op");
            }
        }
        self.shim.as_ref()
    }

    /// The first connected pad with the standard mapping, read by the
    /// standard indices. A pad with another mapping is skipped: its fire
    /// button could be anywhere, and a wrong guess is worse than no pad.
    fn poll(&mut self) -> (Option<PadState>, bool) {
        use wasm_bindgen::JsCast;
        let Some(window) = web_sys::window() else {
            return (None, false);
        };
        // `getGamepads` throws in an insecure context and is absent on old
        // browsers; both read as "no pad".
        let Ok(list) = window.navigator().get_gamepads() else {
            return (None, false);
        };
        for entry in list.iter() {
            let Ok(pad) = entry.dyn_into::<web_sys::Gamepad>() else {
                continue;
            };
            if !pad.connected() {
                continue;
            }
            if pad.mapping() != web_sys::GamepadMappingType::Standard {
                if !self.warned_mapping {
                    self.warned_mapping = true;
                    tracing::debug!(id = %pad.id(), "gamepad without the standard mapping ignored");
                }
                continue;
            }
            let state = Self::snapshot(&pad);
            // The first sight of a pad asks the shim whether an actuator is
            // behind it, with the same silent zero-length call `stop` makes,
            // so the status is right before the first shot rather than after.
            if self.actuator.is_none() {
                self.stop();
            }
            return (Some(state), self.actuator == Some(true));
        }
        (None, false)
    }

    fn snapshot(pad: &web_sys::Gamepad) -> PadState {
        use wasm_bindgen::JsCast;
        let axes = pad.axes();
        let axis = |i: u32| axes.get(i).as_f64().unwrap_or(0.0) as f32;
        // The browser reports stick Y down-positive; the engine's contract is up-positive.
        let left = PadState::stick([axis(0), -axis(1)]);
        let right = PadState::stick([axis(2), -axis(3)]);
        let buttons = pad.buttons();
        let button = |b: PadButton| {
            buttons
                .get(u32::from(b as u8))
                .dyn_into::<web_sys::GamepadButton>()
                .ok()
        };
        let mut bits = 0u16;
        for b in PadButton::ALL {
            if button(b).is_some_and(|gb| gb.pressed()) {
                bits |= b.mask();
            }
        }
        let value = |b: PadButton| button(b).map_or(0.0, |gb| gb.value() as f32);
        PadState {
            left,
            right,
            lt: value(PadButton::LT),
            rt: value(PadButton::RT),
            buttons: bits,
        }
    }

    /// `emberRumble(strong, weak, ms)`. Each call replaces the running
    /// effect on the actuator, which is what the merge wants: the merged
    /// magnitudes for the merged remaining time.
    fn play(&mut self, strong: f32, weak: f32, remaining_ms: u128) {
        self.call(strong, weak, remaining_ms as f64);
    }

    /// Zero magnitudes for zero time: the replacement that ends the running effect.
    fn stop(&mut self) {
        self.call(0.0, 0.0, 0.0);
    }

    /// Call the shim and record its answer. The shim returns `true` only
    /// when it found a pad with a `vibrationActuator`; anything else (an
    /// older shim's `undefined`, a thrown call) leaves the last answer as
    /// it was, so an unknown reads as "input-only" until proven otherwise.
    fn call(&mut self, strong: f32, weak: f32, ms: f64) {
        use wasm_bindgen::JsValue;
        let answer = self
            .shim()
            .and_then(|shim| {
                shim.call3(
                    &JsValue::NULL,
                    &JsValue::from_f64(f64::from(strong)),
                    &JsValue::from_f64(f64::from(weak)),
                    &JsValue::from_f64(ms),
                )
                .ok()
            })
            .and_then(|v| v.as_bool());
        if let Some(found) = answer {
            self.actuator = Some(found);
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use std::time::Duration;

    use super::*;

    const fn r(strong: f32, weak: f32, ms: u16) -> Rumble {
        Rumble { strong, weak, ms }
    }

    #[test]
    fn feedback_merges_per_channel_max() {
        let mut h = Haptics::detached();
        let t0 = Instant::now();
        h.request(r(0.2, 0.8, 40), t0);
        h.request(r(0.9, 0.1, 300), t0);
        let (s, w, end) = h.playing().expect("playing");
        assert_eq!((s, w), (0.9, 0.8));
        assert_eq!(end, t0 + Duration::from_millis(300));

        // A short request arriving mid-rumble neither shortens it nor lowers
        // it, and there is one end, not one per motor: the weak magnitude it
        // raises holds until the merged rumble ends, not for its own 30 ms.
        h.request(r(0.1, 1.0, 30), t0 + Duration::from_millis(100));
        let (s, w, end) = h.playing().expect("playing");
        assert_eq!((s, w), (0.9, 1.0));
        assert_eq!(end, t0 + Duration::from_millis(300));

        // Expiry stops it; a tick just before the end does not.
        h.tick(t0 + Duration::from_millis(299));
        assert!(h.playing().is_some());
        h.tick(t0 + Duration::from_millis(300));
        assert_eq!(h.playing(), None);
        assert!(!h.is_playing());
    }

    #[test]
    fn a_stop_waits_one_server_tick_after_the_last_play() {
        let mut h = Haptics::detached();
        let t0 = Instant::now();
        // A 30 ms hitmarker outlives its own length: the stop is held until
        // gilrs's server has had a tick to start the motors.
        h.request(r(0.5, 0.5, 30), t0);
        h.tick(t0 + Duration::from_millis(30));
        assert!(h.is_playing());
        h.tick(t0 + PLAY_TO_STOP_GRACE.saturating_sub(Duration::from_millis(1)));
        assert!(h.is_playing());
        h.tick(t0 + PLAY_TO_STOP_GRACE);
        assert!(!h.is_playing());

        // A re-play near the end of a long rumble pushes the stop out too,
        // because the re-play is a fresh command in the server's queue.
        h.request(r(0.9, 0.1, 300), t0);
        let t1 = t0 + Duration::from_millis(290);
        h.request(r(0.1, 0.1, 10), t1);
        h.tick(t0 + Duration::from_millis(300));
        assert!(h.is_playing());
        h.tick(t1 + PLAY_TO_STOP_GRACE);
        assert!(!h.is_playing());

        // A direct stop (focus loss, exit) does not wait.
        h.request(r(0.5, 0.5, 300), t0);
        h.stop();
        assert!(!h.is_playing());
    }

    #[test]
    fn an_expired_rumble_does_not_leak_into_the_next() {
        let mut h = Haptics::detached();
        let t0 = Instant::now();
        h.request(r(0.9, 0.1, 300), t0);
        // No tick in between: the request itself must notice the old one is over.
        let t1 = t0 + Duration::from_millis(400);
        h.request(r(0.2, 0.8, 40), t1);
        let (s, w, end) = h.playing().expect("playing");
        assert_eq!((s, w), (0.2, 0.8));
        assert_eq!(end, t1 + Duration::from_millis(40));

        // Silence and zero-length requests are not rumbles and start nothing.
        h.stop();
        h.request(r(0.0, 0.0, 500), t1);
        h.request(r(0.5, 0.5, 0), t1);
        assert_eq!(h.playing(), None);
        assert_eq!(h.status(), "none");
    }

    #[test]
    fn standard_mapping_indices_match_gilrs_names() {
        let expected = [
            "South",
            "East",
            "West",
            "North",
            "LeftTrigger",
            "RightTrigger",
            "LeftTrigger2",
            "RightTrigger2",
            "Select",
            "Start",
            "LeftThumb",
            "RightThumb",
            "DPadUp",
            "DPadDown",
            "DPadLeft",
            "DPadRight",
        ];
        for (i, (g, b)) in GILRS_BUTTONS.iter().enumerate() {
            assert_eq!(format!("{g:?}"), expected[i], "gilrs name at bit {i}");
            assert_eq!(*b as usize, i, "PadButton bit for {g:?}");
            assert_eq!(*b, PadButton::ALL[i]);
        }
    }

    /// Opens the host's backend for real. Ignored by default because it
    /// depends on the machine (WGI on Windows, evdev on Linux); run it
    /// with `--ignored` to see what this host exposes.
    #[test]
    #[ignore = "touches the host's gamepad backend"]
    // The report is the point of the test, and `--nocapture` is how it is read.
    #[allow(clippy::print_stderr)]
    fn gilrs_opens_on_this_host() {
        let mut pads = NativePads::open().expect("gilrs opens");
        let (pad, rumble) = pads.poll();
        let count = pads.gilrs.gamepads().count();
        eprintln!("gilrs: {count} pad(s); first = {pad:?}; rumble = {rumble}");
    }
}
