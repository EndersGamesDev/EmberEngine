# CLI common integration tests

This directory tests `cli-common` exactly as a downstream helper binary sees it, through public exports only.

`public_api.rs` pins JSON emission to real stdout, `BaseArgs` flattening and debug parsing, unknown-flag rejection, and conversion of clap help into a structured control record; the workspace's ADR-010 tools rely on those externally visible behaviors.

The authoritative API and output rules live in `tools/cli-common/README.md`; narrower private-path coverage remains under `src/tests`.
