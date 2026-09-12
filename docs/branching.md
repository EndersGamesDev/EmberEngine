# Branching, integration and release

Ember integrates changes on `develop` through pull requests and reserves `main` for the source that is live. This separation makes CI evidence part of integration while keeping releases, Pages and hosts on the same tagged commit.

## Branch model

| Branch | Meaning | Writers | Update path |
|---|---|---|---|
| `feature/**` | A bounded human-authored change | The branch author | Pushes may be rebased, amended or replaced with force-with-lease. |
| `lane/**` | A bounded loop-authored change | The lane | Pushes may be rebased, amended or replaced with force-with-lease. |
| `develop` | The default integration line; every integrated change passed the required checks in a pull request | GitHub pull-request merges | Merge commits only; direct pushes and force pushes are blocked. |
| `main` | The live line; after its one-time migration anchor, it advances only to commits selected by valid release tags | The release workflow through its deploy key | Plain fast-forward pushes of tag target commits only. |

`ci-passed` no longer exists, while `gh-pages` remains frozen as the publication record because the changelog ledger cites publications by their `gh-pages` commit ids and stamps, `deploy/deploy-pages.sh` reads the branch as the read-only seed of the archived first web build, no archive name is creatable under live ruleset `22770318`, and no non-release tag is creatable under live ruleset `22795785`. Actions owns publication and nothing writes `gh-pages`, so a no-bypass update-and-deletion freeze constrains no active writer and prevents an accidental legacy publication from changing the live site while Pages still builds from that resolvable ref; the branch-name ruleset also refuses recreation of `ci-passed`.

`main` is always an ancestor of `develop`, so a release promotion cannot introduce a commit that was not integrated. Nothing is committed or merged directly on `main`, so the branch cannot diverge from `develop` and never needs a reconciliation merge.

The migration creates `main` once at the then-current tip of `develop`, even when that commit is newer than the last release tag. This one-time anchor prevents every host from downgrading to source that may be months old; every later update of `main` comes from the release workflow and points at a valid tag target.

## Pull requests into `develop`

Every change, including release-process changes, reaches `develop` through a pull request. Direct integration would separate the resulting commit from the CI and review record that establish why it is safe to land.

The fixed pull-request body contract and step-by-step landing procedure live in [`pull-requests.md`](pull-requests.md), keeping every candidate's gate and review record consistent with this branch policy.

Only merge commits are enabled; squash and rebase merges are disabled because they replace authored commits and flatten their reviewable history. The merge subject and body follow [`commit-messages.md`](commit-messages.md), preserving the authored commits beneath the integration record.

The pull-request branch is up to date with `develop` before merging. Strict status checks then prove the current candidate rather than an older tree, and the merge commit carries the same content as that gated candidate.

The required status-check contexts are the literal CI job names `cores + servers`, `deploy scripts` and `workspace`, and each is bound to GitHub Actions integration id `15368`. Required approvals start at `0`: CI is the mechanical pull-request gate, while the adversarial review and its conclusions are recorded in the pull-request body so the reasoning remains auditable without turning one human's availability into an integration lock.

Each merge to `develop` advances the patch grade of every series it changes. The pending changelog entry cites the source commit because the eventual pull-request merge commit does not exist while the candidate is being prepared.

## Release tags

The release namespace uses the ruleset ref pattern `refs/tags/*-[0-9]*.[0-9]*.[0-9]*`, while the workflow trigger omits the `refs/tags/` prefix and uses `*-[0-9]*.[0-9]*.[0-9]*`. GitHub evaluates these as fnmatch patterns, so they are deliberately coarser than the workflow's exact runtime regex; the workflow re-validates every candidate and fails unless the final version suffix has the exact numeric form `MAJOR.MINOR.0`, with the major and minor each either `0` or a non-zero digit followed by digits, and the prefix is `ember`, an exact game `id` in `web/games.json`, or an exact lab directory id under `web/labs/`. Bare versions and `v`-prefixed versions are invalid because every tag belongs unambiguously to one versioned series.

