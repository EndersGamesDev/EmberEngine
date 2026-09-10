# Pull requests into `develop`

This runbook is the landing contract for every change integrated on `develop`. It keeps the authored commits, mechanical evidence, review conclusions and GitHub-created integration commit connected in one pull request while `main` remains reserved for release promotion.

## Prepare the candidate

A candidate lives on a `feature/**` or `lane/**` branch. Those patterns are the only creatable branch names apart from `develop` and `main`, so each candidate has a bounded purpose without creating another integration or release line.

Each commit contains one topic and follows [`commit-messages.md`](commit-messages.md). A focused history makes every change independently reviewable and lets a rebased candidate repeat its evidence commit by commit.

The candidate branch is pushed early, while it is still in progress, so its existence and current source are visible before integration begins. Candidate branches are temporary integration inputs rather than permanent history, so amended or rebased commits are pushed with force-with-lease when their identities change.

Every authored commit passes the shell suites named for its changed surfaces before the pull request opens. Documentation and integration-process changes normally include `bash deploy/tests/test-readmes.sh`, `bash deploy/tests/test-done-lists.sh`, `bash deploy/tests/test-changelog.sh`, `bash deploy/tests/test-commit-messages.sh`, `bash deploy/tests/test-rulesets.sh`, `bash deploy/tests/test-workflows.sh` and `bash deploy/tests/run.sh syntax`; another changed surface adds its own applicable suites.

Code changes also pass the workspace formatting, lint and test suites represented by `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked`, `cargo test --workspace --locked --exclude linter --exclude ember-julibrot-present` and `cargo test -p ember-julibrot-present -- --test-threads=1`. These local or pod results establish the candidate before GitHub repeats the required checks on the proposed merge tree.

Every gate record names the command, the tested commit, its pass and fail counts, and its wall time. An unavailable or omitted gate remains explicitly unverified rather than being inferred from inspection or from a different suite.

Adversarial review happens after the initial gates and before the pull request opens, because mechanical checks and hostile reading cover different failure classes. Every finding is resolved on the candidate branch, and any gate affected by a resolution runs again before the branch becomes the proposed integration tree.

## Open the pull request

The pull request targets `develop` and is opened with `gh pr create`; the explicit base keeps the integration destination visible in the command and in GitHub's record.

```bash
gh pr create --base develop --head BRANCH --title 'TYPE(SCOPE): SUMMARY' --body-file BODY_FILE
```

The title follows the merge-subject rules in [`commit-messages.md`](commit-messages.md) because GitHub uses it as the default merge subject. It is a Conventional Commit subject for the integrated change as a whole, not a `merge`-typed subject.

The body contains exactly the following headed sections in this order:

```markdown
## What changed and why

SUMMARY_AND_REASONING

## What was verified

SUITES_COUNTS_AND_WALL_TIMES

## What was not verified

OMISSIONS_AND_REASONS

## Adversarial review record

FINDINGS_AND_RESOLUTIONS_OR_NO_FINDINGS

## What was deliberately not done

EXPLICIT_BOUNDARIES

## Pending changelog source commit

SOURCE_COMMIT_CITED_BY_THE_PENDING_PARAGRAPH
```

“What changed and why” describes the observable or contractual result and the reason for its shape. “What was verified” lists each suite with its pass and fail counts and wall time, while “What was not verified” separately names every omitted suite or environment and the reason it remains unverified.

“Adversarial review record” lists each finding and the branch change or evidence that resolved it. A completed review with no findings states `NO FINDINGS` as the reviewer's outcome, preserving an explicit non-mechanical review record for every candidate.

“What was deliberately not done” fixes the boundary between the proposed change and adjacent work. “Pending changelog source commit” names the authored commit cited by the pending changelog paragraph, because that source exists before the eventual integration merge commit.

## Pass the required checks

The live `develop` ruleset requires the strict GitHub Actions contexts `cores + servers`, `deploy scripts` and `workspace`, all bound to integration id `15368`. No bypass actor exists, so a candidate reaches integration only through the same recorded checks as every other pull request.

`cores + servers` tests the four shared game cores and four server packages, then builds the host probe examples. `deploy scripts` runs the deployment syntax, workflow, ruleset, host, Pages, SSH-deploy and watchdog contract suites and, on pull requests, the repository README, done-list, changelog and commit-message policy suites.

`workspace` checks formatting, runs locked all-target workspace Clippy, tests the workspace with `linter` and `ember-julibrot-present` excluded, and then tests only `ember-julibrot-present` separately with one test thread; it does not test `linter`. Its complete runner setup and gate have a roughly twenty-minute budget, so a pending result is expected evidence in progress rather than a reason to bypass the check.

Strict up-to-date checking means a green result belongs only to a candidate based on the current `develop`. When `develop` moves, the candidate is rebased onto its new tip, force-pushed with force-with-lease to its own feature or lane branch, and gated again commit by commit; GitHub then runs all three required contexts on the new proposed merge tree.

The pending changelog paragraph lives in its own later commit because a rebase changes every rewritten source identity. After every rebase, that later commit replaces the cited id with the rebased source commit's id, an open pull-request body receives the same correction through `gh pr edit NUMBER --body-file BODY_FILE`, the affected gates run again, and the corrected stack is force-pushed with force-with-lease; the cited source commit remains untouched by the citation repair.

A reconciliation merge from `develop` into the candidate branch is not part of this process. Rebasing preserves the authored topic commits, while the GitHub-created merge commit remains the single record that integrates the candidate with `develop`.

## Merge the pull request

After all three required contexts are green on an up-to-date candidate, the pull request is merged with the merge-commit method and an explicit subject and body.

