//! Online protocol: JSON text frames over WebSocket.
//!
//! JSON suits the small traffic volume and lets the web lobby speak it directly.
//!
//! v2: the match is the drop-in arena shooter. A lobby IS a running game —
//! creating one starts it with the host inside; joiners drop straight in.

use serde::{Deserialize, Serialize};

/// Protocol v9 adds the reflecting off-hand shield.
///
/// `shield` is `#[serde(default)]` on input and player state, so both directions
/// decode. Decoding is not the test; ask instead what an old peer does.
///
/// A pre-v9 client against a v9 server can never raise a shield, and — the
/// part that decides this — its own perfectly-aimed shot can now come back
/// and kill it, sent by an opponent whose shield it does not render, with no
/// visible cause anywhere on screen. A v9 client against a pre-v9 server has
/// the mirror problem: the server drops the field, so holding Q raises a
/// shield that is drawn, believed, and does nothing, while the player stands
/// still in the open trusting it.
///
/// That is the same shape as the v8 pitch bump rather than the jump one.
/// Jump was additive — an old peer played a smaller game and every existing
/// interaction still resolved the same way. Pitch gave the hit volume a
/// finite top, and the shield gives a hit a second possible outcome: a shot
/// that used to land now legitimately comes back. `serde(default)` protects
/// the wire format, not the meaning of a shot, so this bumps.
///
/// v10: `PState.vy`, the vertical speed at the tick being acked. Position
/// alone cannot restart a second-order integrator. Without it a client
/// rebases the jump replay on the server's y paired with its OWN present
/// velocity, re-integrates gravity across a window forward prediction has
/// already covered, and collapses its predicted arc from 1.687 m to 1.393 m,
/// landing ~200 ms early and writing 29-160 cm of camera correction into the
/// eye thirty times a second.
///
/// Additive in the sense the jump bump was: no existing interaction resolves
/// differently and hit resolution is untouched. It bumps anyway, because the
/// failure mode of NOT bumping is silent. A cached client that reads a field
/// its server never sends defaults it to 0.0 and seeds every replay with
/// zero vertical velocity - worse than the bug being fixed, and invisible.
/// Being told to reload is the better error.
///
/// The general rule, since the same reconciler is one line from repeating
/// this on any future velocity-carrying state: reconciliation must seed
/// EVERY integrator input from the authoritative snapshot, not just the
/// position that snapshot happens to carry.
/// v11: `PState.ack_age_ticks`, plus `Input.jump` becomes a PRESS rather
/// than a held key.
///
/// The age lets a client place the state on its own clock. `ack` said WHICH
/// command the server last had, never how long it had been integrating it,
/// so the replay window was guessed from the send cadence and was 0-50 ms
/// wrong - up to 46 cm of vertical position at take-off speed, alternating
/// sign through a jump because vy does.
///
/// The press is a meaning change, which is what makes this a bump rather
/// than an additive field. The server re-applies the last input every tick,
/// so a held `jump: true` re-launched the player on every grounded tick:
/// land on a crate with Space down and you launched again from 1.5 m, apex
/// 3.19 m, clearing the 2.4 m containers that are supposed to be hard cover.
/// The client now latches the rising edge (which also stops a sub-50 ms tap
/// falling between two sends) and the server consumes it after one tick.
/// Note what that is and is not: a contract about what the flag means, not an
/// enforcement. A client that sets it in every packet still gets one launch
/// per packet - so does this repo's own wsbot unless its jump mode pulses.
/// v12: `Input.melee`, a headshot zone, and both are meaning changes.
///
/// The melee flag is a PRESS, for the same reason `jump` became one in v11:
/// the server re-applies the last input it received every tick, so a held
/// melee would re-swing every tick, and at one kill per connect that is not a
/// weapon, it is a proximity field. The client latches the rising edge and the
/// server consumes it after one tick.
///
/// Why this is a bump and not an additive field, by the standard test - what
/// does an OLD peer DO when the field is absent? It plays a different game in
/// both directions. An old client cannot swing, and worse, cannot see that
/// melee exists: it is killed at contact range by an attack its build has no
/// concept of, through a shield its build believes is cover. An old server
/// silently drops the flag, so a v12 client presses E into nothing. Neither
/// side degrades gracefully, which is exactly the case `#[serde(default)]`
/// cannot rescue.
///
/// The headshot is a bump for the same reason even though it adds no field:
/// the top `HEAD_H` of the hit volume now kills outright whatever the weapon.
/// Nothing on the wire changed, but what a round DOES changed, and a client
/// predicting against the old rule would disagree with the server about who
/// is alive.
///
/// The melee deliberately does not travel as a bullet. The shield is tested
/// inside the bullet sweep, so a strike that never becomes a bullet is never
/// offered to it - the "goes through the shield" behaviour is structural, not
/// a flag, and a later change to the shield cannot silently start blocking it.
/// v13: `GameJoined.map`, an authored arena, and an obstacle with a bottom.
///
/// `Level` existed since bite 6 but nothing consumed it: `Sim::new` took a
/// seed and every peer regenerated the same random boxes. The server now
/// builds each lobby from `Level::trench_city()` and names it in `map`; the
/// client rebuilds the same level from that name (`Level::named`) and
/// predicts its own movement against it. `Obstacle` also grows `base`, the
/// bottom of a box, so a roofed trench section is walkable underneath and
/// standable on top, and the shared `blocked`, `support_height`,
/// `step_vertical` and the bullet sweep all read it.
///
/// `map` is `#[serde(default)]`, so the frame decodes everywhere - and that
/// is precisely not the test. What does an OLD peer DO when the field is
/// absent? A v12 client joining a v13 server predicts its movement against
/// the seeded boxes while the server resolves it against the trench city:
/// every wall is either invisible or imaginary, it walks through cover the
/// server stops it at and is stopped by cover it cannot see, and its own
/// rounds vanish into roofs it does not draw. A v13 client on a v12 server
/// reads an empty `map`, builds the seeded arena, and would be fine - but
/// the gate is exact equality, so that direction is moot. "Plays a different
/// game" in the direction that matters is the bump.
///
/// The name is a string rather than a bump-per-map so the NEXT map is an
/// additive value: a peer that knows the name plays it, and one that does
/// not falls back to the seeded arena, which is where the gate steps in
/// again rather than here.
/// v14: a weapon id on the wire, a map per lobby, `Input.ads`, and the
/// loot, hit and blast events.
///
/// The field that decides it did not change shape. `PState.weapon` carried
/// a level 1..3 and now carries an id 1..7. A v13 client receiving
/// `weapon: 7` calls `weapon_stats(7)`, lands in the pistol arm, draws the
/// KSVR and prints an eight-round magazine for a rocket launcher: every
/// number on its HUD is a lie, and the round it then sees fly is a rocket
/// it draws as a blue streak and never hears explode. Nothing on the wire
/// failed to decode, and the game it plays is not the game the server is
/// running. That alone is the bump.
///
/// The rest compounds it. `CreateLobby.map` defaults to Freight Yard, so a
/// v13 client that never sent it would predict its movement against Trench
/// City while the server resolved it against a yard it has never heard of,
/// which is the v13 bump's own failure mode over again. `Input.ads` is
/// dropped by a v13 server, so a v18 sniper scopes to a line and gets a
/// hip-fire cone. `State.loot`, `S2C::Loot`, `S2C::Hit` and `S2C::Blast`
/// are additive on their own (an unknown variant is dropped by the net
/// layer, an absent list reads as empty) and would not have bumped alone.
///
/// So this goes to 14, frozen pages v13-v17 go list-only against a v14
/// server exactly as at every earlier bump, and the lobby browser is
/// unaffected (`proto: 0` against the ungated `ListLobbies`).
///
/// One rule is kept so the next map is not a bump: the map still travels
/// by name. A peer that knows the name builds it; one that does not is
/// stopped by the gate here, not by a level it cannot decode. Deliberately
/// NOT bump triggers, so the next one is cheaper: `LobbyInfo.map` (listing
/// only), the three cosmetic events, and a later addition of the M4 to the
/// loot pool (its stats already ship; the client draws a fallback mesh for
/// any id whose node is missing).
pub const PROTO_VERSION: u16 = 14;
pub const MAX_HANDLE_LEN: usize = 20;
pub const MAX_LOBBY_LEN: usize = 24;
pub const MAX_PASSWORD_LEN: usize = 40;
/// State broadcast every Nth sim tick (60 Hz sim -> 30 Hz state).
pub const STATE_EVERY_TICKS: u64 = 2;
/// Clients ping at least this often; the server drops peers silent > 30 s.
pub const CLIENT_PING_SECS: u64 = 5;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct LobbyInfo {
    pub name: String,
    pub host: String,
    pub has_password: bool,
    pub players: u8,
    pub cap: u8,
    /// Which `Level` this lobby runs, so a browser can show it before
    /// joining. Listing only: defaulted, and deliberately not a bump
    /// trigger, because nothing a peer does resolves differently for
    /// not knowing it.
    #[serde(default)]
    pub map: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PlayerMeta {
    pub id: u8,
    pub handle: String,
    pub color: [f32; 3],
}

/// Per-player state inside a State broadcast.
// The wire-format booleans are independent protocol fields and cannot be consolidated compatibly.
#[allow(clippy::struct_excessive_bools)]
#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct PState {
    pub id: u8,
    pub x: f32,
    pub z: f32,
    /// Feet height: 0 on the floor, a crate top when standing on cover.
    /// Defaulted, so a pre-jump server simply reports everyone grounded.
    #[serde(default)]
    pub y: f32,
    /// Vertical speed at the tick this state acks. The client's jump replay
    /// has to restart gravity from the server's velocity, not from its own;
    /// defaulted, so a pre-v10 server simply reports everyone at rest.
    #[serde(default)]
    pub vy: f32,
    /// HORIZONTAL aim direction (normalized).
    pub ax: f32,
    pub az: f32,
    /// Aim elevation in radians, positive = up. Sent so remote players'
    /// weapons tilt with where they are actually looking; defaulted, so a
    /// pre-pitch server simply reports everyone aiming level.
    #[serde(default)]
    pub pitch: f32,
    pub hp: u8,
    pub score: u32,
    pub alive: bool,
    pub crouch: bool,
    /// Off-hand shield raised. Sent because a shield you cannot see is a
    /// mechanic that kills you for no visible reason; defaulted, so a
    /// pre-shield server simply reports everyone unshielded.
    #[serde(default)]
    pub shield: bool,
    /// A weapon id, `1..=WEAPON_COUNT` (v14; it carried a level 1..3
    /// before, and that meaning change is what bumped the version, see
    /// `PROTO_VERSION`). Read through `weapon_stats`, whose `_` arm is the
    /// sidearm for any id a client does not know.
    #[serde(default)]
    pub weapon: u8,
    #[serde(default)]
    pub ammo: u8,
    /// Rounds outside the magazine; `RESERVE_INFINITE` for the sidearm.
    /// Display only, so it defaults: a HUD that reads 0 for a v13 server's
    /// sidearm is wrong on a screen, not in a fight.
    #[serde(default)]
    pub reserve: u8,
    #[serde(default)]
    pub reloading: bool,
    /// Authoritative death count for the scoreboard.
    #[serde(default)]
    pub deaths: u32,
    /// Sequence number of this player's last applied Input — their own
    /// client rebases its movement prediction on it.
    #[serde(default)]
    pub ack: u32,
    /// How many ticks the server has been applying that acked command. With
    /// it (and the round trip it lets the client solve for) the replay can
    /// start at the instant this state describes instead of at a guess.
    #[serde(default)]
    pub ack_age_ticks: u16,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct BState {
    pub x: f32,
    pub z: f32,
    pub vx: f32,
    pub vz: f32,
    /// Height above the floor and its rate of change. Sent so the client
    /// can draw a tracer along the bullet's REAL path; it used to guess the
    /// vertical part locally from its own aim, which is why the tracer and
    /// the shot disagreed. Defaulted, so a pre-pitch server reads as flat.
    #[serde(default)]
    pub y: f32,
    #[serde(default)]
    pub vy: f32,
    /// Firing player — clients use it for shot audio cues.
    #[serde(default)]
    pub owner: u8,
    /// The weapon that fired it, so the tracer, the rocket mesh and the
    /// remote shot cue are the right ones. Defaulted: 0 reads as the
    /// sidearm through `weapon_stats`, so a pre-v14 server's rounds draw
    /// as the blue streak they always were.
    #[serde(default)]
    pub weapon: u8,
}

/// Client -> server.
#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum C2S {
    /// Must be the first message on a connection.
    Hello {
        proto: u16,
        handle: String,
    },
    ListLobbies,
    CreateLobby {
        name: String,
        password: Option<String>,
        /// Which `Level` the lobby runs, by name. Empty means
        /// `MAP_FREIGHT_YARD`; a name that is no map is answered with
        /// `Error("unknown map")`, never silently seeded, so a typo on a
        /// page is told rather than played. Defaulted so the frame decodes
        /// from a v13 client; the gate is what stops that client creating a
        /// yard it would then predict against Trench City.
        #[serde(default)]
        map: String,
    },
    JoinLobby {
        name: String,
        password: Option<String>,
    },
    LeaveLobby,
    /// Held intents: movement, aim, trigger, stance. Doubles as the
    /// keepalive.
    Input {
        /// Client-assigned sequence number, echoed back as PState.ack.
        #[serde(default)]
        seq: u32,
        /// The sim tick this client is currently rendering remote players
        /// at — the server rewinds hit tests to it (lag compensation).
        #[serde(default)]
        view_tick: u64,
        mx: f32,
        my: f32,
        ax: f32,
        az: f32,
        /// Aim elevation in radians, positive = up. A SCALAR beside the
        /// horizontal aim, deliberately not a third component of it — the
        /// sim keeps `ax`/`az` unit-length and reads elevation separately.
        /// Defaulted, so an older client simply always fires level.
        #[serde(default)]
        pitch: f32,
        fire: bool,
        #[serde(default)]
        sprint: bool,
        #[serde(default)]
        crouch: bool,
        #[serde(default)]
        reload: bool,
        /// A Space PRESS, not the held key: true means "the player pressed
        /// jump since my last input", and the sim consumes it on one tick.
        /// Held-key semantics re-launched the player on every grounded tick,
        /// because the server keeps applying the last input it received.
        /// Defaulted so an older client simply never jumps.
        #[serde(default)]
        jump: bool,
        /// Q, held. Defaulted so an older client simply never raises one —
        /// which is exactly why the version gate above had to move too.
        #[serde(default)]
        shield: bool,
        /// An E PRESS, not the held key: true means "the player swung since
        /// my last input", and the sim consumes it on one tick. Defaulted, so
        /// an older client simply never swings - but see the version note
        /// above for why defaulting is NOT sufficient here and the gate moved.
        #[serde(default)]
        melee: bool,
        /// Aiming down the sights (RMB or LT), HELD like `shield`: it
        /// tightens the spread cone of the round fired this tick. Defaulted
        /// so a v13 client simply never tightens its cone, and a v13 server
        /// simply drops a v18 client's scope, which is one of the reasons
        /// the gate moved (`PROTO_VERSION`).
        #[serde(default)]
        ads: bool,
    },
    Ping {
        nonce: u32,
    },
}

