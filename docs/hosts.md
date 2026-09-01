# Hosts: many machines, one address book

How the games run on more than one server at once, how a page picks the one to play on, and how anyone with a Linux box can add theirs. Read this before touching `server.json`, the deploy scripts, `web/hosts.js`, or a server's `Welcome`.

*Written because one server was one point of failure: when it went down, nobody could play, and there were idle machines everywhere that could have carried the game.*

## 1. The model, in one paragraph

A **host** is one machine running the game servers (the arena's `pong-server` and Fire Racer's `fire-server`) behind its own public address. Hosts do not talk to each other and share no state: a lobby lives on exactly one host. What they share is the **address book**, `server.json` on the Pages site, which lists every host with its name, its addresses, the commit it was built from and the protocol each server speaks. The pages read the book, probe every listed host, and pick one by a fixed rule: **the newest build that speaks the page's protocol wins; among hosts on the same build, the emptiest and then the nearest.** The rest of the list is the fallback order. This is the shape of the peer-network address book: one published index, many independent machines, and the client doing the choosing so that no host has to know about any other.

## 2. Vocabulary

| Word | Meaning here |
|---|---|
| host | One machine, one auto-generated name, up to two game servers and their tunnels |
| address book | `server.json` on gh-pages: the list of hosts plus the legacy single-host keys |
| writer | Whoever can push to gh-pages; the deploy scripts write the book on their behalf |
| mirror | A host that cannot write the book and publishes its own `host.json` somewhere public instead; the book links to it |
| version | `r<N>`, the commit count of the checkout a host was built from, plus its short sha. Higher is newer |
| preferred host | The host a page will pick first, by the rule in §5 |

## 3. The address book

