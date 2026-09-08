# Deployment

This directory holds the scripts that build and publish the Pages site, start or ship game hosts, maintain the multi-host address book, stamp releases and operate host watchdogs and timers.

The live launcher and game servers depend on these scripts for reproducible bundles, protocol-aware addresses and process ownership across Linux, Windows and WSL hosts.

Use [`retag.sh`](retag.sh) for the completed historical tag migration and [`github-releases.sh`](github-releases.sh) to derive GitHub Releases from those tags and `CHANGELOG.md`.

Start with [`docs/hosts.md`](../docs/hosts.md) for the host model, use [`README-watchdog.md`](README-watchdog.md) for watchdog operation, and preserve the operational and verification constraints in [`CLAUDE.md`](../CLAUDE.md); shell behavior is pinned under [`tests/`](tests/).
