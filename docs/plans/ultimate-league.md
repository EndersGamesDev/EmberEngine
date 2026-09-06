# UltimateLegue — v1 design and release record

A one-lane, top-down MOBA on ember: 1v1 and 3v3, an authoritative `league-server` running the same `league-core` sim the client renders, champion select with rune pages and summoner spells before the game, and a DOM page for everything with text (the scene pass has none).

The three founding champions and their kits are the user's; two more (Bog Maw, Tessera) are ours. The game ships as **UltimateLegue**, id `league`, retaining the spelling in the owner's request.

## Shape

- `league-core` — pure 60 Hz sim + wire protocol. No std threads, wasm-safe, deterministic: the only randomness is `hash(tick, who, salt)`, no per-tick RNG state. Everything is server-authoritative (the client never simulates abilities), so the f32 transcendentals in movement/aim cannot desync a peer — there is only one peer that simulates.
- `league-server` — the fire/arena hub pattern: thread per connection, one hub thread at 60 Hz, tagged JSON frames, join gate on exact `PROTO_VERSION` (1), own port 7783, own `league_ws`/`league_proto` book keys.
- `league` — the client: top-down perspective camera following your champion, click-to-move, procedural meshes, and a `cmd_json`/`state_json` handshake with the page, which owns HUD, shop, champion select, rune pages, scoreboard and minimap.

## The lane

One lane along X. Blue core at x=-62, red at x=+62; lane corridor z∈[-7,7]; whole field x∈[-68,68], z∈[-40,40]. The fountain is an axis-aligned box around your core, with half-extents 7 along X and 6 along Z (`abs(x - core_x) <= 7` and `abs(z) <= 6`); it restores 6% maximum HP + 5% maximum mana per second.

- **Cores** (win condition): 3200 HP, +0.4 HP/s regen while no enemy unit within 18. Enemy core dies, you win.
- **Courts** (the objective on each side of the lane): North Court at (0, +16), South Court at (0, -16). 1400 HP, respawn 150 s. Killing blow team gets 150 gold each and a 100 s boon: North = +12% damage, South = +15% ability haste and +15% gold. Only champions damage courts.
- **Minions**: a wave per team every 30 s (first at 10 s): 3 melee + 1 caster, capped at 16 alive. They march the lane, fight anything hostile within 8, scale +15% every 2 min. They do not attack courts.

## Champions

Level 1 start, cap 12. XP curve `150 + 85·(lv-1)`. One skill point per level; Q/W/E to rank 3; R ranks need level 6/9/12. Respawn `5 + 1.6·level` s (cap 25).

Stats: health, mana, damage, ability power, ability haste (flat %, CD scales `×1/(1+h)`), critical strike chance, critical strike damage, attack speed, move speed. Runes and items add to the same model.

Auto-attack: right-click an enemy unit to attack it (chase within leash, then hold); each champion has its own range, cooldown and attack visual (drones, beam slash, bolt, hook, flame arc). Crit rolls are per-attack `hash(tick, attacker, 7)`.

Roster (base HP/mana/MS/AD/AP, attack range/cooldown):

- **SW4RM, AI Swarm** — 560/320/330/54/55, 5.6/0.95. Q homing micro-drones (3 bites at target), W piercing laser beam (skillshot, slow), E split into you + a hologram (5 s, 50/60/70% AD attacks, invulnerable), R split into 4 and every copy casts Q and W on one target (8 s).
- **EmberKnight** — 660/260/340/66/30, 1.9/0.85. Q flame tornado (ground zone, ticks, slow), W immune to damage 2 s + shield after, E sword in flames 4 s (attacks burn), R demon form 12/14/16 s by rank: all damage doubled and 3 charges of teleport-to-cursor.
- **The Hallow One** — 600/420/330/50/65, 4.6/1.0. Q heal ally, W ally speed +35% 3 s, E ally shield, R mark ally: a lethal hit inside 5 s revives them at 30/40/50% maximum HP by rank instead. In 1v1 the ally is yourself.
- **Bog Maw** (ours) — 740/210/320/64/15, 1.8/0.95. Q bog hook (pull + root), W fen shroud (draining decay aura), E silt lunge (dash + AoE slow), R bogquake (big AoE root + burst).
- **Tessera the Clockmaker** (ours) — 550/340/325/52/70, 5.2/1.05. Q gear shot (piercing bolt), W chrono trap (planted root + burst, max 3), E chrono step (short blink + burst of speed), R grand mechanism (zone root, ticking, detonates).

Summoner spells, one for D and one for F at pick time: **Flash** (7 m blink, 210 s), **Heal** (80 + 10% missing, hits nearby allies, 150 s), **Smite** (`350 + 30·level` base true damage to nearby enemy minions or neutral courts only, 60 s; champions and cores are ineligible), **Exhaust** (−40% damage, −30% speed, 3 s, 150 s).

Runes: a page is any 3 of Fury (+7% attack speed), Vigor (+70 HP), Focus (+14 AP), Swift (+5% MS), Riches (+15% gold), Haste (7% ability haste), Cruelty (+5% crit, +6% crit damage), Ruin (+8% spell damage). Pages are saved per browser (localStorage) as A/B/C and chosen in champion select.

Items: 18 in the shop, three tiers, stats only except potions (charges) and Emberbrand/Duskveil (on-hit burn, on-spell slow). Six slots mapped to keys 1–6 (potions drink with their key). Buying is allowed anywhere at any time in v1 — fountain-only shopping is a v2 decision, recorded so it is not mistaken for an oversight.

## Economy

