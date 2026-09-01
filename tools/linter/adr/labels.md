# Label Graph Verdicts · `rec:labelpolicy:graph-verdicts`

**Status:** Decided

## Context · `sec:labelpolicy:context`

The linter turns authored label syntax into separate policy verdicts rather
than one indivisible pass. That separation is useful only if the verdicts
still describe one graph and run in an order that never guesses around an
earlier defect.

ADR-T-014, A calculus of documentation and source labels, supplies the human
language this package implements. ADR-T-019, The layer owner graph, supplies
the reach distinction, and ADR-T-024, Document-title labels, supplies the
title-head specialization. This record derives the linter's policy boundary
from those inputs without depending on any of their files.

## Decision · `sec:labelpolicy:decision`

**Decision (Occurrence formation precedes graph judgment)** · `dec:labelpolicy:formation-first`

`labels.mints-well-formed` owns complete occurrence syntax, delimiter pairing,
and the distinction between a mint, a local citation, and an import.
`labels.mints-unique` then requires one mint per label identity. A malformed
occurrence contributes no partial identity, and a duplicate is never resolved
by choosing the first mint.

**Decision (Heads carry the environment contract)** · `dec:labelpolicy:heads`

`labels.heads-conform` checks that a participating structural head carries the
mint its environment requires and that prose does not create a head-shaped
substitute. Kind vocabulary comes only from the declared registry under
(`dec:kindregistry:conformance`); head recognition does not maintain another
kind table.

**Decision (Resolution is harvested before it is constrained)** · `dec:labelpolicy:resolution-order`

`labels.citations-local-resolve` resolves a bare citation only within its own
owner. `labels.citations-import-form` validates the explicit imported spelling,
and `labels.citations-imported-resolve` resolves that spelling against the
named owner's unique mint. The linter harvests the complete mint graph before
forming any of these verdicts, so source order cannot decide whether a
citation succeeds.

`labels.citations-layer-conform` is a later judgment over an already resolved
import. It asks whether the declaring owner may reach the minting owner; it
does not turn an impermissible import into an unresolved one or infer reach
from the citation itself.

**Decision (Derived surfaces remain accountable to authored heads)** · `dec:labelpolicy:derived-surfaces`

`labels.generated-regions-conform` checks the citations emitted into generated
regions against the same resolved graph while leaving those regions unable to
mint a competing authored identity. `labels.outlines-conform` checks a declared
tracking relation in both directions: every claimed head exists in the tracked
document and every tracked head is claimed. Displayed tracking cells remain
data rather than participating occurrences.

## Consequences · `sec:labelpolicy:consequences`

Each label policy has one diagnostic responsibility, but none has a private
parser, mint table, or reach graph. Prerequisite failures prevent dependent
verdicts from inventing answers, and independent defects remain distinguishable
in the report and in tolerated-debt identity.
