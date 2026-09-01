# One server, many games, evergreen versions

**Decision:** one server binary hosts every supported game and every supported version behind one deploy, one endpoint, and one outer protocol. Every hosted version compiles against the current workspace. Its wire protocol and gameplay semantics remain frozen while its plumbing to the engine and server remains free to move.

## 1. The duplication is architectural, not incidental

This section measures the three server implementations, the parallel lobby and online-client paths, the independent version constants, and the operational cost of separate processes and tunnels. It establishes why another shared helper would leave the actual ownership problem intact.

## 2. The contract boundary: frozen behavior, evergreen plumbing

The load-bearing invariant is: **a hosted version's wire protocol and gameplay semantics are frozen contracts; only its plumbing to the engine and server is evergreen.** An old deployed client receives the same game it always received. Internal APIs, adapters, source layout, and runtime integration may be rewritten only while version-specific behavior gates prove that observable contract unchanged.

This section defines wire, simulation, refusal, determinism, and compatibility fixtures as behavior, while treating engine types, server handles, scheduling adapters, and source organization as plumbing. It explains why protocol-version identity names behavior rather than a historical engine snapshot.

## 3. One outer protocol

One connection performs one outer hello and uses one lobby surface across all games and versions. Listing is ungated and enumerates `(game_id, game_version, lobby)` tuples. Creation and joining require exact equality for the selected `(game_id, game_version)`. Once joined, the outer layer frames but does not interpret the selected version's inner payload.

### 3.1 Message and state-machine contract

This subsection will specify hello negotiation, list, create, join, refusal, transition to joined traffic, limits, disconnects, and the boundary between outer envelopes and opaque inner codecs.

### 3.2 Exact equality and useful refusal

The existing join-gate law becomes a keyed rule: exact equality is checked within a game, against an explicitly hosted version, and refusal reports the versions that are actually hosted rather than advertising one global latest number.

### 3.3 Existing-client compatibility matrix

| Existing client class | Reaches the new endpoint | Can list | Can create or join | Required migration behavior |
|---|---|---|---|---|
| Cube-world client | To be derived from its actual hello path | To be derived | To be derived | To be specified |
| Pong client with the lobby-list convention | To be derived from its actual hello and list path | To be derived | To be derived from exact join checks | To be specified |
| Pong client without that convention | To be derived from source history represented at base | To be derived | To be derived | To be specified |
| Fire client with the lobby-list convention | To be derived from its actual hello and list path | To be derived | To be derived from exact join checks | To be specified |
| Fire client without that convention | To be derived from source history represented at base | To be derived | To be derived | To be specified |

## 4. Hosting model and source layout

The server owns an explicit registry from `(game_id, game_version)` to simulation factory, inner codec, resource limits, and behavior-gate identity. Hosted versions are ordinary in-tree source compiled into the server; no old engine snapshot, binary archive, dynamic plug-in ABI, or separately deployed service is part of the model.

### 4.1 Versioned crates, not versioned modules

This subsection will choose a workspace layout, account for workspace and target-directory cost, preserve lint-law coverage, define dependencies, and make copying the latest version plus changing its identity the deliberate replacement for orphaning clients through an in-place protocol bump.

### 4.2 The manifest is the hosted-set authority

This subsection will define an explicit hosted manifest, deliberate delisting, source deletion as a normal reviewed commit, and a firm default curation policy with product-visible consequences.

## 5. `ember-legacy`: a moving in-tree compatibility surface

This section will decide the crate name and specify the smallest surface old versions may use: simulation time, keyed randomness, transport and session handles, and client-facing asset or mesh access where needed, but not renderer internals. The API is intentionally unstable because every consumer is in-tree, buildable, and updated in the same change that moves it.

The update duty is atomic at repository scale: a change that moves `ember-legacy` updates every hosted consumer in the same series, and per-version behavior gates demonstrate that evergreen rewiring did not change frozen semantics.

## 6. Client-side consolidation

The duplicated transport, prediction, and reconciliation scaffolding converges into one shared client networking library with per-game hooks. The deployed web client carries the latest client for each game; frozen deployed clients remain playable because the server retains their exact hosted versions.

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
