# Hosts: many machines, one address book

How the games run on more than one server at once, how a page picks the one to play on, and how anyone with a Linux box can add theirs. Read this before touching `server.json`, the deploy scripts, `web/hosts.js`, or a server's `Welcome`.

*Written because one server was one point of failure: when it went down, nobody could play, and there were idle machines everywhere that could have carried the game.*

## 1. The model, in one paragraph

A **host** is one machine running the game servers (the arena's `arena-server` and Fire Racer's `fire-server`) behind its own public address. Hosts do not talk to each other and share no state: a lobby lives on exactly one host. What they share is the **address book**, `server.json` on the Pages site, which lists every host with its name, its addresses, the commit it was built from and the protocol each server speaks. The pages read the book, probe every listed host, and pick one by a fixed rule: **the newest build that speaks the page's protocol wins; among hosts on the same build, the emptiest and then the nearest.** The rest of the list is the fallback order. This is the shape of the peer-network address book: one published index, many independent machines, and the client doing the choosing so that no host has to know about any other.

## 2. Vocabulary

| Word | Meaning here |
|---|---|
| host | One machine, one auto-generated name, up to two game servers and their tunnels |
| address book | `server.json` on gh-pages: the list of hosts plus the legacy single-host keys |
| writer | Whoever can push to gh-pages; the deploy scripts write the book on their behalf |
| mirror | A host that cannot write the book and publishes its own `host.json` somewhere public instead; the book links to it |
| version | `r<N>`, the commit count of the checkout a host was built from, plus its short sha. Higher is newer |
| preferred host | The host a page will pick first, by the rule in §5 |
| game id | The `id` in `games.json`: `arena`, `fire`, and whatever comes next. Every per-game key in the book is derived from it |

## 3. The address book

`server.json` is written only by the deploy scripts through `deploy/publish-host.sh`, never by hand. Every writer merges its own host entry and leaves the others alone.

The writer takes `--name <host>`, a `--game <id> --url <wss> --proto <n>` triple repeated once per game, and `--version`, `--commit` and `--by` for the build stamp; `--remove` deletes an entry and `--recompute` touches nothing but the legacy address keys. It writes either a local file (`--book <path>`, no git) or a branch of a repository (`--repo <url> --branch <b>`, which fetches that one branch one commit deep, rewrites, commits and pushes). `--file` names the file inside that repository and defaults to `server.json`; any other name — `host.json` is the convention — switches to **mirror mode**, where the file is one host entry on its own rather than the whole book. A book that will not parse is never overwritten: one bad byte on gh-pages must not become the silent loss of every other host's entry.

```json
{
  "v": "1788261977",
  "proto": 12,
  "fire_proto": 1,
  "ws": "wss://…trycloudflare.com",
  "fire_ws": "wss://…trycloudflare.com",
  "hosts": [
    {
      "name": "amber-otter",
      "ws": "wss://…trycloudflare.com",
      "fire_ws": "wss://…trycloudflare.com",
      "version": "r211",
      "commit": "502414c",
      "proto": 12,
      "fire_proto": 1,
      "updated": "2026-09-01T12:34:56Z",
      "by": "end@specht"
    }
  ],
  "mirrors": [
    { "url": "https://someone.github.io/EmberEngine/host.json", "name": "quiet-raven" }
  ]
}
```

**Game keys are derived from the game id, so a new game needs no new code in the book or in `hosts.js`.** The arena, being first, owns the bare names: its address key is `ws` and its protocol key is `proto`. Every other game uses `<id>_ws` and `<id>_proto`: Fire Racer is `fire_ws` and `fire_proto`, and a game with id `kings` would be `kings_ws` and `kings_proto`. The same two keys are used at the top level of the book and inside every host entry.

**Host entry keys.** `name` is the host's name (`[a-z0-9-]`, 3 to 32 characters) and is the merge key: publishing an entry replaces the entry with the same name. Each game's address key holds that server's public address; a missing key means that game is not running on this host. Each game's protocol key is the version that server speaks, present exactly when its address is. `version` and `commit` describe the build. `updated` is the UTC time of the publish. `by` is free text saying who deployed it from where, and is optional.