Release tags are annotated and signed. Their signatures verify against an armored public key committed under `deploy/keys/`, which makes the allowed signing material reviewable in the same history as the release policy.

A release tag points at a commit reachable from `origin/develop`; the newest completed `push` run for `develop` at that exact SHA whose workflow path is `.github/workflows/ci.yml` has concluded `success`, and its jobs are exactly `cores + servers`, `deploy scripts` and `workspace`, each with conclusion `success`; every dedicated manifest in the series mapping in `docs/versioning.md` declares the tag version, or the Ember workspace and every shared crate agree through workspace inheritance; and the changelog contains the accepted entry. A release entry authored on its own pull-request branch names its tag as pending, records the authored version commit as its source and uses `stamp —` so the absent tag does not fail that candidate's check. After GitHub creates the merge commit, the annotated, signed tag is cut on that commit, and release validation proves it is the exact two-parent merge that integrated the row: the first parent lacks the exact release line, the merged tree contains it and the recorded source is reachable from the second parent. Release tooling omits the pending marker from public notes, identifies the peeled tag commit as their source and renders `stamp r<commit count>` from that commit. After publication, a documentation pull request changes `source` to that merge commit, changes `stamp` to the same deterministic value and returns the tag field to its plain form; `published` stays absent because the publication is the GitHub release's archive rather than a `gh-pages` commit. The archive-root `version.json`, copied from source `web/version.json`, must name that merge commit and `r<commit count>` version. The temporary authored source is necessary because the ledger defines a version's source as the commit its stamp names, while neither the future merge commit nor its stamp exists on the release branch. These provenance checks bind the name, source, tested result, workflow definition, manifests and release notes to one commit.

Only deliberate major and minor versions receive release tags; patch grades record integration and are not independently tagged. Release tags are immutable: an incorrect tag is followed by a corrected version rather than updated or deleted, because released provenance cannot remain auditable if a name changes meaning.

### Cutting a release

1. Identify the merged release pull request's commit with `gh pr view NUMBER --json mergeCommit --jq .mergeCommit.oid`; record its result as `SHA`, and do not substitute a branch tip or a local pre-merge commit. For the first release, the worked tag in the commands below is `julibrot-1.2.0`.

2. Run `gh run list --branch develop --event push --commit SHA --workflow ci.yml` and confirm the exact-SHA run is completed with conclusion `success` before creating the tag.

3. Before the first release only, switch Pages from the legacy branch build to Actions with `gh api --method PUT repos/OWNER/REPO/pages -f build_type=workflow`.

4. Confirm the signing key's armored public key is committed under `deploy/keys/`; for the current signing subkey recorded in `deploy/keys/wild-sky-maker.asc`, create the annotated signed tag with `git tag -s -u 309F3BF0ABAAC226494C8D9FFAB00BAD8B48D17C julibrot-1.2.0 SHA -m 'julibrot 1.2.0'`. A different signing key may be used only after its armored public key is committed there.

5. Verify the local tag and its signature with `git verify-tag julibrot-1.2.0`.

6. Put the hosts on `SHA` before the tag is pushed, and prove the book the release will actually publish. Run `EMBER_SHIP_REF=SHA bash deploy/ship-host.sh deploy` for each prebuilt host, then republish its mirror with `bash deploy/republish-host.sh <ssh alias> --repo <address-book repo> --branch hosts/book --per-host`. Then assemble the release tree on the build server with `EMBER_PAGES_PREBUILT=1 EMBER_PAGES_ARCHIVE=<archive> bash deploy/deploy-pages.sh` and run `node deploy/check-hosts.mjs --tree <assembled tree>` over it, requiring exit 0.

   Prove the assembled tree, not the live site. The two books differ by exactly this release: the live `server.json` carries the bindings of the last release, while the assembled one carries the seed plus this release's `web/mirrors.json` — so a binding introduced by this release would fail a live check that step 8 would then pass, and a binding this release removes would pass a live check that step 8 would then fail. Checking `--book https://endersgamesdev.github.io/EmberEngine/server.json` answers a question about the site that is already published. If the tree cannot be assembled, that live check is the fallback and its answer is about the old book.

   State the consequence rather than discovering it. From the moment a host is shipped at `SHA` until the Pages deploy lands, the OLD live pages have no host on their protocol unless a second host keeps the old build, because the join gate is exact equality and their protocol is their own. The window is the length of the release run and it is announced, not hidden; `ship-host.sh deploy` prints it with both commits named when the shipped commit differs from the published one.

