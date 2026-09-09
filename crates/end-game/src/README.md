# End Game client source

`lib.rs` implements EmberGame and the WASM command/state boundary. The game runs fixed 60 Hz steps independently of presentation refresh. UI pause stops simulation and clears pending touch input. `cell.rs` authors the v2 architectural kit and cached environmental animation meshes. `scene.rs` imports the optimized GLBs and produces Ember frames; neither accesses the GPU. `main.rs` is the native entry point.
