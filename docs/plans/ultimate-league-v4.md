# UltimateLegue V4 — crystalforge visual feedback and interface parity

The owner approved a focused V4 pass for UltimateLegue to improve map readability and feedback clarity without changing core rules. The scope is additive presentation and UI polish only, with protocol compatibility retained.

## Owner and integration model

This release is coordinated from the Codex seat (`codex/league-environment-feedback`) using V4 scene and client work prepared on shared branches. Fable owns environmental art and placement details, OpenCode owns read-only review of `web/games/league/v4` feedback behavior, and the release guard stays in Codex-owned `league` root wiring. The V4 work is designed to be a single branchable change with no cross-game process impact.

## V4 scope

- `scene.rs` changes from `e8631c96` rebuild the Crystalforge objective and lane silhouette: updated court rings, core silhouettes, objective geometry and non-trivial lane floor detail.
- `scene/art.rs` and `tools/league/bake_environment.py` changes from `39390fa5` replace the V4 ground, lane and court textures.
- `league-core` and `league-client` feedback contracts from `49a33c17` add session-scoped, monotonic feedback events with optional normalized screen coordinates, source/unit IDs and a compact `impact` envelope.
- UI iteration `70d4263f` updates the V4 overlay contract: target readout, ready/impact cues, unavailable action toast, reduced-motion fallback and gesture-gated sound cues.
- `a3e5e6ea` routes the launcher and landing path to V4 runtime files.
- `18f707eb` keeps the existing release guard in place so scoped V4 publishes do not overwrite unrelated game trees.

## Compatibility and constraints

No gameplay protocol bump is part of V4. Core movement, damage, cooldowns, wave timing and economy rules remain unchanged.

No new server features or network fields are introduced in this cycle. No host process or tunnel changes are part of this branch; this is a scene/UI pass in the existing V4-compatible runtime.

## Deliberately excluded from V4

- No combat formula, XP, or economy changes.
- No champion ability tuning or mechanics changes.
- No capture-time input automation.
- No launch story, trailer, or landing narrative changes.
- No protocol migration.

## Planned release record

Landing route and scope are set by this cycle, and the plan is to publish once V4 visual passes are green and browser captures confirm map readability and UI usability on desktop and 390px widths.

This plan documents the design and integration target; the final proof matrix and published hash list are recorded at release time.

## Release checklist intent

- Preserve V3 and keep all non-League pages intact.
- Run V4 smoke/builder checks in the League tooling with `--version=v4 --require-ui`.
- Record commit IDs, branch tip, and wall-time around publication.
