# SPDX Header Policy · `rec:spdx:header-policy`

**Status:** Decided

## Context · `sec:spdx:context`

Licence-header conformance depends on repository choices that the recognizer cannot derive: the identifier text, the copyright text, and the files to which each applies. Those choices remain with the header judgment and its toleration. Catalog contents, worked examples, census evidence, and the campaign diary remain outside this record.

## Decision · `sec:spdx:decision`

**Decision (A governed file carries the declared header in raw bytes)** · `dec:spdx:exact-header`

The checker uses the file's final path component to select a code-owned catalog entry. The entry gives a line-comment leader and any documentation-comment prefixes that do not participate. After an optional first-line shebang, the raw header region is the maximal leading run of lines beginning with that leader and none of its excluded prefixes.

The identifier half is satisfied by exactly one `SPDX-License-Identifier` line carrying its selected text. The copyright half is satisfied by at least one `SPDX-FileCopyrightText` line carrying its selected text; other copyright lines are permitted. Each required line is byte-exact: leader, one space, field name and colon, one space, selected text, and nothing else. The fields have no required order within the region.

**Decision (Header carriers are adopted explicitly)** · `dec:spdx:opt-in-carriers`

The catalog says how a carrier can hold a header; configuration says whether an owner adopts that carrier. The accepted catalog may therefore be much larger than the set a repository governs. Adding a catalog entry makes a carrier available but changes no obligation until an owner opts it in through its rows. This polarity is deliberate: the structured-envelope policy is closed over its catalogued types and guards that domain against drift (`dec:envelope:closed-domain`), while the licence-header policy is open to catalog growth and keeps adoption explicit.

**Decision (An owner section assigns each header half over its own share)** · `dec:spdx:owner-parameters`

Policy parameters are named identifier texts and named copyright texts. Each active owner section has an identifier half and a copyright half. An include row in either half names the applicable set entry, and that entry supplies the required bytes. These includes are load-bearing: changing the name changes the header requirement even when pattern coverage is unchanged.

Each half receives only its owner's share (`dec:rows:owner-input`), subtracts its exclusions, and partitions the remainder with its inclusions (`dec:rows:subtract-then-partition`). The uniform Rust-only configuration may therefore use one `non-rust` exclusion with a simple does-not-end-in-`.rs` pattern and one wildcard inclusion naming the chosen set. The wildcard sees only the owner's post-exclusion share, so no row enumerates owner directories.

**Decision (One file-level debt row tolerates every failure)** · `dec:spdx:uniform-toleration`

An activated owner carries one file-level path-set list for the policy. A path is present while the file fails any governing half and leaves only when every governing half is satisfied under ADR-T-020, The migration disciplines.

The row tolerates failure uniformly. A required line with wrong bytes is absent for toleration exactly as a missing line is absent; neither conforms, and either fails immediately when its path has no row.

## Consequences · `sec:spdx:consequences`

Catalog growth remains harmless until configuration adopts it. Parameter choice stays visible in each owner section, while broad uniform patterns cannot escape the share to which they are offered.

File-level debt keeps one list readable but cannot record a half-repair. Wrong headers may be drained through the same mechanism as missing headers without weakening the clean-state judgment.

Git commits `8244bc96` and `aad48713` record declaration followed by drain completion; this record retains no campaign narrative.
