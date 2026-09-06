# Arena v29: vertical Breakwater Harbor and parkour

## Scope and ownership

Start from main `b97b87bf` in clean worktree `C:/Users/end/dev/ember-parkour`, branch `codex/harbor-parkour-v29`. Preserve original checkout's unrelated Arena/League changes and every independently published peer release. Root owns protocol/server snapshot/client prediction integration, gates and coordinated release. Workers own shared parkour simulation, authored Harbor geometry/art, and browser page/test preparation plus read-only deployment audit. Only root runs Cargo at Idle priority; no process may operate the user's keyboard, cursor, focus, fullscreen or pointer lock.

## Requested behavior

Make Harbor vertical with enterable industrial houses/buildings, accessible roofs, paired walls for repeat wall jumps, alternate ground routes and tactical cover for eight players. Keep the authored 96 m harbor footprint and industrial landmarks. Add sprint-then-crouch ground sliding, jump-out slide momentum, crouch-held wall sliding and fresh-jump wall kicks retaining tangential momentum. Allow chains between opposing walls, not unlimited climbs against one wall or unbounded speed. Existing remappable sprint/crouch/jump controls and gamepad equivalents remain. Walking stays 4 m/s. Preserve shield, weapon accuracy, hit volumes and multiplayer authority.

## Shared simulation and release

One movement routine and serializable parkour state must serve authoritative simulation, client prediction and reconciliation. Jump remains a consumed PRESS so short taps survive the network send window. Obstacle bases, body/ceiling clamps, loot bonks, roof support, boundaries and spawn/round resets must remain valid. New motion and map collision are semantic changes: protocol 22 and frozen web v29, never silently swap v28's map/WASM. Test deterministic state replay, momentum bounds, no collision tunneling, ordinary map routes, held-jump behavior and authoritative snapshot transfer before publishing.

The new CI/ci-passed host timer introduced by a peer must be audited before a main push because main may now trigger automatic restarts. Do not run the broad Pages publisher while main's Fire source is older than the live Fire V2. Use an Arena-only release preserving all other public paths and catalog entries. Check live player counts and obtain current restart approval if anyone will be disconnected. Historical v25 approval does not apply to this release.

## Verification and handoff

Implementation in progress. Not yet compiled or published. Root will run shared/core/client/server tests and strict Clippy, release server and WASM builds, private headless gameplay/network tests with stubbed OS APIs, and visual capture of authored vertical routes. Record wall times, exact tested revisions, public artifact hashes and backend protocol. Do not report trusted user-gesture permission or physical controller validation from stubs. Public release must preserve frozen v28 and other games byte-for-byte.
