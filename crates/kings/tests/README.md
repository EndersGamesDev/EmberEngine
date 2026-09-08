# Four Kings client integration tests

`online_e2e.rs` connects the production native transport and `Online` client state to a real `kings-server`, with the `Selection` machine producing the first move.

The main scenario follows two clients through creation, joining, start, an agreed move, two invalid-action refusals, and a silent-turn timeout while requiring their boards to agree after every transition.

Additional cases pin hello and ping behavior without engine updates and verify that a refused join becomes a client-visible reason, covering client responsibilities beyond the server's own wire tests.

This is the client half of the test plan in [`../../../docs/kings-design.md`](../../../docs/kings-design.md), especially its online authority and page-state boundaries.
