# Shared shader templates

`ember-shader` is the repository-wide boundary between Rust-owned GPU interfaces and WGSL rendered at runtime for engine, lab, and game pipelines.

Julibrot's `ember-julibrot-shader` crate remains the upstream design source and stays independent of this crate. Mechanism changes originate there and are copied here; this shared implementation does not diverge without the corresponding upstream change.

Its metadata describes host-shareable Rust types and enum discriminants from the same layouts and values that pipeline owners use. Runtime rendering, validation, and anti-drift auditing mirror the proven Julibrot mechanism under repository-neutral names as later structural steps land.

Repository shader policy and migration evidence live in [`../../docs/shaders.md`](../../docs/shaders.md).
