# Branching, integration and release

Ember integrates changes on `develop` through pull requests and reserves `main` for the source that is live. This separation makes CI evidence part of integration while keeping releases, Pages and hosts on the same tagged commit.

## Branch model

| Branch | Meaning | Writers | Update path |
|---|---|---|---|
| `feature/TOPIC` | A bounded human-authored change | The branch author | Pushes may be rebased, amended or replaced with force-with-lease. |
| `lane/NAME` | A bounded loop-authored change | The lane | Pushes may be rebased, amended or replaced with force-with-lease. |
| `develop` | The default integration line; every integrated change passed the required checks in a pull request | GitHub pull-request merges | Merge commits only; direct pushes and force pushes are blocked. |
| `main` | The live line; after its one-time migration anchor, it advances only to commits selected by valid release tags | The release workflow through its deploy key | Plain fast-forward pushes of tag target commits only. |
| `gh-pages` | Retired because Pages deploys the release asset selected by `main` | Nobody | No updates. |
| `ci-passed` | Retired because hosts follow tagged releases on `main` | Nobody | No updates. |

`main` is always an ancestor of `develop`, so a release promotion cannot introduce a commit that was not integrated. Nothing is committed or merged directly on `main`, so the branch cannot diverge from `develop` and never needs a reconciliation merge.

The migration creates `main` once at the then-current tip of `develop`, even when that commit is newer than the last release tag. This one-time anchor prevents every host from downgrading to source that may be months old; every later update of `main` comes from the release workflow and points at a valid tag target.

## Pull requests into `develop`

Every change, including release-process changes, reaches `develop` through a pull request. Direct integration would separate the resulting commit from the CI and review record that establish why it is safe to land.

Only merge commits are enabled; squash and rebase merges are disabled because they replace authored commits and discard their signatures. The merge subject and body follow [`commit-messages.md`](commit-messages.md), while GitHub signs the merge commit with its key and every source commit remains signed by its author.

The pull-request branch is up to date with `develop` before merging. Strict status checks then prove the current candidate rather than an older tree, and the merge commit carries the same content as that gated candidate.

The required status-check contexts are the literal CI job names `cores + servers`, `deploy scripts` and `workspace`, and each is bound to GitHub Actions integration id `15368`. Required approvals start at `0`: CI is the mechanical pull-request gate, while the adversarial review and its conclusions are recorded in the pull-request body so the reasoning remains auditable without turning one human's availability into an integration lock.

Each merge to `develop` advances the patch grade of every series it changes. The pending changelog entry cites the source commit because the eventual pull-request merge commit does not exist while the candidate is being prepared.

## Release tags

The release namespace uses the ruleset ref pattern `refs/tags/*-[0-9]*.[0-9]*.[0-9]*`, while the workflow trigger omits the `refs/tags/` prefix and uses `*-[0-9]*.[0-9]*.[0-9]*`. GitHub evaluates these as fnmatch patterns, so they are deliberately coarser than the workflow's exact runtime regex; the workflow re-validates every candidate and fails unless the final version suffix has the exact numeric form `MAJOR.MINOR.0`, with the major and minor each either `0` or a non-zero digit followed by digits, and the prefix is `ember`, an exact game `id` in `web/games.json`, or an exact lab directory id under `web/labs/`. Bare versions and `v`-prefixed versions are invalid because every tag belongs unambiguously to one versioned series.

Release tags are annotated and signed. Their signatures verify against an armored public key committed under `deploy/keys/`, which makes the allowed signing material reviewable in the same history as the release policy.

A release tag points at a commit reachable from `origin/develop`; the newest completed `push` run for `develop` at that exact SHA whose workflow path is `.github/workflows/ci.yml` has concluded `success`, and its jobs are exactly `cores + servers`, `deploy scripts` and `workspace`, each with conclusion `success`; every dedicated manifest in the series mapping in `docs/versioning.md` declares the tag version, or the Ember workspace and every shared crate agree through workspace inheritance; and the pending changelog contains the accepted entry. These provenance checks bind the name, source, tested result, workflow definition, manifests and release notes to one commit.

Only deliberate major and minor versions receive release tags; patch grades record integration and are not independently tagged. Release tags are immutable: an incorrect tag is followed by a corrected version rather than updated or deleted, because released provenance cannot remain auditable if a name changes meaning.

