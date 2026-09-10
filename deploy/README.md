# Deployment

This directory holds the scripts that build the Pages release asset, start or ship game hosts, maintain the multi-host address book, stamp releases and operate host watchdogs and timers.

The live launcher and game servers depend on these scripts for reproducible bundles, protocol-aware addresses and process ownership across Linux, Windows and WSL hosts.

Changes integrate through pull requests to `develop`; validated release tags advance the live `main` branch so Pages and hosts select the same source. The full branch and promotion contract lives in [`docs/branching.md`](../docs/branching.md).

Use [`retag.sh`](retag.sh) for the completed historical tag migration and [`github-releases.sh`](github-releases.sh) to derive GitHub Releases from those tags and `CHANGELOG.md`.

Start with [`docs/hosts.md`](../docs/hosts.md) for the host model, use [`README-watchdog.md`](README-watchdog.md) for watchdog operation, and preserve the operational and verification constraints in [`CLAUDE.md`](../CLAUDE.md); shell behavior is pinned under [`tests/`](tests/).
