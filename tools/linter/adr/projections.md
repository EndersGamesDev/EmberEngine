# Generated Inventory Projections · `rec:projections:inventory-views`

**Status:** Decided

## Context · `sec:projections:context`

Authored inventories are easiest to review where the source is maintained,
but summaries assembled by hand inevitably drift from that source. The linter
therefore owns generated views whose authority is always the current accepted
profile census.

ADR-L-012, An adjudication procedure for identities, digests, and evidence,
establishes exact regeneration as the relevant evidence. ADR-L-017, The test
documentation policy, and ADR-L-018, The constant label profile, establish the
particular views. This record derives the package policy without reaching any
outside document at runtime.

## Decision · `sec:projections:decision`

**Decision (A projection begins with a conforming source census)** · `dec:projections:profile-first`

`projection.test-indexes-current` and `projection.test-matrices-current`
consume the test-profile census. `projection.constant-pins-current` consumes
the constant-profile census. Each program depends on its source profile so a
malformed inventory cannot be rendered into an apparently authoritative view.

**Decision (Freshness is exact expected output)** · `dec:projections:exact-output`

The generator has one deterministic recipe for ordering, layout, and terminal
line endings. Verification regenerates the expected bytes and compares them
with the standing projection; writing stores those same expected bytes. A
digest, timestamp, or remembered count cannot substitute for that comparison.

**Decision (Generated views do not become a second inventory)** · `dec:projections:no-second-authority`

An in-file index presents tests beside their source, a folder matrix presents
the tests below that folder, and a constant-pin view presents the identities
and derived values already accepted by the profile. None may introduce an
authored item, identity, or ordering rule that the source census does not
supply. Participating citations in generated text remain checked, but the
projection cannot mint a second copy of the authored inventory.

## Consequences · `sec:projections:consequences`

Reviewers can regenerate every view and attribute a difference to current
source or to the fixed recipe. Missing and stale projections are findings of
their own programs, while profile defects remain findings at the authored
source where they can be repaired.
