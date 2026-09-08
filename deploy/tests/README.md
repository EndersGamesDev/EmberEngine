# Deployment tests

This directory contains shell suites for Pages assembly, host lifecycle, publishing, shipping, watchdog decisions, syntax, changelog consistency, folder README coverage and Julibrot shader-template enforcement.

Deployment scripts depend on these tests to prove behavior without contacting a real host or network; `run.sh` selects suites, `lib.sh` supplies assertions, and `shims/` replaces external programs where needed.

Each suite documents its invocation and isolation contract in its header; follow the timing and verification rules in [`CLAUDE.md`](../../CLAUDE.md), and keep a changed deploy contract paired with the focused suite that demonstrates it.

`test-shaders.sh` reads `julibrot-shader-allowlist.txt`, rejects new `.wgsl` files and inline WGSL under the Julibrot crates, and permits only the closed migration debt linked to [`../../docs/julibrot/shaders.md`](../../docs/julibrot/shaders.md). Test fixtures do not bypass that boundary; WGSL used by shader-crate tests lives in its test-only template registry.

The same check reads `julibrot-shader-validation-tests.txt` and requires every production template to name its runtime registry constant, production-context renderer and native Naga validation test. A template without that pairing fails before it can enter the wasm bundle.
