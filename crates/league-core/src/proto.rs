//! The wire: tagged JSON enums, the house style of every ember game.
//!
//! `PROTO_VERSION` is league's own number, exactly like fire's and kings' —
//! a bump must never gate another game's join. `#[serde(default)]` lets old
//! snapshots decode, but every addition still needs a behavior check. V3's
//! attack-move command cannot run on a protocol-1 server, so it requires 2.

use serde::{Deserialize, Serialize};

/// The protocol this build speaks. The join gate is exact equality.
pub const PROTO_VERSION: u16 = 2;

pub const MAX_HANDLE_LEN: usize = 20;
pub const MAX_LOBBY_LEN: usize = 24;
pub const MAX_PASSWORD_LEN: usize = 40;
/// Slots per team in a squad lobby; a duel is 1.
pub const MAX_TEAM: u8 = 3;
/// Roster slots: 2 x team size, so 6 in a squad.
pub const MAX_SLOTS: usize = 6;
/// 60 Hz sim, 20 Hz on the wire.
pub const STATE_EVERY_TICKS: u64 = 3;
pub const CLIENT_PING_SECS: u64 = 5;
pub const CLIENT_TIMEOUT_SECS: u64 = 30;
pub const MAX_FRAME_BYTES: usize = 64 * 1024;

/// Missing presentation identity in a legacy snapshot or a non-champion effect.
pub const UNKNOWN_PRESENTATION: u8 = u8::MAX;

const fn unknown_presentation() -> u8 {
    UNKNOWN_PRESENTATION
}

pub use ember_net::{is_transient_read, sanitize};

#[must_use]
pub fn sanitize_handle(s: &str) -> String {
    ember_net::sanitize_handle(s, MAX_HANDLE_LEN, "summoner")
}

/// A lobby row for the browser. `mode` is the team size, 1 or 3.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct LobbyInfo {
    pub name: String,
    pub host: String,
    pub has_password: bool,
    pub players: u8,
    pub cap: u8,
    #[serde(default)]
    pub mode: u8,
    #[serde(default)]
    pub racing: bool,
}

/// One roster seat, human or bot, with everything the page shows in the
/// select screen.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct SlotInfo {
    pub slot: u8,
    pub team: u8,
    pub handle: String,
    pub bot: bool,
    pub champ: u8,
    pub picked: bool,
    pub d: u8,
    pub f: u8,
    pub runes: [u8; 3],
    #[serde(default)]
    pub connected: bool,
}

/// The lifecycle a league lobby walks. Select is the pick screen; Over is
/// the result screen, then the lobby resets to Select.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    #[default]
    Select,
    Live,
    Over,
}

/// A player command: one event with a world aim point and an optional
/// unit id / inventory index.
///
/// There are no held inputs in league; move is a destination, attacks and
/// casts are orders the authoritative sim executes.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(tag = "a", rename_all = "snake_case")]
pub enum Cmd {
    /// Right-click ground: walk here.
    Move { x: f32, z: f32 },
    /// Right-click an enemy unit: chase and auto-attack it.
    Attack { target: u32 },
    /// Walk toward a destination, engaging nearby enemy units along the way.
    /// Neutral courts still require an explicit `Attack` order.
    AttackMove { x: f32, z: f32 },
    /// Q/W/E/R at the cursor. `slot` is 0..=3.
    Cast { slot: u8, x: f32, z: f32 },
    /// D/F summoner spell. `slot` is 0 or 1, aim as above.
    Spell { slot: u8, x: f32, z: f32 },
    /// Spend a skill point into Q/W/E/R (`slot` 0..=3).
    Rank { slot: u8 },
    /// Inventory key 1-6: drink a potion.
    UseItem { slot: u8 },
    /// Buy by item id.
    Buy { item: u16 },
}

impl Cmd {
    /// Clamp everything a hostile client could inject: NaN positions and
    /// out-of-range indices become harmless no-ops, not sim state.
    #[must_use]
    pub fn sanitized(self) -> Self {
        let coord = |v: f32| {
            if v.is_finite() {
                v.clamp(-200.0, 200.0)
            } else {
                0.0
            }
        };
        match self {
            Self::Move { x, z } => Self::Move {
                x: coord(x),
                z: coord(z),
            },
            Self::AttackMove { x, z } => Self::AttackMove {
                x: coord(x),
                z: coord(z),
            },
            Self::Cast { slot, x, z } => Self::Cast {
                slot: slot.min(3),
                x: coord(x),
                z: coord(z),
            },
            Self::Spell { slot, x, z } => Self::Spell {
                slot: slot.min(1),
                x: coord(x),
                z: coord(z),
            },
            Self::Rank { slot } => Self::Rank { slot: slot.min(3) },
            Self::UseItem { slot } => Self::UseItem { slot: slot.min(5) },
            other => other,
        }
    }
}

