# Arena server source

`lib.rs` implements the server library and publishes `ServerConfig`, `build_stamp`, and `run`; it owns the event bridge, lobby lifecycle, simulation ticks, admission checks, sanitization, broadcasts, and disconnect cleanup.

`main.rs` is the production binary entry point, assembling configuration and a TCP listener before handing control to the library. Deployment consumes its build stamp to distinguish the exact server revision exposed by a running host.

The in-module tests pin input coalescing, shield release and repress behavior, loadout validation, protocol rejection, and shared lobby configuration before the real-socket suite exercises the same boundaries.

The hub must preserve the fixed update and exact protocol gate described in [`../../../CLAUDE.md`](../../../CLAUDE.md); encoded and authoritative state responsibilities are detailed in [`../../../docs/state-model.md`](../../../docs/state-model.md).
