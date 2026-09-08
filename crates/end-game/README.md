# End Game

Version 2.0.0, a single-player dark fantasy dungeon built on Ember. The chapter begins in a prison cell and ends after the player retrieves the greatsword and breaks the far gate's chain. Rules and material physics live in `end-game-core`; this client owns scene construction, input translation, the camera and presentation.

Build the browser game with `powershell -File tools/end-game/build.ps1` from the repository root, then serve `web/` and open `games/end-game/v2/`. Run the native client with `cargo run -p end-game --bin end-game-app`. Builds run at Idle priority through the helper. Native player launches take focus; unattended captures use `EMBER_CAPTURE_PATH` and disable activation and input capture.

The game is a v1 vertical slice. Shapes are assembled from material-based parts with seeded per-face surface coordinates, gravity and friction. The renderer supports scalar roughness/metallicity, local inverse-square torch lights, directional shadows, alpha dust/embers and adaptive browser resolution. This is not a path-traced renderer or a measured photorealistic 5K result. Imported hero meshes have no animation rig; armor evolution swaps forms under a close camera and particles. The supplied film is a prerecorded prologue, including the dragon encounter, and retains the original asset set's continuity limitations.

Input: WASD/mouse, E interact, left click strike, Space jump, Alt dodge, C crouch, Shift sprint, Q form, V camera. Standard-mapped controllers use left/right sticks, Square interact, R2/R1 strike, Cross jump, Circle dodge, L1 crouch, L3 sprint, Triangle form, R3 camera and Options pause. Touch has a left stick, right look area and six action buttons. Gamepad haptics are best-effort browser dual-rumble, not DualSense adaptive-trigger effects.