`server.json` is written only by the deploy scripts through `deploy/publish-host.sh`, never by hand. Every writer merges its own host entry and leaves the others alone.

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
    "https://someone.github.io/EmberEngine/host.json"
  ]
}
```

**Host entry keys.** `name` is the host's name (`[a-z0-9-]`, 3 to 32 characters) and is the merge key: publishing an entry replaces the entry with the same name. `ws` and `fire_ws` are the arena and Fire Racer addresses; a missing key means that game is not running on this host. `proto` and `fire_proto` are the protocol versions those two servers speak, present exactly when their address is. `version` and `commit` describe the build. `updated` is the UTC time of the publish. `by` is free text saying who deployed it from where, and is optional.

**Legacy keys.** The top-level `ws`, `fire_ws`, `v`, `proto` and `fire_proto` predate the list and are kept so that every frozen page on gh-pages keeps working unchanged. `proto` and `fire_proto` are the protocols the **live pages** speak and are written by `deploy-pages.sh`. `ws` is the address of the preferred host among the hosts whose `proto` equals the top-level `proto`, and `fire_ws` likewise for `fire_proto`; `publish-host.sh` recomputes both every time it writes the book, and leaves them untouched when no host matches. `v` is the deploy stamp the pages use to cache-bust their bundles.

**Mirrors.** A `host.json` at a mirror URL is one host entry, same keys. The pages fetch every mirror URL cache-busted, merge the entries into the list, and treat a mirror that does not answer as absent. A mirror URL is added to the book once, by a writer; after that the mirror updates its own entry as often as it likes. GitHub Pages and raw GitHub URLs both send the CORS header the fetch needs.

## 4. What a server says about itself

Both servers answer `Hello` with a `Welcome` that now carries the host's identity and load, so a page can rank hosts from one round trip and show the player where they are:

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

The identity comes from two places. The name is `--name <name>` on the command line, else the `EMBER_HOST_NAME` environment variable, else empty. The build is stamped at compile time from `EMBER_BUILD_VERSION` and `EMBER_BUILD_COMMIT` through `option_env!`, with a `build.rs` in each server crate that re-runs the build when either changes; the deploy scripts set both, and a plain `cargo build` produces an unstamped binary that says so in its first log line.

## 5. How a page picks a host

The logic lives in `web/hosts.js`, shared by the hub and the live game pages (frozen pages keep their own inline discovery and read the legacy keys).

1. **Load** the address book cache-busted, then every mirror in parallel with a short timeout, and merge the entries by name.
2. **Filter** to the hosts that run this game (`ws` for the arena, `fire_ws` for Fire Racer) and, on a game page, speak its protocol (`proto` or `fire_proto` equal to the bundle's `proto_version()`). The hub filters by game only.
3. **Order** by version, newest first (the integer in `r<N>`; unparseable counts as zero), then by `updated`, newest first.
4. **Probe** every candidate in parallel: open a WebSocket, send `Hello` (the page's protocol, or `0` from the hub), and wait up to five seconds for `Welcome`. The reply gives the round trip and the live `host`, `version`, `players` and `lobbies`, which override what the book said.
5. **Pick**, among the hosts that answered and share the newest version: fewest `players`, then lowest round trip, then name. The remaining responders, in order, are the fallbacks.
6. **Pin** beats rank: a name in `localStorage['ember-host']` is used when it answered; the hub's host picker sets and clears it. The manual URL override in the hub's server settings beats everything, as before.

A game page runs the probe again immediately before it launches the game, and moves to the next fallback if the chosen host stops answering in between. Once the wasm client owns the socket there is no failover: a host that dies mid-game ends that game, exactly as before. Fallback is about never being unable to **start**.

**Showing where you are.** Every page shows a chip with the chosen host's name, version and round trip, and a tooltip with its commit and address. The hub also lists every host that answered, with its build and player count, and marks the preferred one.

**Lobbies across hosts.** The hub asks every responding host for its lobby list and shows them all, each row tagged with its host. Joining a row hands the host's address to the game page whose `games.json` entry declares the same `proto`; a lobby on a host whose protocol no published page speaks is listed but not joinable, and says so. A game page lists lobbies on its chosen host, and on the other hosts of its protocol below, so two people on two hosts can still find each other.

## 6. Names

A host name is generated once per machine and then kept. `deploy/host-name.sh` prints it: `EMBER_HOST_NAME` if set, else the contents of `~/.ember/host-name`, else a new name derived from a hash of the machine's hostname and user, written to that file so the same box always publishes under the same entry. Names are two words from fixed lists, adjective then noun, such as `amber-otter` or `flint-heron`. If two machines ever land on the same name the later publish overwrites the earlier entry; set `EMBER_HOST_NAME` on one of them.

## 7. Choosing a version

A host runs whatever commit it was deployed from. `EMBER_REF` names it for the workstation deploys (`git archive` ships that ref; the default is `HEAD`) and for the self-service script (default `origin/main`). A host that stays on an older commit keeps serving the frozen pages that speak its protocol, which is the compatibility story the hub was missing: every frozen page finds the hosts on its own protocol, and the preferred-host rule keeps the live page on the newest build.

## 8. Running a host

There are three ways in, in increasing order of independence.

**From a workstation over ssh.** `EMBER_HOST=<ssh name> bash deploy/deploy-pong-online.sh` and `…/deploy-fire-online.sh`, as before. They now stamp the build, start the server with its name, and publish a host entry into the book instead of overwriting the single address. `EMBER_REF` picks the commit, `EMBER_HOST_NAME` overrides the generated name.

**On the machine itself.** `bash deploy/host.sh up` clones or updates the repo, builds both servers from `EMBER_REF`, starts them and their tunnels, proves each answers `Hello` through its public address, and publishes. `host.sh update` rebuilds only when the ref moved; `host.sh status` says what is running; `host.sh down` stops it. Configuration lives in `~/.ember/host.env`. Where the entry goes is `EMBER_PUBLISH`: `upstream` merges it into the book on the project's gh-pages (needs push rights), a `<git url>#<branch>` writes a `host.json` into that repository for use as a mirror, and `none` only prints it. Needs git, a Rust toolchain, `cloudflared`, `python3` and `curl`; the first build takes minutes, later ones seconds.

**As a mirror.** Run `host.sh` with `EMBER_PUBLISH` pointing at a repository you own (a fork's gh-pages is the natural choice), send the resulting `host.json` URL to a writer once, and you are in the book. Every later update is yours alone.

## 9. The watchdog

`deploy/watchdog.sh` now loops over the ssh names in `EMBER_HOSTS` (default: `EMBER_HOST`, default `specht`). For each it resolves the host's name, finds its entry, probes both addresses, and redeploys that one host when an address stops answering or when `origin/main` has moved, with the same never-over-players rule as before. State is kept per host, so one failing host cannot stop another from being retried. The on-host units from `install-watchdog.sh` pass the name through `EMBER_HOST_NAME`.

## 10. Deliberately not built

- Hosts do not talk to each other. There is no lobby sharing, no player migration between hosts and no cross-host matchmaking; the client-side merge in §5 covers discovery without any of it.
- No automatic pruning of the book. A host that vanishes is dropped by the probe, not by the book; `publish-host.sh --remove <name>` deletes an entry by hand.
- Quick tunnels still change address on restart. A named tunnel per host would make a reboot self-healing; see `deploy/README-watchdog.md`.
- The `feature/one-server` branch folds the three server binaries into one process behind one tunnel. It changes what runs **on** a host, not how many hosts there are: a one-server host publishes the same entry with the same keys, pointing at its legacy selectors.
