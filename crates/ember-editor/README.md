# Ember editor

`ember-editor` is a native-first level builder built on the same `EmberGame` frame contract as the games, with a reusable library and the `ember-editor-app` shell.

It edits `arena-core::Level` data directly instead of maintaining a parallel schema, providing camera navigation, selection and picking, transform gizmos, a box-based palette, and JSON export.

The exported document is an honest authoring artifact rather than a currently loadable arena map; connecting it to the running shooter remains a protocol and deployment change recorded in `docs/plans/milestone-2-editor.md`.

Renderer constraints that shape the gizmos and palette live in [`../../CLAUDE.md`](../../CLAUDE.md), while the simulation meaning of the edited data is analysed in [`../../docs/state-model.md`](../../docs/state-model.md).
