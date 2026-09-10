# Killshot v30 scoped publication

These retained tools prepare the historical GitHub Pages release, not a new hosting service. Internal game identity and bundle names remain `arena`; the player-facing title is **Killshot**, release v30, protocol23. They no longer publish remotely because current release assets and Pages deployments are created by the tag workflows.

The inherited v29 guard contract is in `tools/v29/README.md`. Run Node at Idle priority. No script builds Rust, drives computer input, starts/stops services, creates tunnels or changes host-list entries. Do not invoke the all-game updater as an Arena-only action. Preserve the already-running tunnel and peer game processes during any separately approved Windows server swap; Knecht's paused/rate-limited updater is not repaired by this publisher.

## Offline gates

Run `node --test tools/v30/release-book.test.cjs tools/v30/release-scope.test.cjs`. These tests set their own process priority to Idle and inject every mirror/socket/commit lookup. They never access the network. Coverage includes bound mirrors, stale protocol metadata, exact live build identity, ambiguous commits, mirror rotation, peer host fields, frozen v29 catalog history, the single launcher fallback change and the eight-path allowlist. Syntax-check the three operational modules with `node --check` only; running the preparation tool intentionally fetches Git refs, probes public servers and creates a retained temporary worktree.

## Server-first release

Record the full clean source revision and tested `web/pkg/arena_bg.wasm` SHA256 during the build/QA. After that exact source is current main and an approved server deployment answers protocol23 with its exact commit and revision-count version, run `node tools/v30/publish-arena.cjs --build-commit=<full-SHA> --wasm-sha256=<tested-SHA256>` to prepare only. The operator supplies build provenance; the publisher verifies the supplied revision/hash but cannot prove how the compiler produced those bytes. The source checkout must be completely clean, including untracked files.

The fresh Pages base must still have v29 as its only live Arena version and exactly one v29 root launcher fallback. The source catalog must rename the existing Arena entry to Killshot, add one live v30/protocol23 entry, retire v29 and leave all archived version metadata unchanged. Only these eight paths may change: root `index.html` (one v29→v30 fallback), `games.json` (existing Arena object only), `version.json`, `server.json` (only legacy `ws`, `proto`, `v` keys), and v30's `index.html`, `settings.js`, `pkg/arena.js`, `pkg/arena_bg.wasm`. The complete sparse worktree index must initially equal the untouched base tree; peer/frozen trees are rechecked before a push. Every other host field and all `hosts`/`mirrors` entries retain their current writer's values. A discovered mirror is never promoted into `hosts`, which would hide later mirror rotations.

Inspect the retained worktree and `target/killshot-publish/results.json`. The tool stops after fresh source, artifact, server and remote-ref checks; it cannot commit or push the result. The script intentionally leaves worktrees available for review and performs no recursive cleanup. The ignored generated `web/version.json` is the only source-checkout file written during preparation.

## Public proof

After GitHub Pages finishes, run `node tools/v30/public-release.cjs`. It requires the successful publication report, checks its exact Pages parent/tree identity, and takes expected client/settings/build-stamp bytes from that committed release rather than mutable local output. It verifies the tested WASM/settings hashes, public v30 HTML/cache token, sole live Killshot/protocol23 catalog entry, preserved peer catalog/live files and frozen v29 files, scoped book transition, single launcher change and read-only exact-version Welcome. Results go to `target/killshot-public/results.json`. This proves publication bytes and server identity; gameplay, pickup/inventory behavior, reload presentation and multiplayer modes require the separate native, browser and network gates.