7. Push only the new tag ref with `git push origin refs/tags/julibrot-1.2.0`; do not push a branch or a broader tag refspec as part of this operation.

8. Find the matching release run with `gh run list --workflow release.yml --event push --commit SHA`, then wait for it with `gh run watch RUN_ID --exit-status`; after it succeeds, find the triggered Pages run with `gh run list --workflow pages.yml --event workflow_run --commit SHA` and wait for that run with `gh run watch RUN_ID --exit-status`.

9. Confirm `main` moved to `SHA` with `gh api repos/OWNER/REPO/git/ref/heads/main --jq .object.sha`, then run `gh release view julibrot-1.2.0 --json isDraft,assets --jq '{isDraft, assets: [.assets[].name]}'` and require a published release (`isDraft` is false) with exactly one asset named `ember-pages.tar.gz`.

10. Download the published archive with `gh release download julibrot-1.2.0 --pattern ember-pages.tar.gz --dir DIR` and read its archive-root stamp with `tar -xOf DIR/ember-pages.tar.gz ./version.json`; require its `commit` to resolve to `SHA` and its `version` to equal `r$(git rev-list --count SHA)`, then complete the pending ledger row in a later documentation pull request by replacing its source with `SHA`, recording that version as its stamp, removing `(pending)` so the tag field is plain and leaving `published` absent because the GitHub release archive is the publication. The archive-root file was copied from source `web/version.json`.

11. After the first workflow deployment succeeds, list the environment policies with `gh api repos/OWNER/REPO/environments/github-pages/deployment-branch-policies`, identify the policy id for `gh-pages`, and remove it with `gh api --method DELETE repos/OWNER/REPO/environments/github-pages/deployment-branch-policies/POLICY_ID`; retain the frozen branch itself as the historical record.

## Push permissions

| Actor | Feature or lane branches (no signature requirement) | `develop` (no signature requirement) | `main` (no signature requirement) | Release tags (workflow signature required) |
|---|---|---|---|---|
| Repository owner | Push and force-with-lease | Pull-request merge only | No direct update | Create matching annotated, allowed-key-signed tags |
| Loop account | Push and force-with-lease on `lane/*` | Pull-request merge only | No direct update | Create matching annotated, allowed-key-signed tags |
| Release workflow deploy key | No update | No update | Plain fast-forward push of the validated tag commit | No creation or update |
| Every other actor | No policy-granted update | No policy-granted update | No policy-granted update | No policy-granted creation or update |

Quarantine fetches and integration comparisons use `develop`; a candidate is pushed to its feature or lane branch and merged through a pull request rather than pushed directly to `develop`. Local `main` mirrors the hosted `main` and only fast-forwards after a release promotion.

## GitHub repository rulesets

The files under `deploy/rulesets/` are complete request bodies for GitHub's repository-ruleset REST API. The rule names below use GitHub's repository-ruleset labels exactly, every payload has `enforcement` set to `active`, and authorization is split from integrity wherever a bypass is required so no bypass actor can delete or rewrite protected history. `bash deploy/tests/test-rulesets.sh` checks the payloads against this contract and the CI workflow.

### `branch-names.json`

This branch ruleset records live ruleset `22770318`, includes all branch refs, excludes only `develop`, `main`, `feature/**` and `lane/**`, and has no bypass actors. Its sole `Restrict creations` rule refuses creation of any branch outside those documented names while leaving existing branches untouched.

### `gh-pages-frozen.json`

This branch ruleset targets only `refs/heads/gh-pages`, has no bypass actors and records live ruleset `22835292` in `deploy/rulesets/README.md`.

