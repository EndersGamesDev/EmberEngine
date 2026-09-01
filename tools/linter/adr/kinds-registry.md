# Declared Kinds Registry · `rec:kindregistry:declared-authority`

**Status:** Decided

## Context · `sec:kindregistry:context`

The linter needs one stable relation from environment names to kind tokens and
one set of kinds reserved for generated mints. Its commands must apply those
data to every corpus they inspect, including a corpus with no decision records
of its own. Reading a document outside the package would make the executable's
meaning depend on a file beyond its sealed dependency boundary.

The repository's human account of the shared base registry is ADR-T-011,
Environment kinds. The linter needs the resulting data and enforcement seam,
not that record's argument or document structure. Repository-local profiles
also add rows and reserved kinds that are not part of the shared base.

## Decision · `sec:kindregistry:decision`

**Decision (Runtime authority is declared configuration)** · `dec:kindregistry:runtime-authority`

The linter constructs its effective kind registry only from the accepted
``environments.toml`` member of the declared configuration snapshot. The
``environments`` and ``reserved_kinds`` relations state the shared base;
``extensions`` and ``reserved_extensions`` state repository-local additions.
The effective name-to-kind relation and reserved set are the respective unions.

All four relations are declarative data validated by the existing atomic
snapshot loader. There is no compiled base, external document read, fallback,
override, or ordering precedence. An exact duplicate within a declared
relation refuses with the rest of the snapshot; repeated names under distinct
kinds remain distinct rows of the relation.

**Decision (A repository-level test is the synchronization seam)** · `dec:kindregistry:sync-seam`

A repository-level integration test parses the human record through the
linter's Markdown registry parser and loads ``environments.toml`` through the
sanctioned snapshot loader. It compares the base projections as sets of exact
environment-name/kind pairs and exact reserved-kind tokens.

Row order, Convention provenance, attestation status, presentation-device
rows, and repository-local extensions are deliberately outside the comparison.
They do not participate in both runtime surfaces: the runtime relation needs
the normalized pair, while the Markdown parser alone needs the source metadata
to project that pair and its reserved status. The comparison is therefore
lossless over the behavior the linter consumes.

**Decision (Adoption edits both authorities together)** · `dec:kindregistry:joint-adoption`

Adopting a new revision of the human registry means editing the human record
and the shared base relations in ``environments.toml`` together. The
repository-level integration test fails when either side moves alone. A
repository-local extension changes only its explicit extension relation and is
not represented as an adoption of the shared base.

**Decision (Kind conformance consumes only the effective registry)** · `dec:kindregistry:conformance`

The `labels.mints-kind-conform` policy validates each mint against the effective
declared relation. A head's presented environment name must reduce to the kind
the mint carries, and a reserved kind may be minted only through its declared
derivation rule. The policy neither reads the human record nor carries a second
compiled vocabulary; failure to form the effective registry refuses the
snapshot before kind conformance can run.

## Consequences · `sec:kindregistry:consequences`

The linter package remains sealed: runtime registry construction reaches no
file outside its declared configuration input. Its package tests use a
fictional package-local registry to exercise Markdown parsing, validation,
presentation reduction, reserved kinds, and extension behavior. Assertions
about the repository's real human record live only at repository test level.

The synchronization test makes drift visible without giving runtime two
authorities. A configuration refusal still precedes policy analysis under the
atomic loading decision (`dec:snapshot:atomic-refusal`).
