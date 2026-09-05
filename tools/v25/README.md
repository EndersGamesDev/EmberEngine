# Harbor verification

These scripts do not build the game, change production source, restart a server, or drive operating-system input. Run them at Idle priority. The real-server test and the browser rendering fixtures intentionally make different claims.

## Real eight-player server check

`network-harbor.cjs` uses Node 22+ built-in WebSocket against a separately started protocol-20 server. It creates password-protected disposable Harbor lobbies in FFA, team deathmatch and king of the hill, joins eight peers, checks finite state and distinct non-overlapping spawn positions, requires four players on each TDM team, checks TDM leave/rejoin, and verifies a ninth peer is rejected. All test sockets leave and close afterward. No existing player or lobby is touched.

```powershell
[System.Diagnostics.Process]::GetCurrentProcess().PriorityClass = 'Idle'
$env:EMBER_QA_OUTPUT = 'C:/path/to/checkout/target/harbor-network'
node tools/v25/network-harbor.cjs ws://127.0.0.1:17925
```

In the FFA lobby, the first player follows the authored northern spawn-pocket exit `[-37,42] → [-37,45.4] → [-31,45.4]`, then measures grounded walking, crouching and sprinting south along the clear outer lane. Expected speeds are 4, 2.2 and 6.4 m/s, measured from authoritative positions and server ticks, with 0.16 m/s tolerance for transport batching. It finishes at `[-31,32]`. This proves actual movement outside the old ±24 boundary; it is not a pathfinding implementation or a complete map-connectivity proof. The server alone resolves collisions. ADS/crouch accuracy, settled ADS, real Shot events, recovering spread and protocol-19 join rejection are also checked. Partial ADS is recorded rather than required because a tunnel may batch that brief transition away.

`EMBER_QA_APPROACH`, `EMBER_QA_DIRECTION` and `EMBER_QA_ROUTE` accept JSON arrays to select another already-reviewed clear route; direction must be a unit XZ vector. These are input waypoints, not collision geometry. The default route is a contract with the hand-authored Harbor layout and must be reviewed if that layout changes. `network-results.json` records the actual responding host/build, all three observed rosters, movement phases, route endpoints, errors and elapsed time. Passwords are deliberately omitted. This is a bounded functional smoke, not a load test, latency benchmark, respawn-cycle proof, or competitive-balance claim.

## Real WASM rendering fixture

Build Arena WASM and run `wasm-bindgen` separately. `browser-harbor.cjs` serves existing files from `web` on an ephemeral loopback HTTP port and opens only a headless Chromium page. Playwright intercepts the fixture socket; it never connects to a game server. Seven remote players use the authored spawn coordinates, while the eighth roster entry is an intentional camera observer. At a spawn viewpoint, that spawn's displaced remote player moves to spawn0 so the camera does not clip through another body; the fixture asserts at least two metres of separation. A rendering fixture is not evidence that eight actual clients joined or that all seven remote models are visible from each viewpoint.

```powershell
[System.Diagnostics.Process]::GetCurrentProcess().PriorityClass = 'Idle'
$env:EMBER_QA_PLAYWRIGHT = 'C:/path/to/node_modules/playwright'
$env:EMBER_QA_OUTPUT = 'C:/path/to/checkout/target/harbor-after'
node tools/v25/browser-harbor.cjs
```

The default 15 captures cover two overviews, central stacks, the warehouse, quay/ship, cranes, cargo viewed from the quay, and all eight spawn positions. All images are 1600×900; a contact sheet is saved alongside them. `EMBER_QA_VIEWS='overview,quay-ship,west-warehouse'` selects a quick subset. `EMBER_QA_WEB_ROOT` selects another built checkout and `EMBER_QA_BROWSER` selects a Chromium executable; installed Microsoft Edge is the default. `EMBER_QA_MAX_DRAWS` changes the broad safety bound of 2,000 WebGL draw calls in a checkpoint frame. This bound catches runaway rendering; it is not a performance target.

The disposable page suppresses actual focus and pointer-capture methods and never reads a physical gamepad. Camera rotation is delivered through synthetic DOM focus/pointer events to the headless document's existing winit listeners, with the single synthetic coalesced-motion sample winit expects. No browser automation mouse/keyboard methods, visible window, production debug hook or operating-system input is used. Each camera target is checked against the real client's outgoing aim. A local 60 Hz clock freezes each checkpoint; the observer is then marked dead to omit its first-person weapon while retaining the initialized camera. This is intentional rendering input, not newly implemented spectator gameplay.

`results.json` contains protocol, WASM/bindings and image hashes, exact viewpoint/roster fixtures, outgoing camera aim, WebGL adapter/error status, draw calls in a checkpoint frame, warnings and elapsed time. `harborBuildLogs` retains the runtime's `authored harbor built` mesh/triangle/base-texture-byte log when emitted; an empty array means no such log was observed, not a zero-sized map. `passed` means the rendering and fixture assertions succeeded, not that the map looks correct. Inspect full-resolution images and the contact sheet for actual Harbor landmarks, blocked sightlines, ship/crane silhouettes, cover placement, shadow/lighting defects and visual collision mismatch. Native map tests and the real-server smoke must independently verify collision, routes, loot, hill reachability and spawn allocation. Do not treat synthetic roster snapshots as authority or these captures as a frame-rate benchmark.

## Isolated shadow diagnostic

`EMBER_QA_SHADOW_DIAGNOSTIC='dump'` records generated WebGL shader sources without modifying them. The value `unshadowed` additionally replaces only the generated fragment `shadow_visibility` function body with `return 1.0` inside this disposable browser document and requires that a matching function was actually patched. This optional A/B test distinguishes shadow artifacts from textures; it changes no production shader, WASM or asset. Use a separate output folder such as `target/harbor-no-shadow` and a one-view selection. The report/contact sheet explicitly label the diagnostic, and the original function is preserved in the JSON. Unset this variable for every release capture. A diagnostic pass is never final visual acceptance of the real shadowed build.