- `Restrict updates` sets `update_allows_fetch_and_merge` to false and refuses every push, preserving each publication commit and stamp under its original ref.
- `Restrict deletions` keeps the publication record resolvable because the branch-name ruleset would refuse its recreation.

`Block force pushes` is not a separate rule because `Restrict updates` already refuses both fast-forward and non-fast-forward pushes.

### `develop.json`

This branch ruleset targets only `refs/heads/develop` and has no bypass actors. It replaces live ruleset `22736795`, whose deletion and non-fast-forward rules are retained while the complete integration policy is added.

- `Restrict deletions` preserves the integration history.
- `Block force pushes` prevents an integrated commit from changing identity.
- `Require a pull request before merging` allows only the merge-commit method, dismisses stale reviews after new pushes and requires `0` approving reviews for now; code-owner review, last-push approval and resolved review threads are not mechanical merge requirements.
- `Require status checks to pass` requires the literal contexts `cores + servers`, `deploy scripts` and `workspace`, each from GitHub Actions integration id `15368`, with `Require branches to be up to date before merging` enabled and creation not exempted.

### `main-integrity.json`

This branch ruleset targets only `refs/heads/main` and has no bypass actors.

- `Restrict deletions` prevents removal of the live-line reference.
- `Block force pushes` makes every promotion a fast-forward in addition to the workflow's plain-push check.

Commit signatures are not required on `develop` or `main` for now. Release provenance does not depend on a branch-signature rule: `release.yml` still verifies each annotated release tag against `deploy/keys/` after selecting a green exact-SHA `develop` run. If branch signatures become part of the policy later, `required_signatures` can be added to both `develop.json` and `main-integrity.json` without changing the authorization payloads.

### `main-authorization.json`

This branch ruleset targets only `refs/heads/main`. Its single bypass actor is the GitHub `DeployKey` category with bypass mode `always`, which GitHub represents without an actor id. It replaces live ruleset `22736890` after `main` exists, preserving the creation restriction while adding update authorization.

- `Restrict creations` permits the workflow deploy key to create the branch after the migration setup and blocks later recreation by other actors.
- `Restrict updates` permits only the workflow deploy key to advance the branch.

### `release-tags-authorization.json`

This tag ruleset targets `refs/tags/*-[0-9]*.[0-9]*.[0-9]*`. User `322515484` (`wildskymaker`) and user `196965598` (`enderPeer`) are its only bypass actors, each with bypass mode `always`, because they are the two identified release-tag creators.

- `Restrict creations` limits matching tag creation to the two release actors.

The workflow separately verifies a tag signature against the armored public keys actually present in `deploy/keys/`. Only the Wild Sky Maker key is present now; the second identified signer's armored public key must enter `deploy/keys/` through a pull request before that signer's tags can pass release validation.

### `release-tags-integrity.json`

This tag ruleset targets the same `refs/tags/*-[0-9]*.[0-9]*.[0-9]*` pattern and has no bypass actors.

- `Restrict updates` makes an existing release tag immutable.
- `Restrict deletions` preserves the name-to-commit provenance of every release.

### `other-tags.json`

This tag ruleset includes `~ALL`, excludes `refs/tags/*-[0-9]*.[0-9]*.[0-9]*`, and has no bypass actors.

- `Restrict creations` prevents tags outside the release namespace, eliminating names that would bypass the series and semantic-version checks.

Feature and lane branches remain rewritable because changing a private candidate is useful; the branch-naming ruleset limits only which names can be created, while pull-request and CI requirements take effect when a candidate enters `develop`.

## Workflow contract

### Continuous integration

`.github/workflows/ci.yml` runs for every pull request targeting `develop` and every push to `develop`, with no path exclusions. Pull-request checks run against GitHub's synthetic merge SHA and gate the proposed merge; the final commit pushed to `develop` has a different SHA, so its push run supplies the exact-SHA workflow and job evidence that `release.yml` consumes. The resulting one complete CI run per merge, including a documentation-only merge, is the cost of making every integrated commit potentially releasable.

