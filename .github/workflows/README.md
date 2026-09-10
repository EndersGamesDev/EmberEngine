# GitHub Actions workflows

This directory contains the continuous-integration, release-promotion, and Pages-deployment workflows.

Pull requests into `develop` depend on `ci.yml` for automated gates, `release.yml` validates a signed release tag, attaches its archive to a draft, promotes it and only then publishes it, and `pages.yml` waits for that workflow to succeed before deploying the published release asset at `main` HEAD.

The former hourly heartbeat workflow is retired because pull requests and CI now provide the integration record; [`marketing/HEARTBEAT.md`](../../marketing/HEARTBEAT.md) describes the closed committed log and the unaffected local watcher.

Keep workflow changes within GitHub Actions' least required permissions and follow the verification and hand-off rules in [`CLAUDE.md`](../../CLAUDE.md) and [`docs/worker-protocol.md`](../../docs/worker-protocol.md).
