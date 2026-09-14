# Typed web generator

`ember-webgen` is the native release-artifact generator for compiler-owned web boundary declarations. It links the Arena, Fire, Kings, League, loader and catalog/address-book registration slices directly, renders strict embedded Minijinja templates, and writes JSON schemas from the same descriptions. It is a standalone native tool; none of its dependencies enters a game or loader wasm graph.

Run `cargo run -p ember-webgen --release -- --out target/web-generated/$SOURCE_SHA` from the repository root. The source identifier is `EMBER_SOURCE_SHA` when set and otherwise `git rev-parse HEAD`; an empty identifier is refused. The SHA-scoped directory owns provenance and the manifest, while every run removes and refreshes `target/web-generated/ts` as an exact copy of the scoped `ts` compiler inputs. The checked-in TypeScript projects therefore retain stable, reviewable paths without losing commit-specific output.

Generated input types have no index signature. TypeScript already accepts extra keys on non-literal structurally typed values, while object-literal excess-property checks remain useful and an index signature would weaken property access under `noPropertyAccessFromIndexSignature`. The JSON schemas express Serde's actual unknown-key behavior from each descriptor's `deny_unknown_fields`; output declarations list only serialized keys, with `boundary(omit_none)` keys optional and non-null because absence is their only omitted-state representation. A model registered for both directions uses its Input view as the bare canonical alias because consumers parse that shape; emitters use its explicit Output alias.

`GameId` follows the real catalog, while `ServerGameId` is its subset whose address, protocol, version and commit keys all exist on the descriptor-backed `HostEntry`; the generated template-literal host keys cannot therefore invent keys for labs or unhosted games.

The manifest lists every generated file except itself, including byte lengths and SHA-256 hashes, both scoped and fixed compiler roots, the production-template source hash, and every field containing a 64-bit integer. Those integers lower to JavaScript `number` with a declaration comment stating the exactness limit; the field-by-field representation decision remains deferred.

The generated compiler-only compatibility declaration supplies the disposal symbol used by current wasm-bindgen declarations while keeping the checked-in ES2022 project literal and `skipLibCheck` disabled; it does not enter the Pages tree.

Tests render every production template twice, pin the template hash and each emitted byte sequence, compare every union's current variant count, validate the real catalog, and audit a written manifest against its files. The golden-byte test obtains `GameId` and `ServerGameId` from the real `web/games.json`, while `real_catalog_validates_against_the_generated_schema` also reads the catalog as data; regenerate `ts__ember-boundary.d.ts.golden` when the resulting sorted unions change, and otherwise leave unrelated expected bytes alone.

This crate completes the Stage 1 phase 0 generator foundation. No page or hand-written script consumes the declarations yet; phase 1 starts by converting the host-selection, loader and host-check behavior to checked TypeScript templates.
