# Canonical outer server frames

These JSON fixtures pin every server-side outer response family: welcome, lobby listings, successful admission, unsupported game and version refusals, and a structured protocol error.

The codec tests in `src/outer.rs` encode matching `ServerMessage` values and require byte-for-byte equality, preserving tuple fields such as occupancy, capacity, hosted alternatives, and optional status detail.

Because deployed clients use these frames to browse and choose immutable inner versions, field or tag changes belong to an outer-protocol version decision, not a fixture normalization.

The compatibility promise these responses serve is documented in [`../../../../../docs/one-server-evergreen.md`](../../../../../docs/one-server-evergreen.md).
