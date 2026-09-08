# Rejected outer frames

This directory contains committed negative inputs for the outer state machine: malformed JSON, an encoded oversize length, an inner frame before admission, and otherwise valid messages sent while awaiting hello, browsing, or already joined.

`src/outer.rs` consumes them to pin the distinction between codec rejection and illegal state transition, including the rule that a repeated hello or post-join lobby query is not silently accepted.

The `oversize.bytes` decimal value records one byte beyond the 64 KiB outer-frame limit without storing a large fixture; tests construct the boundary case from that number.

Admission sequencing and the untrusted-input budget are part of the evergreen host contract in [`../../../../../docs/one-server-evergreen.md`](../../../../../docs/one-server-evergreen.md).
