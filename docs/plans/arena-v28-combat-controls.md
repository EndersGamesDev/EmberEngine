# Arena v28: health, headshots, shield rules and personal controls

## Scope and ownership

Start from main `7500af5d` in isolated `codex/combat-controls`. Preserve the original checkout's uncommitted Arena/League work and the newer Julibrot work already merged into main. Root owns client integration, removal of health displays, verification and coordinated release. Independent workers own shared combat/server rules, the Rust controls adapter and the v28 browser settings UI. Only root runs Cargo against the shared warm target; builds and headless test processes use Idle priority and never drive the operator's input.

## Requested behavior

Player maximum health becomes 5. Remove health hearts and floating health pips, rather than drawing five replacements. Keep damage feedback and team-colored bodies. Restrict lethal bullet headshots to the actual head footprint and height; shoulders in the old body-wide top cylinder must not count as heads. Keep authoritative lag compensation, stance and cover ordering consistent.

A held shield lasts at most 3 seconds. Lowering or exhausting it starts a 0.35-second firing recovery and a 5-second shield reuse cooldown. Held input must not auto-reactivate after cooldown; require release/repress. Both peers use the same deterministic shield transitions, with authoritative cooldown state broadcast and reconciled for movement/ADS prediction. These values are initial tuning choices for the user's requested short shooting delay and reuse cooldown, not undocumented existing behavior.

The live browser game gets an accessible Settings menu for keyboard/mouse bindings and a 0.1–3× sensitivity multiplier. Save locally in the browser, validate corrupted/duplicate bindings, retain controller defaults, provide reset/resume, and suppress all gameplay input while editing without pausing the multiplayer simulation. Escape/pointer-unlock opens the menu; resuming must not accidentally fire or leave held movement stuck. Native clients retain default controls unless a separate native menu is added later.

## Compatibility and release

This is a semantic protocol change, 20 to 21, not just a page update. Ship a frozen v28 page, update the catalog and existing GitHub Pages publisher without replacing newer lab files. Build/test the server before coordinated restart/publication; check the live player count and obtain fresh restart approval if occupied. Preserve v27 as an archived build (its protocol 20 clients become list-only against the new server). Use Barza for coordination and record exact evidence; do not claim a Fable acknowledgement or external review.

## Gates

Shared combat tests: five-health spawn/respawn, head versus shoulder/edge/body rays in both stances, cover/rewind ordering, shield 3-second cap, early release, fire recovery, cooldown, held-input exhaustion, life resets and no weapon/reload bypass. Client tests: authoritative shield state/replay, actual shield movement/ADS gating, removed health geometry/status, custom bindings and sensitivity, menu neutralization and edge-buffer clearing. Browser tests use a disposable headless document and synthetic DOM events only, never OS keyboard/mouse or focus capture. Check settings persistence/reset/conflicts and actual outgoing neutral inputs while open. Run native suite, strict Clippy, actual GPU regressions, all five published WASM bundles and current Pages fixtures. Inspect main for concurrent changes before normal fast-forward push and verify public artifacts plus live protocol after release.

## Status

### Fullscreen menu follow-up (2026-09-06)

Added a prominent Fullscreen mode card near the top of Settings, available before and during a match, with current state, Enter/Exit, browser-F11 guidance and unavailable/refused feedback. The existing quick button shares the same controller. Fullscreen now targets the document so lobby, HUD and menu remain available; match-only styling expands the canvas, and an open modal is restored above the fullscreen top-layer entry. The button cancels pending rebinding without assigning a mouse button, never resumes gameplay or requests pointer lock, and keeps Resume pinned on mobile. This is a DOM-only UI change: no Rust/WASM/protocol/server restart, no other game's release changes. Publish only `games/arena/v28/index.html` and `settings.js`, using the first 12 SHA256 characters of settings.js as its cache token; preserve deployed server/version/catalog and all other files. Do not run the broad publisher while main has older Fire source.

Verified: 69 pure settings/inline/storage checks and 42 real-page/WASM/private-server browser assertions (7.436 seconds browser; 7.929 seconds combined command), including fullscreen state/refusal/unavailable, pre-match/in-match pointer gesture during rebinding, neutral outgoing input, shared button and mobile visibility. Desktop/mobile screenshots inspected. Independent read-only review found no blocker. Not verified: actual trusted fullscreen/pointer-lock permission or real browser Escape/top-layer transitions, deliberately stubbed to avoid operating the user's screen. PR posting is unavailable without the repository's WSL gh setup; source branch and Barza carry the handoff.

