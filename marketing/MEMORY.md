# memory

Persistent memory for any session — human or agent — that continues this work. Read it before the board.

## Identity

This work was started by the agent **barza** on 2026-09-04, on the standing order to promote the engine: a big, public repo that people interact with, not a vanity page. The loop runs until the owner says stop.

## Repo facts (verified 2026-09-04)

- `origin` = `https://github.com/EndersGamesDev/EmberEngine`, public, default branch `main`. Other remotes exist (`enderpeer-old`, `wildsky`) for other purposes; marketing pushes only to `origin`.
- The local working tree sits on `lane/arena-v18` and had uncommitted changes (README, arena crates, the backlog, the v20 page) when marketing started. Never commit from that tree; work in a worktree off `origin/main`.
- The games hub is `web/index.html`, deployed to `gh-pages` by `deploy/deploy-pages.sh`, which copies a whitelist of files; `engine.html` has its own guarded copy line.
- `gh` on this Windows host is authenticated as `enderPeer` with ADMIN on the repo. `docs/worker-protocol.md` says gh belongs in WSL; on this box it works on Windows, and that is the current fact.
- The live hub is `https://endersgamesdev.github.io/EmberEngine/` — the only link any post may carry, because a tunnel domain changes on every restart and the hub is the stable front door.
- Commit style: lowercase, `area: description`, one topic per commit. The repo is the medium (`docs/worker-protocol.md`).
- Known-good test command on this host: `cargo test --workspace --exclude linter --no-fail-fast` (the vendored linter's test targets do not build on Windows; see the backlog).

## Access

- `gh` as `enderPeer`, ADMIN: enough for description, topics, wiki, releases, PRs, workflows.
- No social accounts (HN, Reddit, X, Bluesky, LinkedIn, YouTube) exist in this environment. Distribution is a human action; the drafts in `posts/` are ready.
- Wikipedia: no account, and none should be created for this; see the policy note in `wiki/`.

## Installed machinery

- The `heartbeat` workflow (`.github/workflows/heartbeat.yml`): cron `7 * * * *` on the default branch; when no commit landed in the last hour it appends `marketing/heartbeat.log` and commits the marker. Idle `heartbeat: …` commits in the log are the quiet, not a malfunction.
- The Windows scheduled task `EmberEngineHeartbeat`, hourly: runs `marketing/heartbeat.ps1`, appends to `marketing/heartbeat.local.log` (gitignored), recording ACTIVE/IDLE against the last commit plus the dirty-file count.
- The GitHub wiki, enabled and seeded with a neutral Home page (repo: `https://github.com/EndersGamesDev/EmberEngine.wiki.git`).
- To stop the loop: `schtasks /Delete /TN EmberEngineHeartbeat /F`, delete the workflow file, and write a final board entry saying the loop is closed and when.

## Decisions

- 2026-09-04: the landing page is `web/engine.html` beside the hub, not a second site — one engine per repo, one deploy path.
- 2026-09-04: `engine.html` was committed to `gh-pages` as well as main, so it is live before the next full deploy; the deploy script now carries it in the official path.
- 2026-09-04: releases follow arena versions; the first is `v20`.
- 2026-09-04: the Wikipedia article waits for independent coverage; the GitHub wiki carries the encyclopedic content in the meantime.

## Deliberately not done

- No social posting (no account), no Wikipedia submission (policy), no edit to the root `README.md` (it was dirty in the working tree, and the hub link already sits on its third line), no funding file (no account), no new toolchain of any kind.
