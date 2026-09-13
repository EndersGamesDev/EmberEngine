# Plans

This directory holds active implementation designs, gate definitions, adoption surveys and the shared one-line backlog for work that has not yet landed.

Workers and reviewers depend on these pages to recover intended behavior and known gaps; shipped results belong in [`CHANGELOG.md`](../../CHANGELOG.md), not in a finished-work list here or in the root README.

Use [`backlog.md`](backlog.md) for concise follow-ups, keep deeper rationale in the relevant plan, and follow the coordination and verification rules in [`../worker-protocol.md`](../worker-protocol.md) and [`../../CLAUDE.md`](../../CLAUDE.md).

## Active studies

This table is the complete index of active studies; implementation plans and completed records remain organized by their existing filenames and changelog entries.

|Study|Decision surface|
|-----|----------------|
|[Typed web generation](typed-web-study.md)|How Rust-generated declarations make pedantic browser and Node TypeScript templates fail on boundary drift before selective Rust ownership, with frozen publication bytes treated explicitly.|
