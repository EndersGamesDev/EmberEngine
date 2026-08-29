//! Debug overlay (native only): the ATW test rig from
//! docs/atw-first-rendering.md §6. Composited in the PRESENTER pass — never
//! into the SceneFrame — so the UI stays warp-stable. Provides a scene-Hz
//! throttle (the presenter keeps re-presenting the last SceneFrame while the
//! scene pass idles) and frame-timing / scene-staleness readouts.
//!
//! Toggle with F3. Starts visible when EMBER_OVERLAY=1.

use std::sync::Arc;

use winit::window::Window;

/// Tessellated egui output the renderer composites after the present pass.
pub struct OverlayDraw {
    pub textures_delta: egui::TexturesDelta,
    pub primitives: Vec<egui::ClippedPrimitive>,
    pub screen: egui_wgpu::ScreenDescriptor,
}

pub struct Overlay {
    pub visible: bool,
    /// Scene-pass rate cap in Hz; 0 = uncapped.
    pub scene_hz_cap: f32,
    throttle_on: bool,
    slider_hz: f32,
    ctx: egui::Context,
    state: egui_winit::State,
    fps_smoothed: f32,
}

impl Overlay {
    pub fn new(window: &Arc<Window>) -> Self {
        let ctx = egui::Context::default();
        let state = egui_winit::State::new(
            ctx.clone(),
            egui::ViewportId::ROOT,
            window,
            Some(window.scale_factor() as f32),
            None,
            None,
        );
        Self {
            visible: std::env::var("EMBER_OVERLAY").is_ok_and(|v| v == "1"),
            scene_hz_cap: 0.0,
            throttle_on: false,
            slider_hz: 30.0,
            ctx,
            state,
            fps_smoothed: 60.0,
        }
    }

    /// Feed a window event; true when egui consumed it (visible overlay
    /// interaction — don't also treat it as game input).
    pub fn on_window_event(&mut self, window: &Window, event: &winit::event::WindowEvent) -> bool {
        let response = self.state.on_window_event(window, event);
        self.visible && response.consumed
    }

    /// Build this frame's UI. `scene_age_ms` = how stale the presented
    /// SceneFrame is; the core ATW readout.
    pub fn run(
        &mut self,
        window: &Window,
        frame_dt: f32,
        scene_age_ms: f32,
        size: [u32; 2],
    ) -> OverlayDraw {
        if frame_dt > 0.0 {
            let fps = 1.0 / frame_dt;
            self.fps_smoothed = self.fps_smoothed * 0.95 + fps * 0.05;
        }
        let raw = self.state.take_egui_input(window);
        let mut throttle_on = self.throttle_on;
        let mut slider_hz = self.slider_hz;
        let fps = self.fps_smoothed;
        let output = self.ctx.run(raw, |ctx| {
            egui::Window::new("ATW rig")
                .default_pos([12.0, 12.0])
                .resizable(false)
                .show(ctx, |ui| {
                    ui.checkbox(&mut throttle_on, "throttle scene pass");
                    ui.add_enabled(
                        throttle_on,
                        egui::Slider::new(&mut slider_hz, 5.0..=120.0)
                            .text("scene Hz cap")
                            .integer(),
                    );
                    ui.separator();
                    ui.label(format!("presenter: {fps:5.1} fps"));
                    ui.label(format!("scene-frame age: {scene_age_ms:6.1} ms"));
                    ui.small("age >> frame time = presenter re-presenting (warp rig)");
                });
        });
        self.throttle_on = throttle_on;
        self.slider_hz = slider_hz;
        self.scene_hz_cap = if throttle_on { slider_hz } else { 0.0 };

        self.state
            .handle_platform_output(window, output.platform_output);
        let primitives = self
            .ctx
            .tessellate(output.shapes, output.pixels_per_point);
        OverlayDraw {
            textures_delta: output.textures_delta,
            primitives,
            screen: egui_wgpu::ScreenDescriptor {
                size_in_pixels: size,
                pixels_per_point: output.pixels_per_point,
            },
        }
    }
}
