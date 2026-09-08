# Julibrot device responsibilities

This directory divides the presenter's GPU implementation by pass and evidence type: `shade` creates escape-value shading, `scene` builds relief targets, `warp` reprojects retained textures, and `redraw` fills exposed relief from retained records.

`uniforms` owns immutable layouts and aligned payloads, `poll` converts bounded fence observations into typed events, `readback` routes requested frame copies, and `census` measures glitch counts and candidate references.

`ledger` applies retention, hold, lattice, and source-selection policy, while `tests.rs` exercises the device-level seams with the same private state used by `Presenter`.

Pass ordering, resource lifetimes, fence deadlines, and visible facts are specified in [`../../../../../../../docs/julibrot/present.md`](../../../../../../../docs/julibrot/present.md).
