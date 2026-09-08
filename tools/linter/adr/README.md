# Linter architecture records

This directory is the normative record set for the linter's declared-surface model: labels and references, divisions and kinds, owners, row semantics, inventory profiles and projections, migration families, policy catalogues, interchange envelopes, snapshots, publications, SPDX checks, and the command boundary.

The `linter` library and binary implement these decisions, while repository declarations under `.linter` supply the data they interpret; `catalogue.md` and `tools/linter/docs/adrs.md` provide the route into the individual records.

Changes here must preserve the label and ownership rules described in `CLAUDE.md` and the linter crate's own `README.md`, because these records define behavior rather than serving as historical commentary.
