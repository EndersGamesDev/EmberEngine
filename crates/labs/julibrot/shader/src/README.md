# Julibrot shader template source

`lib.rs` exports `type_meta.rs`, whose built-in descriptions and struct macro derive WGSL names and layouts from bytemuck-safe Rust types, and `enum_meta.rs`, whose enum macro derives shader constants from Rust discriminants.

`runtime.rs` owns the lab's embedded Minijinja environment and resolves its declaration, enum, binding and constant filters exclusively from a `ShaderContext` supplied by the pipeline owner. Its public renderer parses and validates every expanded source with the same naga version used by wgpu, reports the template line on failure, and gives validated source a stable hash for owner-held caching.

`oracle.rs` tests every type and enum in its real `ShaderContext` registry. It compares naga's rendered struct offsets and sizes with the Rust metadata, including a padded `vec3`, compares rendered enum constants with Rust discriminants, and proves a missing type is named at render time.

`present-shade.wgsl.jinja` is the first production template. Julibrot keeps this crate when a later lane builds a separate shared implementation for the remainder of the repository.
