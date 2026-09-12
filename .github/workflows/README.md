# GitHub Actions workflows

This directory contains the continuous-integration, release-promotion, and Pages-deployment workflows.

Pull requests into `develop` depend on `ci.yml` for automated gates, `release.yml` validates a signed release tag, attaches its archive to a draft, promotes it and only then publishes it, and `pages.yml` waits for that workflow to succeed before deploying the published release asset at `main` HEAD.

`pages.yml` also gates the deployment on the site being playable. Between extracting the release asset and configuring the deployment it runs `node deploy/check-hosts.mjs --tree` against the extracted tree and fails the deploy when any live server game has no host it can join, using that tree's own `games.json`, `server.json` and `hosts.js` so it ranks exactly as the pages being shipped will. `actions/setup-node` pins node 22 because the check opens a real WebSocket from the global. Position and blocking are both the contract: a step that ran after the deploy, or one carrying `continue-on-error`, would be a log line rather than a gate, and `deploy/tests/test-workflows.sh` rejects a fixture of each shape.

The release workflow uses the `release` concurrency group with `queue: max` and cancellation disabled, so tag runs wait and execute sequentially instead of replacing an older pending release.

The former hourly heartbeat workflow is retired because pull requests and CI now provide the integration record; [`marketing/HEARTBEAT.md`](../../marketing/HEARTBEAT.md) describes the closed committed log and the unaffected local watcher.

Keep workflow changes within GitHub Actions' least required permissions and follow the verification and hand-off rules in [`CLAUDE.md`](../../CLAUDE.md) and [`docs/worker-protocol.md`](../../docs/worker-protocol.md).
