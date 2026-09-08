# Setup

This is the shortest repository-backed path from a fresh checkout to a native game, the local browser launcher, and a dedicated Linux host; [`CONTRIBUTING.md`](../CONTRIBUTING.md) explains the project rules and where to start contributing.

## Toolchain

The repository pins the minimal `nightly-2026-09-01` Rust toolchain in [`rust-toolchain.toml`](../rust-toolchain.toml), including `clippy`, `rustfmt`, and the `wasm32-unknown-unknown` target.

Browser builds also require `wasm-bindgen-cli`, and the deployment scripts require a working `python3`; these prerequisites come from [`CONTRIBUTING.md`](../CONTRIBUTING.md#prerequisites).

## Run one game natively

From the repository root, run Killshot:

```sh
cargo run -p arena --bin arena-app
```

That command is the native Arena command in [`CONTRIBUTING.md`](../CONTRIBUTING.md#run-it).

## Build and serve the browser launcher

The launcher itself is the static `web/` tree; the following repository recipe builds its Julibrot wasm entry and serves the whole tree locally:

```sh
cargo build --target wasm32-unknown-unknown --release -p ember-julibrot-app --lib
wasm-bindgen --target web --no-typescript --out-dir web/labs/julibrot/pkg target/wasm32-unknown-unknown/release/ember_lab_julibrot.wasm
python3 -m http.server 8000 --directory web
```

The three commands are the served-proof recipe in [`docs/julibrot/refactor-survey.md`](julibrot/refactor-survey.md); open `http://localhost:8000/` for the launcher or `http://localhost:8000/labs/julibrot/` for the built lab.

The full release assembly is handled by [`deploy/deploy-pages.sh`](../deploy/deploy-pages.sh), which builds every current bundle and publishes the Pages tree; it is not the local serving command.

## Run a dedicated server

On an unprivileged Linux account with Git, the Rust toolchain, `python3`, `curl`, and `sha256sum`, clone the repository into `~/ember-host/src`, enter that checkout, and run:

```sh
bash deploy/bootstrap-host.sh
bash deploy/host.sh up
```

These commands and the checkout location are the first-time host walkthrough in [`docs/hosts.md`](hosts.md#8-running-a-host); the bootstrap installs the pinned tunnel binary, and `host.sh up` builds, starts, probes, and exposes the game servers.

Publishing defaults to `none`, so read the same host guide before setting `EMBER_PUBLISH` to the upstream address book or to a host-owned mirror.
