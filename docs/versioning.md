# Versioning

Ember, every game, and every lab use three-grade `MAJOR.MINOR.PATCH` versions. A version identifies shipped source; protocol, asset, loader, and directory-schema numbers remain separate compatibility mechanisms.

## Series and crates

Each independently released game or lab owns a series. Its application, core, server, and other dedicated crates carry the same explicit package version rather than inheriting the workspace version.

| Series | Crates | Initial three-grade version |
|---|---|---|
| `arena` | `arena`, `arena-core`, `arena-server` | `31.0.0` |
| `fire` | `fire`, `fire-core`, `fire-server` | `1.0.0` |
| `kings` | `kings`, `kings-core`, `kings-server` | `1.0.0` |
| `league` | `league`, `league-core`, `league-server` | `2.0.0` |
| `what-is-this` | `what-is-this` | `1.0.0` |
| `julibrot` | `ember-julibrot-app`, `ember-julibrot-kernels`, `ember-julibrot-math`, `ember-julibrot-present`, `ember-julibrot-worker` | `1.0.0` |
| `heap` | `ember-lab-heap` | `0.1.0` |
| `layer` | `ember-lab-layer` | `0.1.0` |

A lab with no historical release tag begins at `0.1.0`; a lab with a historical tag begins at that tag's major in `.0.0` form.

Frozen game crates under `games/` carry the three-grade form of the major represented by their directory, such as `games/arena/v012` at `12.0.0`. They remain buildable history rather than inheriting a current series version.

Every crate that is not dedicated to a game or lab remains an engine or shared crate and inherits the workspace version. That workspace version is the `ember` series. Its starting version is `1.0.0`: an engine already used to ship thirty-one Arena release lines is production software, so a pre-release workspace version would misstate its maturity.

## Grades

The major is the release line a player sees and the launcher retains. Historical directory slots such as `v31` remain stable locators even after the catalog gains a full version. A label may say `Version 31`, meaning the most recent minor and patch in major 31, but a build stamp, tooltip, settings page, release record, or other precise identity shows all three grades.

A minor bump is a deliberate release. It always receives an annotated, signed tag named `<series>-MAJOR.MINOR.PATCH`, where the series is the game or lab id, or `ember` for the engine and workspace. Examples are `arena-31.1.0`, `league-2.1.0`, `julibrot-1.1.0`, and `ember-1.1.0`. New tags never use a bare name or a `v` prefix.

After baseline migration, each merge advances the patch grade of every game, lab, or Ember workspace series it changes. The bump belongs to the merge itself and is not tagged.

The merge establishing these baseline versions is the one-time migration baseline; the per-merge patch rule begins with the next merge to `main`.

The next deliberate release of every series is its first new three-grade minor tag. For example, the next Arena release is `arena-31.1.0`; patch-only work merged before it advances `31.0.1`, `31.0.2`, and so on without tags.

## Release migration

Historical tags are re-issued at the same commits: `vN` becomes `arena-N.0.0`, and `<series>-vN` becomes `<series>-N.0.0`. The replacement is annotated and signed, preserves the old tag message, and names the old tag it replaces. After every requested remote has the replacement, the old local and remote tag is removed. Directory slots and launcher paths retain their historical `vN` names.

The orchestrator owns the one-time migration and runs `bash deploy/retag.sh --apply origin sokol osprey` from a clean `main` checkout after the versioning merge.

The tag migration is complete, and the changelog check accepts only series-prefixed tag names. A series prefix makes every release tag self-identifying in the shared repository.

Release tags are created only from `main`. Branch and merge work prepares versions, catalog data, documentation, and migration tooling but does not create a release tag.

## Display and catalog data

Rust code reads its full version from `env!("CARGO_PKG_VERSION")`. Launcher code reads it from the selected catalog entry's `version` field. Neither source hard-codes the current version in display logic.

Every launcher version entry keeps its historical `v` slot and adds a three-grade `version`. The slot selects the stable directory; the full version identifies the build. User-facing prose derives `Version MAJOR` from the full version and exposes the complete value beside the release note or in another precise version surface.

For pre-policy launcher slots without their own tag, `version` records the latest tagged series baseline and the source stamp remains the exact build identity.
