# ember marketing plan

Goal: make the repo a big, public thing that people interact with — stars, forks, issues, PRs, players. The engine is the product, the games are the demo, the docs are the proof.

## KPIs and baseline

Measured 2026-09-04 with `gh repo view`: 1 star, 0 forks, 0 watchers, 2 issues, 0 releases, wiki off, description empty. The baseline row in `LOG.md` is the record; every session re-measures and appends. Thirty-day targets: 25 stars, 3 forks, 10 issues of any kind, 1 merged external PR, a release per arena version, the wiki live, and a post on every channel in `posts/`.

## Phase 0 — inventory

The finished inventory is recorded in [the pending changelog](../CHANGELOG.md#documentation-and-marketing).

## Phase 1 — presence

The finished public-presence work is recorded in [the pending changelog](../CHANGELOG.md#documentation-and-marketing).

## Phase 2 — distribution (human action, drafts ready)

A session cannot post to social channels without an account, so the drafts in `posts/` are the deliverable until a human pastes them. Order: (1) the release, wiki, description and engine page — already done by the machine; (2) Show HN, the single best reach for a project like this, posted 09:00–11:00 US Eastern with the author answering the first replies; (3) r/rust the same day under its own engineering framing, r/webassembly the next day; (4) r/gamedev and r/indiegaming with a short clip; (5) the X/Bluesky thread on HN day, LinkedIn the same week. Rules: one account per person, never cross-post identical text verbatim, answer every comment, pin the hub link.

## Phase 3 — sustain (the loop, weekly)

- Monday: re-measure the KPIs, append `LOG.md`, pick the top board task.
- One deep post per week drawn from `docs/` — the address book (`hosts.md`), the asset pipeline, the determinism story, the os-error-997 hunt — the docs are already written; the post is a retelling.
- Triage every issue within a day; label the small backlog lines `good first issue` (the one-line `Obj::from_kind` editor fix, the `publish-host.sh` mirror-mode guard, the `deploy-pong-online.sh` port-comment drift).
- A release per arena version, notes taken from the README's section.
- The heartbeat keeps the lane honest: an idle marker in `heartbeat.log` means the last session ended without closing the board, and the next session starts by asking why.

## Constraints, stated once

- No account means no post. The drafts are the deliverable until a human pastes them.
- Wikipedia: a promotional article written by the developer violates WP:NOTADVERTISING and is deleted, not improved; the draft waits for independent coverage (`wiki/` explains the policy and the template).
- One engine per repo: nothing promotional may pull in a second toolchain or a second runtime; the hub is the site, `web/engine.html` is the developer page.
- The working tree on `lane/arena-v18` carried uncommitted work when this work began; every marketing commit is additive and none touches that tree.