## Push permissions

| Actor | Feature or lane branches | `develop` | `main` | Release tags |
|---|---|---|---|---|
| Repository owner | Push and force-with-lease | Pull-request merge only | No direct update | Create matching annotated, allowed-key-signed tags |
| Loop account | Push and force-with-lease on `lane/*` | Pull-request merge only | No direct update | Create matching annotated, allowed-key-signed tags |
| Release workflow deploy key | No update | No update | Plain fast-forward push of the validated tag commit | No creation or update |
| Every other actor | No policy-granted update | No policy-granted update | No policy-granted update | No policy-granted creation or update |

Quarantine fetches and integration comparisons use `develop`; a candidate is pushed to its feature or lane branch and merged through a pull request rather than pushed directly to `develop`. Local `main` mirrors the hosted `main` and only fast-forwards after a release promotion.

## GitHub repository rulesets

The files under `deploy/rulesets/` are complete request bodies for GitHub's repository-ruleset REST API. The rule names below use GitHub's repository-ruleset labels exactly, every payload has `enforcement` set to `active`, and authorization is split from integrity wherever a bypass is required so no bypass actor can delete, rewrite or evade signature protection. `bash deploy/tests/test-rulesets.sh` checks the payloads against this contract and the CI workflow.

### `develop.json`

This branch ruleset targets only `refs/heads/develop` and has no bypass actors. It replaces live ruleset `22736795`, whose deletion and non-fast-forward rules are retained while the complete integration policy is added.

- `Restrict deletions` preserves the integration history.
- `Block force pushes` prevents an integrated commit from changing identity.
- `Require a pull request before merging` allows only the merge-commit method, dismisses stale reviews after new pushes and requires `0` approving reviews for now; code-owner review, last-push approval and resolved review threads are not mechanical merge requirements.
- `Require status checks to pass` requires the literal contexts `cores + servers`, `deploy scripts` and `workspace`, each from GitHub Actions integration id `15368`, with `Require branches to be up to date before merging` enabled and creation not exempted.
- `Require signed commits` verifies commits introduced by updates after activation rather than retrospectively checking history already present on `develop`; GitHub's signed merge commit closes the pull request.

### `main-integrity.json`

This branch ruleset targets only `refs/heads/main` and has no bypass actors.

- `Restrict deletions` prevents removal of the live-line reference.
- `Block force pushes` makes every promotion a fast-forward in addition to the workflow's plain-push check.
- `Require signed commits` verifies commits introduced by non-bypassed updates after activation; history already present when the rule is activated, including unsigned commits, is not rechecked.

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

Feature and lane branches have no ruleset because rewriting a private candidate is useful; the signed-commit and CI requirements take effect when that candidate enters `develop`.

## Workflow contract

### Continuous integration

`.github/workflows/ci.yml` runs for every pull request targeting `develop` and every push to `develop`, with no path exclusions. Pull-request checks run against GitHub's synthetic merge SHA and gate the proposed merge; the final commit pushed to `develop` has a different SHA, so its push run supplies the exact-SHA workflow and job evidence that `release.yml` consumes. The resulting one complete CI run per merge, including a documentation-only merge, is the cost of making every integrated commit potentially releasable.

The existing `cores + servers` and `deploy scripts` jobs remain. The deploy job runs `deploy/tests/test-readmes.sh`, `test-done-lists.sh`, `test-changelog.sh` and `test-commit-messages.sh` on every pull request. The `workspace` job installs `pkg-config`, `libudev-dev` and `libasound2-dev` before Rust setup because the runner image carries neither the package metadata tool nor the native development headers required by the engine's Linux gamepad and audio crates. It then selects `rust-toolchain.toml` through `rustup show`, checks `cargo fmt --all -- --check`, runs `cargo clippy --workspace --all-targets --locked`, tests the workspace with the linter and presentation package excluded, then tests `ember-julibrot-present` with one test thread; `Swatinem/rust-cache` keeps that complete gate practical.

There is no promotion job in CI. Passing integration CI proves `develop`; it does not turn an arbitrary merge into a release or move the live line.

### Release

