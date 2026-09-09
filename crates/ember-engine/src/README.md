# Ember engine source

`lib.rs` defines `EmberGame` and re-exports the public runtime surface; `app.rs` owns platform events, timing, input, diagnostics, and haptics, while `renderer.rs` owns GPU work and `shader_templates.rs` renders the engine's scene and presentation WGSL from Rust-owned interfaces through `ember-shader`.

`assets.rs` imports GLB meshes and base-colour textures, `environment.rs` creates presentation-only weather particles, and `occlusion.rs` bakes deterministic static directional visibility fields. `feedback.rs` carries game-to-platform effects without involving the renderer.

`rig.rs` supplies jointed characters and arm solving, `puppet.rs` retains the simpler articulated-part path, `input.rs` normalizes keyboard, pointer, and gamepad state, and `overlay.rs` provides the native timing diagnostic composed in the presenter.

The ordinary tests pin data sanitization, input focus and pause behavior, texture mip generation, environment packing, asset and rig geometry, occlusion, and feedback. `renderer_gpu_test.rs` is the explicit ignored gate that renders the shipping pipelines offscreen; its required invocation and capture evidence live in [`../../../docs/environment-weather.md`](../../../docs/environment-weather.md).