Start 500 gold, passive 1.2/s. Melee minion 25 / caster 30 on last hit; minion XP is 20/26 respectively, with 100% to the last hitter and 60% to each other living opposing champion within 12. Champion kill: `280 + 40·victim_level` (killer receives the full bounty without assists; otherwise 70% to the killer and the remaining 30% split between assists, 8 s assist window), first blood +100, kill XP `150 + 20·victim_level`. Court capture grants 150 base gold to each living teammate. Riches adds 15% gold and the active South boon adds 15%; their bonuses add, giving ×1.15 with either and ×1.30 with both. These bonuses apply to passive income and awarded gold; there is no time-based gold multiplier.

## Match flow (server)

`Select (60 s, host may start early when all humans picked) → Live → Over (12 s) → Select`. Slots: mode is 1 or 3 per team; a human joining takes the lowest free slot; the lowest connected human slot hosts, and unfilled slots become deterministic bots. Teams: slots 0..ts blue, ts..2ts red. Picks must be unique within each team; opposing teams may mirror champions, so all six seats can draft from the five-champion roster. A human who times out or disconnects gets a hashed pick, and a disconnected champion becomes a bot mid-game. Passive XP (2/s) ensures progression even without last hits.

## Wire (PROTO_VERSION 1)

House style: `#[serde(tag="t", rename_all="snake_case")]`, text frames, `#[serde(default)]` on late additions, 64 KiB cap, `ping/pong` every 5 s, 30 s silent-peer drop, listing ungated at proto 0.

- `C2S`: `Hello`, `ListLobbies`, `CreateLobby{name,password,mode}`, `JoinLobby`, `LeaveLobby`, `Pick{champ,d,f,runes}`, `StartMatch`, `Cmd` (tag `a` with variant-specific fields: Move/Attack/Cast/Spell/Rank/UseItem/Buy — all point-and-event, no held inputs), `Ping`.
- `S2C`: `Welcome{proto,host,version,commit,players,lobbies}`, `Rejected{reason}`, `Lobbies`, `Joined{lobby,id,mode,roster}`, `PlayerJoined/PlayerLeft`, `Phase{phase,left}`, `Roster`, `State{tick,secs,units,champs,buffs,projs,zones,kills,boon,boon_left,court_respawn,fx,log}`, `Result{winner,kills,gold}`, `Pong`.

State is 20 Hz (`STATE_EVERY_TICKS = 3`). Units is one flat struct with `#[serde(default)]` champion extras; `fx` is a transient effects stream; `log` carries kills/captures by unit id for the kill feed. Persistent `projs` and `zones` snapshots keep moving attacks and ground abilities visible between broadcasts. The client renders authoritative positions and smooths the following camera; unit interpolation and prediction are later work.

`projs` contains `ProjSnap{id,k,t,x,z,dx,dz}`: stable projectile id, kind (0 auto-attack, 1 drone, 2 bolt, 3 hook), team and planar position/direction. `zones` contains `ZoneSnap{k,x,z,r}`: kind (0 tornado, 1 trap, 2 stasis, 3 shroud), planar position and radius. Both lists default to empty when absent from a decoded state.

## Client

Camera: fixed-yaw perspective from (0, 30, 14) above your champion, fov 40, looking along −Z so the lane reads left to right. Click ground = move (ray/plane from `cursor_ndc`), click an enemy = attack (CPU screen-space pick, nearest within threshold); both mouse buttons work. QWER aim at the cursor; self/ally skills cast on self in 1v1 or the ally nearest the cursor. Shift+QWER spends a skill point. D/F, 1–6, B (shop panel — page side). `capture_mouse: false`.

`cmd_json(json)` queues page commands (pick/start/buy/use/lock), `state_json()` is polled each rAF: everything the page shows. Minimap is a page-side 2D canvas drawn from the same JSON.

Native bin `league-app`: local match against bots with instant random picks (dev + practice path), same sim, no DOM (HUD prints nothing; the scene is enough to drive it and to screenshot).

## Deploy

- Build `league` for `wasm32-unknown-unknown --release --lib`, then generate `web/pkg/league.js` and `league_bg.wasm` with wasm-bindgen. `tools/league/browser.cjs` exercises the actual WASM page and a private server using headless browser events; it never drives desktop input.
- `deploy/league-install-windows.ps1`: install only the League server (127.0.0.1:7783) and a separate durable tunnel task. Pin the server SHA256, prove a real loopback match before opening the tunnel, wait for DNS propagation, then prove a public match and exact build commit. Keep the workstation on for online play. A tunnel failure is limited to one mint per hour and only the task's own child is cleaned up.
- `tools/league/publish-host.cjs` calls the canonical `publish-host.sh` on a staged address book and verifies other games and hosts are unchanged before pushing. `tools/league/publish.cjs` publishes only the League catalog entry and game folder, requiring clean current-main source, tested WASM hash, a matching live public server, and an unchanged Pages base. Existing games and their frozen bundles stay byte-identical.
- `web/games.json`: league entry, v1 live, `handover: true` (the page honors `ember-pending`).

The release work was coordinated with Fable through Barza. Core/server validation includes all twenty abilities, economy and assists, revival and buff timing, projectile/zone serialization, real two- and six-player WebSocket matches, and ten complete deterministic bot matches. Exact deployment revision, task state, browser results, and public proof are recorded in the final release handoff below when verified.

## v1 does not have (deliberately)

No wards/vision (whole map is visible), no tower structures beyond the core, no client-side prediction/rollback (30 Hz-ish input events + 20 Hz states), no item components/recipes (flat list), no rune mastery trees (3-of-8 pages), no spectators, no disconnect restore mid-Live (you become a bot until the lobby resets). These go to `docs/plans/backlog.md` one line each.
