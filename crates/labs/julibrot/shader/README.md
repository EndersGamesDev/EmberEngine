# Julibrot shader templates

`ember-julibrot-shader` is the Julibrot-local boundary between Rust-owned GPU interfaces and the WGSL submitted by the lab's kernel and presentation crates.

It describes host-shareable Rust types and, as the migration advances, renders embedded Minijinja templates with Rust-owned types, enum values, bindings, and layout constants before validating the result with naga.

The context owner retains each rendered shader beside the pipeline that consumes it and keys that narrow cache with the shader's stable content hash. The crate has no global cache or mutable static state, so a changed context produces and retains a new rendering.

This implementation remains owned by Julibrot. After its design is complete, a later lane will re-implement the proven mechanism as a separate shared facility for the engine, other labs, and games; those consumers will not depend on this crate.

Native renders parse and validate WGSL directly with naga. Wasm renders return the source and stable hash, then rely on wgpu's shader-module creation for the browser path's single naga validation. Every production template therefore has a native production-context render-and-validate test enforced by the Julibrot shader checker.

The lab-local rule, anti-drift oracle, migration order and measured bundle cost live in [`../../../../docs/julibrot/shaders.md`](../../../../docs/julibrot/shaders.md).
