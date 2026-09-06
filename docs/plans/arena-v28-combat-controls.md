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

Plan recorded. Implementation and all execution/release gates pending.
