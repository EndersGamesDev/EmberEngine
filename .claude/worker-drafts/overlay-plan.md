# Implementation Plan: ATW Debug Overlay (Section 6)

**Objective:** Implement the egui debug overlay for the ember engine as defined in Section 6 of the ATW-first rendering document. This involves pinning dependencies for the wgpu 24/25 API era, creating a presenter-side UI composition pipeline, and implementing a scene-Hz throttle mechanism with latency readout.

---

## 1. Dependency Management (`Cargo.toml`)

We must pin `egui` to a version compatible with the `wgpu` API surface used in the ember renderer. For wgpu 24/25 era APIs (using `RenderPassDescriptor` and `ShaderModule` APIs typical of the 0.20+ era), `egui` 0.27 is the stable baseline.

**File:** `C:\Users\end\dev\ember\Cargo.toml`

```toml
[dependencies]
# ... existing dependencies ...
# Pin egui to 0.27 for stable wgpu 0.20+ integration
egui = "0.27"
egui-wgpu = "0.27" # Provides the render pass integration for wgpu
# Ensure wgpu is pinned to a version that supports the ATW pipeline requirements
wgpu = "0.20" 
```

---

## 2. UI State Management (`presenter/ui_state.rs`)

Create a central state struct to hold the throttle configuration and timing statistics. This struct will be managed by the presenter and passed to the scene renderer to enforce limits.

**File:** `C:\Users\end\dev\ember\src\presenter\ui_state.rs`

```rust
use std::sync::Arc;
use std::time::{Duration, Instant};
// Use atomic for lock-free reads if possible, or Arc<Mutex> for simplicity in MVP
use std::sync::Mutex;

#[derive(Clone)]
pub struct DebugOverlayState {
    // Throttle mechanics
    pub target_scene_hz: Mutex<f32>,
    pub is_throttled: Mutex<bool>,
    
    // Timing data flow
    pub last_frame_stats: Mutex<FrameStats>,
}

#[derive(Debug, Default)]
pub struct FrameStats {
    pub scene_submit_time: Option<Instant>,
    pub warp_present_time: Option<Instant>,
    pub scene_latency_ms: f64,
    pub warp_latency_ms: f64,
}

impl DebugOverlayState {
    pub fn new() -> Self {
        Self {
            target_scene_hz: Mutex::new(60.0),
            is_throttled: Mutex::new(false),
            last_frame_stats: Mutex::new(FrameStats::default()),
        }
    }
}
```

---

## 3. Scene Renderer Throttle Logic (`renderer/scene_renderer.rs`)

Modify the `SceneRenderer` loop to respect the `DebugOverlayState`. The renderer must sleep if the throttle is active, limiting the production rate of `SceneFrame`s.

**File:** `C:\Users\end\dev\ember\src\renderer\scene_renderer.rs`

```rust
use super::ui_state::DebugOverlayState;
use std::time::{Instant, Duration};

// ... existing imports ...

impl SceneRenderer {
    pub fn run(&mut self, state: &Arc<DebugOverlayState>) {
        let mut last_tick = Instant::now();
        
        loop {
            // 1. Read throttle state
            let target_hz = *state.target_scene_hz.lock().unwrap();
            let should_throttle = *state.is_throttled.lock().unwrap();
            
            // 2. Throttle logic (Duty cycle limiter)
            if should_throttle {
                // Calculate sleep time based on target Hz
                let interval = Duration::from_secs_f32(1.0 / target_hz);
                let now = Instant::now();
                
                if now < last_tick + interval {
                    // Sleep until the next allowed tick time
                    std::thread::sleep(last_tick + interval - now);
                }
                last_tick = Instant::now();
            } else {
                // Normal operation: run as fast as possible (or capped by budget)
                last_tick = Instant::now();
            }

            // 3. Render Scene
            // ... (Standard render loop: setup pass, draw scene, output SceneFrame) ...
            
            // Record stats for the presenter
            state.last_frame_stats.lock().unwrap().scene_submit_time = Some(Instant::now());
        }
    }
}
```

---

## 4. Presenter-Side Composition & Timing (`presenter/mod.rs`)

The presenter owns the surface. It reads `SceneFrame`s from the ring buffer, runs the warp pass, and composites the UI (egui) *after* the warp pass but *before* the swapchain flush.

**File:** `C:\Users\end\dev\ember\src\presenter\mod.rs`

```rust
use super::ui_state::{DebugOverlayState, FrameStats};
use crate::renderer::scene_frame::SceneFrame;
// ... other imports ...

pub struct Presenter {
    state: Arc<DebugOverlayState>,
    // ... egui context, wgpu device, queue ...
    egui_ctx: egui::Context,
}

impl Presenter {
    pub fn new(state: Arc<DebugOverlayState>) -> Self { /* ... */ }
    
    pub fn run(&mut self, mut event_loop: winit::event_loop::EventLoop<()>) {
        let mut last_present = Instant::now();

        loop {
            // 1. Wait for rAF (Display Clock)
            event_loop.run_rest_of_frame(|_| {});

            // 2. Acquire Newest SceneFrame
            let scene_frame = self.renderer_ring.get_newest_complete(); // Assume ring buffer exists

            // 3. Timing Data Flow
            let now = Instant::now();
            let stats = self.state.last_frame_stats.lock().unwrap();
            
            // Update HUD data
            if let Some(submit_time) = stats.scene_submit_time {
                stats.scene_latency_ms = (now - submit_time).as_secs_f64() * 1000.0;
            }
            // Warp latency is effectively zero (CPU side), but we track total frame time
            stats.warp_latency_ms = (now - last_present).as_secs_f64() * 1000.0;
            last_present = now;

            // 4. Warp Pass (ATW Stage B/C)
            // Render scene_frame to an intermediate texture using the pose delta
            self.warp_pass.render(&scene_frame);

            // 5. egui Debug Overlay Composition (Presenter-side only)
            // NOTE: We do NOT draw this into the SceneFrame. We draw it to the canvas now.
            self.draw_egui_overlay();

            // 6. Swapchain Present
            self.queue.present();
        }
    }

    fn draw_egui_overlay(&mut self) {
        // A. Prepare egui texture (if dirty)
        self.egui_ctx.begin_frame();
        
        // B. Build UI
        let stats = self.state.last_frame_stats.lock().unwrap();
        let target_hz = *self.state.target_scene_hz.lock().unwrap();

        egui::CentralPanel::default().show(&self.egui_ctx, |ui| {
            ui.heading("ATW Debug Overlay");
            
            // Slider Mechanic
            ui.label("Target Scene Hz");
            if ui.dragged_value(&mut target_hz).changed() {
                *self.state.target_scene_hz.lock().unwrap() = target_hz;
            }
            ui.label(format!("Current: {:.1} Hz", target_hz));

            // Readout Data Flow
            ui.separator();
            ui.label("Frame Timing");
            ui.colored_label(egui::Color32::LIGHT_GREEN, 
                format!("Scene Latency: {:.2} ms", stats.scene_latency_ms)
            );
            ui.colored_label(egui::Color32::LIGHT_BLUE, 
                format!("W
