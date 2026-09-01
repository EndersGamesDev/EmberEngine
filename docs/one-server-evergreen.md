# One server, many games, evergreen versions

**Decision:** one server binary hosts every supported game and every supported version behind one deploy, one endpoint, and one outer protocol. Every hosted version compiles against the current workspace. Its wire protocol and gameplay semantics remain frozen while its plumbing to the engine and server remains free to move.

## 1. The duplication is architectural, not incidental

There are three server crates because the repository has made the server boundary once per game. Counting every Rust source line under each crate at `502414c`, including its tests and examples, gives 1,198 lines for `ember-server`, 1,768 for `pong-server`, and 1,842 for `fire-server`. The first server describes one simulation thread as the sole writer while I/O threads translate socket bytes into events (`crates/ember-server/src/lib.rs:4-11`); the Arena server describes the same ownership pattern around connections and lobbies (`crates/pong-server/src/lib.rs:4-18`); Fire says directly that its structure mirrors Pong's (`crates/fire-server/src/lib.rs:1-11`). That is 4,808 lines around three separately owned loops before counting their clients.

The transport evidence needs a precise correction. These are three independent transport and deployment paths, but not three tungstenite/TLS implementations. Cube world uses length-prefixed Postcard on plain TCP because its WireGuard path is TCP-only (`crates/ember-net/src/lib.rs:4-10`). Arena and Fire each use JSON text over WebSocket; both terminate TLS in the Cloudflare tunnel rather than in the binary (`crates/pong-server/src/lib.rs:8-10`, `crates/fire-server/src/main.rs:1-4`). The duplication remains real: admission, connection tasks, bounded outbound queues, timeouts, rate limits, lobby ownership, shutdown, probes, and error policy have separate owners even where the libraries match.

The lobby protocol is also copied rather than shared. Arena independently defines `Hello`, `ListLobbies`, `CreateLobby`, and `JoinLobby` in `crates/pong-core/src/proto.rs:194-211`; Fire defines the same four operations in `crates/fire-core/src/proto.rs:102-119`. Their replies have already drifted: Arena calls the list `LobbyList` (`crates/pong-core/src/proto.rs:263-278`), while Fire calls it `Lobbies` (`crates/fire-core/src/proto.rs:148-163`). Adding a third online game means choosing one copy as a template, inheriting its accidental policies, and then maintaining another nearly identical surface.

Version ownership shows the same split. Cube world declares protocol 2 (`crates/ember-net/src/lib.rs:17`), Arena declares protocol 12 (`crates/pong-core/src/proto.rs:95`), and Fire declares protocol 1 (`crates/fire-core/src/proto.rs:21-22`). Arena's source records semantic reasons for recent bumps, including a v12 change whose wire and gameplay meaning cannot degrade safely (`crates/pong-core/src/proto.rs:68-94`), but the server can host only the single constant compiled into it. A stale Arena client may list and is then refused at create or join (`crates/pong-server/src/lib.rs:635-704`, `crates/pong-server/src/lib.rs:780-792`). Twelve is therefore not twelve supported contracts; it is the twelfth position of a moving gate, with eleven earlier generations abandoned.

Client networking has forked with the servers. Arena keeps transport, connection state, interpolation, input history, acknowledgement timing, replay, camera smoothing, presentation, and game rules in one 1,960-line `online.rs`; its reconciliation rebases on authoritative position and velocity and replays time slices after the acknowledged instant (`crates/pong/src/online.rs:918-1000`). Fire has a separate 527-line `online.rs`, 260-line `online_game.rs`, and 275-line `net.rs`; it implements the same sequence/history/rebase shape with different game integration (`crates/fire/src/online.rs:109-159`, `crates/fire/src/online.rs:224-261`). The game-specific math should differ. The socket lifecycle, sequence bookkeeping, bounded history, acknowledgement cursor, replay orchestration, interpolation buffer, and connection-state machine should not.

Deployment turns source duplication into operational coupling. Cube world has a WireGuard-only bind, its own source upload, remote build, process kill/start, and port probe (`deploy/deploy-specht.sh:1-38`). Arena and Fire each package and build a different binary, own a different loopback port and process lifecycle, start a different quick tunnel, wait for a newly minted hostname, run a game-specific health client, and publish a different `server.json` key (`deploy/deploy-pong-online.sh:1-8`, `deploy/deploy-fire-online.sh:1-14`). The watchdog installs four units for two public game servers and their two tunnels (`deploy/install-watchdog.sh:28-81`), then probes and redeploys Arena and Fire independently (`deploy/watchdog.sh:58-105`). Every restart carries routing, occupancy, publication, health, and rollback coordination that exists only because game identity is a process boundary.

This is not solved by extracting a WebSocket helper. The owner of admission and lobby identity would remain repeated, game version would remain a compile-time global, deployment would remain multiplied, and client scaffolding would still fork. The correction is to make hosting a server responsibility and game behavior a registered value.

