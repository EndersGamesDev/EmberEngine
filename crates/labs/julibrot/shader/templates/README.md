# Julibrot shader templates

Production templates in this directory are embedded into `ember-julibrot-shader` with `include_str!` and compiled by Minijinja at runtime in the wasm context.

The interface and oracle fixtures exercise Rust-owned declarations, naga-computed layouts and line-bearing diagnostics through the production `render` path, but their sources and environment registrations exist only in test builds. Julibrot's production templates join the production registry as their shaders migrate.
