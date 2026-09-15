# Ember boundary derive

`ember-boundary-derive` is the host-side procedural macro for `ember-boundary`; generated implementations contain only static metadata and the macro and its parser dependencies are not linked into game or loader wasm.

The derive reads only the documented boundary and Serde subset, delegates every `rename_all` spelling and transformation to `serde-rename-rule`, and turns every other representation or item attribute into a compile error naming the unsupported shape.

For an `i64` or `u64` field, `boundary(wide = "exact")` declares exact script identity and `boundary(wide = "precise", bound = "...")` declares a bounded JavaScript-number representation. The derive names the field and both choices when the declaration is absent; `wide` is rejected on every other integer width. Every declaration carries a one-line reason at the field explaining why identity matters or why its bound holds. The commit that introduces or changes a declaration states that justification in its body so review can check the prose the derive cannot.
