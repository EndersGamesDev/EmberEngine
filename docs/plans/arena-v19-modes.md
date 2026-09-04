# Arena v19 — modes: free for all, team deathmatch, king of the hill

The match update. Until now the arena is one endless free-for-all: frags count up, nothing ends, nothing restarts. v19 gives the lobby a mode, chosen at creation beside the map: **free for all** (today's rules, first to a frag limit), **team deathmatch** (blue against red, no friendly fire, team spawns, first team to a limit) and **king of the hill** (the hill is the spot the king block already hangs over: the loading dock on Freight Yard and the statue's plinth on Trench City; alone on it earns a point a second, contested earns nothing, first to a limit). Every mode is a round that ends, announces its winner, and restarts after a pause.

Written before the code, in the shape of `docs/plans/arena-v18-freight-yard.md` and shorter, because the shape of the change is the same (a sim rule, its wire, its drawing) and most of the numbers are limits. Every constraint below was verified in the tree on 2026-09-04.

## 1. What is being asked, in this engine's terms

| Ask | What it means here |
|---|---|
| "every shooter needs modes" | A `GameMode` on the lobby, carried by name on the wire like the map (`CreateLobby.mode`, `LobbyInfo.mode`, `GameJoined.mode`), resolved by the sim into rules: who a round may hit, how a point is earned, when the round ends. |
| "team deathmatch" | Two teams, blue (0) and red (1). Assignment on join to the smaller team (ties by id parity). A team's rounds, rockets, splash and melee never hurt a teammate. Spawns are split by side of the map. The team's frag total is the score; first to `TDM_FRAG_LIMIT` wins. |
| "free for all" | Today's game with an end: first to `FFA_FRAG_LIMIT` frags wins the round. |
| "a mode which makes absolute sense from what the game looks like" | **King of the hill.** Both maps already have a raised centre that the king block hangs over and that the v13 plan called king-of-the-hill: Freight Yard's dock (an 8 × 4 plinth at 1.2 m) and Trench City's statue plinth (3.2 × 3.2 at 2.2 m, reached from the sandbags). A player standing on it alone earns one point per second; two or more on it earn nothing; first to `HILL_LIMIT` wins. Frags still happen and still show, but the hill is the score. The loot blocks stay the way to arm for the fight over the hill. |
| "modes" implies matches | A round ends when a limit is reached: the sim records the winner, the server announces it, and after `ROUND_PAUSE_SECS` the sim resets scores, respawns everyone with the sidearm, re-arms every block and starts the next round. |

## 2. Why this is a protocol bump (15 → 16)

Apply the rule from `CLAUDE.md`. `PState.team` and `State.hill`/`State.team_score` are `serde(default)` and decode everywhere; the question is what an old (v15) peer does. In team deathmatch a v15 client shoots a teammate and sees the round vanish into them with no hit, no marker, no kill: it plays a different game. In king of the hill it watches scores climb with nobody dying. And a v16 client on a v15 server creates a lobby with a mode the server drops and plays free-for-all believing it is on a team. So `PROTO_VERSION` goes to 16; the frozen v18 page goes list-only against a v16 host exactly as at every bump. The map name and the mode name both travel as strings, so the next mode is additive.

### 2.1 Every wire change

| Message / field | Change | serde | What an old peer does |
|---|---|---|---|
| `C2S::CreateLobby.mode: String` | new; `""`/`"ffa"`, `"tdm"`, `"hill"`; unknown → `Error("unknown mode")` | `#[serde(default)]` | plays free-for-all inside a team game |
| `LobbyInfo.mode: String` | new | `#[serde(default)]` | listing only |
| `S2C::GameJoined.mode: String` | new | `#[serde(default)]` | builds free-for-all rules |
| `PState.team: u8` | new; 0 or 1; every player is 0 outside team deathmatch | `#[serde(default)]` | draws everyone in their id colour |
| `S2C::State.team_score: [u32; 2]` | new | `#[serde(default)]` | no team score shown |
| `S2C::State.hill: u8` | new; `HILL_FREE` (255) when nobody holds it, `HILL_CONTESTED` (254) when two or more stand on it, else the holder's id | `#[serde(default)]` (0, which reads as "player 0 holds it" on an old peer; harmless in a mode it cannot play) | no hill drawn |
| `S2C::State.round_pause: f32` | new; seconds left in the pause after a round, 0 while a round runs | `#[serde(default)]` | plays on through the pause |
| `S2C::RoundOver { winner: u8, team: bool, scores: Vec<(u8, u32)> }` | new variant | — | dropped by `NetChan::poll` (unknown tag) |
| `Sim::from_level(&Level, seed, mode)` | shared-code signature | not on the wire | the compiler forces both peers |

Tests in `proto.rs`: `json_roundtrip` grows the fields; `a_v15_create_lobby_reads_as_free_for_all`; `a_state_without_a_hill_reads_as_free`; `the_round_over_event_survives_the_codec`; `a_v15_state_with_a_team_is_why_this_bumps` (documentary). Server tests: `a_lobby_lists_its_mode`; `an_unknown_mode_is_refused`; `old_proto_may_list_but_not_join` unchanged.