/// Server -> client.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum S2C {
    /// Reply to a valid `Hello`, and the one round trip a page uses to rank
    /// the hosts in the address book (`docs/hosts.md`): who this server is,
    /// what it was built from, and how busy it is right now.
    ///
    /// The five identity fields are additive and do NOT bump
    /// `PROTO_VERSION`. The test this repo applies is not "does it decode"
    /// — `serde(default)` guarantees that much for free — but "what does an
    /// old peer DO when the field is absent". Here: nothing. An old client
    /// never reads them and plays exactly the game it played before; a new
    /// client against an old server reads `""` and `0` and shows an unnamed
    /// host with no load figure. No shot, join, hit or lobby listing
    /// resolves differently in either direction, which is precisely the
    /// case the shield, the pitch and the melee were NOT — those changed
    /// what a round does, so they moved the gate. This does not.
    Welcome {
        proto: u16,
        motd: String,
        /// Host name this server was started with (`--name`, else
        /// `EMBER_HOST_NAME`), `""` when it was started without one.
        #[serde(default)]
        host: String,
        /// `r<N>` of the build, `""` for an unstamped dev build.
        #[serde(default)]
        version: String,
        /// Short sha of the build, `""` when unstamped.
        #[serde(default)]
        commit: String,
        /// Humans currently in games on this server, counted when the
        /// `Welcome` is written — the load figure the host ranking uses.
        #[serde(default)]
        players: u32,
        /// Open lobbies on this server.
        #[serde(default)]
        lobbies: u32,
    },
    /// Recoverable failures (wrong password, name taken, ...). The
    /// connection stays open.
    Error {
        message: String,
    },
    LobbyList {
        lobbies: Vec<LobbyInfo>,
    },
    /// You are in a game (created or joined). `players` includes yourself;
    /// build the arena locally from `Level::named(&map, seed)`.
    GameJoined {
        id: u8,
        /// Still sent: it is what `map` falls back to, and the seeded arena
        /// is still a level a lobby can name by naming nothing.
        seed: u64,
        arena_half: f32,
        players: Vec<PlayerMeta>,
        /// Which `Level` this lobby runs - `MAP_TRENCH_CITY`, or anything
        /// else for the seeded arena. Defaulted so the frame decodes from a
        /// pre-v13 server; the version gate is what stops that being
        /// played, for the reasons at `PROTO_VERSION`.
        #[serde(default)]
        map: String,
    },
    PlayerJoined {
        meta: PlayerMeta,
    },
    PlayerLeft {
        id: u8,
    },
    State {
        tick: u64,
        players: Vec<PState>,
        bullets: Vec<BState>,
        /// Weapon-pad availability, index-aligned with the seeded pad
        /// positions every client derives locally.
        #[serde(default)]
        pads: Vec<bool>,
        /// Loot-block availability (true = armed), index-aligned with the
        /// level's `Cover::Loot` obstacles in obstacle order, like `pads`.
        /// Defaulted: a peer that predates blocks draws none, and it built
        /// a level without any.
        #[serde(default)]
        loot: Vec<bool>,
    },
    /// A kill. `killer == victim` is a self-kill (a rocket's own splash)
    /// and the shape is unchanged: a v13 client prints "X fragged X".
    Kill {
        killer: u8,
        victim: u8,
    },
    /// A hit that landed, from `Sim.hits`: authoritative, so the client's
    /// hitmarker no longer has to guess from a vanished bullet and a lost
    /// hit point. `head` is the outright kill. Dropped by a v13 peer.
    Hit {
        shooter: u8,
        victim: u8,
        dmg: u8,
        head: bool,
    },
    /// A rocket detonated at `(x, y, z)`, fired by `owner`, from
    /// `Sim.blasts`. Cosmetic: the damage it did arrives as `Hit`s.
    Blast {
        x: f32,
        y: f32,
        z: f32,
        owner: u8,
    },
    /// `player` bonked loot block `block` (an index into `State.loot`) and
    /// was handed `weapon`, from `Sim.loot_events`. Cosmetic: the weapon
    /// also arrives in the next `State`, so a peer that drops this still
    /// holds the right gun.
    Loot {
        player: u8,
        block: u8,
        weapon: u8,
    },
    Pong {
        nonce: u32,
    },
}

