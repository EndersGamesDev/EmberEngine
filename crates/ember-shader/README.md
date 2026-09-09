# Shared shader templates

`ember-shader` is the repository-wide boundary between Rust-owned GPU interfaces and WGSL rendered at runtime for engine, lab, and game pipelines.

Julibrot's `ember-julibrot-shader` crate remains the upstream design source and stays independent of this crate. Mechanism changes originate there and are copied here; this shared implementation does not diverge without the corresponding upstream change.

Its metadata describes host-shareable Rust types and enum discriminants from the same layouts and values that pipeline owners use. A strict, feature-full Minijinja environment renders embedded templates from those types, values, binding numbers, and named constants without a global cache or mutable static state.

The pipeline owner retains the exact rendered source and its stable hash beside the pipeline. Native renders parse and validate WGSL with naga, audit every registered interface item against the parsed module, and require every CPU-visible declaration's exact bytes to come from its traced filter under a render-local tag. Wasm returns the source for wgpu's shader-module creation to perform the browser path's single validation.

Template debug diagnostics are test- or feature-only and do not enter a default release build. Every production template must be paired with the crate's deterministic native validation macro; the repository checker authenticates and executes those compiled tests.

Repository shader policy and migration evidence live in [`../../docs/shaders.md`](../../docs/shaders.md).
