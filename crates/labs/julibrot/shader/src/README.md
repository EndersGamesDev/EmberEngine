# Julibrot shader template source

`lib.rs` exports `type_meta.rs`, whose scalar descriptions, explicitly aligned vector wrappers and struct macro derive WGSL names and layouts from bytemuck-safe Rust types, and `enum_meta.rs`, whose enum macro derives shader constants from Rust discriminants.

`runtime.rs` owns the lab's embedded Minijinja environment and resolves its declaration, enum, binding and constant filters exclusively from a `ShaderContext` supplied by the pipeline owner. Native renders are directly Naga-validated; wasm renders are validated by wgpu at module creation. Native failures report the rendered WGSL line and its text, and every rendered source carries a stable hash for its pipeline owner.

`oracle.rs` tests every type and enum in its real `ShaderContext` registry. It compares naga's rendered struct offsets and sizes with the Rust metadata, including a padded `vec3`, compares rendered enum constants with Rust discriminants, and proves a missing type is named at render time.

`present-shade.wgsl.jinja` is the first production template. Julibrot keeps this crate when a later lane builds a separate shared implementation for the remainder of the repository.

The deployment checker binds each derived test name to one macro invocation, requires that test in the owning crate's compiled test binary, and runs it normally and with a nonce-bearing invalid-WGSL probe.
