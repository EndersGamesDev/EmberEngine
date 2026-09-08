# Deployment tests

This directory contains shell suites for Pages assembly, host lifecycle, publishing, shipping, watchdog decisions, syntax, changelog consistency, folder README coverage and Julibrot shader-template enforcement.

Deployment scripts depend on these tests to prove behavior without contacting a real host or network; `run.sh` selects suites, `lib.sh` supplies assertions, and `shims/` replaces external programs where needed.

Each suite documents its invocation and isolation contract in its header; follow the timing and verification rules in [`CLAUDE.md`](../../CLAUDE.md), and keep a changed deploy contract paired with the focused suite that demonstrates it.

`test-shaders.sh` reads `julibrot-shader-allowlist.txt`, rejects new `.wgsl` files and inline WGSL under the Julibrot crates, and permits only the closed migration debt linked to [`../../docs/julibrot/shaders.md`](../../docs/julibrot/shaders.md). The sole non-debt exception is the exact legacy shade fixture protected by `#[cfg(test)]`, which proves that the first rendered template preserves the former source bytes.
