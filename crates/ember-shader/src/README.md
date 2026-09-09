# Shared shader source

`lib.rs` exports `type_meta.rs`, whose scalar descriptions, explicitly aligned vector wrappers, and struct macro derive WGSL names and layouts from bytemuck-safe Rust types, and `enum_meta.rs`, whose enum macro derives shader constants from Rust discriminants.

`runtime.rs` owns the embedded Minijinja environment and resolves declaration, enum, binding, and constant filters exclusively from a `ShaderContext` supplied by the pipeline owner. It returns the exact source and a stable hash without caching either value globally.

Native validation and anti-drift auditing join this rendering path as independently gated structural steps.

The mechanism follows `crates/labs/julibrot/shader`; repository-wide policy lives in [`../../../docs/shaders.md`](../../../docs/shaders.md).
