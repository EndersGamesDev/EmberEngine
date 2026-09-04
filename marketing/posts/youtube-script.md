# YouTube script — "An online FPS that lives in a browser tab"

Target length 3:30–4:30. One voiceover track, screen capture throughout. No music claims; suggest an original loop if one is wanted.

## 0:00 — Cold open

B-roll: 10 s of the arena in action — a loot bonk, a headshot, a rocket.

Voice: "This is running in a browser tab. No install, no plugin, no CDN."

## 0:15 — What it is

The hub, the four games, one engine.

Voice: "Ember is a game engine I wrote from scratch in Rust. The games are the demo; the engine is the point. There are four of them live right now: an eight-player shooter, a drift racer, a four-corner chess variant, and a diagnostic that tells you what kind of machine you are sitting on."

## 0:45 — The bundle

B-roll: the network tab. One request, about 39 MB, done.

Voice: "Each game is one wasm bundle. Every texture and mesh is baked in at compile time, so the page is the download. That means it deploys to a static host with no backend on the client side — and it means I have to care about every megabyte, which is where the engine earns its keep."

## 1:30 — The netcode

B-roll: a simple three-box diagram (client sim / server sim / the 30 Hz wire), then the arena with eight players.

Voice: "The simulation is a fixed 60 hertz lockstep. Fixed update order, two seeded random number generators, and nothing else random in the tree. Your client predicts your own movement and reconciles against server state thirty times a second. Bullets are stepped on the server only — that one rule is why hit registration can use ordinary floats. Replays, save states, rollback: they fall out of determinism for free."

## 2:15 — The floor

B-roll: the what-is-this page; a typed refusal on a weak adapter.

Voice: "The renderer is wgpu, and the web floor is WebGL2 plus one extension — no WebGPU. The reason for that exact floor is written down in the repo, including which real devices it excludes. And a device below the floor gets a message naming the missing capability, not a blank canvas."

## 2:45 — The address book

B-roll: the host chip in the page header, then the server.json file.

Voice: "There is no central game server. Any machine can host a game behind a tunnel, and it writes one line into an address book on the static site — its name, its build, its commit, the protocol each of its servers speaks. The page asks every host in one round trip and picks the newest build that speaks its protocol, the emptiest among those, the nearest among those. Old frozen versions stay playable on old hosts, which is how a five-month-old build of the arena is still up right now."

## 3:30 — What is missing

B-roll: the README roadmap, the one-line backlog.

Voice: "What is not there yet: shadow mapping, physics, an offline asset compiler, GPU-driven culling. The roadmap is public, and the backlog is one line per gap — several of them small enough to be a first pull request."

## 3:50 — Close

Voice: "The repo, the design docs, and the backlog are in the description. The engine is the point; the games are the proof it works."

## B-roll checklist

- [ ] Arena: one loot bonk, one headshot, one rocket, one shield reflect (v20 page).
- [ ] Fire Racer: one drift through the fountain chicane.
- [ ] Four Kings: one Joker teleport.
- [ ] The network tab of the arena page load.
- [ ] The host chip in the header, hovered for the tooltip.
- [ ] server.json on gh-pages, one entry visible.
- [ ] The README roadmap section; the backlog, one line at a time.
