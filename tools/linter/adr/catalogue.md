# Policy Catalogue · `rec:catalogue:policy-catalogue`

**Status:** Decided

## Context · `sec:catalogue:context`

Repository declarations decide where a verdict applies; the binary decides what that verdict means. Conflating those authorities would either compile repository families into the linter or let configuration rewrite observation, tolerated-debt identity, and prerequisites. The catalogue owns the first boundary while the declared snapshot owns the second.

## Decision · `sec:catalogue:decision`

**Decision (A program and a key separate meaning from instance)** · `dec:catalogue:program-and-key`

A policy program is compiled meaning: its identifier, parameter shape, judgment, diagnostic identity, and dependency templates. A `PolicyKey` is that program identifier plus an optional declared family. Singleton programs omit the family; instanceable programs require one. Several keys may therefore share one program without teaching the binary which families a repository declares.

**Decision (Each program owns its routing and codec)** · `dec:catalogue:routing-and-codec`

Each program fixes one allowance codec—path count, path set, or fingerprint—and every routed program names its own recognizer and observer. These codecs retain the identities and comparison semantics fixed by ADR-T-020, The migration disciplines.

An unrouted program observes nothing. A catalogue entry may carry its codec while withholding its observer during a shadow stage; that state is a verdict declared and not yet formed, never an observation of zero violations.

**Decision (Activation and debt use the full key)** · `dec:catalogue:full-key-relations`

Activation is the presence of an owner and full `PolicyKey` pair, and its absence is non-applicability. Every tolerated-debt table is keyed by that same owner and full key, so two families sharing a program can neither satisfy one another's activation nor share allowances. An empty table remains an applicable verdict with no tolerated debt, as distinguished from absence by ADR-T-020, The migration disciplines.

**Decision (A repository-wide prerequisite has one activation)** · `dec:catalogue:repository-singleton`

A repository-wide verdict has exactly one activation in a resolved snapshot. A fixed-owner dependency edge resolves to that activation; none or several is a refusal rather than a choice made by the binary. Presence satisfies the edge even when the prerequisite carries tolerated debt, preserving the dependency contract (`req:commandcontract:dependency-contract`).

## Consequences · `sec:catalogue:consequences`

Adding an instance of an existing program changes declarations, not compiled vocabulary. Changing a program's codec or observation route changes policy meaning and requires a policy-schema migration. Snapshot validation rejects partial keys, aliased tables, and ambiguous repository-wide prerequisites before any observer runs; shadow entries cannot report a false clean result. Executable catalogue rows remain code rather than a second table in this record.
