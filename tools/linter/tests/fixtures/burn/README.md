# Burn-policy fixtures

These TOML documents model legacy implementation, TODO, tag, section, record, and unprefixed-reference policies together with repository-wide variants and declared division, prefix-number, and scenario reference policies.

`burn_and_ratchets` unit coverage embeds the corpus to pin migration-burn accounting and ratchets, while `declared_surface` uses the matching filenames to verify the repository's exact declaration inventory.

Expected lifecycle and reference behavior is defined by the migration, references, divisions, and row-semantics records under `tools/linter/adr`; each fixture stays narrowly shaped so a failed policy identifies the contract that moved.
