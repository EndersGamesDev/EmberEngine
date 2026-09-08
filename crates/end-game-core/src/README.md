# Simulation source

`lib.rs` contains material definitions, the dynamic wooden body, player motion, the dungeon's collision footprint, warden state and the chapter progression. `Dungeon::tick` advances exactly one 1/60-second step. Transient actions are supplied only once per press by the client.
