# Row Semantics · `rec:rows:row-semantics`

**Status:** Decided

## Context · `sec:rows:context`

Every per-owner sectioned policy parameter document must answer two questions before any row can mean anything: which files the row may evaluate and which coordinate system spells those files. Treating the owner's share as both an input boundary and a path origin would conflate them.

## Decision · `sec:rows:decision`

**Decision (A section receives only its owner's governed input)** · `dec:rows:owner-input`

In every per-owner sectioned policy parameter document, an owner's section is evaluated only over that owner's share of the declared repository partition and, for a typed policy, only within the policy's declared domain. The partition and domain determine the section's input before any of its rows are evaluated. A row is never offered another owner's file.

**Decision (Subtraction precedes an exact partition)** · `dec:rows:subtract-then-partition`

Exclude rows collectively subtract their matches from the section's input. Include rows then partition the remainder exactly: the partition is complete, so every remaining file is matched by an include row, and pairwise disjoint, so no remaining file is matched by more than one include row.

Subtraction precedes the partition obligation. An exclude row may therefore overlap an include row without conflict: the operation order is fixed, no row precedence is consulted, and the overlap has left the input before inclusion is judged. A wildcard include is the legitimate degenerate partition with one cell.

**Decision (Every row uses the repository coordinate system)** · `dec:rows:repository-coordinates`

Every path in every row is repository-root-relative. The pre-partitioned owner share and any typed domain restrict what a row is offered; neither changes how a path is spelled. No row is share-relative.

## Consequences · `sec:rows:consequences`

The partition failures follow directly: a remaining file matched by no include row is ungoverned, and one matched by several include rows is multiply-included. A row that reaches no file in its offered input is idle and is reported as such.

Containment is not a third check. Because a row cannot be shown a foreign file, it cannot reach one; a row whose pattern reaches nothing in the input is already accounted for by the idle finding. All policy documents consequently use one repository coordinate system while retaining owner-bounded evaluation.
