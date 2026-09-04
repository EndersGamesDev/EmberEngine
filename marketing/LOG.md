# work log

Append-only. Format: `date time (local) — what — evidence — wall time`.

## Baseline (measured 2026-09-04 20:41 local, `gh repo view`)

Repo public, 1 star, 0 forks, 0 watchers, 2 issues, 0 releases, wiki off, description empty, homepage = the hub. The live hub serves arena v20 / fire v2 / kings v1 / what-is-this v1. `gh` authenticated as enderPeer with ADMIN on the repo; default branch `main` at 46ed42b (the lane/arena-v18 merge, so main already carries the v20 client).

## 2026-09-04 — session 1 (barza, the marketing loop)

- 20:40 Session started on the standing order: a big, public repo people interact with.
- 20:46 Recon: repo layout, README, CLAUDE.md, worker protocol, the hub, `deploy-pages.sh` (a whitelist of copied files), the backlog (full of one-line first issues), the repo metadata. Main is current; the working tree on `lane/arena-v18` is dirty and was left untouched.
- 20:58 Worktree off `origin/main`, branch `marketing/launch-kit`: six one-topic commits — the `marketing/` infrastructure (plan, log, board, memory, heartbeat protocol + local watcher), the launch post drafts (HN, r/rust, r/gamedev, r/webassembly, X/Bluesky/LinkedIn, YouTube), the held Wikipedia-style draft with its policy note, `web/engine.html` plus the guarded deploy line, `CONTRIBUTING.md` + issue templates, and the `heartbeat` Actions workflow.
- 21:02 Branch pushed to origin. **PR creation is blocked for enderPeer by the org** — `createPullRequest` returns "does not have the correct permissions" and REST POST /pulls returns 404, reproduced with an empty probe branch; enderPeer is an org member with ADMIN on the repo, so this is a deliberate org restriction, not a token gap. The branch stays pushed; landing it on main needs Ender (see the board).
- 21:05 Wiki enabled via the API (`has_wiki: true`). The wiki git repo had not materialized by session end (clone 404); the ready-to-paste Home seed is `marketing/wiki/github-wiki-home.md`.
- 21:05 Repo description set and two topics added (`webassembly`, `deterministic-simulation`) — confirmed live on the repo page.
- 21:07 Release **v20** created at tag v20 on main's HEAD: <https://github.com/EndersGamesDev/EmberEngine/releases/tag/v20>.
- 21:08 `engine.html` committed to `gh-pages` (05ded8e) and pushed; Pages source is gh-pages and the build status read `built`. The URL 404'd at check time (CDN propagation); verifying is the first board task.
- 21:11 Local heartbeat installed: the scheduled task `EmberEngineHeartbeat` (hourly, next run 22:11, status Ready) runs the installed copy at `C:\Users\end\AppData\Local\Temp\opencode\ember-heartbeat\heartbeat.ps1`; test run appended `2026-09-04 21:11:20 status=IDLE last=6f8e424 dirty=18`.
- 21:12 Verification: every new file `i/lf w/lf` per `git ls-files --eol`, UTF-8 without BOM; `bash -n deploy/deploy-pages.sh` passes (bash from Git at `C:\Program Files\Git\bin\bash.exe`). Not verified: the workflow's first run, the issue templates in a browser, the engine page through a real browser.

**Session 1 wall time: 20:40:09 → 21:13 ≈ 33 min.**

## KPIs (2026-09-04 21:12)

1 star, 0 forks, 0 watchers, 2 issues, 1 release (v20), wiki enabled (unseeded), description set. The machine-side presence work is done; the next movement comes from the distribution phase (human posts) and the merge decision on the board.
