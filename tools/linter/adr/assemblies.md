# Declared Publications · `rec:assemblies:declared-publications`

**Status:** Decided

## Context · `sec:assemblies:context`

A large authored document may be maintained as ordered parts while a committed
publication presents their concatenation. The linter must prove that the
standing publication is exactly the declared parts without learning which
publication a repository happens to maintain.

ADR-L-012, An adjudication procedure for identities, digests, and evidence,
supplies the exact-evidence discipline. ADR-L-020, The migration disciplines,
supplies the declared-parts convention. This record re-derives the executable
policy around a repository-supplied publication relation.

## Decision · `sec:assemblies:decision`

**Decision (A publication row owns the complete relation)** · `dec:assemblies:declared-relation`

Each row declares an owner, a parts directory, and a target. The parts manifest
declares membership and order rather than asking file names or directory order
to imply them. Membership is checked in both directions: a named missing part
and an unnamed present part are both defects, and neither is silently repaired
by the assembler.

The target set, generated-document nonparticipation, containment checks, and
the one-generator-per-target obligation all derive from the same rows. No
second generated-target list or compiled publication table may disagree with
them.

**Decision (Freshness compares exact assembled bytes)** · `dec:assemblies:exact-publication`

A live assembly is current only when its target equals the deterministic
concatenation of its declared parts. Verification and writing use the same
recipe, including ordering and terminal line endings. A digest or timestamp
does not substitute for the expected bytes.

A declared assembly with no parts directory is dormant. A parts manifest may
explicitly mark an active draft, which keeps membership checks but suspends
freshness and writing until the marker leaves. Neither state is inferred from
the standing target's contents.

**Decision (Both catalogue identifiers share one observer)** · `dec:assemblies:programme-aliases`

`assembly.publications-current` is the repository-neutral program over
declared rows. The earlier `assembly.assayer-spec-current` identifier remains a
singleton catalogue entry for declaration compatibility, but routes to the
same assembly observer and fingerprint identity. It carries no compiled
publication path or sibling data.

## Consequences · `sec:assemblies:consequences`

Repositories may declare no publications or many without changing the binary.
Every publication has one source relation, a draft cannot overwrite its
target, and two generators cannot make last-writer order decide committed
bytes.
