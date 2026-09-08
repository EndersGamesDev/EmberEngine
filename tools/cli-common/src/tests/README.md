# CLI common unit tests

This directory contains the in-crate test module for structured JSON output, exit codes, clap failures, tracing precedence, secret redaction, panic reporting, publication modes, alias rejection, and compare-if-changed behavior.

These tests pin private failure branches and process-state helpers that the public integration suite cannot safely invoke, including writer errors and publication staging recovery; helper binaries consume the guarantees through `cli-common` rather than depending on this module.

Expected records and invariants are described in `tools/cli-common/README.md`, with test ownership and placement following the crate-local testing rules in `CLAUDE.md`.