The existing `cores + servers` and `deploy scripts` jobs remain. The deploy job runs `deploy/tests/test-readmes.sh`, `test-done-lists.sh`, `test-changelog.sh` and `test-commit-messages.sh` on every pull request. The `workspace` job installs `pkg-config`, `libudev-dev`, `libasound2-dev` and `mesa-vulkan-drivers` before Rust setup: the first three provide the package metadata and native development headers required by the engine's Linux gamepad and audio crates, while Mesa's lavapipe provides the software Vulkan adapter that engine GPU readback tests need on a runner without a GPU. It then selects `rust-toolchain.toml` through `rustup show`, checks `cargo fmt --all -- --check`, runs `cargo clippy --workspace --all-targets --locked`, tests the workspace with the linter and presentation package excluded, then tests `ember-julibrot-present` with one test thread; `Swatinem/rust-cache` keeps that complete gate practical.

There is no promotion job in CI. Passing integration CI proves `develop`; it does not turn an arbitrary merge into a release or move the live line.

### Release

`.github/workflows/release.yml` runs for pushes of tags matching `*-[0-9]*.[0-9]*.[0-9]*` in the `release` concurrency group. `cancel-in-progress: false` preserves the active release, and `queue: max` retains later tag runs so the group processes them sequentially rather than replacing an older pending release. Job permissions are read-only by default, and `contents: write` exists only where GitHub release publication requires it.

Each validation fails the run immediately: the tag shape and allowed series are checked; the tag object is re-fetched after checkout because the checkout action materialises the triggering tag as its peeled commit, so the annotated-tag and signature checks would otherwise run against a lightweight ref; the tag is annotated; its signature verifies after the armored keys in `deploy/keys/*.asc` are imported into a temporary `GNUPGHOME`; its target is an ancestor of `origin/develop`; the Actions REST API selects the newest successful completed `push` run for `develop` at that SHA whose path is `.github/workflows/ci.yml` and proves that run contains exactly the three successful required jobs; every manifest assigned to the series by `docs/versioning.md` equals the tag version, with the Ember series additionally requiring every shared crate to inherit its checked workspace version; `bash deploy/tests/test-changelog.sh --tag TAG` proves the parsed ledger has exactly one accepted entry in the series's section whose launcher slot carries the tag's three-grade version and, for a pending row, proves the annotated tag targets the exact two-parent merge that integrated that release line from a second parent containing its recorded source; and `origin/main` is either absent or an ancestor of the target.

After validation, the workflow uses `deploy/deploy-pages.sh` to build and stamp the whole Pages site archive, regardless of which series names the tag, because Pages deploys the complete hub and every retained game and lab as one byte-identical artifact. It uploads that archive between jobs, runs `deploy/github-releases.sh --apply --tag TAG --draft` and attaches the archive while the release is unpublished, then uses the SSH key in `RELEASE_DEPLOY_KEY` for a plain push of the tag's peeled commit to `refs/heads/main`; git and the ruleset both reject a non-fast-forward promotion. A failed promotion deletes the draft so no public release survives a failed move of `main`; a successful promotion is followed by the explicit draft-to-published transition. The single-tag path refuses the historical annotation and series-path fallback when the exact changelog entry is absent, while bulk backfill retains that fallback so historical annotated tags can still be reconciled deliberately.

The release deploy key is the ruleset's only bypass actor on `main`. Generating the deploy key and storing `RELEASE_DEPLOY_KEY` are credential steps performed outside repository history by the repository owner; this boundary keeps credential material separate from reviewable workflow code.

### Pages

`.github/workflows/pages.yml` runs after the `release` workflow completes successfully, then checks out `main` and finds a release tag that points at `HEAD`. Waiting for successful completion prevents the push-to-`main` event from racing the draft-to-published transition. The workflow fails unless that tag has a published release with exactly one `ember-pages.tar.gz` asset, downloads that asset rather than rebuilding it, and deploys the archive through `actions/configure-pages`, `actions/upload-pages-artifact` and `actions/deploy-pages` into the `github-pages` environment.

