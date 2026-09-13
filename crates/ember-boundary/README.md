# Ember boundary

`ember-boundary` is a platform crate below the engine, games, loader and web renderer; it owns the compiler-derived description of data that crosses between Rust and scripts and depends on none of those higher layers.

The descriptor accepts named structs, unit or named enum variants, internally tagged enums, explicit input/output direction, Serde `rename_all`, top-level `Option` fields that accept absence or null on input, input omission through Serde defaults, fixed arrays and sequences, plus the explicitly marked object-intersection newtype used by League commands.

It rejects flattening, untagged enums, skipped or individually renamed items, custom serializers, unsupported tuple shapes, generic instantiations and target-sized integers instead of guessing at their wire meaning.

Input schemas allow unknown properties as Serde does unless `deny_unknown_fields` is present; output schemas deny them and require every field except an `Option` carrying the explicit `boundary(omit_none)` contract used by loader events.

The default feature set exposes the derive only; data-file models and the JSON Schema renderer are opt-in so a bare dependency cannot pull `serde_json` into a wasm build.
