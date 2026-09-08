# Legacy layouts

This directory contains `arena.json`, the filesystem-loaded level layout for the native legacy game path, not the authored levels shared by the current Arena client and server.

The legacy native game depends on its dimensions and collision data; current wasm Arena code must not acquire a dependency on it.

The ownership boundary is recorded in [`docs/plans/milestone-2-editor.md`](../../docs/plans/milestone-2-editor.md), with repository layering rules in [`CLAUDE.md`](../../CLAUDE.md).
