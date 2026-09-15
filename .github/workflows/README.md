# GitHub Actions workflows

This directory contains the continuous-integration, release-promotion, and Pages-deployment workflows.

Pull requests into `develop` depend on `ci.yml` for automated gates, `release.yml` validates a signed release tag, attaches its archive to a draft, promotes it and only then publishes it, and `pages.yml` waits for that workflow to succeed before deploying the published release asset at `main` HEAD.

`pages.yml` also gates the deployment on the site being playable. Between extracting the release asset and configuring the deployment it selects Node 24.20.0, installs the locked npm dependencies, selects the pinned Rust toolchain, regenerates the TypeScript inputs and emits `target/web-generated/node-js/check-hosts.mjs` from the checkout. It runs that gate against the extracted tree and fails the deploy when any live server game has no host it can join, using that tree's own `games.json`, `server.json` and emitted `hosts.js` so it ranks exactly as the pages being shipped will. The release archive stays exactly the site; the rebuilt gate is a checkout-provenance tool, not a shipped sidecar. Step order and blocking are both the contract, and `deploy/tests/test-workflows.sh` rejects missing, reordered and softened fixtures.

The release workflow uses the `release` concurrency group with `queue: max` and cancellation disabled, so tag runs wait and execute sequentially instead of replacing an older pending release.

The former hourly heartbeat workflow is retired because pull requests and CI now provide the integration record; [`marketing/HEARTBEAT.md`](../../marketing/HEARTBEAT.md) describes the closed committed log and the unaffected local watcher.

Keep workflow changes within GitHub Actions' least required permissions and follow the verification and hand-off rules in [`CLAUDE.md`](../../CLAUDE.md) and [`docs/worker-protocol.md`](../../docs/worker-protocol.md).
