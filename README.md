# ember

ember is a from-scratch game engine in Rust, and this repository holds the games and graphics labs built on it.

Each game or lab runs in the browser as one WebAssembly (wasm) bundle.

**[Play ember in your browser](https://endersgamesdev.github.io/EmberEngine/)**

## The games

- [Killshot](https://endersgamesdev.github.io/EmberEngine/games/arena/v31/) — an eight-player first-person arena shooter.
- [Fire Racer](https://endersgamesdev.github.io/EmberEngine/games/fire/v2/) — castle-circuit drift racing with online lobbies.
- [Four Kings](https://endersgamesdev.github.io/EmberEngine/games/kings/v1/) — four-corner chess for two to four players with 15-second turns.
- [UltimateLegue](https://endersgamesdev.github.io/EmberEngine/games/league/v4/) — the current Crystalforge build.
- [what is this?](https://endersgamesdev.github.io/EmberEngine/games/what-is-this/v1/) — a browser and hardware diagnostic suite.

Labs:

- [Julibrot Lab](https://endersgamesdev.github.io/EmberEngine/labs/julibrot/) — the live four-dimensional slice viewer.
- [Browser labs](web/labs/README.md) — the GPU heap and fragment-compute lattice experiments; only Julibrot is currently listed in the launcher.

## Follow the project

- [`CHANGELOG.md`](CHANGELOG.md) records releases, while its Pending section records work that has landed but has not yet been assigned to a release.
- [GitHub Releases](https://github.com/EndersGamesDev/EmberEngine/releases) has one release for every version tag.
- [`docs/plans/backlog.md`](docs/plans/backlog.md) holds open follow-ups and known gaps.

## Run a dedicated server

[`docs/hosts.md`](docs/hosts.md) explains the current multi-host model and operations, [`docs/one-server-evergreen.md`](docs/one-server-evergreen.md) describes the one-server evergreen design, and [`deploy/README.md`](deploy/README.md) maps the deployment and host scripts.

For the shortest first-time path, provision an unprivileged Linux account, clone this repository into `~/ember-host/src`, enter that checkout, and run:

```sh
bash deploy/bootstrap-host.sh
bash deploy/host.sh up
```

This builds, starts, probes, and exposes the game servers; publishing defaults to `none`, so follow the address-book or mirror setup in the host guide when the server should appear in the public launcher.

## Build it yourself

[`CONTRIBUTING.md`](CONTRIBUTING.md) is the contributor orientation and command reference; the separate [`docs/setup.md`](docs/setup.md) walks through the pinned toolchain, a native game, the local browser launcher, and a dedicated server from a fresh checkout.

## How the engine is put together

[`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) maps the crates and hosting flow, [`docs/engine-design.md`](docs/engine-design.md) follows the engine from authority to surface, and [`docs/minimum-requirements.md`](docs/minimum-requirements.md) defines the browser and GPU floor.

The layering is one-way: `game → scene/simulation → renderer → platform`; nothing reaches back up, the renderer owns wgpu, and nothing above it touches the GPU.

## Working in this repository

- [`CLAUDE.md`](CLAUDE.md) sets the repository-wide architecture and working constraints.
- [`docs/worker-protocol.md`](docs/worker-protocol.md) decides branch, hand-off, and verification practice.
- [`docs/versioning.md`](docs/versioning.md) decides series versions, release tags, and frozen release identity.
- [`docs/commit-messages.md`](docs/commit-messages.md) decides commit subjects and the evidence recorded in commit bodies.
- [`docs/readmes.md`](docs/readmes.md) decides how every tracked folder documents its contents, consumers, and governing rules.

## Repository map

| Folder | What it holds | README |
|---|---|---|
| `.github/` | Issue forms and GitHub Actions workflows | [read](.github/README.md) |
| `assets/` | Concept references, layouts, runtime GLBs and textures | [read](assets/README.md) |
| `crates/` | The engine, current games, shared protocols, servers and labs | [per-crate READMEs](crates/) |
| `deploy/` | Pages publication, host lifecycle, address-book and watchdog scripts | [read](deploy/README.md) |
| `docs/` | Architecture, protocol, operations, asset, design and planning records | [read](docs/README.md) |
| `games/` | Frozen hosted game-version contracts and their registry | [read](games/README.md) |
| `marketing/` | Promotion plans, evidence, hand-offs and publication drafts | [read](marketing/README.md) |
| `tools/` | Offline asset, review, release and repository tooling | [read](tools/README.md) |
| `web/` | The launcher, versioned game pages, shared host selection and browser labs | [read](web/README.md) |
