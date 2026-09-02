# ember architecture

## Crate responsibilities and boundaries

The workspace separates current platform plumbing from frozen game contracts: engine, transport, hosting, and client lifecycle may evolve together, while every hosted version preserves its wire protocol and gameplay semantics.

* **`ember-engine`** is the platform and renderer layer. It owns windowing, GPU resources, the `EmberGame` application boundary, scene rendering, and presentation.
* **`ember-legacy`** is the narrow moving capability surface between current plumbing and hosted versions. It defines neutral time, randomness, session, frame, and asset values without exposing sockets, renderer internals, or game rules.
* **`ember-net`** owns only the canonical game-neutral outer JSON/WebSocket protocol. It defines hello, lobby listing, exact game/version selection, admission responses, outer errors, frame limits, and the pre-join connection state machine.
* **`ember-server`** is the sole game-neutral host. It owns the listener, connection lifecycle, admission, registry, bounded queues, limits, lobby execution, diagnostics, and dispatch of opaque joined payloads to a selected version.
* **`ember-client-net`** owns reusable client connection plumbing on native and wasm, including outer handshake state, WebSocket lifecycle, bounded queues, sequence/history bookkeeping, replay orchestration, and connection diagnostics.
* **`games/<game>/vNNN`** crates own frozen game wire and gameplay contracts. The current registry contains `games/arena/v012` and `games/fire/v001`; each version depends on `ember-legacy` rather than host or renderer implementation types.
* **`games/hosted.toml`** is the hosted-set authority. It declares each exact game/version key, package, latest flag, limits profile, fixture suite, and any legacy selector; server dependencies and registry entries must match it.
* **`arena`** and **`fire`** remain the current client shells. They own rendering, controls, and game-specific prediction or replay behavior above shared engine and networking plumbing.

Dependency flow is inward toward narrow contracts: client shells use `ember-engine` and shared client plumbing, hosted versions use `ember-legacy`, and `ember-server` composes the outer protocol with the closed hosted registry.

## One-server protocol and hosting flow

A canonical connection moves through `AwaitHello`, `Browsing`, `Joined`, and `Closed`. Before admission, `ember-net` decodes only outer messages; after exact `(game_id, game_version, lobby_name)` admission, `ember-server` forwards each text or binary payload unchanged to the selected version codec.

Listing is independent of game-version compatibility and projects full game/version/lobby tuples. Create and join require an exact hosted key; the server neither substitutes a nearby version nor interprets a joined version's inner messages.

At startup, `ember-server` constructs the registry from the two hosted version crates and verifies it against `games/hosted.toml`. Each lobby has one authoritative session owner, while the host retains responsibility for clocks, peer identity, admission order, budgets, outbound routing, and cleanup.

Arena 12 and Fire 1 carry legacy ingress adapters for already deployed clients. These adapters translate only the pre-join legacy lobby surface and then preserve each version's exact inner protocol; new clients use the canonical outer protocol.

## Honest interim migration state

The versioned Arena and Fire contracts coexist with `arena-core` and `fire-core`, and the deploy-continuity binaries `arena-server` and `fire-server` still build and run. They remain until the probe, end-to-end, health, drain, and deployment responsibilities move into the sole host and public routing switches during [migration stages 5 and 6](one-server-evergreen.md#10-migration-buildable-steps-visible-behavior).

The `arena` crate's Arena client still imports its online protocol and shooter simulation through `arena-core`; it has not yet moved to `ember-client-net` or the versioned Arena crate. As of arena v13 that live pair speaks `PROTO_VERSION` 13: the arena is an **authored `Level` named by the server** (`GameJoined.map`, resolved by `Level::named`) rather than a seed every peer regenerates, which is why the version moved — an old client would predict against a different set of obstacles than the server resolves against — while the frozen `games/arena/v012` keeps hosting protocol 12 unchanged. Fire has begun using `ember-client-net`, while its existing server path continues through `fire-core` for deployment continuity.

The native raw-TCP cube demo, its Postcard protocol, its verification bot, and its dedicated deploy path are retired. `ember-net` and `ember-server` no longer retain a second cube-specific transport or simulation surface.

## Frozen behavior and deterministic simulation

Hosted wire fixtures, deterministic transcripts, lobby/admission fixtures, and frozen-client transcripts form the semantic boundary. Plumbing changes may alter source layout and internal APIs, but a behavior change requires a new version directory and manifest key rather than rewriting an existing contract.

Arena 12 and Fire 1 retain their established fixed-step rules inside their version boundaries. The outer protocol carries versions, timestamps, limits, and opaque payloads without imposing a global gameplay tick rate.

## Hosts

The game servers run on many independent machines at once. They share no state and never talk to each other; what they share is one published address book, `server.json` on the Pages site, which lists every host with its addresses, its build and the protocol each of its servers speaks. The client does the choosing — `web/hosts.js` loads the book, probes every host, and picks the newest build that speaks its own protocol. The model, the file format, the `Welcome` fields that carry a server's identity, and how to run a host are all in **`docs/hosts.md`**.

## Rendering and assets

`ember-engine` renders scene geometry into offscreen color and depth targets, then presents the scene texture through the fullscreen presenter pass. This keeps surface acquisition and swapchain presentation below game code and permits presentation transforms without changing simulation.

Client shells submit cameras, instances, meshes, and texture data through engine-owned types. GLB loading and procedural geometry remain client-side presentation concerns; hosted server versions receive no renderer, GPU, window, or asset-loader implementation objects.
