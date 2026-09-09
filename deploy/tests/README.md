# Deployment tests

This directory contains shell suites for Pages assembly, host lifecycle, publishing, shipping, watchdog decisions, syntax, changelog consistency, folder README coverage and Julibrot shader-template enforcement.

Deployment scripts depend on these tests to prove behavior without contacting a real host or network; `run.sh` selects suites, `lib.sh` supplies assertions, and `shims/` replaces external programs where needed.

Each suite documents its invocation and isolation contract in its header; follow the timing and verification rules in [`CLAUDE.md`](../../CLAUDE.md), and keep a changed deploy contract paired with the focused suite that demonstrates it.

`test-shaders.sh` reads `julibrot-shader-allowlist.txt`, rejects new `.wgsl` files and inline WGSL under the Julibrot crates, and permits only the closed migration debt linked to [`../../docs/julibrot/shaders.md`](../../docs/julibrot/shaders.md). Test fixtures do not bypass that boundary; WGSL used by shader-crate tests lives in its test-only template registry.

The same check reads `julibrot-shader-validation-tests.txt` and requires every production template to name its runtime registry constant and production-context renderer. From those records it derives the deterministic native Naga validation-test name, binds its sole source occurrence to the exact absolute-path macro call, and uses locked Cargo metadata to authenticate the workspace shader dependency behind that extern name. `ember_julibrot_shader` is a reserved extern-prelude name in every template-owning crate; source may not rebind it with an `extern crate … as …` declaration. An owning crate may not define its own `macro_rules! production_template_test` macro, regardless of attributes. The checker verifies that `cargo test --list` finds the test in the owning crate's compiled test binary, then runs it normally and in a nonce-bearing mode that must report the Naga diagnostic naming the deliberately unresolved probe identifier. A template without both passing receipts fails before it can enter the wasm bundle.