**Legacy keys.** The top-level address and protocol keys (`ws`, `proto`, `fire_ws`, `fire_proto`, and so on per game) plus `v` predate the list and are kept so that every frozen page on gh-pages keeps working unchanged. The top-level protocol keys are the protocols the **live pages** speak and are written by `deploy-pages.sh`. Each top-level address key is the address of the best host among the hosts whose protocol key for that game equals the top-level one; `publish-host.sh` recomputes it on every write, and leaves a key untouched — including absent — when no host matches, because the address a frozen page already holds may well still work. Two details matter here. The recompute is driven by the **protocol** keys the book carries: a game whose top-level protocol key is absent has its address key left alone, so `deploy-pages.sh` writes the protocol keys first and then calls `publish-host.sh --recompute` to re-point the addresses against them. And "best" is the book-only ranking — newest `version`, then newest `updated`, then name — with no probe and no player count, because a writer holds no socket to the hosts. The live ranking in §5 is the page's job. `v` is the deploy stamp the pages use to cache-bust their bundles, and it moves only when the book's bytes actually change.

**Mirrors.** A `host.json` at a mirror URL is one host entry, same keys. The pages fetch every mirror URL cache-busted, merge the entry into the list, and treat a mirror that does not answer as absent — a mirror that hangs, 404s, returns nonsense or returns half a megabyte of it is an absence, never an error that reaches the player. **A mirror is bound to one name.** The book lists it as `{"url": …, "name": "quiet-raven"}`, and that name is the only entry that URL may publish: what it serves must be exactly one entry object whose own `name` matches, or the whole payload is dropped. An array, a whole book with its own `hosts` list, another host's name, a name outside `[a-z0-9-]{3,32}` — all rejected, and an entry the book's own `hosts` already carries wins over the mirror's. The privilege a writer grants by adding a mirror URL is "add your box to the list", and it stays that: without the binding one linked third-party URL could rewrite any host's address and silently take every player with it. A bare string in `mirrors[]` is the old shape; it names no host, so nothing it serves can be attributed to anyone and it is ignored with a console warning — re-add it in the object form to bring that mirror back. A mirror URL is added to the book once, by a writer; after that the mirror updates its own entry as often as it likes. GitHub Pages and raw GitHub URLs both send the CORS header the fetch needs.

## 4. What a server says about itself

Every game server answers `Hello` with a `Welcome` that now carries the host's identity and load, so a page can rank hosts from one round trip and show the player where they are. The two servers in the tree today, and the pattern any new game server follows:

| Field | Arena `Welcome` | Fire `Welcome` | Meaning |
|---|---|---|---|
| `proto` | yes (existing) | yes (existing) | The protocol this server speaks |
| `motd` | yes (existing) | no | Unchanged |
| `host` | new | new | The host name, `""` when the server was started without one |
| `version` | new | new | `r<N>` of the build, `""` for an unstamped dev build |
| `commit` | new | new | Short sha of the build, `""` when unstamped |
| `players` | new | new | Humans currently in games on this server |
| `lobbies` | new | new | Open lobbies on this server |

Every new field is `#[serde(default)]` and purely informational: an old client ignores them, a new client reads empty strings and zeros from an old server, and no shot, join or race resolves differently either way. That is the test this repo applies before deciding whether a field bumps the protocol, and these fields pass it, so **`PROTO_VERSION` does not move** for either game.

The identity comes from two places. The name is `--name <name>` on the command line, else the `EMBER_HOST_NAME` environment variable, else empty; both servers accept `--name` and `--help`, and they differ only in how they take their bind address (`arena-server --bind ADDR:PORT`, `fire-server ADDR:PORT` positionally, which is how the deploy scripts and the units already pass it). The build is stamped at compile time from `EMBER_BUILD_VERSION` and `EMBER_BUILD_COMMIT` through `option_env!`, with a `build.rs` in each server crate that re-runs the build when either changes; the deploy scripts set both, and a plain `cargo build` produces an unstamped binary that says so in its first log line. Compile time is the point: a host may sit on an old commit for months and must keep reporting **that** commit, and the failure mode of a compile-time stamp is a cache hit shipping the previous build's identity, which is exactly what the `build.rs` exists to prevent.

`players` and `lobbies` are counted when the `Welcome` is written rather than kept as running totals: a stale counter is precisely how a host would advertise itself as empty and then collect every player.

## 5. How a page picks a host

The logic lives in `web/hosts.js`, shared by the hub and the live game pages (frozen pages keep their own inline discovery and read the legacy keys).

