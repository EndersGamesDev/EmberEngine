# ember

ember is a from-scratch Rust game engine plus the games and graphics labs built on it. The launcher lists each published engine-backed title and loads one wasm bundle for the selected game or lab.

**Play the live site:** [endersgamesdev.github.io/EmberEngine](https://endersgamesdev.github.io/EmberEngine/)

The projects are non-commercial, and the launcher has no shop or advertising. Release history, build provenance and protocol compatibility are recorded in [`CHANGELOG.md`](CHANGELOG.md).

## Repository map

| Folder | What it holds | README |
|---|---|---|
| `.github/` | Issue forms and GitHub Actions workflows | [read](.github/README.md) |
| `assets/` | Concept references, layouts, runtime GLBs and textures | [read](assets/README.md) |
| `crates/` | The engine, current games, shared protocols, servers and labs | [read](crates/README.md) |
| `deploy/` | Pages publication, host lifecycle, address-book and watchdog scripts | [read](deploy/README.md) |
| `docs/` | Architecture, protocol, operations, asset, design and planning records | [read](docs/README.md) |
| `games/` | Frozen hosted game-version contracts and their registry | [read](games/README.md) |
| `marketing/` | Promotion plans, evidence, hand-offs and publication drafts | [read](marketing/README.md) |
| `tools/` | Offline asset, review, release and repository tooling | [read](tools/README.md) |
| `web/` | The launcher, versioned game pages, shared host selection and browser labs | [read](web/README.md) |

Every tracked folder documents its contents, consumers and governing rules in a local README; [`docs/readmes.md`](docs/readmes.md) defines and explains that policy.

## Architecture

The dependency direction is strict and one-way: `game → scene/simulation → renderer → platform`. A lower layer never reaches back into a higher one, the renderer owns wgpu, and nothing above it touches the GPU.

The current crate boundaries and hosting flow are mapped in [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md). The deeper engine argument, asset compiler direction and GPU-table model live in [`docs/engine-design.md`](docs/engine-design.md), while device constraints live in [`docs/minimum-requirements.md`](docs/minimum-requirements.md).

## Build and run

Rust is pinned by [`rust-toolchain.toml`](rust-toolchain.toml). Browser builds also need the `wasm32-unknown-unknown` target and `wasm-bindgen-cli`.

### One game: Arena

Build and launch the native Arena client with the existing game command:

```sh
cargo run -p arena --bin arena-app
```

### One lab: Julibrot

Build the lab bundle, generate its browser bindings and serve `web/` with the existing Julibrot commands from [`docs/julibrot/refactor-survey.md`](docs/julibrot/refactor-survey.md):

```text
cargo build --target wasm32-unknown-unknown --release -p ember-julibrot-app --lib
wasm-bindgen --target web --no-typescript --out-dir web/labs/julibrot/pkg target/wasm32-unknown-unknown/release/ember_lab_julibrot.wasm
python3 -m http.server 8000 --directory web
```

Open `http://localhost:8000/labs/julibrot/`. Full multi-game Pages assembly and publication belong to [`deploy/deploy-pages.sh`](deploy/deploy-pages.sh), not to a hand-maintained root recipe.

## Rules and project record

- [`CLAUDE.md`](CLAUDE.md) contains the repository-wide architecture and working constraints every change must obey.
- [`docs/worker-protocol.md`](docs/worker-protocol.md) defines branches, hand-offs, verification claims and the repository-as-medium rule.
- [`docs/versioning.md`](docs/versioning.md) defines compatibility, frozen releases and series-prefixed three-grade tags such as `arena-31.1.0` and `ember-1.1.0`.
- [`docs/commit-messages.md`](docs/commit-messages.md) defines Conventional Commit subjects and the evidence expected in commit bodies.
- [`CHANGELOG.md`](CHANGELOG.md) is the record of finished game and lab releases; completed-work lists do not live in this README.

The launcher catalogue and version notes live in [`web/games.json`](web/games.json), multiplayer and host selection live in [`docs/one-server-evergreen.md`](docs/one-server-evergreen.md) and [`docs/hosts.md`](docs/hosts.md), asset production lives in [`docs/asset-pipeline.md`](docs/asset-pipeline.md), and unfinished work lives in [`docs/plans/backlog.md`](docs/plans/backlog.md).

## Contributing

Read [`CLAUDE.md`](CLAUDE.md) and the README nearest the files you intend to change, then follow the focused design document for that subsystem. Keep the one-way layering intact, treat shared simulation edits as protocol questions, and add focused verification for changed behavior.

Work on a branch, keep each commit to one coherent topic, use the format in [`docs/commit-messages.md`](docs/commit-messages.md), and state exactly what was and was not verified. [`CONTRIBUTING.md`](CONTRIBUTING.md) has the practical prerequisites and starting points; open work is tracked in [`docs/plans/backlog.md`](docs/plans/backlog.md).
