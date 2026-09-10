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

The required status checks are `ci / cores + servers`, `ci / deploy scripts` and `ci / workspace`. Required approvals start at `0`: CI is the mechanical pull-request gate, while the adversarial review and its conclusions are recorded in the pull-request body so the reasoning remains auditable without turning one human's availability into an integration lock.

Each merge to `develop` advances the patch grade of every series it changes. The pending changelog entry cites the source commit because the eventual pull-request merge commit does not exist while the candidate is being prepared.

## Release tags

The release workflow is triggered by the literal GitHub Actions tag pattern `*-*.*.*`. It then fails unless the final version suffix has the exact numeric form `MAJOR.MINOR.PATCH`, with each component either `0` or a non-zero digit followed by digits, and the prefix is `ember`, an exact game `id` in `web/games.json`, or an exact lab directory id under `web/labs/`. Bare versions and `v`-prefixed versions are invalid because every tag belongs unambiguously to one versioned series.

Release tags are annotated and signed. Their signatures verify against an armored public key committed under `deploy/keys/`, which makes the allowed signing material reviewable in the same history as the release policy.

A release tag points at a commit reachable from `origin/develop`; the `ci` check suite at that exact commit has concluded `success`; the series manifest version equals the tag version; and the pending changelog contains the accepted entry. These provenance checks bind the name, source, tested result, manifest and release notes to one commit.

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

The rule names below use GitHub's repository-ruleset labels exactly. Each ruleset is active, and the target patterns are disjoint so their effects are explicit rather than accidental.

### `develop`

This branch ruleset targets only `develop` and has no bypass actors.

- `Restrict deletions` preserves the integration history.
- `Block force pushes` prevents an integrated commit from changing identity.
- `Require a pull request before merging` is configured for merge commits only and `0` required approvals; repository merge settings disable squash and rebase methods.
- `Require status checks to pass` requires `ci / cores + servers`, `ci / deploy scripts` and `ci / workspace`, with `Require branches to be up to date before merging` enabled so the checks cover the candidate that lands.
- `Require signed commits` preserves attributable source history; GitHub's signed merge commit closes the pull request.

### `main`

This branch ruleset targets only `main`. The release workflow deploy key is its single bypass actor with bypass mode set to always; no person or general-purpose automation bypasses the live-line policy.

- `Restrict creations` permits the workflow deploy key to create the branch after the migration setup and blocks later recreation by other actors.
- `Restrict updates` permits only the workflow deploy key to advance the branch.
- `Restrict deletions` prevents removal of the live-line reference.
- `Block force pushes` makes every promotion a fast-forward in addition to the workflow's plain-push check.
- `Require signed commits` admits only the signed history already integrated through `develop`.

### Release tags

This tag ruleset targets `*-*.*.*`. The repository owner and loop account are its only bypass actors because they are the release-tag signers; signature allow-list enforcement remains in the release workflow.

- `Restrict creations` limits matching tag creation to the two release actors.
- `Restrict updates` makes an existing release tag immutable.
- `Restrict deletions` preserves the name-to-commit provenance of every release.

### All other tags

This tag ruleset targets every tag not matched by `*-*.*.*` and has no bypass actors.

- `Restrict creations` prevents tags outside the release namespace, eliminating names that would bypass the series and semantic-version checks.

Feature and lane branches have no ruleset because rewriting a private candidate is useful; the signed-commit and CI requirements take effect when that candidate enters `develop`.

## Workflow contract

### Continuous integration

`.github/workflows/ci.yml` runs for every pull request targeting `develop` and for relevant pushes to `develop`. Pull requests have no path exclusion because repository-policy tests apply to documentation and automation changes too; push path exclusions avoid redundant builds after docs-only integration.

The existing `cores + servers` and `deploy scripts` jobs remain. The deploy job runs `deploy/tests/test-readmes.sh`, `test-done-lists.sh`, `test-changelog.sh` and `test-commit-messages.sh` on every pull request. The `workspace` job selects `rust-toolchain.toml` through `rustup show`, checks `cargo fmt --all -- --check`, runs `cargo clippy --workspace --all-targets --locked`, tests the workspace with the linter and presentation package excluded, then tests `ember-julibrot-present` with one test thread; `Swatinem/rust-cache` keeps that complete gate practical.

There is no promotion job in CI. Passing integration CI proves `develop`; it does not turn an arbitrary merge into a release or move the live line.

### Release

`.github/workflows/release.yml` runs for pushes of tags matching `*-*.*.*` in the serial `release` concurrency group, with cancellation disabled so one release cannot interrupt another. Job permissions are read-only by default, and `contents: write` exists only where GitHub release publication requires it.

Each validation fails the run immediately: the tag shape and allowed series are checked; the tag is annotated; its signature verifies after the armored keys in `deploy/keys/*.asc` are imported into a temporary `GNUPGHOME`; its target is an ancestor of `origin/develop`; the REST API reports the `ci` check run at that SHA concluded `success`; the series manifest version equals the tag; `bash deploy/tests/test-changelog.sh` passes at the target; and `origin/main` is either absent or an ancestor of the target.

After validation, the workflow builds the selected series' wasm bundle and the hub by the same path as `deploy/deploy-pages.sh`, stamps the version, and uploads the resulting site archive as the release asset. It runs `deploy/github-releases.sh --apply` for that one tag, then uses the SSH key in `RELEASE_DEPLOY_KEY` for a plain push of the tag's peeled commit to `refs/heads/main`; git and the ruleset both reject a non-fast-forward promotion.

The release deploy key is the ruleset's only bypass actor on `main`. Adding that key to the repository ruleset and storing its private half as `RELEASE_DEPLOY_KEY` are owner-run credential steps outside repository history, which keeps the credential boundary separate from reviewable workflow code.

### Pages

`.github/workflows/pages.yml` runs only when `main` advances. It finds the release tag that points at `HEAD`, fails if none exists, downloads that tag's release asset rather than rebuilding it, and deploys the archive through `actions/configure-pages`, `actions/upload-pages-artifact` and `actions/deploy-pages` into the `github-pages` environment.

The environment accepts deployments from `main` only. Reusing the release asset makes the published bytes identical to the reviewed release instead of producing a second build with merely equivalent source.

## Host rule

Production hosts use `EMBER_REF=main`. A host therefore rebuilds only after the release workflow has validated a tag and advanced `main`, keeping server source and the Pages client on the same release commit and preventing protocol-version skew between them.

`deploy/deploy-pages.sh` remains the local build and dry-run path, including the byte-identity check against a release artifact, but it does not publish a `gh-pages` branch. Actions owns publication because `main` is the auditable signal that a release is live.

## Order of operations

1. The replayed history establishes `develop` at `eefd43d0`, providing the integration baseline before protections begin.
2. Surviving working branches are named under `feature/*`; `ci-passed` is retired; and `main` is created once at the current `develop` tip so hosts do not roll back to an old tag.
3. The document and workflow changes land together through the first pull request governed by the new `develop` process, making the repository contract and its automation agree.
4. The repository rulesets are activated, the release deploy key is registered as the only `main` bypass actor, its private key is stored as `RELEASE_DEPLOY_KEY`, and the `github-pages` environment is restricted to `main`; these external settings make the checked-in specification effective.
5. The first conforming release tag proves the end-to-end contract by creating the release asset, fast-forwarding `main`, deploying Pages from the same bytes and allowing hosts to update from the same source.