## 2. The contract boundary: frozen behavior, evergreen plumbing

The load-bearing invariant is: **a hosted version's wire protocol and gameplay semantics are frozen contracts; only its plumbing to the engine and server is evergreen.** An old deployed client receives the same game it always received. Internal APIs, adapters, source layout, and runtime integration may be rewritten only while version-specific behavior gates prove that observable contract unchanged.

Frozen wire means byte or text-frame shape, message ordering, defaults, error variants the client can decode, authoritative state meaning, and the transition from lobby traffic into play. A field that decodes but changes what an old peer does is not compatible; the repository's join law already states that distinction (`CLAUDE.md:19-22`). Frozen gameplay means the same seed and timestamped input transcript produces the same authoritative checkpoints and events, including collision, movement, scoring, spawn, weapon, lap, AI, and win rules. Presentation may improve in the latest client, but a server may not silently run v12 rules under the v11 key.

Evergreen plumbing is everything below or beside that observable boundary: engine types, capability traits, task ownership, channel types, registry construction, metrics, logging, rate-limit implementation, scheduling adapters, asset handles, module layout, and the code used to translate a version's result into a transport write. A refactor may replace all of it. It may not change a golden frame or simulation trace unless it cuts a new game version.

Game version is consequently a semantic identity, not a snapshot of the whole repository. `arena/12` does not mean “the engine as it looked when v12 shipped”; it means “the Arena v12 wire and rules implemented on today's engine.” Keeping an old engine checkout would freeze bugs, dependencies, and security posture, prevent one workspace from proving compatibility, and eventually turn every version into a separate product. Recompiling every version against one engine keeps the maintenance debt visible and payable.

The invariant is enforced by a named `hosted-contract` gate. For every manifest entry it runs immutable wire fixtures in both directions, a deterministic simulation transcript from seed plus timestamped inputs to authoritative checkpoints, lobby/create/join/refusal fixtures, and an end-to-end frozen-client transcript through the current server adapter. A plumbing-only change must leave those fixtures unchanged. Deliberately changing a fixture requires a new version directory and selector; review does not permit overwriting the old expectation in place.

“Hosted” is an explicit product state. Once a version is in the hosted manifest, the invariant applies on every change. A deliberate delisting ends the promise to admit that version; it does not license serving different behavior under the old key. An unhosted request is refused by name, and a source deletion removes the key rather than reassigning it.

## 3. One outer protocol

One connection performs one outer hello and uses one lobby surface across all games and versions. Listing is ungated and enumerates `(game_id, game_version, lobby)` tuples. Creation and joining require exact equality for the selected `(game_id, game_version)`. Once joined, the outer layer frames but does not interpret the selected version's inner payload.

### 3.1 Message and state-machine contract

The canonical transport is JSON text over WebSocket. It is the existing browser-capable choice and permits the hub to list without loading a game bundle; Arena already records that reason for JSON (`crates/pong-core/src/proto.rs:1-6`). TLS terminates once in front of the one listener. The outer crate `ember-net` owns the bootstrap decoder, message types, stable identifiers, maximum outer-frame size, and codec tests; it owns no simulation type.

Every canonical connection moves through `AwaitHello → Browsing → Joined → Closed`. In `AwaitHello`, the only legal data message is `Hello { outer_version, handle }`; WebSocket control frames remain legal. `Welcome { outer_version, supported_outer_versions }` acknowledges the selected outer decoder. The bootstrap shape of `Hello` is permanent. If a breaking outer change is ever unavoidable, a new `outer_version` adds a decoder beside the old one; it does not reinterpret or remove an outer version still used by a hosted client. A decodable old outer version can always list. This prevents a global outer bump from recreating the orphaning the registry exists to remove.

In `Browsing`, `ListLobbies` has no game-version gate and returns `Lobbies { entries }`. Each entry contains `game_id`, `game_version`, `lobby_name`, password presence, occupancy, capacity, and a small version-owned status projection such as `waiting` or `racing`; it contains no inner state. Lobby names are scoped by `(game_id, game_version)`, so the same human name may exist in two games or versions without collision. Pagination and a server-issued list generation may be added before scale requires them, but a list response is always a set of full tuples rather than a “latest game” view.

`CreateLobby { game_id, game_version, lobby_name, password }` and `JoinLobby { game_id, game_version, lobby_name, password }` carry the complete selector. Successful admission returns `Joined { game_id, game_version, lobby_name }` and atomically switches the connection into `Joined`. A connection joins exactly one lobby. Returning to the browser requires closing and opening a new connection; this makes every post-join application frame unambiguously inner traffic and removes a control-versus-game tag collision.

