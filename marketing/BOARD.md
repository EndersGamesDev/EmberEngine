# the barza board

How this board works: the repo is the medium (`docs/worker-protocol.md`). A session — human or agent — that picks up marketing work reads `MEMORY.md`, then this board, does the top open task, appends `LOG.md`, and updates the board's state before it ends. A task that cannot be finished gets a "next concrete step", not a direction.

## Open tasks (top = next)

| # | Task | Owner | State | Next concrete step |
|---|---|---|---|---|
| 1 | ~~**Merge decision**~~ — branch landed on `main` by direct push, owner-approved (the org blocks PR creation for enderPeer, so a PR was impossible from this account; recorded as an exception to the branch-only rule) | Ender/barza | done 21:2x | `b595d60..eebec28` — included a merge of `origin/main` (arena v21 landed while the branch was in flight; `deploy-pages.sh` auto-merged, both the v21 live dir and the engine.html guard verified present) |
| 2 | ~~Verify `engine.html` is live~~ | barza | done 21:1x | HTTP 200 at https://endersgamesdev.github.io/EmberEngine/engine.html after CDN propagation (the 21:09 404 was propagation, not a miss) |
| 3 | Create release **v22** on main | barza | open | tag `v22` at d85a40f + release notes (v22: outdoor sun, shadows, moving clouds and rain; script-input parser fix); `gh release create v22` from a clean tree |
| 4 | Seed the GitHub wiki | barza | open | retried at 22:0x, still 404 (~15 min after enabling); retry `git clone https://github.com/EndersGamesDev/EmberEngine.wiki.git` each session; when it clones, push `marketing/wiki/github-wiki-home.md` as `Home.md` |
| 5 | Paste the Show HN post | Ender | open | `marketing/posts/show-hn.md`, 09:00–11:00 US Eastern; the author account answers the first replies; pin the hub link |
| 6 | Record a 20 s arena clip | Ender | open | one loot bonk + one headshot + the new sky (v22 page); needed for r/gamedev and the YouTube cold open |
| 7 | Paste r/rust, then r/webassembly | Ender | open | r/rust the same day as HN (different framing, the drafts differ), r/webassembly the next day |
| 8 | Paste r/gamedev with the clip | Ender | open | after task 6; r/indiegaming the same week |
| 9 | Triage: answer every issue within a day | barza | recurring — first pass done 22:0x | reactions posted to #1 and #2 (gate pressure on the 4D corpus; rollback re-run clause + protocol-bump order on authority-and-time); re-run `gh issue list` each session |
| 10 | Weekly: re-measure the KPIs, append `LOG.md` | barza | recurring — measured 22:0x | stars=1 forks=0 watchers=0 issues=2; `gh repo view ... --json stargazerCount,forkCount,watchers,issues` |
| 11 | Deep post: "how we did the address book" from `docs/hosts.md` | barza | queued | after the first week of HN comments — use the questions asked there as the outline |

## Notes for the next worker

- **The merge wall is deliberate.** The org `EndersGamesDev` blocks `createPullRequest` for enderPeer (reproduced two ways, including with an empty probe branch), while direct pushes are allowed and `main` is unprotected. Do not treat the wall as a bug to route around on your own authority: it is the owner's setup, and the owner is the user in this loop. Task 1 is the only path.
- Deploys run from the clean worktree `C:\Users\end\dev\ember-host` (session 2). The `ember-arena-host` task wrapper builds there with the warm shared cargo target (`C:\Users\end\dev\ember\target`) at `/LOW`; before any deploy, `git fetch && git reset --hard origin/main` in that worktree. The wrapper may be temporarily re-pointed at another agent's worktree by that agent's release flow (that is how codex's v22 reached the host); after integration it is re-pointed at ember-host. Backups: `~/.ember/arena-local/ember-arena-host.cmd.bak-barza` (original, lane tree) and `.bak-v22` (codex era).
- Idle `heartbeat: …` commits on main are the quiet, not a malfunction (the workflow). `marketing/heartbeat.local.log` is this machine's record; `marketing/heartbeat.log` is the public one.
- Known-good test command on this host: `cargo test --workspace --exclude linter --no-fail-fast` (the vendored linter's test targets do not build on Windows; see the backlog).
- Release `v20` tags main's HEAD as of 2026-09-04 (46ed42b). v22 (d85a40f, skies and weather) is on main but untagged — that is task 3.
- The wiki repo (once it materializes) is a separate git repo: `https://github.com/EndersGamesDev/EmberEngine.wiki.git`.
- The barza board (the inter-agent channel) is the live service at `127.0.0.1:8901` behind a trycloudflare tunnel; `barza-up.ps1` brings it up, `barza-watchdog` (scheduled task) keeps it alive. If the board is down, fix it before posting — a message that cannot be read is not coordination.

## Agent briefs (ready to paste when help is wanted)

- **Netcode reviewer**: "Read `docs/hosts.md`, `crates/arena-core/src/proto.rs` and `crates/arena-core/src/shooter.rs` in EndersGamesDev/EmberEngine. Find the first thing you would not ship in the lockstep + host-picking design. Report findings with file:line; do not edit. Return the three sharpest findings."
- **Docs reteller**: "Read `docs/asset-pipeline.md` in EndersGamesDev/EmberEngine. Write a 600-word post for a game-dev audience about how a map's pictures and meshes are generated offline. No repo links except the doc itself. Return the draft."
- **First-issue picker**: "Read `docs/plans/backlog.md` in EndersGamesDev/EmberEngine. Pick the three smallest safe gaps (one-line fixes, no protocol), and draft one issue each from the bug/feature template, labelled `good first issue`. Do not fix them; return the three drafts."

## Board log

- 2026-09-04 20:4x: board created by barza; phase 1 presence work done on branch `marketing/launch-kit` (pushed); the loop machinery installed (scheduled task `EmberEngineHeartbeat` hourly; the `heartbeat` workflow lands with the branch); release v20, description, topics, gh-pages engine page done.
- 2026-09-04 21:13: session 1 closed. Top of board = the merge decision (task 1), which needs Ender.
- 2026-09-04 21:1x: engine page verified live (HTTP 200); task 2 closed. The wiki repo is still not materialized; task 3 stays open.
- 2026-09-04 21:2x: task 1 closed — the owner approved a direct push to main (`b595d60..eebec28`). Main had moved first (arena v21), so the branch merged it; `deploy-pages.sh` auto-merged with both changes verified. The `heartbeat` workflow is now on main; a manual run (33911297701) was triggered to prove it executes.
- 2026-09-04 22:1x: session 2 (the superloop with codex over the barza board). Barza service + tunnel recovered (seq 51; watchdog handed to its scheduled task). Host redeployed: the book now names r1265/d85a40f, which is exactly main after codex's v22 merge. The deploy design landed: clean `ember-host` worktree + wrapper at `/LOW` with the warm shared target. Triage done: reactions on #1 and #2. KPIs measured. The v22 pages deploy runs from the worktree. New top task: release v22 (task 3). Wiki still 404.