Published UI source `e0c1dab0` to main and scoped Pages commit `0803a462` (workflow `34036180705`, deploy succeeded). Exactly two public paths changed; every other tree entry is identical to parent `708d7265`. Public proof passed in 7.66 seconds: exact v28 HTML with content cache token `ddbb0d62bbea`, settings.js SHA256 `ddbb0d62bbea57e9218b9d379969640faadf044647523ba67af648c26107d6ed` (24,401 bytes), 16 release file hashes, preserved Fire V2/catalog and frozen v27, and unchanged Arena WASM/server protocol21 source `f28a145f` / r1469. The live server reported two players/two lobbies; it was not restarted. No required implementation work remains. If the browser still shows the previous menu, refresh v28 and open Settings.

Completed and publicly verified on 2026-09-06. Arena v28 and its protocol21 server serve source `f28a145f` / r1469. Public URL: https://endersgamesdev.github.io/EmberEngine/games/arena/v28/. Worktree: `C:/Users/end/dev/ember-controls`, branch `codex/combat-controls`, base main `7500af5d`. Checkpoint `0df4f4af` remains available in history.

### Implemented

- Maximum HP 5; no floating health pips or status hearts. Damage/team feedback stays.
- Separate lower-body and unpadded head bullet volumes, earliest actual traveled
  contact wins against cover. No looking ahead into future bullet segments.
- Production SWAT skull/helmet proxy: radius 0.16 m, height 0.24 m; standing
  Y 1.46–1.70 m, crouching Y 1.18–1.42 m. Forward offsets 0.03/0.20 m use the
  rewound facing together with rewound position/stance. These are deterministic
  stance proxies, not full animated skeletal hitboxes. Measured whole neck-mesh
  bounds (including collar) were Y 1.3518–1.6993 / 1.0676–1.4171 at body scale .95.
  The crop deliberately excludes collar. Movement/ceiling/body clearance and eye
  height remain unchanged. Level standing shots now pass OVER a fully crouched
  head; actually aimed crouch-head shots hit. Headshot/lethal melee/direct RPG
  behavior remains lethal; ordinary weapon damage is unchanged with the new HP.
- Shared `ShieldState` / `advance_shield`: 3 s maximum, 0.35 s firing lock and
  5 s reuse after lowering/exhaustion; fresh release/repress required. Nested
  authoritative state in `PState`, protocol 21, client replay and active-only
  movement/ADS/viewmodel gating, and small shield countdown in status.
- Internal `PlayerIn.shield_released` remembers true→false network transitions
  when release/repress packets are drained before a tick. Server consumes it
  once like jump/melee; no new wire field. Coalescing does not shorten recovery.
- Browser settings adapter (`settings.rs`), generic opt-in platform pause gate
  (`__emberInputPaused`) and v28 `settings.js`/page. 13 keyboard/mouse actions,
  sensitivity .1–3×, duplicate/reserved validation, local persistence/reset,
  explicit Resume, paused neutral input and held-input quarantine. Native and
  controller default bindings unchanged. F is available for rebinding; use
  the Fullscreen button/F11. Storage denial uses stable visit-only accounts.
- Catalog, landing fallback and Pages publisher target v28/protocol 21. Publisher
  requires/copies settings.js and stamps its exact cache token after recompute.
  Preserves latest Julibrot lab.js/drive.html assembly and frozen older releases.

### Passed execution gates

- Full native suite across arena/core/server, engine, editor, fire, kings,
  what-is-this and Julibrot: 752 passed, 16 intentionally ignored, 81.19 s.
  This was before the final head crop/queued-release refinement; affected
  packages were rerun afterward as below.
- Final affected native tests: Arena 138, arena-core 200 (+1 ignored), server
  unit 2 and actual WebSocket integration 16, all passed. Includes narrow
  head/cover/TTL/segment-boundary/rewound-facing tests and shield recovery tests.
- Strict Clippy for arena, arena-core, arena-server, ember-engine, fire, kings,
  ember-editor with all targets and `-D warnings`: passed (final run 2.49 s
  compiler time, 2.86 s wall). Only trivial lint/format changes followed tests.