1. **Load** the address book cache-busted, then every bound mirror in parallel with a short timeout (four seconds each), and merge by name. Inside the book's own `hosts` the later entry wins outright, because a publish REPLACES an entry rather than patching it; a mirror only fills a name the book does not carry, under the one name it is bound to (§3). Every entry is normalised as it is merged: the name must be the documented `[a-z0-9-]{3,32}` shape or the entry is not a host, address keys leave this edge as non-empty strings or not at all, and protocol keys as numbers or not at all — so a hostile `"ws": 123` is an absence downstream rather than a `TypeError` that wedges a page. The merged list is capped, as are the number of mirrors and the size of any one payload: none of the three has a natural end, and two of them are bytes a third party chooses. **Every game the book addresses at the top level but no merged entry carries is promoted here** into one nameless legacy entry holding just those keys, so a page meeting a book nobody has republished yet — or a HALF-migrated book, where the first per-game publish wrote a `hosts[]` for the arena and the still-live fire address is only at the top level — finds the server that is right there, and nothing downstream carries a legacy branch. The synthesised entry is seeded before the mirror entries, so a mirror can stand next to the legacy address but can never take its place.
2. **Filter** to the hosts that run this game (its address key is present) and, on a game page, speak its protocol (its protocol key equals the bundle's `proto_version()`). The hub filters by game only. `hosts.js` takes the game id and derives both keys; it knows nothing about any particular game.
3. **Order** by version, newest first (the integer in `r<N>`; unparseable counts as zero), then by `updated`, newest first, then by name so the result is stable.
4. **Probe** every candidate in parallel: open a WebSocket, send `Hello` (the page's protocol, or `0` from the hub), and wait up to five seconds for `Welcome`. The reply gives the round trip and the live `host`, `version`, `commit`, `proto`, `players` and `lobbies`, which override what the book said — the book records what was published, the `Welcome` is what is running. The round trip is measured from socket construction, so it includes the TCP and TLS handshake: what the ranking wants is "how long until this host can start my game", not a warm ping. Probing happens **before** the protocol filter on purpose, because the book's protocol key can be stale or missing and one extra socket is cheaper than silently hiding a host that would have worked.
5. **Pick**, among the hosts that answered and share the newest version: fewest `players`, then lowest round trip, then name. The remaining responders, in order, are the fallbacks. Older builds keep their version order below, so an emptier old host never beats a newer one — otherwise nobody would ever move to a new build.
6. **Pin** beats rank: a name in `localStorage['ember-host']` is moved to the front when it answered, with the rest left in rank order behind it, so failover from a pinned host is still the normal rule; the hub's host picker sets and clears it. Reading and writing the pin are both guarded, because a browser with storage disabled throws on the getter itself and a pin is a convenience, not a requirement.

**The manual URL override is the arena's, and it beats everything.** A URL typed into the hub's server settings replaces the arena's whole host list with that one address — no book, no ranking, no protocol filter — and disables the host picker with the reason in its tooltip. The arena game page short-circuits on the same value. Fire Racer deliberately has no override: the one in the hub's settings is the **arena's** address, and pointing the fire page at it would be worse than having none.

A game page runs the probe again immediately before it launches the game (with a shorter three-second timeout, and serially down the fallback list, since the second host is only interesting once the first has actually failed), and moves to the next fallback if the chosen host stopped answering in between. Once the wasm client owns the socket there is no failover: a host that dies mid-game ends that game, exactly as before. Fallback is about never being unable to **start**.

**Everything re-ranks on a timer.** The hub refreshes every ten seconds, and a game page's menu polls on the same cadence and re-ranks as it re-lists, so a host that dies while the player reads the hint text leaves the chip and the fallback list rather than sitting there looking playable. The poll stops the moment the page goes online.

**Showing where you are.** Every page shows a chip with the chosen host's name, version and round trip, and a tooltip with its commit and address — the address, because it is a tunnel URL nobody wants to read and everybody needs when reporting a problem. When nothing answered the chip says so rather than going blank. The hub also carries a hosts strip: one row per **machine**, not per server, because a host running both games answers twice and the player cares about the box. Its players add up across its servers, the round trip is the best of them, the build is the newest it reported (a host mid-redeploy can briefly disagree with itself), and the chosen one carries a "preferred" badge naming the games it is preferred for.

**Lobbies across hosts.** The hub asks every responding host for its lobby list and shows them all, each row tagged with its game and its host. The list request is `list_lobbies` for every game; the reply is tagged `lobby_list` by the arena and `lobbies` by Fire Racer, and `hosts.js` accepts either, so a new game may use either tag. Joining a row hands the host's address to the game page whose `games.json` version entry declares the same `proto`; a lobby on a host whose protocol no published page speaks is listed but not joinable, and says which protocol it would need. The handover travels in `sessionStorage['ember-pending']` and carries the row's own address, and the game page uses that address **verbatim** — probed once so a dead handover says so instead of hanging, but never re-ranked, because that lobby exists on that machine and nowhere else. A handover without an address (from a page that predates the host list) falls back to the normal pick. A game page lists lobbies on its chosen host, and on the other hosts of its protocol below, so two people on two hosts can still find each other; Join targets that row's host, Create targets the chosen one.

## 6. Names

A host name is generated once per machine and then kept. `deploy/host-name.sh` prints it: `EMBER_HOST_NAME` if set, else the contents of `~/.ember/host-name`, else a new name derived from `sha256("<hostname>|<user>")`, written to that file so the same box always publishes under the same entry. The derivation is deterministic on purpose — a box that loses its home to a reimage comes back under the same name rather than orphaning its old entry — and an unwritable home is a warning, not a failure, for the same reason. Names are two words from fixed lists of 24, adjective then noun, such as `amber-otter` or `flint-heron`. 576 combinations is not collision-proof and is not meant to be: if two machines ever land on the same name the later publish overwrites the earlier entry, and the fix is to set `EMBER_HOST_NAME` on one of them.

The `[a-z0-9-]{3,32}` shape is checked on **every** path, the manual override included, because a name with a space or a slash in it would go into JSON and into a pid-file path and fail somewhere much less obvious; a stored name that fails the check is reported and replaced rather than used silently. The script deliberately depends on nothing but itself, because the workstation deploys and the watchdog pipe it into `bash -s` over ssh onto hosts with no checkout, and it takes whichever of `sha256sum`, `shasum` or `openssl` the box has. `EMBER_NAME_FILE` moves the name file, which is how the tests exercise this without touching the real one.

## 7. Choosing a version

A host runs whatever commit it was deployed from. `EMBER_REF` names it for the workstation deploys (`git archive` ships that ref; the default is `HEAD`) and for the self-service script (default `origin/main`). A host that stays on an older commit keeps serving the frozen pages that speak its protocol, which is the compatibility story the hub was missing: every frozen page finds the hosts on its own protocol, and the preferred-host rule keeps the live page on the newest build.

Everything a deploy publishes is read from **the ref**, never from the working tree: the version, the commit, and the protocol number (`git show <ref>:crates/<crate>/src/proto.rs`). A build stamped from the working tree while an older ref is deployed would rank as the newest build and take every player to an older server, and a protocol number read from the working tree would put a number in the book that the running binary does not speak. For the same reason the workstation deploys refuse a dirty tree only when the ref **is** `HEAD` — pinning a host to an older commit has nothing to do with what happens to be edited here, and refusing then would mean stashing first.

## 8. Running a host

There are three ways in, in increasing order of independence.

**From a workstation over ssh.** `EMBER_HOST=<ssh name> bash deploy/deploy-pong-online.sh` and `…/deploy-fire-online.sh`, as before. They now stamp the build, start the server with its name, and publish a host entry into the book instead of overwriting the single address: each calls `deploy/publish-host.sh --name <host> --game <id> --url <wss> --proto <n> …`, which upserts that game's two keys on the host's entry and recomputes the legacy keys. `EMBER_REF` picks the commit, `EMBER_HOST_NAME` overrides the generated name. The name is resolved **on the target machine**, by piping `host-name.sh` into `bash -s` over ssh, because the name is a property of the machine and not of the workstation deploying to it — which is also what makes both games deployed to the same box land on one entry rather than two. Publishing happens only after a health check that speaks the protocol through the public URL, so a failed deploy leaves the previous address in the book rather than pointing every player at a server nobody has proved is alive; Fire Racer additionally refuses to redeploy over people mid-race unless `EMBER_FORCE` is set. A new game's deploy script is the same call with its own id.

**On the machine itself.** `bash deploy/host.sh up` clones or updates the repo, builds both servers from `EMBER_REF` at idle priority, starts them, proves each answers on loopback first, brings up their tunnels, proves each answers again through its public address, and only then publishes. Loopback before public, so a failure at the public URL is unambiguously the tunnel rather than the server. `host.sh update` redeploys when the ref moved **or** when a server is not running, and does nothing when the ref is unchanged and both are up; `host.sh status` says what is running, from what, at what address, and prints this host's own entry as currently published; `host.sh down` stops the servers and their tunnels.

Configuration lives in `~/.ember/host.env`, which the script writes on first run with every setting commented out at its default; a real value in the environment always beats the file, so a one-off run needs no edit (`EMBER_REF=v12 bash deploy/host.sh up`). The knobs are `EMBER_REPO` (default the project's GitHub URL), `EMBER_REF` (default `origin/main`), `EMBER_HOST_NAME`, `EMBER_PUBLISH` (default `none`), `EMBER_ARENA_PORT` and `EMBER_FIRE_PORT` (7780 and 7781, loopback only — the tunnels are what the world sees), `EMBER_HOME` (default `~/ember-host`, holding the checkout, logs and pid files) and `EMBER_TUNNEL_BIN` (default `~/bin/cloudflared`).

Where the entry goes is `EMBER_PUBLISH`: `upstream` merges it into the book on the project's gh-pages (needs push rights), a `<git url>#<branch>` writes a `host.json` into that repository for use as a mirror, and `none` — the default — prints the entry it would have published and changes nothing. All three go through `publish-host.sh`, so what `none` shows is exactly what the other two would write.

Needs git, a Rust toolchain, `cloudflared` and `python3`; the first build takes minutes, later ones seconds. The servers are started with the name in `EMBER_HOST_NAME` and never a `--name` flag, because a host is allowed to stay on an older commit and an older binary that has never heard of the flag would crash-loop on it, where an unknown environment variable is ignored by every build there has ever been.

`EMBER_TUNNEL_BIN` is load-bearing beyond "which cloudflared". `host.sh` reads the public address out of the tunnel's log, and it accepts a `https://…trycloudflare.com` line always; when the binary is **not** the default it additionally accepts any `ws://` or `wss://` line verbatim. That is what lets the whole path — build, start, probe through the "public" URL, publish — be exercised on loopback against a stub tunnel with no Cloudflare account and no network, which is how `deploy/tests/test-host-loopback.sh` works.

**As a mirror.** Run `host.sh` with `EMBER_PUBLISH` pointing at a repository you own (a fork's gh-pages is the natural choice), send the resulting `host.json` URL to a writer once, and you are in the book. Every later update is yours alone.

## 9. The watchdog

`deploy/watchdog.sh` now loops over the ssh names in `EMBER_HOSTS` (a space-separated list; default: `EMBER_HOST`, default `specht`). For each it resolves the host's name over ssh, finds its entry in `hosts[]`, probes the addresses that entry carries, and redeploys that one host when an address stops answering or when `origin/main` has moved, with the same never-over-players rule as before. It reads `hosts[]` and never the legacy top-level keys: those name whichever host is preferred, so probing them would test the same machine once per host in the list. State is one file per host (`.watchdog-state-<ssh name>`), and the commit is recorded only when every deploy in that pass succeeded, so one failing host cannot stop another from being retried and a failed publish cannot become permanent. It also redeploys the **pages** on its own state file when `origin/main` moves. Its probe bar is a completed WebSocket handshake — enough to tell "reachable" from "gone", which is all it needs; the deploys are where the protocol is actually spoken.

The never-over-players check reads `players_in_game=` out of `~/pong-server.log` on the host being redeployed, and defers that host to the next pass when it is above zero.

The on-host units from `install-watchdog.sh` pass the name through `EMBER_HOST_NAME` in the unit file, so a reboot brings the games back under the same entry rather than a second one. The two watchdogs are deliberately separate and are documented together in `deploy/README-watchdog.md`.

## 10. Deliberately not built

- Hosts do not talk to each other. There is no lobby sharing, no player migration between hosts and no cross-host matchmaking; the client-side merge in §5 covers discovery without any of it.
- No automatic pruning of the book. A host that vanishes is dropped by the probe, not by the book; `publish-host.sh --remove <name>` deletes an entry by hand.
- Quick tunnels still change address on restart. A named tunnel per host would make a reboot self-healing; see `deploy/README-watchdog.md`.
- The `feature/one-server` branch folds the three server binaries into one process behind one tunnel. It changes what runs **on** a host, not how many hosts there are: a one-server host publishes the same entry with the same keys, pointing at its legacy selectors.