Between extracting that asset and configuring the deployment the workflow runs `node deploy/check-hosts.mjs --tree` against the extracted asset and fails the deploy when any live server game has no host it can join. The check reads the extracted tree's own `games.json`, `server.json` and `hosts.js`, so it ranks exactly as the pages in that archive will, and it opens a real socket to each candidate rather than trusting what the book claims. The position and the blocking are both the contract: a step that ran after the deploy, or one marked `continue-on-error`, would be a log line rather than a gate, and `deploy/tests/test-workflows.sh` rejects a fixture of each shape. A site whose pages can find no host on their protocol is a site no player can play, and the moment to discover that is before it is served rather than after.

The environment accepts deployments from `main` and, until the release workflow's first Pages deployment replaces the legacy build, from `gh-pages`; that first deployment removes the `gh-pages` policy entry so `main` becomes the environment's sole accepted ref. Pages still builds from the frozen `gh-pages` branch until that replacement, and the branch remains afterward as the resolvable publication record. Reusing the release asset makes the published bytes identical to the reviewed release instead of producing a second build with merely equivalent source.

## Host rule

Production hosts use `EMBER_REF=main`. A host therefore rebuilds only after the release workflow has validated a tag and advanced `main`, keeping server source and the Pages client on the same release commit and preventing protocol-version skew between them.

`deploy/deploy-pages.sh` remains the local build and dry-run path, including the byte-identity check against a release artifact, but it does not publish a `gh-pages` branch. Actions owns publication because `main` is the auditable signal that a release is live.

## Order of operations

1. The replayed history established `develop` at `eefd43d0` as the integration baseline before any protection existed. This fixed starting point keeps the replayed source and the boundary of protected history auditable.
2. Surviving working branches have completed their renames under `feature/*`, and live branch-name ruleset `22770318` admits creation only of `develop`, `main`, `feature/**` and `lane/**` with no bypass actors. This state keeps candidate work on bounded names while preserving the two governed long-lived lines.
3. `main` exists at the `develop` tip used for the one-time migration anchor, and `ci-passed` no longer exists because its replayed content is contained in `develop`; ruleset `22770318` refuses its recreation. The anchor prevents hosts from rolling back to older source, while deleting the obsolete integration ref removes a competing line without discarding its content.
4. `develop` is governed by live ruleset `22736795` from `deploy/rulesets/develop.json`: deletion and force pushes are blocked, pull requests with merge commits are the sole integration method, required approvals are zero, and the strict required contexts `cores + servers`, `deploy scripts` and `workspace` are bound to GitHub Actions integration id `15368`; no bypass actor exists. This policy joins every integration commit to current-tree CI evidence without depending on an approval count.
5. `main` is governed by live rulesets `22736890` from `main-authorization.json` and `22795584` from `main-integrity.json`: creation and updates are blocked except for the release deploy key, while deletion and force pushes are blocked without bypass. The deploy key is registered and its private key is stored as `RELEASE_DEPLOY_KEY`, giving the release workflow only the narrow fast-forward capability required for promotion.
6. Release tags are governed by live rulesets `22795757` from `release-tags-authorization.json` and `22795767` from `release-tags-integrity.json`, while live ruleset `22795785` from `other-tags.json` blocks creation outside the release namespace. This separation permits the identified release-tag creation path without permitting any actor to rewrite or delete release provenance.
7. `gh-pages` is governed by live ruleset `22835292` from the tracked `gh-pages-frozen.json` payload. Its no-bypass update and deletion rules close the legacy write path while keeping every cited publication commit and stamp resolvable under the only available archive name.
8. The `github-pages` environment accepts deployments from `main` and, until the release workflow's first Pages deployment replaces the legacy build, from `gh-pages`; that deployment removes the `gh-pages` policy entry while the branch remains as the frozen record. This overlap keeps the existing site available before the reviewed archive from promoted `main` makes `main` the environment's sole accepted ref.
9. The first conforming release tag has not yet been cut and remains the final migration step. Its successful run will prove the end-to-end contract by creating the release asset, fast-forwarding `main`, publishing Pages from the same bytes and allowing hosts to update from the same source.
