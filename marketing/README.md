# marketing

The home of this engine's promotion: the plan, the log, the board, the launch material, and the heartbeat that keeps the work moving between sessions. The repo is the medium — anything one worker needs another to know goes here, not in a chat transcript (`docs/worker-protocol.md`).

| File | What it is |
|---|---|
| `PLAN.md` | The detailed work plan: phases, KPIs, cadence. Read it first when resuming. |
| `LOG.md` | The work log: what was done, when, with what evidence. Append-only; never rewrite history. |
| `BOARD.md` | The barza board: open tasks, notes for the next worker, briefs ready to paste for other agents. |
| `MEMORY.md` | Persistent memory: repo facts, access, decisions, what is deliberately not done. |
| `HEARTBEAT.md` | The heartbeat protocol: the hourly check that work is moving. |
| `heartbeat.ps1` | The local watcher. Run hourly by the `EmberEngineHeartbeat` scheduled task; appends to `heartbeat.local.log` (gitignored). |
| `heartbeat.log` | The committed heartbeat: the `heartbeat` Actions workflow appends a marker when the lane is quiet for an hour. |
| `posts/` | Launch posts per channel, ready to paste. Posting needs a human account; the drafts are the deliverable. |
| `wiki/` | A neutral encyclopedic article draft for later, with a policy note on when it is safe to publish. |
