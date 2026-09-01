# Inventory Profiles · `rec:profiles:inventory-verdicts`

**Status:** Decided

## Context · `sec:profiles:context`

Several policy programs ask the same structural question of different authored
assets: whether an item that a corpus has chosen to inventory carries the
label required at that item's standard place. Treating those programs as one
text search would lose the distinct carrier, derivation, and staging rules
that make each inventory meaningful.

The source doctrines are ADR-T-015, The test label profile; ADR-T-016, The TODO
label profile; ADR-T-017, The test documentation policy; ADR-T-018, The
constant label profile; and ADR-T-020, The migration disciplines. This record
re-derives the common linter contract and the distinctions its policy programs
must preserve.

## Decision · `sec:profiles:decision`

**Decision (Each profile owns one census and one standard place)** · `dec:profiles:one-census`

`profile.tests-conform` reads test functions, `profile.todos-conform` reads
deficiency notices, `profile.claims-conform` reads authored test claims,
`profile.constants-conform` reads production constants, and
`profile.legacy-conform` reads marked legacy implementation sites. Each
program owns the recognizer that defines its covered item and the exact place
where the corresponding mint or citation may stand.

One shared comment and prose reader may serve several profiles, but a covered
item belongs to a profile only through that profile's own recognizer. A marker
inside a string, a test outside the selected test carrier, or a constant
outside production input cannot enter by resemblance.

**Decision (Derivation is mechanical and authorship stays visible)** · `dec:profiles:derived-identity`

Tests, notices, claims, and legacy markers derive the portion of identity their
profile defines from stable authored structure or words. Constant identities
also carry an authored program choice because the value's interpretation
cannot be recovered from syntax alone. A profile never accepts a separately
authored slug where its derivation can be computed, and never pretends to
derive a semantic choice only the author can make.

**Decision (Activation states an enforceable inventory)** · `dec:profiles:activation`

An owner activates each profile independently. Presence says the profile's
inventory is enforceable for that owner's resolved source selection; absence
means non-applicability, not an empty census. During staged adoption, labelled
items are checked immediately and the unlabelled remainder is counted by the
matching migration policy until it reaches zero.

The claim profile additionally depends on the test profile because a claim is
accountable to the test it documents. That dependency does not merge their
inventories or their findings.

## Consequences · `sec:profiles:consequences`

The package has one reusable profile shape without erasing why its programs
are separate. New inventory kinds require an explicit recognizer, derivation
or authorship rule, standard place, activation, and staging disposition rather
than admission through a generic marker scan.
