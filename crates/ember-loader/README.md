# ember-loader

The shared browser loader: the rules a live page follows while its game arrives.

A page used to learn which server it would join only after its game bundle had downloaded, compiled and initialised, because the protocol number that filters hosts came from the bundle's own export. On the page carrying the largest bundle that is about a minute of a host chip reading "server …", with nothing said about how far the download had got. The protocol is in `web/games.json` already, so nothing about discovery needs the bundle.

This crate owns the load's rules and nothing else: the phase machine, the download arithmetic, the stall test and the one line each event carries. It holds no socket, opens no request and touches no DOM. `web/loader.js` performs the browser input and output — discovery through `web/hosts.js`, the counting fetch, the compile, and the call into the game's own wasm-bindgen glue — and drives this machine with the clock as it goes.

The split is what makes the rules testable: a test here is a list of calls and timestamps with no browser anywhere, and `cargo test -p ember-loader` runs natively with no dependencies at all. The browser bindings are behind `cfg(target_arch = "wasm32")`, so a native build of the workspace compiles the rules alone.

Every call carries the page's own clock instead of reading one. That is what lets `web/loader.js` start the fetch and discovery in the page's first tick and replay the drive calls afterwards, in order and with their real timestamps, once this module has instantiated: the loader's own arrival never delays the work it reports on.

Two tracks run at once. Host discovery and the bundle download start together and neither waits for the other, so a host failure leaves the bundle track running — a page with no server still loads its game and says so — while a failure on the bundle track ends the load, because there is nothing after a bundle that did not arrive.

The machine does not trust its driver. Every call is checked against the state the load is actually in, and an out-of-order drive produces a visible refusal rather than a percentage computed from a download that had not started.

## Layering

Platform code: no game crate, no engine crate, no dependency at all when built natively. One bundle for every page, built once by `deploy/deploy-pages.sh` and shipped once, so no game carries loading code of its own.

## Building

The deploy builds it with the game bundles:

```text
cargo build --target wasm32-unknown-unknown --release -p ember-loader --lib
wasm-bindgen --target web --no-typescript --out-dir web/pkg target/wasm32-unknown-unknown/release/ember_loader.wasm
```

Every byte ships to every player on every page, so the size of `ember_loader_bg.wasm` is a budget, not an afterthought.

The page-side half, its events and the fallback when this bundle cannot be loaded are described in [`../../web/README.md`](../../web/README.md); the host model it defers to is [`../../docs/hosts.md`](../../docs/hosts.md).
