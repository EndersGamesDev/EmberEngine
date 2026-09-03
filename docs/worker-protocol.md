# Worker protocol

How several agents work this repo without a live channel between them, and without rediscovering the same constraints.

*Written because the alternative was tried: findings lived in chat transcripts, and the next worker — human or agent — could not read them.*

## 1. The repo is the medium

**Anything one worker needs another to know goes in the repo.** Not in a chat transcript, not in a session that will be compacted, not in an agent's final report.

| What | Where |
|---|---|
| A constraint that will bite the next person | `CLAUDE.md` if every session needs it; a `docs/` page if only some do |
| A follow-up, a known gap | `docs/plans/backlog.md`, one line |
| Why a change is shaped the way it is | The commit message |
| A finding about work in flight | A comment on that work's PR |
| What was and was not verified | The commit message, explicitly |

The test for `CLAUDE.md` is narrow: it is loaded into **every** session in this repo, so it earns its tokens only if a session that never touches assets still needs the line. Depth belongs in a linked doc that gets read on demand.

## 2. Branch and PR shape

- Work lands on a branch, never directly on `main`.
- One topic per commit. Three independent changes are three commits, not one.
- The branch is pushed early, before it is finished, so other workers can see it exists.
- The PR is the coordination surface: questions, findings, and hand-offs are comments on it.

`gh` is the tool for this. It is not on the Windows host by policy and belongs in a WSL distro:

```bash
wsl -d claude-sdk -- bash -lc 'gh pr comment <n> --body "..."'
```

Until that exists, PR comments are a human action. Say so rather than silently skipping the hand-off.

## 3. What a hand-off must contain

A worker picking up someone else's branch needs, in the PR thread or the commit message:

1. **What is verified and what is not** — separately, in those words. "Reviewed by reading" is not "compiled".
2. **What blocked** — and whether the blocker is environmental (no toolchain) or a real design question.
3. **The next concrete step**, not a direction.
4. **What was deliberately not done**, so it is not mistaken for an oversight.

## 4. Verification honesty

This is the rule most worth keeping.

- Never report work as done because it looks done. Report what was run.
- An unbuildable environment does not lower the bar; it changes what you may claim. Write the code, review it hard, and say plainly that it has never been compiled.
- Adversarial review is a **substitute for**, not an equivalent of, a compiler. When both are available, **gate first, then spend the review on what execution cannot reach.** They fail in different directions, and the difference is predictable:

| Bug class | Found by |
|---|---|
| Tick order, and anything that only exists at runtime | **Execution**, in seconds. Systematically invisible to review — a reader checks the arithmetic of the new code and the expectations of the old tests, and never simulates the new tests against the loop's actual order. |
| Data shapes, orderings nothing in the repo exercises yet, silent-failure surfaces | **Reading**. No test finds them, because every input anyone has tried has the safe shape. |

Both classes cost a real bug here in one session. Gating first clears the mechanical class fast and cheaply; the review then earns its keep on the half a green test run says nothing about.

## 5. Delegation

Delegate work that needs **context you do not have**: reading files you have not read, sweeping a surface too wide for one pass, an independent adversarial opinion on something you wrote.

Do **not** delegate synthesis of what is already in your context — a subagent has to re-derive it from scratch, which costs more than doing it and produces something you then have to check.

Every dispatch states the model, the reasoning depth, and what the worker must read back before starting. A worker that begins without restating the brief will solve the wrong problem thoroughly.

## 6. External machines

`sokol` runs the live services and remote repository gates. It is reachable from an agent session only through an SSH alias backed by an account and credentials that a human placed there.

- Do not assume access. Probe, and report the result plainly.
- Never ask a peer session to perform an action your own session was denied. Route it back to the human.
- Never paste a credential into a chat, a commit, or a PR comment. Name where it should live and let the human put it there.

## 7. Environment facts belong in the machine's own rules

Toolchain policy, distro names, priority rules and heartbeat discipline are properties of the *machine*, not of ember, and live in the global `CLAUDE.md`. Repo docs should not restate them — they drift. Reference them.
