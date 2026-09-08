# Fire server source

`lib.rs` contains the authoritative service and publishes `ServerConfig`, `build_stamp`, and `run`; it owns connection events, lobby membership and passwords, input sequencing, AI substitution, fixed-step races, broadcasts, and cleanup.

`main.rs` preserves the deployed positional bind address and optional host name before constructing a plain TCP listener for the library.

The in-module tests pin collision-free slot allocation, full-lobby behavior, AI occupancy, one-shot boost latching, stale-input rejection, exact protocol admission, and lobby removal after the final departure.

Both this authority and the predicting client execute `fire-core`, so the fixed-order rules in [`../../../CLAUDE.md`](../../../CLAUDE.md) apply across the crate boundary.
