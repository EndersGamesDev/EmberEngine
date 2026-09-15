# Ember boundary

`ember-boundary` is a platform crate below the engine, games, loader and web renderer; it owns the compiler-derived description of data that crosses between Rust and scripts and depends on none of those higher layers.

The descriptor accepts named structs, unit or named enum variants, internally tagged enums, explicit input/output direction, Serde `rename_all`, top-level `Option` fields that accept absence or null on input, input omission through Serde defaults, fixed arrays and sequences, plus the explicitly marked object-intersection newtype used by League commands.

Every 64-bit integer field declares its script representation. `boundary(wide = "exact")` preserves integer identity as TypeScript `bigint`; `boundary(wide = "precise", bound = "...")` uses TypeScript `number` and records the engineering bound that keeps the value within the safe-integer range. The derive rejects a 64-bit field without either declaration, and a precise declaration without a non-empty bound. Every declaration carries a one-line reason at the field explaining why identity matters or why its bound holds. The commit that introduces or changes a declaration states that justification in its body so review can check the prose the derive cannot.

This metadata does not change Serde or the wire. In particular, an exact field currently encoded as a JSON number remains a JSON number, so a consumer that may receive values above 2^53 - 1 must preserve the number token and construct a `bigint` without first passing it through JavaScript `number`. Whether such fields should later use JSON strings is a separate transport decision.

It rejects flattening, untagged enums, skipped or individually renamed items, custom serializers, unsupported tuple shapes, generic instantiations and target-sized integers instead of guessing at their wire meaning.

Input schemas allow unknown properties as Serde does unless `deny_unknown_fields` is present; output schemas deny them and require every field except an `Option` carrying the explicit `boundary(omit_none)` contract used by loader events.

The default feature set exposes the derive only; data-file models and the JSON Schema renderer are opt-in so a bare dependency cannot pull `serde_json` into a wasm build.
