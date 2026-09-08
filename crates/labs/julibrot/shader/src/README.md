# Julibrot shader template source

`lib.rs` exports `type_meta.rs`, whose built-in descriptions and struct macro derive WGSL names and layouts from bytemuck-safe Rust types, and `enum_meta.rs`, whose enum macro derives shader constants from Rust discriminants.

`runtime.rs` owns the lab's embedded Minijinja environment and resolves its declaration, enum, binding and constant filters exclusively from a `ShaderContext` supplied by the pipeline owner. Its public renderer parses and validates every expanded source with the same naga version used by wgpu, reports the template line on failure, and gives validated source a stable hash for owner-held caching.

The anti-drift oracles join this foundation as a separate migration step.
