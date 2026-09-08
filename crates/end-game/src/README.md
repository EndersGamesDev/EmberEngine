# End Game client source

`lib.rs` implements EmberGame and the WASM command/state boundary. The game runs fixed 60 Hz steps independently of presentation refresh. UI pause stops simulation and clears pending touch input. `scene.rs` builds material meshes, imports the optimized GLBs and produces Ember frames; it never accesses the GPU. `main.rs` is the native entry point.