In `Joined`, every WebSocket text or binary data message is handed to the selected version's codec as the exact frame payload, and every version-produced data frame is sent without outer JSON wrapping or parse-and-reserialize. The outer layer continues to own WebSocket ping/pong/close, byte and message budgets, bounded outbound queues, timeouts, peer identity, metrics, and disconnect cleanup. It cannot inspect, default, upgrade, compress, or translate inner fields. The version codec owns whether its payload is JSON, Postcard, or a later encoding and owns the first game-specific message after `Joined`.

Malformed outer messages, messages illegal in the current state, oversized frames, repeated hello, or payload before join receive a stable outer error when safe and then close when continued interpretation would be ambiguous. Password failures, full lobbies, and version-owned admission failures remain in `Browsing`. A slow or hostile codec cannot escape its registry limits: the host charges bytes before decode, messages before dispatch, wall budget around a step, outbound bytes before enqueue, and players/lobbies against both per-version and global caps.

### 3.2 Exact equality and useful refusal

The existing join law is retained and generalized, not relaxed. A selection succeeds only if the registry contains the exact key and that entry admits the request. The server never substitutes latest, chooses a “compatible” nearby version, aliases one number to another, or trusts an inner codec to notice a mismatch after state exists. Exact selection occurs before lobby lookup, factory invocation, password work, or inner payload.

A failed selection returns structured `VersionNotHosted { requested_game, requested_version, hosted_versions_for_game }`. An unknown game returns `GameNotHosted { requested_game, hosted_games }`. The lists are derived from the live registry rather than constants in the client or prose in an error. This generalizes today's useful refusal, which names stale and live versions in the Arena test (`crates/pong-server/tests/ws_e2e.rs:239-265`), from one global “live” number to the actual hosted set. It also distinguishes delisting from a missing lobby.

Outer compatibility and game compatibility are separate axes. A connection whose supported outer decoder exists may browse even when none of its desired game versions is hosted. A connection whose hello cannot be decoded cannot be promised listing: “ungated” means independent of game-version equality, not independent of speaking any recognizable lobby protocol.

### 3.3 Existing-client compatibility matrix

The canonical outer wire cannot retroactively appear in a frozen client. Arena and Fire legacy hellos, creates, and joins have the same JSON field shapes, while their version numbers are independent and may collide. Guessing from `proto`, lobby name, first input, or response tolerance would eventually run the wrong game. The one server therefore has a narrow legacy ingress at the same public origin, WebSocket path, listener, process, and deploy: `?legacy_game=arena` or `?legacy_game=fire` selects a manifest-declared legacy decoder before the first frame. The two existing `server.json` keys point to those query forms; canonical clients use the same URL without the query. This is selection metadata for otherwise unchangeable clients, not a second listener, tunnel, server, registry, or deploy, and arbitrary query values are rejected.

After that selector, the version comes from the legacy hello's `proto`. The adapter synthesizes the canonical `(game_id, game_version)` selection internally, projects list entries back into the selected legacy list schema, and hands joined frames to that version's exact codec. A legacy adapter is part of its hosted version and remains only while that version is hosted. This exception is necessary to honor frozen wire; new clients may never use it, and no future game receives a legacy selector.

The source also corrects a tempting claim about `proto: 0`. Both server suites prove that a proto-zero browser can list (`crates/pong-server/tests/ws_e2e.rs:202-266`, `crates/fire-server/tests/ws_e2e.rs:345-381`), and the hub actually sends proto zero to Arena (`web/index.html:331-355`). The deployed Arena v7-v12 page browsers instead send their bundle's `proto_version()` before listing (`web/games/arena/v7/index.html:206-228`, `web/games/arena/v12/index.html:214-236`), and the Fire v2 page does the same with `wasm.proto_version()` (`web/games/fire/v2/index.html:189-219`). Ungated listing makes both patterns work today; it does not make their two response tags or their gameplay hellos into the new outer protocol.

