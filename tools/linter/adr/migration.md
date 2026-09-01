# Migration Family Verdicts · `rec:migrationpolicy:family-verdicts`

**Status:** Decided

## Context · `sec:migrationpolicy:context`

Removing a superseded notation or an unlabelled inventory remainder takes more
than a search result. The linter needs a stable definition of each family, a
finite source selection, and a debt identity that can fall without permitting
an occurrence to move elsewhere unnoticed.

ADR-L-020, The migration disciplines, is the human doctrine from which these
programs descend. This record re-derives the linter's common policy semantics;
the argument for the division-name family remains in this package's Division
names record.

## Decision · `sec:migrationpolicy:decision`

**Decision (A family is bounded before it is counted)** · `dec:migrationpolicy:bounded-family`

The section-reference, record-reference, unprefixed-record, tag, scenario, and
residual programs each own a recognizer with a closed token boundary. The
repository-wide variants of section and record recognition differ only in
their declared source selection, not in what the tokens mean. No program
accepts an open-ended pattern whose future matches could widen the family
without a policy change.

`references.mark-numbered-absent`, `references.literal-set-absent`, and
`references.prefix-numbers-absent` separate code-owned recognition mechanics
from declared family data. Marks, literal members, prefixes, numeric bounds,
source regions, and exclusions arrive through the resolved policy declaration;
the binary does not carry a current corpus's values.

**Decision (One recognizer forms census and verdict)** · `dec:migrationpolicy:one-reading`

The same reader supplies findings, tolerated-debt comparison, reporting, and
writer input for one program. Displayed examples and excluded regions are
handled before the occurrence is counted. Overlapping programs have explicit
precedence or a configuration refusal, so one token cannot satisfy two debts
by accident.

The policy identifiers covered by this reading are
`legacy.section-references-repository`,
`legacy.record-references-repository`, `legacy.section-references`,
`legacy.record-references`, `legacy.unprefixed-record-references`,
`legacy.tag-references`, `legacy.scenario-numbers`, and
`legacy.residual-litter`. `legacy.division-names` uses the same execution
discipline with the family rationale recorded separately.

The staged identifiers `references.prefix-numbers-absent` and `legacy.residual-litter` are unactivated under the absence-is-non-applicability rule.

**Decision (Inventory remainder shares its profile reader)** · `dec:migrationpolicy:profile-remainder`

`legacy.todos` counts the unlabelled remainder of the todo profile and
`legacy.implementation` counts the unlabelled remainder of the legacy profile.
Each uses the profile's own recognizer and standard place rather than a second
search for marker text. A correctly labelled item leaves the remainder while
remaining subject to its conforming profile.

**Decision (Debt is a per-source ratchet)** · `dec:migrationpolicy:path-ratchet`

Each applicable owner and policy key compares observed occurrences with its
declared path-count rows. Growth fails, shrinkage requires the standing list to
lower, and an empty list remains an applicable clean verdict rather than
absence. Moving an occurrence between files therefore cannot masquerade as
progress in a total count.

## Consequences · `sec:migrationpolicy:consequences`

Family data can change only through declarations, recognizer semantics only
through the program, and tolerated debt only through the controlled writer.
Zero means the selected corpus no longer contains the family; deleting a
register or deactivating a policy is not another spelling of completion.
