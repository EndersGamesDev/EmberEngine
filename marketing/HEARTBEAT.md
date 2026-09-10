# Heartbeat

The committed hourly heartbeat is retired. Integration now moves through pull requests to `develop`, where CI records the gate result, so idle marker commits no longer provide a useful integration signal.

`marketing/heartbeat.log` is a closed historical record. Its final line records why collection ended.

## Local watcher

`heartbeat.ps1` and the workstation scheduled task `EmberEngineHeartbeat` are unaffected. They append ACTIVE or IDLE state, the last commit, and the dirty-file count to the untracked `marketing/heartbeat.local.log` as a machine-local continuity record.

The local record is not an integration gate and does not create repository commits. `BOARD.md` remains the source for what work is next.

## Resume protocol

1. `git log -3` and the last lines of `marketing/heartbeat.log` and `marketing/heartbeat.local.log`.
2. Read `BOARD.md` top to bottom; pick the top open task.
3. Do it, append `LOG.md`, update the board's state, commit.
4. If nothing is open, that is the finding: write it on the board and pick from `PLAN.md` phase 3.