| Existing deployed client at `502414c` | Direct canonical outer URL | Same-server legacy selector | Create or join outcome |
|---|---|---|---|
| New consolidated hub/client | Full outer hello and all-game tuple list | Not used | Exact hosted pair joins; refusal lists the hosted versions for that game. |
| Hub's raw Arena browser (`proto: 0`) | Its old hello is not a canonical outer hello, so no list is promised. | `legacy_game=arena` returns the legacy `lobby_list` projection and preserves its showcase until the hub deploy moves to the canonical list. | The hub itself never creates or joins; it launches a selected Arena page. |
| Tracked Arena v7-v12 page browser | Its old hello and `lobby_list` expectation do not speak the canonical outer wire. | `legacy_game=arena` records the bundle's protocol and returns only Arena lobbies in the legacy schema. | Its frozen gameplay bundle creates or joins exactly that Arena version if the manifest hosts it; otherwise legacy `Error` names the requested and hosted Arena versions. |
| Catalogued Arena v3-v6 build | Not enough source is present at base to claim compatibility. | It is admitted only after its historical source or a complete transcript is recovered, audited, and added as a hosted version; catalog presence alone is not a codec. | No silent mapping to a later Arena version. |
| Fire v2 page browser and gameplay bundle | Its old hello, `lobbies` tag, and selector-free join do not speak the canonical outer wire. | `legacy_game=fire` returns Fire's legacy `lobbies` projection; its protocol-1 gameplay bundle reaches `fire/1`. | It creates or joins Fire 1 while that entry is hosted; refusal uses the legacy `Rejected` variant and names the actual Fire set. |
| Fire v1 page | It imports and starts only `start_local` at base (`web/games/fire/v1/index.html:68-76`), so it sends no hello and has no server behavior to preserve. | Not used. | It remains local; it is not evidence of a Fire network version. |
| Pong Classic v1/v2 pages | Pong Classic is local-only; “Pong protocol 12” is the Arena protocol despite the crate names. | Not used. | No online contract exists to host. |
| Native cube-world `game` client | It speaks length-prefixed Postcard over raw TCP and immediately joins one global world (`crates/game/src/net.rs:45-76`); it cannot handshake with the WebSocket endpoint. | No legacy selector is provided because the demo is retired. | Connection fails cleanly; the catalog never advertised it as a hosted web game. |

This matrix makes the promise bounded and testable. Tracked Arena v7-v12 and Fire protocol 1 are candidates for reconstruction, but only entries with in-tree source and frozen fixtures are hosted. The migration does not claim that an HTML catalog entry, an archived binary without source, or serde's ability to ignore an unknown field is a working game.

## 4. Hosting model and source layout

The server owns an explicit registry from `(game_id, game_version)` to simulation factory, inner codec, resource limits, and behavior-gate identity. Hosted versions are ordinary in-tree source compiled into the server; no old engine snapshot, binary archive, dynamic plug-in ABI, or separately deployed service is part of the model.

`game_id` is a permanent lowercase ASCII slug and `game_version` is a monotonically allocated unsigned integer scoped to that slug. The online shooter is `arena`, not `pong`: the catalog uses those identities distinctly (`web/games.json:3-12`, `web/games.json:91-107`), even though both currently share the `pong` crate. Fire's web-page label v2 and its network protocol 1 are likewise different namespaces. The registry version names the frozen server wire and gameplay contract, not marketing copy, page layout, or asset revision.

Each entry supplies `GameKey`, an inner frame decoder/encoder, a lobby simulation factory, a projection from version state to outer lobby status, `VersionLimits`, a legacy decoder only when required by an already deployed client, and the identifier of its `hosted-contract` fixture suite. The factory receives only legacy capabilities and immutable creation data such as lobby seed and configured rules. The returned session owns version state and exposes deterministic `step(timestamp, inputs)`, join/leave, and outbound-event operations. The host owns connection identity, admission order, clocks, queues, and destruction.

The registry is closed at build time. No network request names a library path, loads a plug-in, downloads code, enables a Cargo feature, or instantiates an unregistered generic. Startup constructs every manifest entry and fails before listening on duplicate keys, missing fixture IDs, duplicate legacy selectors, invalid limits, or a manifest/dependency mismatch. Runtime selection is a lookup in that immutable map.

### 4.1 Versioned crates, not versioned modules

Hosted versions are separate workspace crates under `games/<game>/vNNN/`, with package names such as `ember-game-arena-v12` and `ember-game-fire-v1`. `games/hosted.toml` is the product manifest. `crates/ember-server` is the only server binary, `crates/ember-net` is the outer protocol, `crates/ember-client-net` is shared client scaffolding, and `crates/ember-legacy` is the moving capability surface. A version crate may depend on `ember-legacy` and pure libraries; it may not depend on `ember-server`, a client shell, renderer internals, or another version of itself.

Crates are preferable to modules because the workspace can lint, test, feature-check, and name each contract independently. Unique package names make failures say which frozen contract broke, permit a gate to target one version, and prevent accidental access to a sibling's private module. Modules would make a single large crate cheaper to declare but would share features and dependencies, encourage cross-version imports, and make “all hosted versions were checked” harder to prove.

The cost is real. Each retained version adds parsing and simulation code to the workspace, target directory, test matrix, final link, and server binary; a change to the small legacy surface can rebuild every one. Shared third-party and engine dependencies remain deduplicated, and pure helper crates may be shared only when their behavior is not part of a frozen game rule. A version-specific formula is copied even if identical today, because changing a shared gameplay helper would otherwise change several contracts at once. The hosted set must be curated because compilation is the mechanism that prevents bitrot.

Cutting a version is copy-on-write. Before a wire or gameplay change, copy the current directory to the next unused version, change its package name and `GameKey`, add it to the workspace and hosted manifest, point the latest client at it, and make the new behavior only in the copy. The old directory remains editable for plumbing migrations but not semantic change. This copy is intentional product machinery: it spends source duplication once to retain clients instead of spending a protocol bump to abandon them.

