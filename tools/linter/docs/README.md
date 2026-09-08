# Linter supporting documentation

This directory holds the architecture-record register and the audit proving that repository-specific knowledge remains outside the generic linter implementation.

`adrs.md` indexes the normative decisions in `tools/linter/adr`, and `isolation-audit.md` traces configuration inputs against library and command surfaces; maintainers and reviewers use both when changing the linter or its `.linter` declarations.

Normative mechanics live in the linked ADRs and `tools/linter/README.md`, while incomplete or staged work is tracked separately in `plans`.