- Actual GPU environment regressions: 11/11 passed, 6.12 s wall.
- `node --experimental-vm-modules tools/v28/settings.test.cjs`: 69/69 passed;
  pure config, syntax, denied/quota storage and startup/account tests only.
- `bash deploy/tests/test-pages.sh`: 71/71 passed, 18.64 s wall, fully shimmed,
  no network/publish. Logs `target/v28-pages-tests.stdout.log` and stderr.log.
  Safe non-reparse TMPDIR was `C:/Users/end/AppData/Local/Temp/ember-v28-tests`.
- All five release WASM bundles and release arena-server built successfully, including wasm-bindgen, in 18.24 s. Arena WASM SHA256: `ba5e71a490d496749c2a7ac09982b44caf102bd76ab8363bb8a883dd95f0e61d`.
- Actual v28 browser/WASM/private protocol21 server: 25/25 interaction checks passed (7.63 s, then 7.27 s after keeping Resume pinned outside the scrolling content). Settings pure tests rerun 69/69. Only synthetic DOM events in an isolated headless Edge session; real user-gesture pointer-lock permissions and physical controllers are not claimed as verified.
- Real WebGL2 Harbor render: 15/15 views, 101 draws/frame, zero GL/runtime errors, 21.92 s. Freight Yard and Trench City both passed real-server rendering smoke in12.28 s; each logged one ~101–102 ms initialization/frame warning. Close standing/crouching body captures passed in5.10 s and visually confirm health symbols absent. Screenshots live under `target/controls-browser`, `controls-harbor`, `controls-old-maps`, `controls-bodies`; these generated artifacts are intentionally outside git.
- `git diff --check` passed. Source pushed to main; server and public publication verified. The numbered list below is the historical recovery/reproduction recipe, not unfinished release work.

### Final public release and peer-work preservation

The durable `ember-environment` checkout was fast-forwarded to tested source `f28a145f`. After a fresh zero-player check, the old server was stopped before rebuilding its Windows-locked executable. Existing scheduled task `ember-arena-host` built in8 s, passed local and public wsbot health checks, finished with LastTaskResult0, and now runs its own durable binary on127.0.0.1:7780. Public address: `wss://python-firefox-marine-atmospheric.trycloudflare.com`, protocol21/r1469/f28a145f, verified after the task finished.

The full publisher initially overwrote a separately published Fire V2 release from unmerged `origin/codex/fire-racer-v2` (source86086a2, protocol2). This was detected from the publication diff and corrected immediately: all seven `games/fire/v2` files plus only the Fire catalog object were restored from `00e097b1`, preserving Arena and the new server book. Final Pages commit `708d726569bb5d41bbb97943e535b35ecb4256ef` restores the exact Fire tree `acad2b3bbc1b7dae49c1fd89a687b8266da4dac8`. The newer Fire source was deliberately not blindly merged into main. A fail-closed publisher guard now refuses to overwrite a higher-protocol independently published Fire release; 74/74 shimmed Pages tests passed in16.27 s. Integrate that source deliberately or use a scoped publisher before the next full-site deployment.

`node tools/v28/public-release.cjs` passed against the actual public site in8.89 s. It verified live Arena v28/protocol21 catalog, page settings cache stamp, version/source/server agreement, all16 checked files (four newly built JS/WASM pairs plus settings; seven byte-identical preserved Fire files), and the frozen v27 WASM hash. Arena is43,224,569 bytes with SHA256 `ba5e71a490d496749c2a7ac09982b44caf102bd76ab8363bb8a883dd95f0e61d`. Public proof is saved at `target/controls-public/results.json`. Final browser checks25/25, including visible Resume on narrow screens, passed in7.45 s. Generated evidence stays outside source control; reproducible verification scripts are committed.

### Exact next steps

1. Read this plan, `CLAUDE.md` and `docs/worker-protocol.md`; verify the checkpoint
   branch is clean. **Do not touch/stash/reset the original `ember` checkout**:
   it contains unfinished League and Arena work by other agents on `lane/arena-v18`.
   `ember-controls/target` is a junction to `ember-grips/target`; only one Cargo
   owner at a time, all build/headless processes Idle. No OS input/focus/capture.
2. Fetch origin/main and check for concurrent changes; merge normally if needed.
   Barza `http://127.0.0.1:8901/api/messages?since=72` was empty at last check;
   seq72 announced v28 ownership. No Fable acknowledgement was received.
