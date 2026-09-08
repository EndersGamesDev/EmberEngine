# Julibrot shader template source

`lib.rs` exports `type_meta.rs`, whose built-in descriptions and struct macro derive WGSL names and layouts from bytemuck-safe Rust types, and `enum_meta.rs`, whose enum macro derives shader constants from Rust discriminants.

The rendering environment, enum and binding metadata, validation, cache key, and anti-drift oracles join this foundation as separate migration steps.