`.github/workflows/release.yml` runs for pushes of tags matching `*-[0-9]*.[0-9]*.[0-9]*` in the `release` concurrency group. `cancel-in-progress: false` preserves the active release, and `queue: max` retains later tag runs so the group processes them sequentially rather than replacing an older pending release. Job permissions are read-only by default, and `contents: write` exists only where GitHub release publication requires it.

Each validation fails the run immediately: the tag shape and allowed series are checked; the tag is annotated; its signature verifies after the armored keys in `deploy/keys/*.asc` are imported into a temporary `GNUPGHOME`; its target is an ancestor of `origin/develop`; the Actions REST API selects the newest successful completed `push` run for `develop` at that SHA whose path is `.github/workflows/ci.yml` and proves that run contains exactly the three successful required jobs; every manifest assigned to the series by `docs/versioning.md` equals the tag version, with the Ember series additionally requiring every shared crate to inherit its checked workspace version; `bash deploy/tests/test-changelog.sh --tag TAG` proves the parsed ledger has exactly one accepted entry for that candidate; and `origin/main` is either absent or an ancestor of the target.

After validation, the workflow uses `deploy/deploy-pages.sh` to build and stamp the whole Pages site archive, regardless of which series names the tag, because Pages deploys the complete hub and every retained game and lab as one byte-identical artifact. It uploads that archive between jobs, runs `deploy/github-releases.sh --apply --tag TAG --draft` and attaches the archive while the release is unpublished, then uses the SSH key in `RELEASE_DEPLOY_KEY` for a plain push of the tag's peeled commit to `refs/heads/main`; git and the ruleset both reject a non-fast-forward promotion. A failed promotion deletes the draft so no public release survives a failed move of `main`; a successful promotion is followed by the explicit draft-to-published transition. The single-tag path refuses the historical annotation and series-path fallback when the exact changelog entry is absent, while bulk backfill retains that fallback so historical annotated tags can still be reconciled deliberately.

The release deploy key is the ruleset's only bypass actor on `main`. Generating the deploy key and storing `RELEASE_DEPLOY_KEY` are credential steps performed outside repository history by the repository owner; this boundary keeps credential material separate from reviewable workflow code.

### Pages

`.github/workflows/pages.yml` runs after the `release` workflow completes successfully, then checks out `main` and finds a release tag that points at `HEAD`. Waiting for successful completion prevents the push-to-`main` event from racing the draft-to-published transition. The workflow fails unless that tag has a published release with exactly one `ember-pages.tar.gz` asset, downloads that asset rather than rebuilding it, and deploys the archive through `actions/configure-pages`, `actions/upload-pages-artifact` and `actions/deploy-pages` into the `github-pages` environment.

The environment accepts deployments from `main` only. Reusing the release asset makes the published bytes identical to the reviewed release instead of producing a second build with merely equivalent source.

## Host rule

Production hosts use `EMBER_REF=main`. A host therefore rebuilds only after the release workflow has validated a tag and advanced `main`, keeping server source and the Pages client on the same release commit and preventing protocol-version skew between them.

`deploy/deploy-pages.sh` remains the local build and dry-run path, including the byte-identity check against a release artifact, but it does not publish a `gh-pages` branch. Actions owns publication because `main` is the auditable signal that a release is live.

## Order of operations

1. The replayed history establishes `develop` at `eefd43d0`, providing the integration baseline before protections begin.
2. Surviving working branches are named under `feature/*`; `ci-passed` is retired; and `main` is created once at the current `develop` tip so hosts do not roll back to an old tag.
3. The corrected `develop` ruleset is activated from `deploy/rulesets/develop.json` before the first pull request, so that pull request is governed by the status, signature and merge-method policy it introduces into repository history.
4. The document, payload and workflow changes land together through that governed pull request, making the repository contract and its automation agree.
5. The remaining repository rulesets are activated, the release deploy key is registered as the only `main` bypass actor, its private key is stored as `RELEASE_DEPLOY_KEY`, and the `github-pages` environment is restricted to `main`; these external settings make the checked-in specification effective.
6. The first conforming release tag proves the end-to-end contract by creating the release asset, fast-forwarding `main`, deploying Pages from the same bytes and allowing hosts to update from the same source.
