# Ember editor source

`lib.rs` publishes the editor model, fly camera, grid and axis builders, and `engine_config`; `main.rs` is the native window entry point that drives the same command queue intended for the web shell.

`pick.rs` maps cursor rays to oriented boxes, `gizmo.rs` builds selectable box-based handles and cages, and `drag.rs` applies delta-based translation, yaw, and scale without inventing transforms the simulation cannot store.

`palette.rs` defines placeable cover and spawn presets plus the shell-neutral command mailbox, while `level.rs` converts editor objects to and from the simulation's `Level` and readable JSON.

In-module tests pin ray algebra, nearest picking, edge-triggered commands, screen-scaled gizmos, drag continuity and clamping, palette coverage, representable yaw, lifted-box bases, level round trips, and selection lifecycle; the renderer and simulation constraints behind them are in [`../../../CLAUDE.md`](../../../CLAUDE.md).
