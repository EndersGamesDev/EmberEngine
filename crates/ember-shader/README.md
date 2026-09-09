# Shared shader templates

`ember-shader` is the repository-wide boundary between Rust-owned GPU interfaces and WGSL rendered at runtime for engine, lab, and game pipelines.

Julibrot's `ember-julibrot-shader` crate remains the upstream design source and stays independent of this crate. Mechanism changes originate there and are copied here; this shared implementation does not diverge without the corresponding upstream change.

The public rendering, validation, and anti-drift contracts will mirror the proven Julibrot mechanism under repository-neutral names. Repository shader policy and migration evidence live in [`../../docs/shaders.md`](../../docs/shaders.md).
