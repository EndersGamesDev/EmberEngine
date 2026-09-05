# ADS rendering checks

`browser-ads.cjs` runs the real Arena WASM and embedded assets in headless Chromium. It drives the actual browser input path with synthetic DOM `PointerEvent` and `KeyboardEvent` objects. It never uses operating-system input, opens a visible window, captures the pointer, reads the operator's gamepad, or connects to a game server. The disposable document suppresses focus and pointer-capture calls made by the client. No production code or API hook is added.

Build Arena WASM and run `wasm-bindgen` separately before capturing. From the checkout root:

```powershell
$env:EMBER_QA_PLAYWRIGHT = 'C:/path/to/node_modules/playwright'
$env:EMBER_QA_OUTPUT = 'C:/path/to/checkout/target/ads-after'
node tools/v24/browser-ads.cjs
```

The default browser is installed Microsoft Edge. `EMBER_QA_BROWSER` selects another Chromium executable, and `EMBER_QA_WEB_ROOT` selects an already-built checkout's `web` directory without copying bundles. The files are served only on an ephemeral loopback HTTP port. `EMBER_QA_WEAPONS='1,3,6'` selects a quick subset.

The default run produces 35 1600×900 captures and five contact sheets: all seven weapons at hip, half the intended raise duration, held ADS, crouched ADS, and after release. A stationary second player stands 25 metres downrange for sight alignment and magnification comparison. The fixture supplies fixed positions and weapon state, but RMB/C are actual local input. Every capture asserts that the real client's outgoing `input` contains the expected ADS/crouch booleans. Protocol-19 `ads_fraction` and `spread` state fixtures advance once per controlled frame through the real message handler, using the documented stationary/no-bloom weapon handling values; static zero state cannot overwrite an aimed checkpoint. This is authored rendering input, not a second implementation claiming to validate the authoritative simulation. Keep the copied timing/cone fixture values aligned with the core table and use the separate real-server network smoke for authority checks.

After startup the fixture freezes `performance.now()` and gates animation frames in its own document. It advances at 60 Hz and stops again before each screenshot. Screenshot latency therefore cannot advance a raising checkpoint into fully aimed. The default intended raising times are 0.14/0.18/0.24/0.22/0.19/0.45/0.30 seconds for IDs 1–7; `EMBER_QA_ADS_SECONDS` can override all seven comma-separated values. Keep these aligned with `arena_core::shooter::weapon_handling`. The exact frames and supplied timings are recorded. A before-run against older WASM does not thereby gain the newer client's ADS timing; compare the actual rendered output.

Each `results.json` records WASM/bindings hashes, GPU, protocol, fixture state, outgoing input, draw progression, virtual time, legacy DOM crosshair visibility, screenshot hashes, errors and warnings. `passed` means the requested frames rendered without browser errors and expected input reached the client. It does **not** certify sight alignment, reticle quality, gameplay authority, shot accuracy, live networking or frame rate. Inspect the contact sheets and full-resolution images, and run native unit/protocol tests separately.

Use different output directories for baseline and final runs. The fixed render steps make input phases repeatable, but asset loading precedes the frozen clock and network arrival is asynchronous: screenshots are visual evidence, not pixel-identical golden tests.

## Real-server handling check

`network-smoke.cjs` uses Node 22+ built-in WebSocket to create a disposable, password-protected one-player lobby on the supplied server. It verifies protocol 19, the starter sidearm, reported ADS/spread/recoil state, gradual aim, crouch precision, real shot events, spread recovery, 2 m/s walking, 1.1 m/s crouching and a preserved jump. It also verifies that a protocol-17 client is refused entry. Only the test's own lobby is used, and sockets close afterward. Run against a separate loopback server before the live upgrade.

```powershell
[System.Diagnostics.Process]::GetCurrentProcess().PriorityClass = 'Idle'
$env:EMBER_QA_OUTPUT = 'C:/path/to/checkout/target/ads-network-check'
node tools/v24/network-smoke.cjs ws://127.0.0.1:17924
```

The JSON report includes the responding host/build, tick-based movement measurements, phase snapshots, shot count and elapsed time. A tunnel can batch away the short partial-ADS transition, so its presence is reported rather than required; fixed-tick core tests verify the duration. This is a small end-to-end handling smoke, not a load test, latency benchmark, or proof of competitive balance. Run builds and browser checks at Idle priority too; neither needs a visible window or operating-system input.

Set `EMBER_QA_NEW_GAME_SMOKE='1'` to check the newly integrated diagnostic/Julibrot work after Arena captures. This requires fresh `what_is_this` and `ember_lab_julibrot` WASM/bindings as well. The smoke loads the diagnostic page, verifies its ninth stage and new scenario/budget exports, then opens the real Julibrot main/worker application and waits at most 90 seconds for its first completed scene. It saves page screenshots, facts and artifact hashes. No benchmark, upload, login, external request or live game-server connection is made. The smoke uses an ordinary clock and closes the Arena page first to avoid concurrent rendering load.
