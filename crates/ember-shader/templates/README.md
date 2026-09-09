# Shared shader templates

Production templates in this directory are embedded into `ember-shader` and compiled by Minijinja at runtime in each consuming pipeline's context.

Test fixtures exercise Rust-owned declarations and strict missing-name diagnostics through the same render path, but their sources and environment registrations exist only in test builds. Production templates join the production registry as repository shader migrations land.
