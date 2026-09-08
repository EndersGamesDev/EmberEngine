# Markdown serializer source

This directory contains the vendored event-to-Markdown serializer: `lib.rs` implements `cmark`, resumable formatting state, options, and public link, image, alignment, and error types; `source_range.rs` preserves source escaping when offset ranges are available; `text_modifications.rs` holds internal escaping and padding helpers.

The workspace linter consumes these entry points to render parsed Markdown deterministically, including source-aware rewrites, while callers can suspend and resume serialization through the public `State` APIs.

Upstream identity, licensing, and the deliberately small workspace divergences are recorded in the crate's `README.md` and `PROVENANCE.md`; behavior otherwise follows the vendored upstream v22.0.1 implementation.
