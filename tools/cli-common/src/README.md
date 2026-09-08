# CLI common source

This source directory implements the shared process contract for Ember command-line helpers: `lib.rs` owns exit classes, structured control-plane records, argument parsing, redaction, tracing setup, runners, stamps, and output identity checks, while `publication.rs` stages compare-if-changed multi-file publications with explicit permission modes.

Downstream workspace helpers enter through public types and functions such as `BaseArgs`, `CommandExit`, `parse_args_or_exit`, `run_stdout_json_command`, `run_check_command`, `finish_check_command`, `publish_batch`, and `emit`; the private `tests` module exercises branches that cannot be reached through the external API alone.

The complete behavioral and output contract is documented in the crate's `README.md` and derives from ADR-010, while publication atomicity and recovery limits are recorded beside the implementation in `publication.rs`.