After the final candidate push, the reviewed head identity is recorded locally and compared with GitHub's current pull-request head. Equality proves that the gated and reviewed local commit is still the proposed head before the merge attempt begins.

```bash
REVIEWED_HEAD="$(git rev-parse HEAD)"
test "$REVIEWED_HEAD" = "$(gh pr view NUMBER --json headRefOid --jq .headRefOid)"
```

The merge request carries the reviewed identity through `--match-head-commit`, so GitHub refuses a head moved by any push between the comparison and the server-side merge.

```bash
gh pr merge NUMBER --merge --match-head-commit "$REVIEWED_HEAD" --author-email AUTHOR_EMAIL --subject 'TYPE(SCOPE): SUMMARY' --body-file MERGE_BODY_FILE
```

The merge subject follows [`commit-messages.md`](commit-messages.md), and the merge body describes the integrated change as a whole, explains why it has that shape, and states explicitly what was and was not verified. Required approvals remain at zero because CI is the mechanical merge gate and the adversarial reasoning is already recorded in the pull-request body.

Administrative override is not part of the merge path. GitHub creates the merge commit with the selected author identity after the ruleset accepts it; that server-created commit is not signed by the integrator's key, and release provenance instead rests on the signed annotated tag that later selects an integrated commit.

A refused merge is resolved by its recorded cause. An out-of-date candidate returns to the rebase and per-commit gates, a red check returns to a fix on the candidate branch and a fresh check run, and a moved head returns to identity recording, affected gates and review; a repository rule remains enabled because removing the evidence boundary would not resolve the candidate defect.

## Inspect and clean up after the merge

The hosted `develop` is fetched only into a quarantine ref before it is used for local integration, release work or any downstream action. Suppressing tag following and `FETCH_HEAD` writes confines the fetch to that ref, while querying the merged pull request identifies its own merge commit even when a later pull request has already advanced `develop`.

```bash
git fetch --no-tags --no-write-fetch-head origin refs/heads/develop:refs/quarantine/develop
MERGE_OID="$(gh pr view NUMBER --json mergeCommit --jq .mergeCommit.oid)"
git merge-base --is-ancestor "$MERGE_OID" refs/quarantine/develop
test "$(git rev-parse "$MERGE_OID^2")" = "$REVIEWED_HEAD"
git merge-base --is-ancestor "$REVIEWED_HEAD" "$MERGE_OID"
git show --no-patch --format=fuller "$MERGE_OID"
git diff --stat "$MERGE_OID^1" "$MERGE_OID"
git diff --check "$MERGE_OID^1" "$MERGE_OID"
git diff "$MERGE_OID^1" "$MERGE_OID"
```

The first containment test proves that the selected merge commit is present in quarantined `develop`, the second-parent equality binds that merge to the reviewed head, and the reviewed-head containment test guards cleanup. Every subsequent inspection names `MERGE_OID`, so a newer `develop` tip cannot substitute another pull request's merge record.

Cleanup covers every remote to which the candidate branch was pushed, including its GitHub remote and each mirror, because those are the complete locations where its rewritable ref can remain. The remote deletion line repeats for each such remote after containment succeeds; a linked worktree is removed before its local branch ref because Git protects a checked-out branch from deletion.

```bash
git merge-base --is-ancestor "$REVIEWED_HEAD" "$MERGE_OID"
git push REMOTE --delete BRANCH
git worktree remove WORKTREE
git branch -D BRANCH
```

Removing the contained candidate from every target prevents stale refs from being mistaken for continuing work while the permanent authored commits remain beneath the merge on `develop`.

Local `main` does not move with the integration merge. It follows hosted `main`, which advances only when the release workflow promotes the commit selected by a conforming release tag.

The pending changelog paragraph cites the final rebased source commit named in the pull-request body. Its separate later commit lets every rebase replace the citation without touching the cited source, so the recorded identity remains beneath GitHub's eventual integration merge.

## Keep participation on the pull request

Review comments, findings, resolutions and hand-offs live on the pull request because it is the shared coordination record for the candidate. [`worker-protocol.md`](worker-protocol.md#2-branch-and-pr-shape) records the environment boundary: where `gh` is unavailable to a worker, including the named Windows case, posting the comment remains a human action and the worker states that limitation explicitly.

## Pre-merge checklist

- [ ] The candidate uses a `feature/**` or `lane/**` branch, contains one topic per conforming commit, and was pushed early.
- [ ] Every applicable per-commit shell suite passed with counts and wall times, and code changes also passed the workspace suites.
- [ ] Adversarial review completed before the pull request, and its record lists every resolved finding or states `NO FINDINGS` as the reviewer's outcome.
- [ ] The pull request targets `develop`, its title is a valid merge subject, and its body contains all six contract sections in order.
- [ ] The branch is based on the current `develop`, with no reconciliation merge; any rebase repaired the later changelog citation and pull-request body before the affected gates ran again.
- [ ] `cores + servers`, `deploy scripts` and `workspace` are green on the current tree, and `REVIEWED_HEAD` equals GitHub's `headRefOid` after the final push.
- [ ] The merge uses `gh pr merge --merge --match-head-commit`, a conforming merge subject and body, the selected author identity, no required approval and no administrative override.
- [ ] After merging, the exact `MERGE_OID` is contained in quarantined `develop`, its second parent is `REVIEWED_HEAD`, the contained candidate is removed from every pushed remote and its local worktree and branch, and local `main` remains aligned with hosted `main`.
- [ ] Review comments and hand-offs are present on the pull request, or a worker without `gh` has identified the required human action.
