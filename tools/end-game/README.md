# End Game tooling

`build.ps1` runs the game-specific checks and produces the Ember WASM bundle and web bindings. Optional `-TargetDir` selects an existing Cargo cache. `publish.py` assembles only End Game's version-local files and updates its entry in the live catalog, preserving every other game, host book and landing-page change. It defaults to staging; `--publish` sends a normal fast-forward push to GitHub Pages. The staging directory is retained for inspection. `publish_test.py` checks scoped assembly offline.

`quality.test.mjs` verifies pixel caps and overload behavior. `generate_surfaces.py` creates the three deterministic material maps using numpy and Pillow; pass an output directory as the first argument. Runtime mesh provenance is recorded alongside the assets.

Unattended native capture: set `EMBER_CAPTURE_PATH` to an output PNG and optionally `END_GAME_SCENE` to `cell`, `corridor`, `sword` or `wolf`, then run `end-game-app`. This captures only Ember's scene texture, never the desktop, and the game disables focus activation, input capture and simulation. Native capture exits after one rendered frame. The engine's GPU regression tests can run without any window: `cargo test -p ember-engine environment_gpu -- --ignored`.

To reproduce meshes, set END_GAME_ASSET_WORK to an empty scratch directory; run prepare_textures.py with the original End Game asset root as its argument, then run repair_assets.py in Blender background mode with four threads at Idle priority, and verify_repaired.py in Python with numpy, Pillow and scipy. The scripts reconstruct geometry, generate UVs, rebake diffuse color, pad atlas gutters, and validate texture format and mesh connectivity. Source assets are read only.
