# Four Kings server WebSocket tests

`ws_e2e.rs` starts the actual server and speaks its public protocol, proving two players can create, join, start, and make the first move through real WebSockets.

The suite also pins silent-turn timeout, pre-hello rejection, version mismatch details, ungated listing, password admission, the deployment probe's deep lobby step, and termination when a player disconnects mid-game.

These cases complement the synthetic-time lobby tests by covering socket ownership, serialization, admission order, and delivery without involving the graphical client.

They implement the server half of section 4.9 in [`../../../docs/kings-design.md`](../../../docs/kings-design.md).
