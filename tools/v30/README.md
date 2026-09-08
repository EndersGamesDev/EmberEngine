# Killshot v30 release tools

This directory served the Killshot-branded Arena v30 release on protocol 23, covering browser controls and supply rendering, multiplayer killshot behavior, settings, scoped publication, release-book transitions, public-byte proof, and an isolated Windows server swap.

The test modules pin publication and catalog allowlists offline, the browser and network harnesses consume the real v30 WASM and server, and `publish-arena.cjs` prepares only the eight approved Pages paths before `public-release.cjs` verifies their deployed identities; evidence is written beneath ignored `target` directories.

`PUBLISHING.md` is the operational release contract and `WEB-HUD.md` defines telemetry, controls, and presentation ownership; both preserve the internal `arena` identity while naming the player-facing v30 release Killshot.
