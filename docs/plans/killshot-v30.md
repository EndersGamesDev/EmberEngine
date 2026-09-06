# Killshot v30

User request: rename the shooter Killshot v30; show life and ammunition clearly; add health and ammo boxes; animate every weapon reload with a small circular countdown; keep collected weapons on number slots 1–9; offer a selectable starting-weapon mode.

## Contract and scope

Development branch: `codex/killshot-v30`, based on released runtime `129bcac4` plus the v29 operational handoff. Preserve every frozen Arena page and every other game's source, catalog entry and public artifact. No broad host updater or tunnel creation is part of this change. The current public server stays on v29 during development.

Protocol 23 coordinates authoritative weapon inventory, reload remaining time, supplies and the loadout choice. Weapon IDs 1–7 occupy fixed matching slots; slots 8–9 are reserved and visibly empty. Collected weapons and their remaining ammunition persist for the current life, not across deaths or matches. Everyone always retains a safety pistol. Classic starts with the weak pistol; Custom loadout lets the host select the same starting gun for all players, independently of FFA/TDM/Hill. Respawn resets to that lobby loadout.

Health boxes restore two health, capped at five; ammo boxes refill only the equipped finite weapon's reserve to its normal cap, never the magazine. Both are authoritative and respawn after 30 seconds. No benefit means no consumption. Reload progress comes from server countdowns; the client may interpolate briefly between snapshots but cannot grant ammo. Weapon switching uses a consumed input edge and never generates ammunition.

## Coordination and verification

Root owns client integration and serial Idle-priority builds. Independent workers own core/server, web/settings, and pure reload poses. Barza announcement records scope; all useful contracts and findings also belong in this repository. No operating-system input, foreground or physical fullscreen manipulation is allowed. Actual browser tests must run in one disposable headless browser against an owned private server.

Required gates: focused core/client/server regressions, existing native tests, strict Clippy, pure settings/HUD tests, clean WASM/server builds, actual private browser HUD/loadout/slot/reload checks and multiplayer snapshot validation. Record wall times and distinguish simulated input or permission stubs from physical-device verification. Publication requires fresh concurrency/occupancy checks, exact artifact provenance and an Arena-only release preserving old versions and peer games.

## Verified candidate and explicit release approval

The user explicitly answered "Publish and restart Arena after checks" to the new v30 approval question. Only Windows Arena may restart; preserve its existing cloudflared PID4072 and public domain, plus every other game. Latest read-only Windows Welcome still reports v29/protocol22/r1490/129bcac4; no live process or artifact has changed yet. Knecht timer was independently confirmed inactive; Barza coordination message91 requests that it remain paused through this Windows-only release. Peer main031f572b's host/tunnel-preservation repair merged cleanly as158d6ccd, with no lost edits.

Native final gate: 161 client +232 core +22 server tests passed, one intentionally ignored core benchmark, in19.094s. Strict all-target Clippy passed in3.181s. The counterfactual resource-rule test restores the exact six v29 fingerprint checkpoints while every raw movement/parkour/shield state matches v30: the new resource rules caused the expected fingerprint change, not a movement regression. Owned-gun switching and duplicate pickups preserve ammunition and trigger recovery. Reload starts its new weapon's ADS from zero. Remote reload IK projects both wrists into feasible reach; shield-hand sockets remain fixed.

Actual private browser gate passed100 checks in35.047s: existing settings/fullscreen-state checks, LIFE/ammo/9-slot HUD at390/1600px, Classic/custom starts, finite/infinite reserves, all seven weapon reloads and screenshots, empty-slot rejection, cancellation conservation, pause safety, and116 matching snapshot ticks seen by a second player. Its first run exposed a fixture variable shadowing the frames helper after52 passing checks; that fixture-only error was corrected and the entire gate rerun. Pointer/fullscreen/focus APIs are deliberately stubbed; no trusted permission or physical-device claim is made.

Eight-peer private network gate passed FFA/TDM/Hill,4v4/rejoin, previous-protocol22/ninth-player rejection, movement/accuracy and uninterrupted opposing-wall jumps observed by all peers:23.206s,5287states,zero errors (24.600s including owned server lifecycle). Four additional real WebGL2 Harbor supply captures passed in7.735s,101draws/frame and zero errors; availability in that fixture is synthetic, so those pictures prove rendering only. Root inspected the supply sheet and reload/HUD captures. Reports are ignored/regenerable under target/killshot-browser, target/killshot-network and target/killshot-supplies.

Tested candidate WASM SHA256 is0b747e770f436a9aafdc6f751c1e6e43d321ca6825b01aeab79eeda31bdefd4e, built asr1498/158d6ccd in13.657s; later changes are QA, menu wording and deployment-path alignment, not gameplay runtime. A fresh clean final stamp/hash comparison is required before publication. Pure JS settings/HUD checks129 and publisher checks10 passed separately. Linux CI34046321216 passed cores/servers but correctly failed Pages assembly because deploy-pages.sh still targetedv29 against the newv30catalog. Its one live-slot constant is now aligned tov30; no guard was weakened. Await rerun before main promotion or any server restart.

Dry weapons stay selected and can be resupplied; press1 for the retained sidearm. No cross-life/cross-match inventory persistence, invented eighth/ninth weapon, or new external asset download is included. Seven authored reload motions use existing assets; fused weapon meshes do not gain fabricated detachable magazines.