Version numbers are never reused, even after deletion. A move-only refactor within a version requires unchanged fixtures. A behavior change discovered while plumbing is underway stops that change and cuts a new version; “the engine forced it” is not an exception because isolating engine movement is the purpose of `ember-legacy`.

### 4.2 The manifest is the hosted-set authority

`games/hosted.toml` lists, for each key, its package, current/latest flag, limits profile, fixture suite, and optional legacy selector. The server dependency list and generated static registry must match it exactly; a repository check rejects a manifest entry without a workspace crate and a server dependency, a registered version absent from the manifest, two “latest” entries for one game, or a client latest selector not present in the manifest. The manifest, not directory discovery, decides what ships.

The default retention policy is firm: once a network client is publicly distributed, its version stays hosted without an automatic age or count expiry. A new client is not published until its version is present. Delisting is an explicit product and security decision made by a manifest change; valid reasons include an exploitable protocol, unlawful content, an operational cost that cannot be bounded, or a consciously ended service promise. Low traffic alone is not automatic deletion.

Delisting removes the version from list/create/join, updates the catalog and supported-version notice, and leaves a structured refusal naming what remains. Source may then be deleted in the same or a later ordinary commit; keeping an unhosted mausoleum in the workspace buys no compatibility. Security emergencies may delist before a notice period, but the old key is never redirected to new rules. This default preserves clients while keeping the hosted set reviewable rather than accidental.

## 5. `ember-legacy`: a moving in-tree compatibility surface

The name remains `ember-legacy`. `compat` would suggest a stable compatibility promise to outside callers, while this crate exists to keep old in-tree behavior alive through current internals. It is unpublished, has no semantic-version guarantee, and is not an ABI. Its only consumers are current client adapters and hosted version crates.

Dependency direction keeps it small. `ember-legacy` defines capability traits and neutral data values without depending on `ember-server`, `ember-engine`, wgpu, winit, tungstenite, or a game. The sole server and a client adapter implement those capabilities over current internals. A hosted version sees a capability object, not the implementation type. This lets the engine and server move together without compiling renderer or transport internals into every game contract.

The surface covers four categories:

- **Time:** monotonic timestamp and duration values supplied by the host, plus scheduling requests expressed in time rather than a global tick frequency. The version may retain a fixed integrator internally, but outer messages and runtime services do not expose `tick_hz`; current cube wire does expose it in `Welcome` (`crates/ember-net/src/lib.rs:71-79`), which is one reason that protocol is not the outer foundation.
- **Randomness:** deterministic values keyed by game key, lobby seed, stable stream key, and explicit event index. There is no ambient RNG, per-tick RNG, wall-clock seed, or shared mutable stream whose call order can change under plumbing. A version owns the meaning of its keys.
- **Session transport:** opaque peer/session identifiers, bounded unicast and broadcast handles, close reasons, admission metadata, and outer-owned metrics. Versions return inner frames and target sets; they do not hold sockets, tungstenite messages, channels, tasks, TLS state, or lobby maps.
- **Client assets and meshes:** asset lookup by stable logical key, neutral decoded mesh/texture data where legacy clients need it, registration into opaque handles, and diagnostic reporting. The adapter converts these values into current engine objects; versions do not receive renderer, device, queue, surface, swapchain, render-pass, or window types.

The surface does not contain gameplay helpers, physics, collision, prediction policy, reconciliation arithmetic, lobby rules, rendering internals, engine application lifecycle, or a generic escape hatch to downcast current services. A convenience added for one version is presumed version code until two consumers demonstrate a plumbing concept. Smallness matters because every exported operation is a multiplied maintenance obligation.

Instability is a feature because all consumers are in one workspace. The engine may replace a mesh representation, the server may move from threads to tasks, and time may become a stronger type without preserving old implementation-shaped methods forever. The compiler identifies every use immediately, workspace lints apply to each version crate, and behavior gates catch accidental meaning change in the same series. A stable external API would either fossilize present mistakes or grow adapters that nobody can safely remove.

The update duty is atomic at repository scale: a change that moves `ember-legacy` updates every hosted consumer in the same series, and per-version behavior gates demonstrate that evergreen rewiring did not change frozen semantics.

That duty is not satisfied by making the workspace compile with an untested default. The moving change runs every hosted version's wire fixtures, deterministic traces, and frozen-client transcript after the update. If one version cannot be adapted without changing behavior, the engine change has not landed; either the plumbing design changes or a deliberate product decision delists that version. The cost is intentionally paid at change time rather than deferred into bitrot.

The honest consequence is breadth cost: one engine change can require mechanical edits across many old crates, review must distinguish plumbing from semantics, and the build estate pays for all of them. That is why the API is narrow and the hosted set explicit. It is still cheaper and safer than operating old engines, frozen binaries, vulnerable dependencies, and separate deploys.

