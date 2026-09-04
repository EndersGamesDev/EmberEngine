# Contributing to ember

Ember is a from-scratch game engine in Rust: wgpu rendering, a deterministic 60 Hz simulation, and games that run natively and in the browser from the same crates. This file is the short version; the deep version is `CLAUDE.md` and `docs/`.

## The one rule

Strict one-way layering: `game → scene/simulation → renderer → platform`. Nothing reaches back up, and nothing above the renderer touches the GPU. If a change needs the renderer to know about a game, the change is wrong.

## Before you touch the sim

`crates/arena-core/` (and its siblings per game) is shared between the client's prediction and the authoritative server. A change there is a protocol question until you have proved it is not. The invariants: fixed 60 Hz, seeded RNG, fixed update order; the only randomness today is two world-generation LCGs — a per-tick RNG would have to be seeded and tick-indexed or replays and rollback die. Bullets are stepped server-side only. An obstacle has a bottom (`base`), and four rules read it — change one, change all four, and re-run the invariants.

The protocol test: an old peer that "plays a different game" when a field is absent gets a `PROTO_VERSION` bump, not a `#[serde(default)]`. Decoding across versions is not the same as playing the same game across versions.

## Prerequisites

- Rust, pinned by `rust-toolchain.toml`; for web builds also the `wasm32-unknown-unknown` target and `wasm-bindgen-cli`.
- A working `python3` for the deploy scripts (not the Windows App Execution Alias stub).
- Git Bash on Windows: the deploy scripts are bash.

## Run it

```
cargo run -p arena --bin arena-app   # the shooter, native; pong v0 at one keyboard
cargo run -p fire --bin fire-app     # Fire Racer
cargo run -p kings --bin kings-app   # Four Kings, hotseat
```

Web builds (the hub is assembled by `bash deploy/deploy-pages.sh`, which does all of this):

```
cargo build --target wasm32-unknown-unknown --release -p arena --lib
wasm-bindgen --target web --no-typescript --out-dir web/pkg target/wasm32-unknown-unknown/release/arena.wasm
```

## Check it

```
cargo test --workspace --exclude linter --no-fail-fast   # the vendored linter's test targets do not build on Windows
cargo clippy --workspace
```

Builds run at minimum priority on the workstation. Measure and report wall time. State plainly what was and was not verified: "compiles" and "reviewed by reading" are different claims, and a commit whose work could not be built says so in its message.

## Where to start

`docs/plans/backlog.md` is the list of real gaps, one line each — several are small and safe (a one-line editor fix, a deploy-script guard, a comment drift). Pick a line, open a PR, and say in the description which line you took. The documents under `docs/` are the design of record; a PR that contradicts one should say so.

## Branch and PR shape

- Work lands on a branch, never directly on `main`. One topic per commit.
- The PR is the coordination surface: what is verified and what is not, what blocked, the next concrete step, what was deliberately not done.
- LF line endings, UTF-8 without BOM, one paragraph per line in markdown.
- A game that brings its own engine, its own runtime or its own build toolchain is not merged and is not put in the launcher, however finished it is: two engines in one repo means two sets of constraints, two deploy paths and two things to keep alive.

## The games

| Game | Crates | Server | Notes |
|---|---|---|---|
| Arena Shooter | `arena` / `arena-core` | `arena-server` | 8-player FPS, three modes, seven guns; the live page is v20 |
| Fire Racer | `fire` / `fire-core` | `fire-server` | drift racing; its own protocol number, on purpose |
| Four Kings | `kings` / `kings-core` | `kings-server` | four-corner chess, 2–4 players, 15 s turns |
| what is this? | `what-is-this` | — | the browser + hardware diagnostic |

Play them: <https://endersgamesdev.github.io/EmberEngine/> — the hub is the front door, and the only link worth sharing, because a tunnel domain changes on every restart.
