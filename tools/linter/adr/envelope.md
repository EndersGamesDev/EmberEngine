# Interchange Envelope · `rec:envelope:interchange-envelope`

**Status:** Decided

## Context · `sec:envelope:context`

The repository adopts the base envelope across TOML, JSON, and YAML under ADR-L-023, Adopting the interchange conventions for first-party structured configuration. This record owns the package policy that recognizes that envelope; registry lookup and acceptance remain deferred under the same record.

## Decision · `sec:envelope:decision`

**Decision (The file name selects one of three carriers)** · `dec:envelope:one-family`

TOML, JSON, and YAML are carrier projections of one policy. The compiled carrier catalog selects the projection only from the file name (`dec:catalogue:routing-and-codec`); content and parse success never select a parser.

**Decision (Source spelling carries the envelope verdict)** · `dec:envelope:source-shape`

Every declared document opens with `namespace` and then `version` as its first two top-level keys in source order. The namespace value satisfies the adopted label grammar and names the parameter schema, never the document instance: its grammar comes from ADR-L-013, The interchange conventions, and its schema allocation from ADR-L-023, Adopting the interchange conventions for first-party structured configuration.

The version value is a triple of non-negative decimal integers in shortest spelling under ADR-L-013, The interchange conventions. Signs, leading zeroes on nonzero values, digit separators, alternate radices, fractions, and exponents fail. Because parser value models erase the ordering and numeric-spelling distinctions the rule draws, conformance is judged by scanning source; parsed values cannot establish it.

**Decision (The type catalog is closed in both directions)** · `dec:envelope:closed-domain`

The configuration declares the suffix set against which it was written. That set and the compiled catalog agree exactly or the snapshot is refused (`dec:snapshot:atomic-refusal`): an undeclared catalog suffix and a declared uncatalogued suffix fail alike. Carrier changes therefore break loudly, deliberately opposite to the licence-header policy's opt-in type catalog (`dec:spdx:opt-in-carriers`).

Within an owner's share (`dec:rows:owner-input`), the typed domain contains exactly the files whose names the catalog resolves. Out-of-domain files are absent from the policy's universe, and a row that reaches only them is idle.

**Decision (Exclusion alone computes governance)** · `dec:envelope:computed-governance`

The governed set is the typed domain minus the union of named exclusions. Include rows do not select or change that set; the existing operative rows are removed, and `include` is absent by default.

When declared, `include` is only a diagnostic gloss. Its rows must be a complete, pairwise-disjoint partition of the already computed governed set under the ordinary partition judgment (`dec:rows:subtract-then-partition`); a false gloss fails configuration but never changes governance.

## Consequences · `sec:envelope:consequences`

Adding or removing a carrier suffix requires the binary and declaration to move together, so an upgrade cannot silently widen or narrow the obligation. Foreign schemas of a known carrier leave through named exclusions; files outside the typed domain need no ceremony.

The policy proves source-shaped envelope conformance only. It neither validates an instance against the parameter schema its namespace names nor revives the deferred registry and acceptance machinery.
