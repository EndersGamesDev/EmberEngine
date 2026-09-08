# Fire Racer client integration tests

`online_e2e.rs` starts a real `fire-server`, connects the production native `Net` transport, and drives the real `Online` client rather than stopping at server-side wire coverage.

The suite pins client prediction convergence, two-client remote movement, surfaced join refusals, and lobby discovery across actual sockets, exercising the interaction among `fire`, `fire-core`, `fire-server`, and shared client networking.

These cases exist because a valid codec does not prove that acknowledgement trimming, replay, and rendering state converge during a live race; failures should be diagnosed across that full loop.

The state distinctions behind the assertions are described in [`../../../docs/state-model.md`](../../../docs/state-model.md), and shared simulation layering remains defined by [`../../../CLAUDE.md`](../../../CLAUDE.md).