## 6. Client-side consolidation

`ember-client-net` owns WebSocket lifecycle on native and web, outer hello/list/select, connection progress, keepalive, bounded inbox/outbox, monotonic input sequence allocation, bounded unacknowledged history, authoritative acknowledgement trimming, replay orchestration, remote-snapshot buffering, and connection diagnostics. It does not own a car, player body, camera, weapon, lap, or render frame.

Per-game hooks define input and authoritative-state types, encode/decode inner frames, extract acknowledgement and server timestamp, apply authoritative state, replay one timestamped input slice, interpolate or dead-reckon a remote entity, and decide when a discontinuity snaps rather than smooths. Arena can retain its acknowledged-command timing and vertical-integrator rules; Fire can retain fixed-step car replay. Sharing the scaffold means a background-tab keepalive, queue bound, WebSocket error, or sequence-wrap fix lands once without pretending the games have identical physics.

The current split demonstrates the seam. Fire waits for `Welcome` before its gated join (`crates/fire/src/online_game.rs:156-184`), while Arena queues hello and create/join together (`crates/pong/src/online.rs:258-278`). The shared outer state machine removes both game-owned variants. Fire's input history is capped at 120 (`crates/fire/src/online.rs:31-34`); Arena caps a differently shaped history and replays with acknowledgement age. The library owns capacity and cursor mechanics, while each hook owns what an input means and how time is integrated.

The deployed web hub ships only the latest client code for each game. It lists every hosted version because the server does, but launching an old version uses the already deployed frozen page/bundle or a deliberately retained client artifact; the current bundle is never asked to impersonate an old wire contract. The server is what keeps that artifact playable: it retains the selected simulation and codec and continually rewires their internals through `ember-legacy`.

This guarantee begins only where evidence exists. Arena v7-v12 and Fire protocol 1 can be admitted after their source and fixtures are reconstructed; catalog entries without recoverable source are refused rather than guessed. Every version cut after consolidation is recoverable by construction because source, fixtures, selector, and client artifact are added together. Pong Classic remains local, and the cube client is deliberately retired rather than used to justify a second transport stack.

## 7. Crate disposition

The eleven-crate inventory is the base workspace's game and engine surface (`Cargo.toml:1-16`); tools remain tools and are outside this disposition. New crates are `ember-client-net`, `ember-legacy`, and the version crates under `games/`. Reusing `ember-net` for the outer protocol and `ember-server` for the host keeps the durable names attached to their intended repository-wide responsibilities.

| Base crate | Target disposition | Reason |
|---|---|---|
| `ember-editor` | Keep as the editor; retarget its Arena dependency to the latest Arena version crate. | It authors game data but owns neither hosting nor compatibility; its current direct `pong-core` dependency shows the path that must move (`crates/ember-editor/Cargo.toml:15-20`). |
| `ember-engine` | Keep as the one current engine, with adapters outside it implementing `ember-legacy`. | Every current and historical client-side source consumer must compile against this engine; renderer ownership does not move (`crates/ember-engine/src/lib.rs:5-11`). |
| `ember-net` | Replace the cube protocol with the universal outer WebSocket protocol and bootstrap codecs. | A repository-wide network name should own hello/list/select/framing, while game payloads and cube constants do not belong in the outer layer. |
| `ember-server` | Replace the cube simulation with the sole host binary and registry runtime. | Its single-writer ownership is the right authority model, but admission, limits, lobby tuples, dispatch, metrics, and shutdown become game-neutral. |
| `fire` | Keep as the local and latest Fire client; move generic networking to `ember-client-net` and implement Fire hooks. | Rendering, controls, and car-specific replay remain game code; its WebSocket channel and connection progress are shared scaffolding (`crates/fire/src/lib.rs:9-17`). |
| `fire-core` | Move into `games/fire/v001` and remove the base crate after consumers retarget. | Its simulation and protocol jointly define Fire 1 (`crates/fire-core/src/lib.rs:6-17`), so they belong inside that frozen version boundary rather than a moving unversioned crate. |
| `fire-server` | Delete after its generic host code, version behavior, tests, and probe move. | Connection/lobby/runtime ownership goes to `ember-server`; race behavior goes to `games/fire/v001`; retaining the binary would preserve a second deploy boundary. |
| `game` | Retire and delete the cube-world demo rather than host it. | It is a native raw-TCP global-world demo with no catalogued lobby contract (`crates/game/src/main.rs:6-10`); keeping it would force the universal server to carry a second transport for a superseded demonstration. |
| `pong` | Keep as the Pong Classic and latest Arena client shell; absorb local Pong `sim`, move generic networking out, and implement Arena hooks. | The crate currently contains two products: local Pong presentation and the online Arena client (`crates/pong/src/lib.rs:6-10`); neither is a server host. |
| `pong-core` | Split and remove: move `proto` plus `shooter` to `games/arena/v012`, and move local `sim` into `pong`. | Its present module list combines an online Arena contract with unrelated local Pong simulation (`crates/pong-core/src/lib.rs:6-12`); copying the whole crate would misname the hosted game. |
| `pong-server` | Delete after its generic host code, Arena behavior, tests, and wsbot move. | Connection/lobby/runtime ownership goes to `ember-server`; Arena behavior goes to version crates; retaining the binary would preserve the copied server and tunnel. |