/// Stable per-player color, by in-lobby id.
#[must_use]
pub const fn color_for(id: u8) -> [f32; 3] {
    const PALETTE: [[f32; 3]; 8] = [
        [0.25, 0.55, 0.95], // blue
        [0.92, 0.32, 0.28], // red
        [0.30, 0.80, 0.40], // green
        [0.95, 0.75, 0.20], // yellow
        [0.70, 0.40, 0.90], // purple
        [0.95, 0.50, 0.20], // orange
        [0.25, 0.80, 0.80], // teal
        [0.90, 0.45, 0.70], // pink
    ];
    PALETTE[id as usize % PALETTE.len()]
}

#[must_use]
pub fn sanitize_text(s: &str, max: usize) -> String {
    // Strip controls, trim, THEN cap — surrounding whitespace must not
    // consume the length budget.
    s.chars()
        .filter(|c| !c.is_control())
        .collect::<String>()
        .trim()
        .chars()
        .take(max)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_roundtrip() {
        let s = serde_json::to_string(&C2S::Input {
            seq: 7,
            view_tick: 120,
            mx: 1.0,
            my: 0.0,
            ax: 0.5,
            az: -0.5,
            pitch: -0.3,
            fire: true,
            sprint: true,
            crouch: false,
            reload: false,
            jump: true,
            shield: true,
            melee: true,
            ads: true,
        })
        .unwrap();
        assert!(s.contains("\"t\":\"input\""));
        assert!(s.contains("\"ads\":true"), "{s}");
        let back: C2S = serde_json::from_str(&s).unwrap();
        assert!(matches!(
            back,
            C2S::Input {
                fire: true,
                shield: true,
                ads: true,
                ..
            }
        ));

        let s = serde_json::to_string(&C2S::CreateLobby {
            name: "yard".into(),
            password: None,
            map: crate::shooter::MAP_FREIGHT_YARD.to_string(),
        })
        .unwrap();
        assert!(s.contains("\"map\":\"freight-yard\""), "{s}");
        let back: C2S = serde_json::from_str(&s).unwrap();
        assert!(
            matches!(back, C2S::CreateLobby { ref map, .. } if map == crate::shooter::MAP_FREIGHT_YARD)
        );

        let s = serde_json::to_string(&S2C::State {
            tick: 9,
            players: vec![],
            bullets: vec![BState {
                x: 1.0,
                z: 2.0,
                vx: 3.0,
                vz: 4.0,
                y: 1.4,
                vy: 0.0,
                owner: 1,
                weapon: 7,
            }],
            pads: vec![true],
            loot: vec![true, false],
        })
        .unwrap();
        assert!(s.contains("\"loot\":[true,false]"), "{s}");
        assert!(s.contains("\"weapon\":7"), "{s}");
        let back: S2C = serde_json::from_str(&s).unwrap();
        assert!(matches!(
            back,
            S2C::State { ref loot, ref bullets, .. } if loot == &[true, false] && bullets[0].weapon == 7
        ));

        let info = LobbyInfo {
            name: "yard".into(),
            host: "ender".into(),
            has_password: false,
            players: 1,
            cap: 8,
            map: crate::shooter::MAP_FREIGHT_YARD.to_string(),
        };
        let s = serde_json::to_string(&info).unwrap();
        assert!(s.contains("\"map\":\"freight-yard\""), "{s}");
        assert_eq!(serde_json::from_str::<LobbyInfo>(&s).unwrap(), info);

        let s = serde_json::to_string(&S2C::GameJoined {
            id: 2,
            seed: 987_654_321,
            arena_half: 24.0,
            players: vec![PlayerMeta {
                id: 2,
                handle: "ender".into(),
                color: color_for(2),
            }],
            map: crate::shooter::MAP_TRENCH_CITY.to_string(),
        })
        .unwrap();
        assert!(s.contains("\"map\":\"trench-city\""), "{s}");
        let back: S2C = serde_json::from_str(&s).unwrap();
        assert!(matches!(
            back,
            S2C::GameJoined {
                id: 2,
                seed: 987_654_321,
                ref map,
                ..
            } if map == crate::shooter::MAP_TRENCH_CITY
        ));
    }

    #[test]
    fn a_game_joined_without_a_map_names_the_seeded_arena() {
        // What `serde(default)` buys, and all it buys: a v12 frame decodes,
        // and the empty name resolves to the arena a v12 server is running.
        // The gate is what keeps a v12 server from being played at all.
        let old = r#"{"t":"game_joined","id":1,"seed":42,"arena_half":24.0,"players":[]}"#;
        let back: S2C = serde_json::from_str(old).unwrap();
        let S2C::GameJoined { seed, map, .. } = back else {
            panic!("expected GameJoined");
        };
        assert_eq!(map, "");
        assert_eq!(
            crate::shooter::Level::named(&map, seed),
            crate::shooter::Level::from_seed(seed)
        );
    }

    #[test]
    fn the_shield_survives_the_codec_in_both_directions() {
        // The player state carries it, so remote shields can be drawn.
        let p = PState {
            id: 3,
            x: 1.0,
            z: -2.0,
            y: 0.5,
            vy: -1.5,
            ax: 0.0,
            az: 1.0,
            pitch: 0.2,
            hp: 2,
            score: 4,
            alive: true,
            crouch: false,
            shield: true,
            weapon: 2,
            ammo: 7,
            reserve: 30,
            reloading: false,
            deaths: 1,
            ack: 42,
            ack_age_ticks: 7,
        };
        let s = serde_json::to_string(&p).unwrap();
        assert!(s.contains("\"shield\":true"), "{s}");
        let back: PState = serde_json::from_str(&s).unwrap();
        assert!(back.shield);
        assert_eq!(
            back.ack_age_ticks, 7,
            "the ack age has to survive the codec"
        );

        // And both fields decode from a frame that predates them: this is
        // what `serde(default)` buys, and it is the whole of what it buys —
        // the version gate is what stops an old peer from PLAYING against
        // this, for the reasons written at PROTO_VERSION.
        let old_state = r#"{"id":1,"x":0.0,"z":0.0,"ax":1.0,"az":0.0,
            "hp":3,"score":0,"alive":true,"crouch":false}"#;
        let p: PState = serde_json::from_str(old_state).unwrap();
        assert!(!p.shield, "an absent shield reads as lowered");
        let old_input = r#"{"t":"input","mx":0.0,"my":0.0,"ax":1.0,"az":0.0,"fire":false}"#;
        let back: C2S = serde_json::from_str(old_input).unwrap();
        assert!(matches!(back, C2S::Input { shield: false, .. }));
    }

    /// The host identity rides on `Welcome` and must survive a peer that
    /// predates it in BOTH directions — that is the whole claim behind not
    /// bumping `PROTO_VERSION` for it.
    #[test]
    fn the_host_identity_survives_a_peer_that_predates_it() {
        let w = S2C::Welcome {
            proto: PROTO_VERSION,
            motd: "hi".into(),
            host: "amber-otter".into(),
            version: "r211".into(),
            commit: "502414c".into(),
            players: 3,
            lobbies: 2,
        };
        let s = serde_json::to_string(&w).unwrap();
        // The book and `hosts.js` read these key names; renaming one is a
        // wire break that no type checks.
        for key in ["host", "version", "commit", "players", "lobbies"] {
            assert!(s.contains(&format!("\"{key}\"")), "{key} missing from {s}");
        }

        // An old server: proto and motd, nothing else. A new client must
        // read an unnamed host with no load rather than fail to decode.
        let old = r#"{"t":"welcome","proto":12,"motd":"ember arena"}"#;
        match serde_json::from_str::<S2C>(old).expect("an old Welcome must decode") {
            S2C::Welcome {
                host,
                version,
                commit,
                players,
                lobbies,
                ..
            } => {
                assert_eq!(host, "");
                assert_eq!(version, "");
                assert_eq!(commit, "");
                assert_eq!(players, 0);
                assert_eq!(lobbies, 0);
            }
            other => panic!("wrong variant: {other:?}"),
        }
    }

    #[test]
    fn a_v13_create_lobby_names_the_freight_yard() {
        // What a v13 page sends: no map. It decodes to an empty name, which
        // the server resolves to the yard (`a_lobby_lists_its_map` in the
        // server's tests pins that side). The gate is what stops a v13
        // client from then predicting against the wrong level.
        let old = r#"{"t":"create_lobby","name":"x","password":null}"#;
        let back: C2S = serde_json::from_str(old).unwrap();
        let C2S::CreateLobby { name, map, .. } = back else {
            panic!("expected CreateLobby");
        };
        assert_eq!(name, "x");
        assert_eq!(map, "", "an absent map is the empty name");
        assert_eq!(
            crate::shooter::Level::named(crate::shooter::MAP_FREIGHT_YARD, 1),
            crate::shooter::Level::freight_yard()
        );
    }

    #[test]
    fn an_input_without_ads_reads_as_hip_fire() {
        let old = r#"{"t":"input","mx":0.0,"my":0.0,"ax":1.0,"az":0.0,"fire":true}"#;
        let back: C2S = serde_json::from_str(old).unwrap();
        assert!(matches!(back, C2S::Input { ads: false, .. }));
    }

    #[test]
    fn a_state_without_loot_reads_as_no_blocks() {
        let old = r#"{"t":"state","tick":3,"players":[],"bullets":[]}"#;
        let back: S2C = serde_json::from_str(old).unwrap();
        let S2C::State { loot, pads, .. } = back else {
            panic!("expected State");
        };
        assert!(loot.is_empty(), "an absent list is no blocks");
        assert_eq!(pads, Vec::<bool>::new());
    }

    #[test]
    fn the_loot_hit_and_blast_events_survive_the_codec() {
        let events = [
            S2C::Hit {
                shooter: 1,
                victim: 2,
                dmg: 3,
                head: true,
            },
            S2C::Blast {
                x: 1.5,
                y: 0.05,
                z: -2.5,
                owner: 4,
            },
            S2C::Loot {
                player: 5,
                block: 6,
                weapon: 7,
            },
        ];
        for (tag, ev) in ["\"t\":\"hit\"", "\"t\":\"blast\"", "\"t\":\"loot\""]
            .iter()
            .zip(&events)
        {
            let s = serde_json::to_string(ev).unwrap();
            assert!(s.contains(tag), "{s}");
            let back: S2C = serde_json::from_str(&s).unwrap();
            assert_eq!(format!("{back:?}"), format!("{ev:?}"));
        }
        // An old peer's net layer drops an unknown tag rather than failing:
        // that is what makes the three additive, and it is the decode
        // error it sees, not a panic.
        assert!(serde_json::from_str::<S2C>(r#"{"t":"no_such_event"}"#).is_err());
    }

    #[test]
    fn a_v13_state_with_weapon_seven_is_why_this_bumps() {
        // Documentary: the v13 table had three rows and read every id
        // above them as the pistol. A v13 client handed `weapon: 7` would
        // print the pistol's eight-round magazine for a one-round rocket
        // launcher. The v14 table disagrees with the pistol on the id, so
        // the field's meaning changed under an unchanged shape.
        let p: PState = serde_json::from_str(
            r#"{"id":1,"x":0.0,"z":0.0,"ax":1.0,"az":0.0,"hp":3,"score":0,
            "alive":true,"crouch":false,"weapon":7}"#,
        )
        .unwrap();
        assert_eq!(p.weapon, 7, "decodes fine; decoding is not the test");
        let v13_pistol_mag = 8;
        assert_ne!(
            crate::shooter::weapon_stats(p.weapon).mag,
            v13_pistol_mag,
            "a v13 client would show the pistol's magazine for this id"
        );
        assert_eq!(p.reserve, 0, "a v13 state carries no reserve");
    }

    #[test]
    fn sanitizers() {
        assert_eq!(sanitize_text("  hi\u{7}there  ", 5), "hithe");
        assert_eq!(sanitize_text("\u{7}\u{8}", 5), "");
    }
}