/// One unit on the field, flat for the wire. `k`: 0 champion, 1 melee
/// minion, 2 caster minion, 3 hologram, 4 north court, 5 south court,
/// 6 blue core, 7 red core. Champion-only fields default for the rest.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default)]
pub struct UnitSnap {
    pub id: u32,
    pub k: u8,
    pub t: u8,
    /// For champions only: the roster seat the champion belongs to.
    #[serde(default)]
    pub slot: u8,
    pub x: f32,
    pub z: f32,
    /// Facing yaw in radians.
    pub fa: f32,
    pub hp: f32,
    pub mh: u16,
    #[serde(default)]
    pub def: u8,
    #[serde(default)]
    pub lv: u8,
    #[serde(default)]
    pub xp: f32,
    #[serde(default)]
    pub xpn: f32,
    #[serde(default)]
    pub g: u32,
    #[serde(default)]
    pub pt: u8,
    #[serde(default)]
    pub mn: u16,
    #[serde(default)]
    pub mm: u16,
    #[serde(default)]
    pub rk: [u8; 4],
    #[serde(default)]
    pub cd: [f32; 4],
    #[serde(default)]
    pub scd: [f32; 2],
    #[serde(default)]
    pub items: [u16; 6],
    #[serde(default)]
    pub charges: [u8; 6],
    #[serde(default)]
    pub dead: bool,
    #[serde(default)]
    pub resp: f32,
    #[serde(default)]
    pub tp: u8,
}

/// A visible status effect on a champion: kind, seconds left, value.
#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct BuffSnap {
    pub u: u32,
    pub k: u8,
    pub ttl: f32,
    pub val: f32,
}

/// An active projectile. Kind: 0 auto-attack, 1 drone, 2 bolt, 3 hook.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct ProjSnap {
    pub id: u32,
    pub k: u8,
    pub t: u8,
    /// Champion definition of the original shooter, including holograms.
    #[serde(default = "unknown_presentation")]
    pub champ: u8,
    pub x: f32,
    pub z: f32,
    pub dx: f32,
    pub dz: f32,
}

/// An active zone. Kind: 0 tornado, 1 trap, 2 stasis, 3 shroud.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct ZoneSnap {
    pub k: u8,
    pub x: f32,
    pub z: f32,
    pub r: f32,
}

/// Transient effect for the renderer, one snapshot's worth.
///
/// `k`:
/// 0 auto-attack (u->v), 1 beam (xy->xy2), 2 explosion (x, v=radius),
/// 3 zone spawn, 4 trap plant, 5 death, 6 level-up, 7 gold, 8 teleport,
/// 9 heal flash, 10 hook cast, 11 cast flash, 12 shield flash,
/// 13 accepted Q/W/E/R cast (pre-cast x/z -> requested aim x2/z2, v=0).
/// For k=0, v is a flag word: bit 0 crit, bit 1 spell, bit 2 attack-start.
/// Starts run x/z -> x2/z2 even when the target is at the origin; impacts
/// are points at x/z with bit 2 clear.
#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct Fx {
    pub k: u8,
    /// Actual emitting unit for attack starts and accepted casts; 0 is unknown.
    /// A hologram uses its own unit id, not its parent's id.
    #[serde(default)]
    pub source: u32,
    /// Champion definition responsible for the effect; 255 means generic.
    #[serde(default = "unknown_presentation")]
    pub champ: u8,
    /// 0..3 Q/W/E/R, 4 auto-attack, or 255 for a generic effect.
    #[serde(default = "unknown_presentation")]
    pub ability: u8,
    pub x: f32,
    pub z: f32,
    pub x2: f32,
    pub z2: f32,
    pub v: f32,
}

/// Kill-feed line. `t`: 0 kill (a=killer unit id or 0, b=victim, g=bounty),
/// 1 first blood, 2 court taken (a=team), 3 core lost (a=losing team),
/// 4 smote (a=unit, b=court), 5 revive (a=unit).
#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct LogEv {
    pub t: u8,
    pub a: u32,
    pub b: u32,
    pub g: u32,
}

/// Client -> server.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
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
        /// Team size, 1 or 3. Anything else becomes 3.
        mode: u8,
    },
    JoinLobby {
        name: String,
        password: Option<String>,
    },
    LeaveLobby,
    /// A pick or a change of pick, during Select.
    Pick {
        champ: u8,
        d: u8,
        f: u8,
        runes: [u8; 3],
    },
    /// The host's "start game". Unfilled seats become bots.
    StartMatch,
    /// One player command. The socket's own pings never travel this path.
    Cmd(Cmd),
    Ping {
        nonce: u32,
    },
}