The cube demo is retired, not selected as the first hosted game. It exercises the easier protocol—one raw-TCP world with no lobby selector—while Arena already has public WebSocket clients, lobby browsing, exact refusal, hostile-input limits, and the largest reconciliation burden. Migrating Arena first tests the abstraction against the difficult existing contract; preserving cube world would test less and permanently charge more.

## 8. Relation to authority and time

The [authority-and-time proposal](https://github.com/EndersGamesDev/EmberEngine/issues/2) treats wire-visible tick dissolution as a deliberate protocol event. Consolidation is such an event: the new outer protocol begins with timestamps and durations and never carries `tick_hz`; each inner protocol drops wire-visible ticks only in its own new version cut, so Arena 12 and Fire 1 may retain their present fields unchanged. One server makes “one writer of truth” literal as ownership: each lobby session has exactly one authoritative executor even if different sessions run concurrently. This design needs neither the proposal's exact types nor its landing order; if the law changes, the frequency-free outer boundary and per-version cuts still stand.

## 9. Honest costs and enforcement

One binary concentrates failure. A bad host release can affect every game, a global admission bug can exhaust all capacity, and a pathological version can starve siblings. The counterweight is isolation inside the process: global caps reserve headroom, each registry entry has player/lobby/frame/message/outbound/time limits, each lobby has one authority owner, queues are bounded, panics are contained at the session boundary where Rust permits, and metrics are labelled by game and version. The deployment gate must prove that overload or failure in one fixture game leaves another listable and playable.

Evergreen versions increase source, compile, lint, test, link, binary, and review cost. Copy-on-write intentionally duplicates semantic code, and a legacy-surface move touches every consumer. The costs are bounded by keeping `ember-legacy` narrow, deduplicating only behavior-neutral libraries, retaining versions by an explicit policy, and requiring the moving change to update every consumer in the same series. They are not hidden behind old binaries that no current build can verify.

Legacy ingress is permanent for a hosted pre-consolidation client and therefore security surface. Its selector is a closed manifest value, its outer budgets apply before legacy decode, and its wire/transcript fixtures run beside canonical ones. It receives no new game, feature, or general negotiation path. Removing it requires delisting every client that depends on it, not merely deleting inconvenient adapter code.

The verification stack has distinct jobs: workspace build and lints prove every source consumer follows current Rust and internal APIs; `hosted-contract` proves frozen frames and gameplay traces; registry tests prove manifest completeness, unique keys, exact selection, and refusal contents; cross-game host tests prove isolation and limits; frozen-client end-to-end tests prove real hello/list/create/join/play transcripts; deployment probes prove the public endpoint, every selected version, drain, and rollback. Compilation is necessary plumbing evidence and never substitutes for behavior evidence.

## 10. Migration: buildable steps, visible behavior

Each stage is sized for one implementation lane, identifies exact responsibility moves, names gates, and states what deployed clients experience. The build estate is unavailable for this design lane, so these are required future gates rather than claims about checks already run.

### Stage 1 — Extract the outer transport and host one latest game

Repurpose `ember-net` as the canonical outer protocol, repurpose `ember-server` as a WebSocket host, add the first `ember-client-net` transport seam, and adapt current `pong-core` Arena 12 behind a provisional static entry. Move the strongest admission, queue, timeout, rate-limit, lobby, and single-writer rules from `pong-server`; retire `game` and the cube-specific protocol/server behavior in the same buildable change because their old dependency contract no longer exists.

Gates: workspace build/lints; outer codec and state-machine fixtures; exact `(arena,12)` create/join/refusal; no inner frame before join; byte/message/queue limits; Arena server simulation parity against `pong-server`; and cube crate absence from the workspace. Deployed clients continue using `pong-server`; no public URL changes and no client is claimed migrated.

### Stage 2 — Add the second latest game

Adapt `fire-core` protocol 1 behind the host, move generic Fire WebSocket/client lifecycle into `ember-client-net`, and leave car replay as a Fire hook. Keep `fire-server` and `pong-server` buildable against their existing cores for deployment continuity.

Gates: Fire simulation and wire parity; one all-game list containing distinct `(arena,12,...)` and `(fire,1,...)` rows; identical lobby names coexisting; codec dispatch never crossing keys; per-version and global limits; and a fault/overload in one game leaving the other playable. Both public old servers remain authoritative, so deployed clients see no change.

### Stage 3 — Introduce the registry and version-crate layout

Add `games/hosted.toml` and `ember-legacy`; move Arena `proto` plus `shooter` into `games/arena/v012`, move Fire core into `games/fire/v001`, move local Pong `sim` into `pong`, and retarget `pong`, `fire`, `ember-editor`, `pong-server`, `fire-server`, and the sole host. This is the first proof of the update duty: the change that introduces or moves the legacy API updates every in-tree consumer in the same series, with no compatibility branch left stale.

Gates: manifest/dependency/registry equality; unique keys and latest flags; workspace build/lints over every version crate and client target; `hosted-contract` wire, refusal, and deterministic traces for Arena 12 and Fire 1; editor build against the latest Arena crate; both old binaries still passing their parity tests. Deployed processes and URLs remain unchanged.

### Stage 4 — Cut the first evergreen old version

Copy `games/arena/v012` to `v013` before changing wire or rules, register both, move the latest Arena client to outer version 1 plus Arena 13, and retain Arena 12 through its manifest-declared legacy selector. Any first post-consolidation gameplay change lands only in v13; if none is ready, the outer transition itself is sufficient reason for the cut because its client wire changes.

Gates: unchanged v12 golden frames and deterministic trace; v12 frozen page/bundle hello, list, join, and play through `legacy_game=arena`; v13 canonical hello, tuple list, exact join, and new fixtures; simultaneous v12/v13 lobbies with the same name; refusal listing both hosted versions; and an `ember-legacy` test mutation adapted across both versions without fixture movement. Old public servers still serve deployed clients, while the new path is exercised only by staging clients.

### Stage 5 — Retire per-game server binaries

Move wsbot, Fire probe, end-to-end suites, occupancy, health, metrics, graceful session drain, and deployment ownership into `ember-server`; delete `pong-server` and `fire-server`; consolidate their scripts into one server build, one process, one named tunnel, and one health publication. Old already-running binaries remain available as rollback artifacts until switchover drains them; source no longer treats them as deploy targets.

Gates: every old test responsibility has a named new owner; public-style probes cover canonical list and every hosted key; occupancy and drain are game/version labelled; shutdown stops admission before sessions; rollback can restore the previous one-server artifact; workspace and `hosted-contract` are green without either old crate. Deployed clients still use the old processes.

### Stage 6 — Switch deployment and drain old servers

Start the sole server and stable tunnel without stopping either old server. Publish the canonical URL to new clients and publish the same origin/path with `legacy_game=arena` and `legacy_game=fire` query selectors in the old `ws` and `fire_ws` keys. This stops new page loads from entering old processes while existing WebSocket sessions continue there. When their occupancy reaches zero, stop the old processes and tunnels; do not kill active games to complete a deploy.

Gates: the compatibility matrix in §3.3 against public routing; create/play probes for every manifest key; hub all-game listing; legacy list tags for Arena and Fire; frozen-client transcripts; cross-game isolation under load; metrics and alerts from the single process; admission-stop and zero-occupancy drain; and a timed rollback that restores routing before the old artifacts are discarded. New clients see one all-game endpoint, retained frozen clients see their exact game through the same server, local-only games are unaffected, and explicitly unhosted clients receive an intelligible refusal rather than substituted gameplay.

## 11. Resolved defaults and remaining product questions

The defaults are settled: `ember-server` is the sole binary; `ember-net` owns a JSON/WebSocket outer protocol; versioned workspace crates are used instead of modules; `ember-legacy` is the deliberately unstable name and surface; public network versions are retained without automatic expiry; the cube demo is retired; Arena migrates first; old servers drain rather than being kicked; and legacy query selection exists only for already deployed Arena and Fire clients.

Open product and rollout questions remain, but none changes those boundaries:

- Which catalogued Arena versions have recoverable source and frozen bundles sufficient to enter the initial manifest? Base directly evidences pages v7-v12 but not a buildable source tree per version; each earlier admission needs an audit rather than a blanket promise.
- Who owns delisting approval, player notice, and emergency security removal, and where is the supported-version notice published? The default is retain; this question assigns the exceptional decision.
- What measured CPU, memory, outbound-byte, connection, lobby, and step-time budgets belong in the first Arena and Fire `VersionLimits` profiles? The architecture requires hard caps, while their numeric values need estate measurements.
- What stable public domain and certificate/tunnel arrangement replaces quick-hostname publication, and how long must the legacy `server.json` keys remain cached? The design requires one stable origin and same-listener selectors; operations must choose the concrete provider configuration.
- Which retained client artifacts are distribution commitments in addition to server source, and how are their hashes recorded beside `hosted-contract` fixtures? The server can preserve behavior only if players can still obtain or already possess the matching client.
