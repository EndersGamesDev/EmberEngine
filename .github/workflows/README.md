# GitHub Actions workflows

This directory contains the continuous-integration workflow and the quiet-lane heartbeat workflow.

Pull requests depend on `ci.yml` for automated gates, while the marketing continuity record depends on `heartbeat.yml` and [`marketing/HEARTBEAT.md`](../../marketing/HEARTBEAT.md).

Keep workflow changes within GitHub Actions' least required permissions and follow the verification and hand-off rules in [`CLAUDE.md`](../../CLAUDE.md) and [`docs/worker-protocol.md`](../../docs/worker-protocol.md).
