# Declared Snapshot · `rec:snapshot:declared-snapshot`

**Status:** Decided

## Context · `sec:snapshot:context`

The command contract requires every operation to load the declared configuration before policy work and distinguishes a configuration refusal from a finding about the repository (`spec:commandcontract:configuration`) (`rule:commandcontract:configuration-verdicts`). The package still needs one non-circular rule for which declaration bytes make that snapshot.

## Decision · `sec:snapshot:decision`

**Decision (Membership comes from the declared directory)** · `dec:snapshot:physical-membership`

The fixed core is ``owners.toml``, ``environments.toml``, ``policies.toml``, and ``lists.toml``. Beyond that core, policy-declaration membership is discovered from the directory: every document present in ``.linter`` whose envelope identifies a policy declaration joins the snapshot.

The fixed core is required, so an absent core member refuses the snapshot. A present policy declaration participates whether or not Git tracks it.

**Decision (The loader reads declarations as written)** · `dec:snapshot:as-written`

The loader enumerates the physical directory and reads the bytes standing there. It never asks Git which declarations exist, substitutes indexed or committed bytes, or applies the repository's global exclusion rows to declaration membership.

This settles both shape questions before any declaration is interpreted: the declaration universe is as written, not Git-tracked, and global exclusions do not filter it. The file answering either question cannot control whether that same file is read; otherwise configuration authority would depend on the answer it is meant to establish.

**Decision (Loading is atomic and refusal precedes analysis)** · `dec:snapshot:atomic-refusal`

The loader discovers the complete membership, reads every member's envelope and content, and cross-validates the declarations as a single operation. An absent required member or any defect knowable from declaration bytes refuses before repository analysis begins. No partial snapshot, per-file default, earlier value, or compiled fallback survives a refusal.

Only after every declaration passes does a snapshot exist. A disagreement knowable only by analysing the repository is then a finding under the command contract, not a late refusal.

## Consequences · `sec:snapshot:consequences`

Staging and committing a declaration do not change the bytes the loader reads; writing or removing it does. Policy declarations can therefore extend the surface without extending a compiled filename list, while the required core keeps the snapshot's shape grounded.

Envelope version claims retain the repository's adopted meaning under ADR-L-023, Adopting the interchange conventions for first-party structured configuration. This record fixes when their byte-level defects refuse; it does not redefine version acceptance or policy semantics.
