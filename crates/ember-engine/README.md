# Ember engine

`ember-engine` is the shared native-and-wasm runtime beneath every shipped game: it owns the window and browser event loop, input and haptics, asset decoding, scene submission, and all wgpu rendering.

Games implement `EmberGame`, return a `Frame` plus optional feedback, and start through `run`; they consume engine-owned camera, mesh, texture, environment, particle, rig, and input types without touching GPU APIs.

The crate targets the WebGL2 feature floor on wasm and the corresponding native wgpu path, keeping one renderer contract across release bundles.

The strict layering and renderer limits are listed in [`../../CLAUDE.md`](../../CLAUDE.md); device requirements, assets, and outdoor rendering are specified in [`../../docs/minimum-requirements.md`](../../docs/minimum-requirements.md), [`../../docs/asset-pipeline.md`](../../docs/asset-pipeline.md), and [`../../docs/environment-weather.md`](../../docs/environment-weather.md).
