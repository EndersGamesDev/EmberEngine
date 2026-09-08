# Julibrot shader template source

`lib.rs` exports `type_meta.rs`, whose built-in descriptions and struct macro derive WGSL names and layouts from bytemuck-safe Rust types, and `enum_meta.rs`, whose enum macro derives shader constants from Rust discriminants.

`runtime.rs` owns the lab's embedded Minijinja environment and resolves its declaration, enum, binding and constant filters exclusively from a `ShaderContext` supplied by the pipeline owner.

Naga validation, the cache key, and the anti-drift oracles join this foundation as separate migration steps.
