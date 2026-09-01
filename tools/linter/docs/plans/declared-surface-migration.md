# Declared-surface migration closure · `plan:emberlinter:declared-surface-migration`

The declared-surface records now hold the campaign's settled policy. This plan names only the work that remains to close the migration.

## Scope and inherited audit debt · `sec:surfmig:scope`

The remaining work turns authored migration material into derived review material, removes the compatibility machinery that made the cut-over observable, discharges the root amendments on which the final surface depends, and proves that the declared surface stands alone.

A completion audit is still owed for the old view-generation and bridge-retirement bites. Their state could not be established with the stale binary available when those bites were reviewed, so neither is presumed complete merely because part of its intended shape is present.

The audit records what is already generated, what still participates in a verdict, which aliases remain edition-visible, which equivalence readers remain callable, and which review tables are still transcribed. Its result is the starting state for the proposals below.

This plan changes no ratified policy meaning. It completes the movement of repository facts into declarations and removes temporary ways to obtain those facts elsewhere.

**Proposal (Markdown burn documents become generated views only)** · `proposal:surfmig:view-generation`

The Markdown burn documents become deterministic views of their declared owner-policy tables. They explain the active debt to reviewers but do not supply rows, maxima, family identity, activation, or any other input to verdict formation.

Generation is exact over the canonical declared ordering. A stale, missing, or manually changed view is a view-exactness failure, while the policy verdict continues to come from the declaration that the view represents.

The controlled writer lowers declared debt first and then renders the views from the resulting snapshot. Repeating generation without a declaration change produces no diff.

The completion audit must prove both halves of the separation: corrupting a view cannot change the policy verdict, and changing declared debt cannot leave a clean but stale view.

**Proposal (Every migration bridge is retired after its review duty)** · `proposal:surfmig:bridge-retirement`

The environment and layer records receive generated review tables derived from the declared relations they govern. Those tables expose the cut-over for review without becoming an alternative source of repository facts.

Old family aliases remain readable only through their command-edition window. Once that window closes, parsing, reporting, fixtures, and documentation use only the ratified owner-policy identities, and the aliases are removed together.

Every equivalence reader is deleted after its corresponding declaration has become authoritative and its generated review surface is exact. No comparison path, compiled fallback, or Markdown fallback survives as a dormant recovery route.

Bridge retirement is complete only when the ordinary command constructs one snapshot from declarations and no test helper can reconstruct the superseded representation.

**Proposal (Three root amendments precede final closure)** · `proposal:surfmig:owed-root-amendments`

The tracked-universe amendment is discharged. The root partition convention now narrows the accounting universe to the git-tracked corpus and states totality and exclusivity at the row under ADR-L-019, The layer owner graph, so the universe used by declaration validation and review generation is settled before the closure verdict is taken.

The command-contract count restatement is discharged. The configuration specification now states the declared snapshot as a fixed core plus one declaration per parameterized policy family, and says explicitly that nothing else about the snapshot changes as it grows (`spec:commandcontract:configuration`), so no earlier file count survives as normative text.

The environment-relation delta remains owed. It must reconcile the root relation record with the ratified declared relation and its generated review table.

The campaign may prepare view generation and bridge deletion while these prerequisites move, but the final closure gate waits for all three amendments to be present and mutually consistent.

**Register (The residual-litter pair retains the ratified common shape)** · `reg:surfmig:residual-litter`

The residual family is the pair `ASSAYER : legacy.residual-litter` with the matching `[ASSAYER."legacy.residual-litter"]` path-count table. It uses the same activation, audit, writer, and empty-table rules as the other legacy families.

Its clean state is an active pair with an empty retained table. Absence of the pair, use of a private codec, or recovery through an old alias is not equivalent to zero debt.

The pair therefore participates in the full audit and idempotence proof even when it has no observed occurrences.

**Gate (Closure proves declarations are the only surviving authority)** · `gate:surfmig:closure`

A clean `check` proves that the declared snapshot loads, cross-validates, partitions the tracked universe, resolves its labels, and produces no policy finding.

A full `burn --audit` covers every active owner-policy pair, including empty retained tables, and reports no growth, stale row, malformed declaration, or view discrepancy.

An idempotence run applies `burn --write`, records its canonical output, applies it again without intervening changes, and observes no second change.

The bridge search finds no compiled fallback, Markdown verdict reader, equivalence reader, expired family alias, or hand-maintained environment or layer review table.

The inherited completion audit is closed in the same evidence set: any unfinished part of the old view-generation or bridge-retirement work is completed before the gate is called clean.

Closure is reached only when the clean check, full audit, idempotent write, root prerequisites, generated views, and no-surviving-bridge search agree on one declared surface.