## 3. The sim

All in `crates/arena-core/src/shooter.rs` unless stated; the hill geometry is a level property beside the spawns.

```rust
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum GameMode { #[default] Ffa, Tdm, Hill }
impl GameMode { pub fn from_name(s: &str) -> Option<Self>  /* "" and "ffa" → Ffa, "tdm", "hill"; else None */; pub const fn name(self) -> &'static str }
pub const FFA_FRAG_LIMIT: u32 = 20;
pub const TDM_FRAG_LIMIT: u32 = 30;
pub const HILL_LIMIT: u32 = 60;
pub const HILL_TICK_SECS: f32 = 1.0;
pub const ROUND_PAUSE_SECS: f32 = 10.0;
pub const HILL_FREE: u8 = 255;
pub const HILL_CONTESTED: u8 = 254;
/// The hill: a footprint and the height feet must be at to stand on it.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Hill { pub min: [f32; 2], pub max: [f32; 2], pub top: f32 }
```

- `Level` gains `#[serde(default)] pub hill: Option<Hill>`: Freight Yard `Hill { min: [-4.0, -2.0], max: [4.0, 2.0], top: 1.2 }` (the dock), Trench City `Hill { min: [-1.6, -1.6], max: [1.6, 1.6], top: 2.2 }` (the plinth), the seeded arena `Some(Hill { min: [-2.0, -2.0], max: [2.0, 2.0], top: 0.0 })` (the open centre, so the seeded tests can drive the mode). A level with no hill plays king of the hill as free-for-all with a warning in the server log; the server refuses `"hill"` on such a level with `Error("this map has no hill")`.
- `Sim` gains `pub mode: GameMode`, `pub team_score: [u32; 2]`, `pub hill_holder: u8` (as on the wire), `hill_t: f32` (the holder's accumulated seconds, resets on a change of holder), `pub round_pause: f32`, `pub round: u32`, `pub round_over: Vec<(u8, bool)>` (winner, is_team; cleared each step like `events`). `PlayerSt` gains `pub team: u8`. `Sim::from_level(level, seed, mode)`; `Sim::new(seed)` is free-for-all.
- **Teams**: `add_player` in `Tdm` assigns the team with fewer living members, ties to `id % 2`; outside `Tdm` every player is team 0. `remove_player` is unchanged (teams rebalance only on join).
- **Spawns**: in `Tdm` team 0 spawns from the level's spawns with `z > 0` and team 1 from the rest (falling back to the whole list if a half is empty), through a `Level::spawns_for(team) -> Vec<[f32; 2]>` that the client never needs. Slot order inside the half is what `spawn_from` already does. On Freight Yard that is the north backlot against the south backlot; on Trench City the north side plus the two eastern-and-western spawns at `z > 0` against the rest; on the seeded ring, the northern half.
- **Friendly fire off** in `Tdm`: in the sweep's body loop `if same_team(b.owner, p.id) { continue }` right after the owner check, so a teammate is neither hit nor pierced nor reflects (a teammate's raised plate does not catch a friendly round; it passes through as if the body were not there); melee skips teammates; `detonate` spares teammates (direct and splash, the owner's own splash still hurts the owner). `same_team` is false for everyone outside `Tdm`.
- **Scoring**: a frag adds to `p.score` as today and, in `Tdm`, to `team_score[killer.team]`; a self-kill adds nothing anywhere. In `Hill`, after the movement pass: the players `alive && feet >= top - 0.05 && centre inside [min, max]` are on the hill; none → `hill_holder = HILL_FREE, hill_t = 0`; one → if it is the same holder, `hill_t += dt`, and for each whole `HILL_TICK_SECS` it crosses `p.score += 1` (score is hill points in this mode; frags are `p.frags`, a new counter that every mode fills and the scoreboard shows beside deaths); a different one → holder changes, `hill_t = 0`; two or more → `HILL_CONTESTED`, `hill_t = 0`.
- **Round end**: at the end of `step`, when `round_pause == 0`: `Ffa` any `p.score >= FFA_FRAG_LIMIT`, `Hill` any `p.score >= HILL_LIMIT`, `Tdm` any `team_score >= TDM_FRAG_LIMIT` → `round_over.push((winner, is_team))`, `round_pause = ROUND_PAUSE_SECS`. While `round_pause > 0`: it counts down, movement and shooting continue, but no score, hill point or team point is awarded and no block pays. When it reaches 0: `round += 1`, every score, frag, death count and team score to 0, every player alive at a fresh spawn with the sidearm (the respawn path), every bullet cleared, every loot block armed, the hill free. Deterministic and stepped in the sim so both the server and any replay agree.

Tests, each a name: `teams_are_balanced_on_join` (8 joins: 4 and 4; a ninth after a leave lands on the smaller team), `teammates_spawn_on_their_side` (Freight Yard: team 0 at z > 0 for 200 respawns), `a_teammate_is_never_hit` (a round through a teammate hits the enemy behind; melee on a teammate does nothing; a rocket at a teammate's feet spares them and hurts the enemy beside them and the owner), `a_teammates_shield_does_not_catch_a_friendly_round`, `team_frags_score_for_the_team_and_a_self_kill_scores_for_nobody`, `alone_on_the_hill_earns_a_point_a_second` (61 s alone → 60 points; the 61st second is the round end), `a_contested_hill_pays_nobody`, `stepping_off_the_hill_resets_the_second` (0.9 s on, off, 0.9 s on → 0 points), `the_hill_is_on_the_dock_and_the_plinth` (Freight Yard at y 1.2 inside the dock counts; at y 0 under the king block does not; Trench City the same for the plinth), `a_round_ends_at_the_frag_limit_and_restarts_after_the_pause` (scores reset, everyone alive at a spawn with the sidearm, blocks armed, `round` incremented, `round_over` seen exactly once), `nothing_scores_during_the_pause`, `free_for_all_is_bit_identical_to_v18_until_the_limit` (the determinism driver in `Ffa` against the v18 expectations for 600 ticks), `every_mode_survives_serde_and_an_unknown_name_is_refused`.

## 4. The server and the bot

`Lobby.mode: GameMode` from `CreateLobby.mode` (empty is `Ffa`; unknown refused; `"hill"` on a level with no hill refused). `GameJoined.mode` and `LobbyInfo.mode` carry the name. The tick loop forwards `sim.round_over` as `S2C::RoundOver { winner, team, scores }` (scores = every player's `(id, score)` at that instant) and fills `State.team_score`, `State.hill`, `State.round_pause`, `PState.team`. wsbot gains `--mode NAME`.

## 5. The client

- **Mode and team on join**: `GameJoined.mode` is kept; in `Tdm` every player is drawn in their team's colour (blue `[0.25, 0.55, 0.95]`, red `[0.92, 0.32, 0.28]`, the palette's first two) instead of their id colour, teammates' hp pips too, and the status line reads `BLUE 12 · RED 9 / 30` with my team named first in my colour. `Ffa`: `frags 5 / 20`. `Hill`: `hill 23 / 60 · king: <name>` or `· hill free` or `· contested`.
- **The hill**: drawn as four thin bars (0.06 thick, 0.12 tall) along the footprint's edges at `top + 0.06`, white when free, the holder's colour when held (my colour when it is me), orange `[1.0, 0.55, 0.15]` pulsing at 4 Hz when contested; plus a 0.3 m marker cube 3 m above the hill's centre in the same colour so the hill is found from across the map. Drawn in `Hill` only.
- **Round over**: on `S2C::RoundOver` the status line says `BLUE wins the round` / `<name> wins the round (20 frags)` / `<name> is king of the hill`, `Sfx::Kill` for a win by me or my team, `Sfx::Death` otherwise, rumble (0.6, 0.6, 250); during `round_pause > 0` the status line counts `next round in N s` and the Tab scoreboard is shown (it already exists; force it on for the pause).
- **Scoreboard**: sorted by team then score in `Tdm`, with a team header line; a `FRAGS` column stays and `SCORE` is added in `Hill`.
- `OnlineConfig.mode: String` (`serde(default)`), passed into `CreateLobby`; the lobby list shows the mode pill beside the map pill.
- Tests: `team_colours_replace_id_colours_in_tdm`, `the_status_line_names_the_mode`, `the_hill_bars_take_the_holders_colour`, `round_over_is_announced_once` (wire test).

## 6. The page and the docs

`web/games/arena/v19/index.html` from v18 with a `<select id="lobby-mode">` (Free for all, Team deathmatch, King of the hill) beside the map selector, its value in the create config as `mode`, a mode pill on lobby rows from `LobbyInfo.mode`, the hint updated (what each mode is, the limits, the pause); `web/games.json` v19 live on proto 16, v18 archived; `deploy/deploy-pages.sh` `ARENA_LIVE=games/arena/v19`; README's arena section gains the modes and the bump; `docs/hosts.md` if it names the version; backlog lines for what is not done (below).

## 7. Work packages, verification, commits

| WP | Owns | Delivers |
|---|---|---|
| **A sim + wire + server** | `crates/arena-core/src/{shooter.rs,freight_yard.rs,proto.rs}`, `crates/arena-server/**` | §3, §2.1, §4 with every test named |
| **B client** | `crates/arena/src/**` | §5 |
| **C page + docs** | `web/games/arena/v19/`, `web/games.json`, `web/index.html` (the live version pointer), `deploy/deploy-pages.sh`, `README.md`, `docs/hosts.md`, `docs/plans/backlog.md` | §6 |

A first (the skeleton of types and signatures lands within its first step so B compiles against it); B and C in parallel with A after the skeleton; then integration, `cargo test --workspace --exclude linter`, clippy, the wasm check, a two-client capture per mode through `tools/v18/capture.ps1` (team colours, the hill bars, a round-over line), wsbot runs with `--mode` on both maps, commits (sim+server; client; page+docs), and the host and pages deploy in that order.

## 8. Not done, for the backlog

Team balancing on leave; a vote or rotation between modes; capture-the-flag (the natural fourth mode with the loot blocks as flags is a bigger design); per-mode block respawn tuning; a team voice or ping; showing the hill on the Tab scoreboard; the seeded arena's hill is a placeholder square.
