# Shared shader templates

Production templates in this directory are embedded into `ember-shader` and compiled by Minijinja at runtime in each consuming pipeline's context.

Test fixtures exercise Rust-owned declarations, naga-computed layouts, emission provenance, exact rendered bytes, and line-bearing diagnostics through the same render path, but their sources and environment registrations exist only in test builds. Production templates join the production registry as repository shader migrations land.

`oracle-test.wgsl.jinja` emits the complete anti-drift registry, while `unregistered-type-test.wgsl.jinja` proves strict missing-name diagnostics.
