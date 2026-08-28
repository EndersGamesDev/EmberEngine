use std::sync::Arc;

use web_time::Instant;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
#[cfg(not(target_arch = "wasm32"))]
use winit::keyboard::KeyCode;
use winit::keyboard::PhysicalKey;
use winit::window::{Window, WindowId};

use crate::input::InputState;
use crate::renderer::Renderer;
use crate::EmberGame;

#[cfg(target_arch = "wasm32")]
use std::{cell::RefCell, rc::Rc};

/// Install the native tracing pipeline: `RUST_LOG`-style filtering via
/// EnvFilter, plus a bridge so `log` records from wgpu/winit land in the
/// same output. Idempotent — game code may call it before `run()` to get
/// tracing during its own startup (e.g. connecting), and `run()` calls it
/// again harmlessly.
#[cfg(not(target_arch = "wasm32"))]
pub fn init_diagnostics() {
    use std::io::IsTerminal;
    use tracing_subscriber::EnvFilter;
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new("info,wgpu_core=warn,wgpu_hal=warn,naga=warn")
    });
    let _ = tracing_log::LogTracer::init();
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        // No color codes when output is redirected to a file.
        .with_ansi(std::io::stdout().is_terminal())
        .try_init();
}

/// A frame gap above this is reported as a stall.
const FRAME_STALL_THRESHOLD_MS: u128 = 100;

pub struct EngineConfig {
    pub title: String,
    /// FPS-style mouse capture: clicking the window grabs the cursor
    /// (pointer lock on the web) and mouse-look deltas start flowing.
    pub capture_mouse: bool,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            title: "ember".to_string(),
            capture_mouse: false,
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
    last_frame: Instant,
    /// Rate limit for stall warnings so one bad stretch doesn't spam.
    last_stall_warn: Option<Instant>,
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

        #[cfg(not(target_arch = "wasm32"))]
        {
            self.renderer = Some(pollster::block_on(Renderer::new(window.clone())));
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
            let _ = canvas.focus(); // keyboard events go to the canvas
            // NOTE: no request_inner_size here — winit would pin an inline
            // CSS size that overrides the page's responsive width rule. CSS
            // owns layout; the per-frame sync below owns the backing store.

            let pending = Rc::clone(&self.pending_renderer);
            let win = window.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let renderer = Renderer::new(win.clone()).await;
                *pending.borrow_mut() = Some(renderer);
                win.request_redraw();
            });
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

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Focused(false) => self.input.clear(),
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
                }
            }
            WindowEvent::Resized(size) => {
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.resize(size.width, size.height);
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                if let Some(window) = self.window.as_ref() {
                    let size = window.inner_size();
                    if size.width > 0 && size.height > 0 {
                        self.input.set_cursor_ndc(Some([
                            (position.x as f32 / size.width as f32) * 2.0 - 1.0,
                            1.0 - (position.y as f32 / size.height as f32) * 2.0,
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
                if self.config.capture_mouse && state == ElementState::Pressed {
                    if let Some(window) = self.window.as_ref() {
                        use winit::window::CursorGrabMode;
                        let _ = window
                            .set_cursor_grab(CursorGrabMode::Locked)
                            .or_else(|_| window.set_cursor_grab(CursorGrabMode::Confined));
                        window.set_cursor_visible(false);
                    }
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
                        .map_or(true, |t| now.duration_since(t).as_secs() >= 1);
                    if ok_to_warn {
                        self.last_stall_warn = Some(now);
                        tracing::warn!(
                            stall_ms = raw_gap.as_millis() as u64,
                            "frame stall: gap since previous frame exceeded {FRAME_STALL_THRESHOLD_MS} ms"
                        );
                    }
                }

                let frame = self.game.update(&self.input, dt);
                self.input.end_frame();
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
        last_frame: Instant::now(),
        last_stall_warn: None,
    };

    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut app = app;
        event_loop.run_app(&mut app).expect("event loop error");
    }
    #[cfg(target_arch = "wasm32")]
    {
        use winit::platform::web::EventLoopExtWebSys;
        event_loop.spawn_app(app); // returns immediately; runs on rAF
    }
}
