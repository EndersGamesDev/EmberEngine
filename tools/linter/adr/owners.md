# Owner Reconciliation · `rec:owners:reconciliation-verdicts`

**Status:** Decided

## Context · `sec:owners:context`

Imported citations and owner-scoped policies need a trustworthy relation among
corpus owners, workspace members, and direct reach. Compiling the current
workspace into the package would make that relation stale as soon as a member
or dependency changed.

ADR-L-019, The layer owner graph, supplies the ownership and reach doctrine.
ADR-L-015, The test label profile, supplies the mechanical crate-name
derivation used by participating package owners. This record re-derives the
linter's reconciliation policies over declared and discovered data.

## Decision · `sec:owners:decision`

**Decision (Reach is declared and independently derived)** · `dec:owners:reach-reconciliation`

`owners.reach-conform` compares the declared owner relation with direct
workspace dependency edges. Self-reach is structural and is not declared;
reach is one hop and never gains edges through transitive closure. Every built
participating member must appear, every declared member must be discovered or
explicitly declared as unbuilt, and every direct edge must agree in both
directions.

The policy forms the graph before citation-layer verdicts consume it. An
import cannot create an edge, and a missing edge cannot be excused because a
particular citation happened not to exercise it.

**Decision (Crate names reconcile in both directions)** · `dec:owners:crate-name-reconciliation`

`owners.crate-names-conform` derives an expected owner spelling from each
participating manifest's package name using the namespace supplied by the
declaration. It then compares that derived roster with the declared owners.
Manifestless members arrive only as declared crate-name and directory data;
the binary knows no pending sibling by name or path.

A discovered package without an owner and an owner claiming no package are
both findings. The directory remains repository-root-relative data throughout;
it does not participate in the spelling derivation.

## Consequences · `sec:owners:consequences`

Workspace evolution changes declarations and manifests rather than compiled
tables. Label-layer reach receives a reconciled graph, policy selection receives
a reconciled roster, and intentionally unbuilt members remain explicit without
making their current identities package knowledge.
