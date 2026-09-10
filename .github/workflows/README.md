# GitHub Actions workflows

This directory contains the continuous-integration, release-promotion, and Pages-deployment workflows.

Pull requests into `develop` depend on `ci.yml` for automated gates, `release.yml` validates and promotes signed release tags, and `pages.yml` deploys the promoted release asset from `main`.

The former hourly heartbeat workflow is retired because pull requests and CI now provide the integration record; [`marketing/HEARTBEAT.md`](../../marketing/HEARTBEAT.md) describes the closed committed log and the unaffected local watcher.

Keep workflow changes within GitHub Actions' least required permissions and follow the verification and hand-off rules in [`CLAUDE.md`](../../CLAUDE.md) and [`docs/worker-protocol.md`](../../docs/worker-protocol.md).
