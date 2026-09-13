# Ember boundary derive

`ember-boundary-derive` is the host-side procedural macro for `ember-boundary`; generated implementations contain only static metadata and the macro and its parser dependencies are not linked into game or loader wasm.

The derive reads only the documented boundary and Serde subset, delegates every `rename_all` spelling and transformation to `serde-rename-rule`, and turns every other representation or item attribute into a compile error naming the unsupported shape.
