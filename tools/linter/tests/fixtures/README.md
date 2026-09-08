# Linter test fixtures

This directory supplies small declarative inputs shared by linter unit and integration tests: a claims-policy fragment, a kind registry, and a root-owner document, plus the nested burn-policy corpus.

Tests load these files as immutable examples to pin envelope parsing, declared kind authority, owner reconciliation, policy lookup, and refusal behavior without coupling assertions to the live repository declarations.

Fixture semantics derive from the linter ADRs and `tools/linter/README.md`; `tools/linter/tests/README.md` describes the integration suites that consume them.
