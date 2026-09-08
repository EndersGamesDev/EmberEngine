# Arena v28 verification

These scripts served Arena v28's protocol-21 combat-controls release by checking the actual page and WASM controls, the pure settings and startup contract, and the deployed public bytes while preserving v27.

`browser-controls.cjs` confines synthetic DOM events to an owned headless page and loopback server, `settings.test.cjs` pins the action table and persistence without a browser or GPU, and `public-release.cjs` compares the published catalog, settings, packages, and frozen predecessor to locally tested artifacts.

The control semantics, isolation limits, evidence paths, and release results are recorded in `docs/plans/arena-v28-combat-controls.md`; generated proof remains outside source control.
