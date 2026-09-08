# Deployment tests

This directory contains shell suites for Pages assembly, host lifecycle, publishing, shipping, watchdog decisions, syntax, changelog consistency and folder README coverage.

Deployment scripts depend on these tests to prove behavior without contacting a real host or network; `run.sh` selects suites, `lib.sh` supplies assertions, and `shims/` replaces external programs where needed.

Each suite documents its invocation and isolation contract in its header; follow the timing and verification rules in [`CLAUDE.md`](../../CLAUDE.md), and keep a changed deploy contract paired with the focused suite that demonstrates it.
