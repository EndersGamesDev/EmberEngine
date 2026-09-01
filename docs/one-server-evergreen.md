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

The source also corrects a tempting claim about `proto: 0`. Both server suites prove that a proto-zero browser can list (`crates/pong-server/tests/ws_e2e.rs:202-266`, `crates/fire-server/tests/ws_e2e.rs:345-381`), and the hub actually sends proto zero to Arena (`web/index.html:331-355`). The deployed Arena v7-v12 page browsers instead send their bundle's `proto_version()` before listing (`web/games/arena/v12/index.html:214-236`), and the Fire v2 page does the same with `wasm.proto_version()` (`web/games/fire/v2/index.html:189-219`). Ungated listing makes both patterns work today; it does not make their two response tags or their gameplay hellos into the new outer protocol.

| Existing deployed client at `502414c` | Direct canonical outer URL | Same-server legacy selector | Create or join outcome |
|---|---|---|---|
| New consolidated hub/client | Full outer hello and all-game tuple list | Not used | Exact hosted pair joins; refusal lists the hosted versions for that game. |
| Hub's raw Arena browser (`proto: 0`) | Its old hello is not a canonical outer hello, so no list is promised. | `legacy_game=arena` returns the legacy `lobby_list` projection and preserves its showcase until the hub deploy moves to the canonical list. | The hub itself never creates or joins; it launches a selected Arena page. |
| Tracked Arena v7-v12 page browser | Its old hello and `lobby_list` expectation do not speak the canonical outer wire. | `legacy_game=arena` records the bundle's protocol and returns only Arena lobbies in the legacy schema. | Its frozen gameplay bundle creates or joins exactly that Arena version if the manifest hosts it; otherwise legacy `Error` names the requested and hosted Arena versions. |
| Catalogued Arena v3-v6 build | Not enough source is present at base to claim compatibility. | It is admitted only after its historical source or a complete transcript is recovered, audited, and added as a hosted version; catalog presence alone is not a codec. | No silent mapping to a later Arena version. |
| Fire v2 page browser and gameplay bundle | Its old hello, `lobbies` tag, and selector-free join do not speak the canonical outer wire. | `legacy_game=fire` returns Fire's legacy `lobbies` projection; its protocol-1 gameplay bundle reaches `fire/1`. | It creates or joins Fire 1 while that entry is hosted; refusal uses the legacy `Rejected` variant and names the actual Fire set. |
| Fire v1 page | It is local-only at base, so it sends no hello and has no server behavior to preserve. | Not used. | It remains local; it is not evidence of a Fire network version. |
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

The final table will give each of the eleven base crates one target and one reason; new outer-protocol, shared-client, legacy-adapter, and version-crate boundaries will be named explicitly.

| Base crate | Target disposition | Reason |
|---|---|---|
| `ember-editor` | Remains the editor | Its responsibilities are outside multiplayer hosting. |
| `ember-engine` | Remains the current engine | Every hosted version rebases onto this one engine. |
| `ember-net` | Outer-protocol disposition to be finalized | Its present cube-world protocol must be separated from the universal lobby and session contract. |
| `ember-server` | Becomes the sole server binary | The existing binary is the natural deploy owner, subject to evidence from its present role. |
| `fire` | Latest Fire client plus shared-net hooks | Game presentation remains per game while generic online machinery moves out. |
| `fire-core` | Versioned Fire simulation and codec source | A frozen Fire behavior contract needs a named in-tree version boundary. |
| `fire-server` | Retired after migration | Generic hosting moves to the sole server and Fire behavior moves to versioned source. |
| `game` | Demo disposition to be decided firmly | Its value as a hosted contract must be weighed against carrying a third compatibility burden. |
| `pong` | Latest Pong client plus shared-net hooks | Game presentation remains per game while generic online machinery moves out. |
| `pong-core` | Versioned Pong simulation and codec source | A frozen Pong behavior contract needs a named in-tree version boundary. |
| `pong-server` | Retired after migration | Generic hosting moves to the sole server and Pong behavior moves to versioned source. |

## 8. Relation to authority and time

The [authority-and-time proposal](https://github.com/EndersGamesDev/EmberEngine/issues/2) treats wire-visible tick dissolution as a deliberate protocol event. Consolidation is such an event: the new outer protocol begins frequency-free and timestamped, while each game's inner payload migrates only at its own version cut. One server also gives session truth one literal writer. This design does not depend on the proposal landing unchanged.

## 9. Honest costs and enforcement

This section will account for larger workspace and binary surfaces, compile and lint cost, the obligation to update every hosted consumer when the moving legacy surface changes, curating a finite hosted set, security limits per codec, and the distinction between source compatibility and behavioral compatibility.

## 10. Migration: buildable steps, visible behavior

Each stage is sized for one implementation lane, identifies exact responsibility moves, names gates, and states what deployed clients experience. The build estate is unavailable for this design lane, so these are required future gates rather than claims about checks already run.

### Stage 1 — Extract the outer transport and host one latest game

Move shared connection, TLS, rate-limit, and lobby ownership into the outer server while retaining one latest game behind it. Gate the outer state machine, exact keyed join, and unchanged latest-version behavior. Existing endpoints remain authoritative during this stage.

### Stage 2 — Add the second latest game

Move the second game's generic server path into the host and register its latest behavior. Gate cross-game lobby isolation, limits, codec dispatch, and parity with its old server. Both old endpoints remain available.

### Stage 3 — Introduce the registry and version-crate layout

Replace ad hoc dispatch with the manifest-backed registry and establish the moving legacy surface. The same series updates every registered consumer. Gate full registry construction, workspace lint coverage, and behavior fixtures for both latest versions.

### Stage 4 — Cut the first evergreen old version

Copy a latest version into a new immutable behavior identity before evolving the latest contract. Gate recorded wire fixtures, deterministic simulation traces, refusal behavior, and side-by-side old-client play so frozen semantics survive evergreen plumbing.

### Stage 5 — Retire per-game server binaries

Move the remaining deployment and operational responsibilities into the sole server and remove the redundant binaries. Gate responsibility coverage, resource isolation, graceful shutdown, and rollback readiness while old processes remain available.

### Stage 6 — Switch deployment and drain old servers

Publish the one endpoint with the old servers still running, direct new clients to it, observe compatibility and session health, stop admitting new sessions to old endpoints, and remove those processes only after their sessions drain. Gate routing, rollback, metrics, and the explicit client-compatibility matrix.

## 11. Resolved defaults and remaining product questions

The full design will choose defaults for the hosted-set policy, demo fate, compatibility-crate name, version layout, and retirement mechanics. Questions that require product judgment after those defaults will remain explicit rather than weakening the implementation contract.