/// Server -> client.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum S2C {
    Welcome {
        proto: u16,
        #[serde(default)]
        host: String,
        #[serde(default)]
        version: String,
        #[serde(default)]
        commit: String,
        #[serde(default)]
        players: u32,
        #[serde(default)]
        lobbies: u32,
    },
    /// A refused Hello/join/create. The connection stays open.
    Rejected {
        reason: String,
    },
    Lobbies {
        lobbies: Vec<LobbyInfo>,
    },
    /// Reply to create/join. `id` is the roster slot; includes the joiner.
    Joined {
        lobby: String,
        id: u8,
        mode: u8,
        roster: Vec<SlotInfo>,
    },
    PlayerJoined {
        slot: SlotInfo,
    },
    PlayerLeft {
        slot: u8,
    },
    /// Pick broadcast: the whole roster, every time anything changes.
    Roster {
        roster: Vec<SlotInfo>,
    },
    /// Match lifecycle. `left` is seconds of the phase remaining.
    Phase {
        phase: Phase,
        left: f32,
    },
    /// The world at `tick`, 20 Hz.
    State {
        tick: u64,
        secs: f32,
        units: Vec<UnitSnap>,
        champs: Vec<ChampView>,
        buffs: Vec<BuffSnap>,
        #[serde(default)]
        projs: Vec<ProjSnap>,
        #[serde(default)]
        zones: Vec<ZoneSnap>,
        kills: [u16; 2],
        boon: [u8; 2],
        boon_left: [f32; 2],
        court_respawn: [f32; 2],
        fx: Vec<Fx>,
        log: Vec<LogEv>,
    },
    Result {
        winner: u8,
        kills: [u16; 2],
        gold: [u32; 2],
    },
    Pong {
        nonce: u32,
    },
}

/// A champion's player-facing view: which slot it belongs to plus the
/// summary the HUD needs. Kept separate from `UnitSnap` so the common
/// per-frame fields of a minion stay tiny.
#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct ChampView {
    pub slot: u8,
    pub team: u8,
    pub alive: bool,
    pub resp: f32,
    pub level: u8,
    pub points: u8,
    pub ranks: [u8; 4],
    pub cds: [f32; 4],
    pub scds: [f32; 2],
    pub gold: u32,
    pub items: [u16; 6],
    pub charges: [u8; 6],
    pub d: u8,
    pub f: u8,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_effects_default_to_generic_and_new_identity_round_trips() {
        let old_fx = r#"{"k":0,"x":1.0,"z":2.0,"x2":3.0,"z2":4.0,"v":0.0}"#;
        let mut fx: Fx = serde_json::from_str(old_fx).unwrap();
        assert_eq!(
            (fx.champ, fx.ability),
            (UNKNOWN_PRESENTATION, UNKNOWN_PRESENTATION)
        );
        assert_eq!(fx.source, 0);
        fx.champ = 4;
        fx.ability = 3;
        fx.source = 77;
        let round_trip: Fx = serde_json::from_str(&serde_json::to_string(&fx).unwrap()).unwrap();
        assert_eq!((round_trip.champ, round_trip.ability), (4, 3));
        assert_eq!(round_trip.source, 77);

        let old_proj = r#"{"id":7,"k":0,"t":1,"x":1.0,"z":2.0,"dx":1.0,"dz":0.0}"#;
        let mut proj: ProjSnap = serde_json::from_str(old_proj).unwrap();
        assert_eq!(proj.champ, UNKNOWN_PRESENTATION);
        proj.champ = 2;
        let round_trip: ProjSnap =
            serde_json::from_str(&serde_json::to_string(&proj).unwrap()).unwrap();
        assert_eq!(proj, round_trip);
    }

    #[test]
    fn the_wire_shape_is_the_house_style() {
        assert_eq!(
            serde_json::to_string(&C2S::ListLobbies).unwrap(),
            r#"{"t":"list_lobbies"}"#
        );
        assert_eq!(
            serde_json::to_string(&S2C::Pong { nonce: 7 }).unwrap(),
            r#"{"t":"pong","nonce":7}"#
        );
        assert_eq!(
            serde_json::to_string(&S2C::Phase {
                phase: Phase::Live,
                left: 0.0
            })
            .unwrap(),
            r#"{"t":"phase","phase":"live","left":0.0}"#
        );
    }

    #[test]
    fn a_cmd_round_trips_and_stays_under_the_frame_cap() {
        let c = C2S::Cmd(Cmd::Cast {
            slot: 2,
            x: 12.5,
            z: -3.25,
        });
        let t = serde_json::to_string(&c).unwrap();
        assert_eq!(serde_json::from_str::<C2S>(&t).unwrap(), c);
        assert!(t.len() < MAX_FRAME_BYTES);
    }

    #[test]
    fn sanitize_cuts_control_characters_and_length() {
        assert_eq!(sanitize("a\u{1}b", 10), "ab");
        assert_eq!(sanitize("   ", 10), "");
        assert_eq!(sanitize_handle("").len(), "summoner".len());
        let long = "x".repeat(40);
        assert_eq!(sanitize_handle(&long).chars().count(), MAX_HANDLE_LEN);
    }

    #[test]
    fn hostile_cmd_values_are_harmless() {
        let c = Cmd::Cast {
            slot: 99,
            x: f32::NAN,
            z: 1e9,
        }
        .sanitized();
        assert_eq!(
            c,
            Cmd::Cast {
                slot: 3,
                x: 0.0,
                z: 200.0
            }
        );
    }
}
