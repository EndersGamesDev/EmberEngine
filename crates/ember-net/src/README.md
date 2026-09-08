# Ember networking source

`lib.rs` exports the `outer` module and the small cross-platform helpers for retryable reads and bounded text sanitization.

`outer.rs` owns outer protocol version 1, its frame-size ceiling, all client and server message types, codec errors, joined inner payloads, and the connection state machine from initial hello through browsing and admission.

Its in-module tests consume the committed fixtures to pin exact JSON, distinguish malformed, oversized, and unsupported-version input, accept the complete legal transition path, and reject messages delivered in the wrong state.

Once the state machine admits a connection, game bytes remain opaque as required by [`../../../docs/one-server-evergreen.md`](../../../docs/one-server-evergreen.md); hosted dependency rules are summarized in [`../../../docs/ARCHITECTURE.md`](../../../docs/ARCHITECTURE.md).
