# Canonical outer client frames

These compact JSON fixtures are the byte-facing client examples for outer protocol version 1: `hello`, `list_lobbies`, `create_lobby`, and `join_lobby` cover discovery and both admission routes.

The tests in `src/outer.rs` decode each file and compare it with the corresponding `ClientMessage`, pinning tag names, payload nesting, null handling, game/version selectors, passwords, and handles.

Changing a fixture changes the canonical pre-admission wire shape and therefore requires the matching codec and compatibility argument rather than a formatting-only refresh.

The protocol's role in selecting frozen inner games is specified in [`../../../../../docs/one-server-evergreen.md`](../../../../../docs/one-server-evergreen.md).