3. Build release arena-server for private browser QA. Build all five WASM bundles:

   ```text
   cargo build --release -p arena-server
   cargo build --target wasm32-unknown-unknown --release -p fire -p arena -p kings -p what-is-this -p ember-julibrot-app --lib
   wasm-bindgen --target web --no-typescript --out-dir web/pkg target/wasm32-unknown-unknown/release/fire.wasm
   wasm-bindgen --target web --no-typescript --out-dir web/pkg target/wasm32-unknown-unknown/release/arena.wasm
   wasm-bindgen --target web --no-typescript --out-dir web/pkg target/wasm32-unknown-unknown/release/kings.wasm
   wasm-bindgen --target web --no-typescript --out-dir web/pkg target/wasm32-unknown-unknown/release/what_is_this.wasm
   wasm-bindgen --target web --no-typescript --out-dir web/labs/julibrot/pkg target/wasm32-unknown-unknown/release/ember_lab_julibrot.wasm
   ```

4. Run `tools/v28/browser-controls.cjs` against actual v28 page/WASM/private
   protocol21 server. This new harness is syntax-checked but **UNEXECUTED**; fix
   any failures, do not treat its existence as proof. It owns/refuses occupied
   ports 7788/8088, instruments actual packets, uses only synthetic DOM events,
   stubs OS-facing pointer lock/focus/fullscreen/gamepad and blocks external URLs.
   Checks pre/in-match menu, W→I remap, middle-button fire, 10px mouse delta at
   1×/2×, paused neutral packets/edges, continuing snapshots/render, safe Resume,
   persistence/reset and 390px/1600px layout. Set `EMBER_QA_PLAYWRIGHT` to
   `C:/Users/end/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/playwright`.
   Edge channel is the default. `web/version.json` is generated/ignored;
   `bash deploy/stamp-version.sh` creates it if missing.
5. Run real rendering smoke with `tools/v25/browser-harbor.cjs` (updated to
   protocol21/HP5) and `tools/v22/browser-environment-smoke.cjs` for old maps.
   Optional close body proof: `tools/v23/browser-grips.cjs` with one weapon and
   front/crouch views; its fixture mirrors client protocol (old fixture HP3 is
   cosmetic). Inspect screenshots to verify removed pips and no graphics regression.
6. Record final checks, commit/push branch and normal fast-forward main only
   after passing gates. No force pushes and no unrelated dirty-work inclusion.
7. Re-check live occupancy before restart. Last read-only public Welcome was
   protocol20, host `dusky-osprey`, r1397/00982ed, players0/lobbies0, at
   `wss://deleted-fancy-ink-surgeon.trycloudflare.com`. If occupied, get fresh
   restart approval. User's historical v25 approval is not blanket approval.
8. Durable live checkout is **`C:/Users/end/dev/ember-environment`**, previously
   clean at00982ed; its target is an ordinary directory, NOT the shared junction.
   Existing scheduled task `ember-arena-host` is Ready; wrapper
   `C:/Users/end/.ember/arena-local/ember-arena-host.cmd` runs that checkout's
   `deploy/deploy-arena-local.sh up` with `EMBER_PUBLISH=upstream`. Fast-forward
   that clean checkout to tested main, then use the existing task so its children
   survive tool exit. Do not unnecessarily reinstall/change the task. Read the
   script before running: it builds/stamps first, stops ONLY processes bound
   to127.0.0.1:7780, starts hidden server/tunnel, tests both with wsbot, waits45s
   for DNS, updates arena.url/stamp and merges the host into the address book.
   Do not kill Barza's separate8901 tunnel. Monitor logs with short polls.
9. Publish GitHub Pages using the existing `deploy/deploy-pages.sh` and
   `EMBER_PAGES_PREBUILT=1` after confirming serverprotocol21. Use an explicitly
   created/validated non-reparse temporary directory for its disposable worktree.
   Existing hosting is GitHub Pages; do not migrate this game into Sites.
10. Verify public v28 HTML/settings.js/all five JS+WASM hashes, version/catalog,
    new live Welcome21 and durable task/process status. Frozen v27 Arena WASM must
    retain SHA256 `ef592c9298a96a3ccda8b8f32090ab1b5b925b85df94d649c629ea951a22e71c`
    (43,188,044 bytes). v27/protocol20 becomes archived/list-only, intentionally.
    Post a final Barza result and give the user the live v28 URL only after proof.
