# Weapon-grip visual checks

`browser-grips.cjs` captures the real Arena WASM client and embedded assets in headless Chromium. It uses fixed, rendering-only protocol fixtures; it does not connect to a game server, authenticate, inject input, or open a visible window.

## Rebuild the assets

Use the already-installed Blender in background at Idle priority. The glove builder needs the original ignored artist sources; the sleeve repair uses the committed SWAT parts and rig. Both write only their new GLB/sidecar outputs and ignored `target/` previews. They preserve the original weapon and character files.

```powershell
[System.Diagnostics.Process]::GetCurrentProcess().PriorityClass = 'Idle'
& 'C:/Program Files/Blender Foundation/Blender 5.2/blender.exe' --background --python tools/v23/build_grips.py -- --source-root C:/Users/end/dev/ember/assets
& 'C:/Program Files/Blender Foundation/Blender 5.2/blender.exe' --background --python tools/v23/build_sleeves.py
```

The glove verifier checks names, texture format, bounds, socket rotations, triangle/download budgets and fingertip-target distances. The sleeve verifier additionally requires closed manifold sleeve shells; the fifth mesh closes the torso's new shoulder cuts while preserving its other original surfaces and separate 1024 atlas. Neither replaces visual inspection. See `docs/weapon-grips.md` for frame conventions, memory cost and limitations.

## Capture the game

Build the Arena WASM client and run the repository's `wasm-bindgen` step first. From the repository root, with Node and Playwright installed:

```powershell
$env:EMBER_QA_PLAYWRIGHT = 'C:/path/to/node_modules/playwright'
$env:EMBER_QA_OUTPUT = 'C:/path/to/checkout/target/grip-before'
node tools/v23/browser-grips.cjs
```

The default browser is installed Microsoft Edge. Set `EMBER_QA_BROWSER` to a Chromium executable if needed. The existing `web/pkg/arena.js` and `web/pkg/arena_bg.wasm` are served only on an ephemeral loopback HTTP port. Use `EMBER_QA_WEB_ROOT` to select another checkout's `web` directory without copying artifacts.

The default run produces 35 full-resolution 1600×900 captures: seven weapons in first person and a second player's front, side, crouched-side, and steep aim-up-side poses. Five contact sheets help triage; inspect contact at full resolution. The observer is positioned beyond the map's decorative facade ring, with the subject 2.15 metres away. The observer's dead fixture state hides its first-person weapon for third-person views. This is a visual test arrangement, not a gameplay or spectator-mode test.

For a quick subset:

```powershell
$env:EMBER_QA_WEAPONS = '1,3,7'
$env:EMBER_QA_VIEWS = 'first-person,side'
node tools/v23/browser-grips.cjs
```

Additional opt-in views are `aim-down` (remote side view at −0.9 radians) and `shield`. Shield views capture only weapon 1, even if other IDs were selected. For example, select `EMBER_QA_WEAPONS='1'` and `EMBER_QA_VIEWS='aim-down,shield'` for two focused captures. Defaults remain the original 35 views. ADS and first-person shield raising depend on local input, not the received player state: they are deliberately not faked through this network fixture. Requesting `first-person-shield` fails explicitly; use the existing native `EMBER_SCRIPT` workflow for that coverage.

For an after run, use the updated WASM and a **different** output directory, for example `target/grip-after`, with identical selection and viewport. Each `results.json` records the WASM and JavaScript hashes, fixture state, WebGL GPU, draw-call progress, console errors, output hashes, and elapsed wall time. `passed` means every requested capture rendered without a browser error; it does **not** mean grip contact is automatically verified. Player state is fixed, but idle animation and environment clocks are not frozen, so compare geometry and contact visually instead of expecting pixel-identical screenshots.

These fixtures do not cover ADS, firing, reload transitions, real networking, or gameplay authority. Run the project's normal tests separately.
