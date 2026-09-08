# Four Kings server source

`lib.rs` contains the service and its testable lobby model: joins and leaves, seat rebuilding and creator handover, formation selection, start admission, move validation, clock broadcasts, timeouts, elimination, results, and return to waiting all produce an explicit outbox.

The outer hub drains connection events and feeds elapsed milliseconds into those pure transitions, while bounded socket threads deliver their messages without sharing board state by lock.

`main.rs` preserves the deployed bind and host-name precedence before starting the plain listener; `BUILD_VERSION` and `BUILD_COMMIT` identify the binary through each welcome.

In-module tests use synthetic time to pin seating, creator rights, grace windows, silent-turn elimination, formation phases, disconnect outcomes, host occupancy, protocol rejection, connection caps, and every lobby-map mutation path under [`../../../docs/kings-design.md`](../../../docs/kings-design.md).
