# Deployment

This directory holds the scripts that build the Pages release asset, start or ship game hosts, maintain the multi-host address book, stamp releases and operate host watchdogs and timers.

The live launcher and game servers depend on these scripts for reproducible bundles, protocol-aware addresses and process ownership across Linux, Windows and WSL hosts.

Changes integrate through pull requests to `develop`; validated release tags advance the live `main` branch so Pages and hosts select the same source. The full branch and promotion contract lives in [`docs/branching.md`](../docs/branching.md).

Use [`retag.sh`](retag.sh) for the completed historical tag migration and [`github-releases.sh`](github-releases.sh) to derive GitHub Releases from those tags and `CHANGELOG.md`.

[`check-hosts.mjs`](check-hosts.mjs) answers the question the rest of the publication path never asked: does every live server game in a catalog have a host it can actually join? It loads an address book, fetches its bound mirrors, merges and ranks with the pure functions of `web/hosts.js`, probes every host that runs the game and only then applies the protocol filter, so a live `Welcome` outranks a book that has gone stale. `--tree <dir>` points it at a site root, which is the shape `pages.yml` gates on between extracting the release asset and deploying it. Its unit and command-line tests are [`check-hosts.test.mjs`](check-hosts.test.mjs) under `node --test`, and they are not part of the shell suites because those run where node is not installed.

[`../web/mirrors.json`](../web/mirrors.json) is the tracked list of mirror bindings that [`deploy-pages.sh`](deploy-pages.sh) merges into the assembled book, so a binding reaches players through the release rather than through the `hosts/book` branch, and [`republish-host.sh --per-host`](republish-host.sh) writes each host's mirror as `<host name>.json` so that one branch carries them all. [`ship-host.sh`](ship-host.sh) takes `EMBER_SHIP_REF` to build and stamp a named ref instead of the published commit, which is how a release puts its hosts in place before the tag is pushed.

Start with [`docs/hosts.md`](../docs/hosts.md) for the host model, use [`README-watchdog.md`](README-watchdog.md) for watchdog operation, and preserve the operational and verification constraints in [`CLAUDE.md`](../CLAUDE.md); shell behavior is pinned under [`tests/`](tests/).
