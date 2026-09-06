# Killshot v30

User request: rename the shooter Killshot v30; show life and ammunition clearly; add health and ammo boxes; animate every weapon reload with a small circular countdown; keep collected weapons on number slots 1–9; offer a selectable starting-weapon mode.

## Contract and scope

Development branch: `codex/killshot-v30`, based on released runtime `129bcac4` plus the v29 operational handoff. Preserve every frozen Arena page and every other game's source, catalog entry and public artifact. No broad host updater or tunnel creation is part of this change. The current public server stays on v29 during development.

Protocol 23 coordinates authoritative weapon inventory, reload remaining time, supplies and the loadout choice. Weapon IDs 1–7 occupy fixed matching slots; slots 8–9 are reserved and visibly empty. Collected weapons and their remaining ammunition persist for the current life, not across deaths or matches. Everyone always retains a safety pistol. Classic starts with the weak pistol; Custom loadout lets the host select the same starting gun for all players, independently of FFA/TDM/Hill. Respawn resets to that lobby loadout.

Health boxes restore two health, capped at five; ammo boxes refill only the equipped finite weapon's reserve to its normal cap, never the magazine. Both are authoritative and respawn after 30 seconds. No benefit means no consumption. Reload progress comes from server countdowns; the client may interpolate briefly between snapshots but cannot grant ammo. Weapon switching uses a consumed input edge and never generates ammunition.

## Coordination and verification

Root owns client integration and serial Idle-priority builds. Independent workers own core/server, web/settings, and pure reload poses. Barza announcement records scope; all useful contracts and findings also belong in this repository. No operating-system input, foreground or physical fullscreen manipulation is allowed. Actual browser tests must run in one disposable headless browser against an owned private server.

Required gates: focused core/client/server regressions, existing native tests, strict Clippy, pure settings/HUD tests, clean WASM/server builds, actual private browser HUD/loadout/slot/reload checks and multiplayer snapshot validation. Record wall times and distinguish simulated input or permission stubs from physical-device verification. Publication requires fresh concurrency/occupancy checks, exact artifact provenance and an Arena-only release preserving old versions and peer games. No new v30 release or restart approval is assumed from the earlier v29 approval.
