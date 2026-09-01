# Index-linter Backlog · `plan:indexlinter:backlog`

This backlog holds open work owned by the linter package. Entries are
the live worklist; completed work leaves the file and remains in git history.

## Generated projections · `sec:indexlinter:backlog-generated-projections`

**Entry (Registers become a generalized projection policy)** · `entry:indexlinter:register-projection-policy`

**OPEN.** The owner ruled that tracked-register generation is not a
per-package tool but a proper policy of this linter: a declared register
surface whose tracking rows the linter regenerates exactly and gates on
staleness, the way burn census regions and projected test matrices already
work. Generalize what those two mechanisms share — a hand-written preamble,
a tool-owned nonparticipating region between markers, exact byte
regeneration, and a check mode that fails when committed bytes differ from
regeneration — into one policy with its own declaration surface, so any
package can declare a register of heads-to-documents rows and receive
generation plus bidirectional truth for free. The first adopter and
acceptance surface is the Assayer record register, registered in that
package as ``reg:assayer:decision-record-register`` with its adoption
entry ``entry:assayer:tool-generated-registers`` (displayed spans: the
citation graph deliberately gives this package no reach into its
adopters' labels), whose ~300-row tracking table becomes a generated
region under the policy. Design the
declaration shape before implementing: the policy needs to say which head
sets a register tracks, and the existing positional-register convention is
the semantic starting point. The owner settled the design's leading question
on 2026-08-29: under ADR-T-014, A calculus of documentation and source labels,
generated-register occurrences participate in full, while a generated region
feeds nothing it presents because it never enters that harvest. The design
therefore makes every row an
ordinary mint or citation while excluding it from the set or registry the row
presents, preserving the no-self-support result. The per-owner dagger registers
of ADR-T-027, The dagger discipline, are further adopter candidates
under those settled semantics. The existing engine now carries generated
imported citations through the same import relation as authored citations, so
the layer-reach law and cited-owner policy dependencies both see them; only the
generalized projection policy remains open. This entry waits behind the
refactor ladder's active bites — it lands on the post-ladder architecture, not
the current module layout.

## Dependency totality · `sec:indexlinter:backlog-dependency-totality`

**Entry (A dependent policy without its prerequisites refuses loudly)** · `entry:indexlinter:policy-prerequisite-refusal`

**OPEN.** The owner ruled, on the bite-6 STOP that exposed a package
participating in the label calculus only by compiled geography: a profile
or legacy policy depends on the label calculus it reads, and the linter
must refuse to run — loudly, as a configuration refusal — when an owner
activates the dependent policy without its label prerequisites, instead of
running from the incomplete declaration. The dependency mechanism exists
and reported zero because the templates never declared the edge: add the
prerequisite edges from `legacy.todos` and each `profile.*` policy to the
`labels.*` family in the compiled dependency templates, and prove the
refusal with an invented fixture activating a dependent policy alone. This
entry is deliberately scheduled BESIDE the Configuration::Absent
contract-closure bite, after the refactor ladder: it changes refusal
behavior on misdeclared corpora, so it cannot ride any byte-identical
ladder bite, and its fixture must show the refusal naming the missing
prerequisite for the user.

## Occurrence scanning · `sec:indexlinter:backlog-occurrence-scanning`

**Entry (An unclosed acute fails at its opening delimiter)** · `entry:indexlinter:unclosed-acute`

**OPEN.** The code carrier currently returns no span for an acute left open at
the end of a commentary line. The calculus instead makes an opening acute
that remains unclosed when its comment region ends a hard failure under
ADR-T-014, A calculus of documentation and source labels. Add the carrier
finding at the opening
delimiter and pin it with code-carrier tests covering ordinary comments,
documentation comments, adjacent comment lines, and a later valid occurrence
whose resolution must continue normally.

Near-miss warnings and commentary-fence boundaries are not part of this
entry: both already have implementation claims and tests in the package. This
entry closes only the unpaired-acute gap still documented beside
`code_spans`.
