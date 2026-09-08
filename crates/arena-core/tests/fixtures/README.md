# Arena core compatibility fixtures

These JSON files preserve serialized Trench City level shapes from arena v18 and v30 so map evolution can be compared against concrete wire-era data rather than reconstructed from current generators.

`freight_yard.rs` includes `trench-city-v30.json` in its compatibility tests to pin the legacy level name and decoded geometry while the authored Freight Yard changes independently. The v18 snapshot is retained as the earlier historical baseline and is not currently loaded by Rust code.

Because these files represent protocol-facing state, edits must reflect an intentional compatibility decision; generated formatting or refreshed coordinates would otherwise erase the behavior the snapshots record.

Protocol versioning rules live in [`../../../../CLAUDE.md`](../../../../CLAUDE.md), with the encoded-state model and compatibility analysis in [`../../../../docs/state-model.md`](../../../../docs/state-model.md).
