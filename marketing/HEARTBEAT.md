# heartbeat

The question the heartbeat answers, every hour: **is the work moving?**

## The two watchers

| Watcher | Where it runs | What it writes | When it acts |
|---|---|---|---|
| the `heartbeat` workflow | GitHub Actions, cron `7 * * * *`, default branch | `marketing/heartbeat.log` plus a marker commit | only when no commit landed in the last hour |
| `heartbeat.ps1` via the scheduled task `EmberEngineHeartbeat` | this workstation, hourly | `marketing/heartbeat.local.log` (never committed) | always: records ACTIVE/IDLE, the last commit, the dirty-file count |

The committed log is the public record; the local log is the machine's record. Neither is a substitute for the board: the heartbeat says whether work moved, the board says what work is next.

## What counts as work

A commit, a PR action, a merge, a release, a published post, an issue answered, a board update. A session that ends without any of those leaves an idle marker behind, and the next session starts by reading the board and asking why.

## Resume protocol (any session)

1. `git log -3` and the last lines of `marketing/heartbeat.log` and `marketing/heartbeat.local.log`.
2. Read `BOARD.md` top to bottom; pick the top open task.
3. Do it, append `LOG.md`, update the board's state, commit.
4. If nothing is open, that is the finding: write it on the board and pick from `PLAN.md` phase 3.

## Shutdown

The owner says stop. Then: `schtasks /Delete /TN EmberEngineHeartbeat /F`, delete `.github/workflows/heartbeat.yml`, and a final board entry stating the loop is closed and when.
