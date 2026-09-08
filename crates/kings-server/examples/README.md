# Four Kings deployment probe

`probe.rs` verifies a deployed `ws` or `wss` endpoint beyond the listener: it exchanges hello and welcome, creates and joins a lobby, then leaves through the real hub path.

An optional expected commit requires the welcome stamp to match the binary just built, catching an old process that survived an in-place rebuild even when its port still accepts connections.

The deeper lobby exchange distinguishes this probe from a transport-only HTTP upgrade or ping and makes version admission part of the deployment health verdict.

The exercised lifecycle and expected messages are defined by [`../../../docs/kings-design.md`](../../../docs/kings-design.md), with host identity rules in `docs/hosts.md`.
