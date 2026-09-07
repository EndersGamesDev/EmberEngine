//! The match: one lane, two teams, 60 Hz of everything.
//!
//! `Match::step` is the only place time moves, and the order inside it is
//! the order of record: phase, queued commands, units (in id order â€” the
//! vec is append-only by id, so it is always sorted), projectiles, zones,
//! sweep, waves and objectives, win check. Every decision that looks random
//! comes from `rng::hash(tick, who, salt)`; there is no RNG state anywhere.
//!
//! The sim is server-authoritative by design: the client renders snapshots
//! and never runs this, so the f32 trig in movement and the hash crit roll
//! cannot desync anybody â€” there is exactly one peer doing physics.
//!
//! The five kits live in `crate::kits`, as `impl Match` methods on the
//! private machinery here; this file is lifecycle, combat resolution, and
//! the world (waves, courts, cores).

use crate::data;
use crate::proto::{
    self, BuffSnap, ChampView, Cmd, Fx, LogEv, Phase, ProjSnap, SlotInfo, UnitSnap, ZoneSnap,
};
use crate::rng;

/// Movement numbers are LoL-style (330 = a normal champion); the lane is
/// 124 world units long, so divide through by this to walk it in seconds.
pub const MS_SCALE: f32 = 1.0 / 30.0;
/// The playable strip; projectiles and dashes stop at the walls.
pub const FIELD_X: f32 = 68.0;
/// Attack-move acquires and keeps opponents within this center-to-center radius.
pub const ATTACK_MOVE_RANGE: f32 = 8.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Champ,
    Melee,
    Caster,
    Clone,
    CourtN,
    CourtS,
    CoreBlue,
    CoreRed,
}

impl Kind {
    pub const fn code(self) -> u8 {
        match self {
            Self::Champ => 0,
            Self::Melee => 1,
            Self::Caster => 2,
            Self::Clone => 3,
            Self::CourtN => 4,
            Self::CourtS => 5,
            Self::CoreBlue => 6,
            Self::CoreRed => 7,
        }
    }

    /// How wide the body is for projectile collisions and target picks.
    pub const fn hit_r(self) -> f32 {
        match self {
            Self::Champ | Self::Clone => 0.9,
            Self::Melee | Self::Caster => 0.55,
            Self::CourtN | Self::CourtS => 1.6,
            Self::CoreBlue | Self::CoreRed => 2.4,
        }
    }

    /// Projectiles and beams can meet this body at all.
    const fn hittable_by_shot(self) -> bool {
        matches!(self, Self::Champ | Self::Melee | Self::Caster)
    }

    pub(crate) const fn is_objective(self) -> bool {
        matches!(
            self,
            Self::CourtN | Self::CourtS | Self::CoreBlue | Self::CoreRed
        )
    }
}

/// Whether `mine` may order an attack on something of team `theirs` with
/// kind `theirs_kind`. Courts (team 2) are hostile to everyone who walks up
/// to them; holograms take no interest from either side.
#[must_use]
pub const fn hostile(mine: u8, theirs_kind: Kind, theirs_team: u8) -> bool {
    match theirs_kind {
        Kind::Clone => false,
        Kind::CourtN | Kind::CourtS => true,
        _ => theirs_team != mine,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuffKind {
    Slow,
    Ms,
    DmgAmp,
    Shield,
    Immune,
    Root,
    Exhaust,
    Burn,
    Regen,
    ManaRegen,
    /// The knight's flaming blade: `val` is bonus % AD on hit, `aux` the
    /// burn its attacks apply.
    Fire,
    Revive,
}

impl BuffKind {
    const fn code(self) -> u8 {
        match self {
            Self::Slow => 0,
            Self::Ms => 1,
            Self::DmgAmp => 2,
            Self::Shield => 3,
            Self::Immune => 4,
            Self::Root => 5,
            Self::Exhaust => 6,
            Self::Burn => 7,
            Self::Regen => 8,
            Self::ManaRegen => 9,
            Self::Fire => 10,
            Self::Revive => 11,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Buff {
    pub k: BuffKind,
    pub ttl: f32,
    pub val: f32,
    pub aux: f32,
    /// Unit id of the applier; burn credits its kill this way. 0 = nobody.
    pub src: u32,
}

pub(crate) fn add_buff(u: &mut Unit, k: BuffKind, ttl: f32, val: f32, src: u32) {
    // strongest-wins refresh on like kinds, so two slows never stack
    if let Some(b) = u.buffs.iter_mut().find(|b| b.k == k) {
        b.ttl = b.ttl.max(ttl);
        if k == BuffKind::Shield {
            b.val += val;
        } else {
            b.val = b.val.max(val);
        }
        b.src = src;
    } else {
        u.buffs.push(Buff {
            k,
            ttl,
            val,
            aux: 0.0,
            src,
        });
    }
}

pub(crate) fn has_buff(u: &Unit, k: BuffKind) -> bool {
    u.buffs.iter().any(|b| b.k == k && b.ttl > 0.0)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Order {
    Hold,
    Move,
    Attack,
    AttackMove,
}

#[derive(Clone, Debug)]
pub struct Unit {
    pub id: u32,
    pub kind: Kind,
    pub team: u8,
    pub x: f32,
    pub z: f32,
    pub facing: f32,
    pub hp: f32,
    pub max_hp: f32,
    pub buffs: Vec<Buff>,
    pub dead: bool,

    // champion (and clone) fields
    pub slot: u8,
    pub def: u8,
    pub level: u8,
    pub xp: f32,
    pub points: u8,
    pub gold: u32,
    /// Passive gold trickles in fractions; the integer purse would eat it.
    pub gold_frac: f32,
    pub mana: f32,
    pub max_mana: f32,
    pub ranks: [u8; 4],
    pub cds: [f32; 4],
    pub scds: [f32; 2],
    pub d: u8,
    pub f: u8,
    pub runes: [u8; 3],
    pub items: [u16; 6],
    pub charges: [u8; 6],
    pub respawn: f32,
    pub order: Order,
    pub ox: f32,
    pub oz: f32,
    pub target: u32,
    pub atk_cd: f32,
    pub tp: u8,
    pub form: f32,
    /// Rolling window of the last eight damage dealers (slot, tick), for
    /// assists and for the revive mark's bookkeeping.
    pub dmg_log: [(u8, u64); 8],
    pub dmg_next: usize,

    // hologram fields
    pub parent: u8,
    pub clone_dmg: f32,
    pub ttl: f32,
}

impl Unit {
    pub(crate) const fn blank(id: u32, kind: Kind, team: u8) -> Self {
        Self {
            id,
            kind,
            team,
            x: 0.0,
            z: 0.0,
            facing: 0.0,
            hp: 1.0,
            max_hp: 1.0,
            buffs: Vec::new(),
            dead: false,
            slot: u8::MAX,
            def: 0,
            level: 1,
            xp: 0.0,
            points: 0,
            gold: data::START_GOLD,
            gold_frac: 0.0,
            mana: 100.0,
            max_mana: 100.0,
            ranks: [0; 4],
            cds: [0.0; 4],
            scds: [0.0; 2],
            d: 0,
            f: 1,
            runes: [u8::MAX; 3],
            items: [0; 6],
            charges: [0; 6],
            respawn: 0.0,
            order: Order::Hold,
            ox: 0.0,
            oz: 0.0,
            target: 0,
            atk_cd: 0.0,
            tp: 0,
            form: 0.0,
            dmg_log: [(u8::MAX, 0); 8],
            dmg_next: 0,
            parent: u8::MAX,
            clone_dmg: 1.0,
            ttl: 0.0,
        }
    }

    /// A stable lane offset so a wave of four does not walk in one column.
    fn lane_offset(&self) -> f32 {
        ((self.id % 5) as f32) * 1.2 - 2.4
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjKind {
    Auto,
    Drone,
    Bolt,
    Hook,
}

#[derive(Clone, Debug)]
pub struct Proj {
    pub id: u32,
    pub owner: u32,
    /// Presentation identity is captured at launch, before a clone can expire.
    pub champ: u8,
    pub ability: u8,
    pub team: u8,
    pub kind: ProjKind,
    pub x: f32,
    pub z: f32,
    pub dx: f32,
    pub dz: f32,
    pub speed: f32,
    pub travel: f32,
    pub dmg: f32,
    pub crit: bool,
    pub burn: f32,
    pub slow_pct: f32,
    pub slow_ttl: f32,
    /// Remaining bolt hits; zero also stops at the first hit.
    pub pierce: u8,
    pub hook_rank: u8,
    pub homing: u32,
    pub hit: [u32; 4],
    pub nhit: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZoneKind {
    Tornado,
    Trap,
    Stasis,
    Shroud,
}

#[derive(Clone, Debug)]
pub struct Zone {
    pub id: u32,
    pub owner: u32,
    pub champ: u8,
    pub ability: u8,
    pub team: u8,
    pub zk: ZoneKind,
    pub x: f32,
    pub z: f32,
    pub r: f32,
    pub ttl: f32,
    pub tick_t: f32,
    pub dmg: f32,
    pub detonate: f32,
    pub root: f32,
    pub slow: f32,
    /// Unit id this zone walks with (the shroud), 0 for planted zones.
    pub attach: u32,
}

#[derive(Clone, Debug)]
pub struct Match {
    pub seed: u64,
    pub tick: u64,
    pub phase: Phase,
    /// Seconds left in the phase: the select clock, then the result clock.
    pub left: f32,
    pub team_size: u8,
    pub roster: Vec<SlotInfo>,
    pub units: Vec<Unit>,
    pub projs: Vec<Proj>,
    pub zones: Vec<Zone>,
    pub next_id: u32,
    pub wave_left: f32,
    pub court_respawn: [f32; 2],
    /// 0 none, 1 north (+12% damage), 2 south (+15% haste and gold).
    pub boon: [u8; 2],
    pub boon_left: [f32; 2],
    pub kills: [u16; 2],
    /// Total gold each team has earned, for the result screen.
    pub earned: [u32; 2],
    pub first_blood: bool,
    pub winner: u8,
    pub fx: Vec<Fx>,
    pub log: Vec<LogEv>,
    pub pending: Vec<(u8, Cmd)>,
}

impl Match {
    /// A fresh lobby in the select phase. `team_size` is 1 or 3; everything
    /// else becomes a squad.
    #[must_use]
    pub fn new(team_size: u8, seed: u64) -> Self {
        let ts = if team_size == 1 { 1 } else { 3 };
        let mut m = Self {
            seed,
            tick: 0,
            phase: Phase::Select,
            left: data::SELECT_SECS,
            team_size: ts,
            roster: Vec::new(),
            units: Vec::new(),
            projs: Vec::new(),
            zones: Vec::new(),
            next_id: 1,
            wave_left: data::WAVE_FIRST,
            court_respawn: [0.0; 2],
            boon: [0; 2],
            boon_left: [0.0; 2],
            kills: [0; 2],
            earned: [0; 2],
            first_blood: false,
            winner: 0,
            fx: Vec::new(),
            log: Vec::new(),
            pending: Vec::new(),
        };
        for s in 0..2 * ts {
            m.roster.push(SlotInfo {
                slot: s,
                team: u8::from(s >= ts),
                handle: format!("bot-{s}"),
                bot: true,
                champ: u8::MAX,
                picked: false,
                d: 0,
                f: 1,
                runes: [u8::MAX; 3],
                connected: false,
            });
        }
        m
    }

    /// Claim the lowest free slot for a human; None when the lobby is full.
    pub fn join(&mut self, handle: &str) -> Option<u8> {
        let s = self.roster.iter().position(|r| r.bot)? as u8;
        let r = &mut self.roster[usize::from(s)];
        r.bot = false;
        r.handle = handle.to_string();
        r.connected = true;
        Some(s)
    }

    /// A human left. The slot's champion becomes bot-driven from the next
    /// tick; the entry stays so a reconnect can reclaim the seat.
    pub fn leave(&mut self, slot: u8) {
        if let Some(r) = self.roster.iter_mut().find(|r| r.slot == slot) {
            r.connected = false;
            r.bot = true;
            if self.phase == Phase::Select {
                r.champ = u8::MAX;
                r.picked = false;
            }
        }
    }

    /// A human came back to a seat they still own.
    pub fn reconnect(&mut self, slot: u8, handle: &str) -> bool {
        if let Some(r) = self.roster.iter_mut().find(|r| r.slot == slot)
            && !r.connected
        {
            r.connected = true;
            r.bot = false;
            r.handle = handle.to_string();
            return true;
        }
        false
    }

    /// Validate and store a pick, during Select only. Champions are unique
    /// within each team; D and F must differ; the page must be three
    /// distinct runes.
    pub fn set_pick(&mut self, slot: u8, champ: u8, d: u8, f: u8, runes: [u8; 3]) {
        if self.phase != Phase::Select || champ >= 5 || d >= 4 || f >= 4 || d == f {
            return;
        }
        let mut seen = 0u32;
        for r in runes {
            if r >= 8 || seen & (1 << r) != 0 {
                return;
            }
            seen |= 1 << r;
        }
        let Some(team) = self.roster.iter().find(|r| r.slot == slot).map(|r| r.team) else {
            return;
        };
        if self
            .roster
            .iter()
            .any(|o| o.slot != slot && o.team == team && o.picked && o.champ == champ)
        {
            return;
        }
        if let Some(r) = self.roster.iter_mut().find(|o| o.slot == slot) {
            r.champ = champ;
            r.picked = true;
            r.d = d;
            r.f = f;
            r.runes = runes;
        }
    }

    /// Everyone who did not pick gets a deterministic random champion from
    /// the ones left; empty seats become bots with hashed pages. Then the
    /// match lives.
    pub fn start(&mut self) {
        if self.phase != Phase::Select {
            return;
        }
        let mut taken = [0u32; 2];
        for r in &self.roster {
            if r.picked {
                taken[usize::from(r.team)] |= 1 << u32::from(r.champ.min(4));
            }
        }
        // A seeded shuffle of the roster order for the fallback picks.
        let mut order: Vec<u8> = (0..5).collect();
        for j in 0..5 {
            let a = (rng::hash(self.seed, j as u64, 1, 91) >> 13) as usize % (5 - j).max(1);
            if j + a < 5 {
                order.swap(j, j + a);
            }
        }
        for r in &mut self.roster {
            if !r.picked {
                let team_taken = &mut taken[usize::from(r.team)];
                r.champ = order
                    .iter()
                    .copied()
                    .find(|c| *team_taken & (1 << u32::from(*c)) == 0)
                    .unwrap_or(0);
                *team_taken |= 1 << u32::from(r.champ);
                r.picked = true;
            }
            if r.bot {
                r.d = (rng::hash(self.seed, u64::from(r.slot), 3, 55) % 4) as u8;
                r.f = (r.d + 1 + (rng::hash(self.seed, u64::from(r.slot), 4, 56) % 3) as u8) % 4;
                let mut runes = [u8::MAX; 3];
                let mut n = 0usize;
                for k in 0..8 {
                    if n < 3
                        && rng::unit(self.seed, u64::from(r.slot) * 8 + u64::from(k), 5, 57) > 0.6
                    {
                        runes[n] = k;
                        n += 1;
                    }
                }
                for k in 0..8 {
                    if n < 3 && !runes.contains(&k) {
                        runes[n] = k;
                        n += 1;
                    }
                }
                r.runes = runes;
            } else if r.runes.contains(&u8::MAX) {
                r.runes = [0, 1, 2];
            }
        }
        self.spawn_armies();
        self.phase = Phase::Live;
        self.left = 0.0;
    }

    fn spawn_armies(&mut self) {
        self.units.clear();
        self.projs.clear();
        self.zones.clear();
        self.next_id = 1;
        let picks: Vec<(u8, u8, u8, u8, u8, [u8; 3])> = self
            .roster
            .iter()
            .map(|r| (r.slot, r.champ, r.team, r.d, r.f, r.runes))
            .collect();
        for (slot, champ, team, d, f, runes) in picks {
            let mut u = Unit::blank(self.next_id, Kind::Champ, team);
            self.next_id += 1;
            u.slot = slot;
            u.points = 1;
            u.def = champ.min(4);
            u.d = d;
            u.f = f;
            u.runes = runes;
            u.facing = if team == 0 { 0.0 } else { std::f32::consts::PI };
            let (fx, fz) = Self::fountain_pos(u.team);
            u.x = fx;
            u.z = fz;
            let (mh, mm) = self.champ_maxes(&u);
            u.max_hp = mh;
            u.hp = mh;
            u.max_mana = mm;
            u.mana = mm;
            self.units.push(u);
        }
        for i in 0..2 {
            let mut court = Unit::blank(
                self.next_id,
                if i == 0 { Kind::CourtN } else { Kind::CourtS },
                2,
            );
            self.next_id += 1;
            court.x = data::COURT_POS[i][0];
            court.z = data::COURT_POS[i][1];
            court.hp = data::COURT_HP;
            court.max_hp = data::COURT_HP;
            self.units.push(court);
        }
        for team in 0..2 {
            let mut core = Unit::blank(
                self.next_id,
                if team == 0 {
                    Kind::CoreBlue
                } else {
                    Kind::CoreRed
                },
                team,
            );
            self.next_id += 1;
            core.x = if team == 0 {
                -data::CORE_X
            } else {
                data::CORE_X
            };
            core.hp = data::CORE_HP;
            core.max_hp = data::CORE_HP;
            self.units.push(core);
        }
    }

    /// Where a team respawns: four units out from its own core.
    fn fountain_pos(team: u8) -> (f32, f32) {
        (
            if team == 0 {
                -data::CORE_X + 4.0
            } else {
                data::CORE_X - 4.0
            },
            0.0,
        )
    }

    /// The wire view of the whole world; drains the transient streams, so
    /// it is called exactly once per broadcast, like arena's shot events.
    pub fn snapshot(&mut self) -> proto::S2C {
        let units: Vec<UnitSnap> = self.units.iter().map(Self::unit_snap).collect();
        let mut champs: Vec<ChampView> = self
            .units
            .iter()
            .filter(|u| u.kind == Kind::Champ)
            .map(|u| ChampView {
                slot: u.slot,
                team: u.team,
                alive: !u.dead,
                resp: u.respawn,
                level: u.level,
                points: u.points,
                ranks: u.ranks,
                cds: u.cds,
                scds: u.scds,
                gold: u.gold,
                items: u.items,
                charges: u.charges,
                d: u.d,
                f: u.f,
            })
            .collect();
        champs.sort_by_key(|c| c.slot);
        let buffs: Vec<BuffSnap> = self
            .units
            .iter()
            .filter(|u| u.kind == Kind::Champ && !u.dead)
            .flat_map(|u| {
                u.buffs.iter().map(move |b| BuffSnap {
                    u: u.id,
                    k: b.k.code(),
                    ttl: b.ttl,
                    val: b.val,
                })
            })
            .collect();
        proto::S2C::State {
            tick: self.tick,
            secs: self.tick as f32 / 60.0,
            units,
            champs,
            buffs,
            projs: self
                .projs
                .iter()
                .filter(|p| p.travel > 0.0)
                .map(|p| ProjSnap {
                    id: p.id,
                    k: match p.kind {
                        ProjKind::Auto => 0,
                        ProjKind::Drone => 1,
                        ProjKind::Bolt => 2,
                        ProjKind::Hook => 3,
                    },
                    t: p.team,
                    champ: p.champ,
                    x: p.x,
                    z: p.z,
                    dx: p.dx,
                    dz: p.dz,
                })
                .collect(),
            zones: self
                .zones
                .iter()
                .filter(|zone| zone.ttl > 0.0)
                .map(|zone| ZoneSnap {
                    k: match zone.zk {
                        ZoneKind::Tornado => 0,
                        ZoneKind::Trap => 1,
                        ZoneKind::Stasis => 2,
                        ZoneKind::Shroud => 3,
                    },
                    x: zone.x,
                    z: zone.z,
                    r: zone.r,
                })
                .collect(),
            kills: self.kills,
            boon: self.boon,
            boon_left: self.boon_left,
            court_respawn: self.court_respawn,
            fx: std::mem::take(&mut self.fx),
            log: std::mem::take(&mut self.log),
        }
    }

    fn unit_snap(u: &Unit) -> UnitSnap {
        UnitSnap {
            id: u.id,
            k: u.kind.code(),
            t: u.team,
            slot: if u.kind == Kind::Champ {
                u.slot
            } else {
                u8::MAX
            },
            x: u.x,
            z: u.z,
            fa: u.facing,
            hp: u.hp.max(0.0),
            mh: u.max_hp as u16,
            def: u.def,
            lv: u.level,
            xp: u.xp,
            xpn: data::xp_needed(u.level),
            g: u.gold,
            pt: u.points,
            mn: u.mana as u16,
            mm: u.max_mana as u16,
            rk: u.ranks,
            cd: [
                u.cds[0].round(),
                u.cds[1].round(),
                u.cds[2].round(),
                u.cds[3].round(),
            ],
            scd: [u.scds[0].round(), u.scds[1].round()],
            items: u.items,
            charges: u.charges,
            dead: u.dead,
            resp: u.respawn,
            tp: u.tp,
        }
    }

    // ------------------------------------------------------------------
    // commands
    // ------------------------------------------------------------------

    /// Queue one player command for the next tick. The server owns the
    /// lobby; this forwards what a human sent, sanitized.
    pub fn command(&mut self, slot: u8, cmd: Cmd) {
        if self.phase == Phase::Live && (usize::from(slot)) < self.roster.len() {
            self.pending.push((slot, cmd.sanitized()));
        }
    }

    fn apply_cmd(&mut self, slot: u8, cmd: Cmd) {
        let Some(ui) = self.champ_by_slot(slot) else {
            return;
        };
        if self.units[ui].dead {
            return;
        }
        match cmd {
            Cmd::Move { x, z } => {
                let u = &mut self.units[ui];
                u.order = Order::Move;
                u.ox = x;
                u.oz = z;
            }
            Cmd::AttackMove { x, z } => {
                let u = &mut self.units[ui];
                u.order = Order::AttackMove;
                u.ox = x.clamp(-FIELD_X, FIELD_X);
                u.oz = z.clamp(-data::FIELD_Z, data::FIELD_Z);
                u.target = 0;
            }
            Cmd::Attack { target } => {
                if self.alive_enemy_of(ui, target) {
                    let (x, z) = (self.units[ui].x, self.units[ui].z);
                    let u = &mut self.units[ui];
                    u.order = Order::Attack;
                    u.target = target;
                    u.ox = x;
                    u.oz = z;
                }
            }
            Cmd::Cast { slot: ab, x, z } => self.cast(ui, ab, x, z),
            Cmd::Spell { slot: sp, x, z } => self.cast_spell(ui, sp, x, z),
            Cmd::Rank { slot: ab } => self.try_rank(ui, ab),
            Cmd::UseItem { slot: s } => self.use_item(ui, s),
            Cmd::Buy { item } => self.buy(ui, item),
        }
    }

    fn alive_enemy_of(&self, ui: usize, target: u32) -> bool {
        let mine = self.units[ui].team;
        self.units
            .iter()
            .any(|o| o.id == target && !o.dead && hostile(mine, o.kind, o.team))
    }

    pub fn champ_by_slot(&self, slot: u8) -> Option<usize> {
        self.units
            .iter()
            .position(|u| u.kind == Kind::Champ && u.slot == slot)
    }

    pub fn index_of(&self, id: u32) -> Option<usize> {
        if id == 0 {
            return None;
        }
        self.units.iter().position(|u| u.id == id)
    }

    fn try_rank(&mut self, ui: usize, ab: u8) {
        let u = &mut self.units[ui];
        let a = usize::from(ab);
        if u.points == 0 || u.ranks[a] >= 3 {
            return;
        }
        if a == 3 && u.level < data::R_LEVELS[usize::from(u.ranks[3])] {
            return;
        }
        u.ranks[a] += 1;
        u.points -= 1;
    }

    fn buy(&mut self, ui: usize, item: u16) {
        let Some(def) = data::item(item) else {
            return;
        };
        let u = &mut self.units[ui];
        if u.gold < def.cost {
            return;
        }
        if let Some(at) = (0..6).find(|&s| u.items[s] == item && u.charges[s] < def.charges) {
            u.gold -= def.cost;
            u.charges[at] = (u.charges[at] + def.charges).min(def.charges);
            return;
        }
        let Some(at) = (0..6).find(|&s| u.items[s] == 0) else {
            return;
        };
        if def.charges == 0 && u.items.contains(&item) {
            return; // no duplicates of permanents
        }
        u.gold -= def.cost;
        u.items[at] = item;
        u.charges[at] = def.charges;
        let (mh, mm) = self.champ_maxes(&self.units[ui]);
        let u = &mut self.units[ui];
        if mh > u.max_hp {
            u.hp += mh - u.max_hp;
        }
        if mm > u.max_mana {
            u.mana += mm - u.max_mana;
        }
        u.max_hp = mh;
        u.max_mana = mm;
    }

    fn use_item(&mut self, ui: usize, slot: u8) {
        let s = usize::from(slot);
        let (item, charges) = {
            let u = &self.units[ui];
            (u.items[s], u.charges[s])
        };
        let Some(def) = data::item(item) else {
            return;
        };
        if def.charges == 0 || charges == 0 {
            return;
        }
        let id = self.units[ui].id;
        self.units[ui].charges[s] -= 1;
        if def.heal > 0.0 {
            add_buff(
                &mut self.units[ui],
                BuffKind::Regen,
                6.0,
                def.heal / 6.0,
                id,
            );
        }
        if def.restore_mana > 0.0 {
            add_buff(
                &mut self.units[ui],
                BuffKind::ManaRegen,
                6.0,
                def.restore_mana / 6.0,
                id,
            );
        }
    }

    // ------------------------------------------------------------------
    // the step
    // ------------------------------------------------------------------

    /// One 60 Hz tick. The order here is the order of record.
    pub fn step(&mut self) {
        self.tick += 1;
        match self.phase {
            Phase::Select => {
                self.left -= crate::DT;
                if self.left <= 0.0 {
                    self.start();
                }
                return;
            }
            Phase::Over => {
                self.left -= crate::DT;
                return;
            }
            Phase::Live => {}
        }

        // 1. player commands (and bots), in queue order.
        let cmds = std::mem::take(&mut self.pending);
        for (slot, cmd) in cmds {
            self.apply_cmd(slot, cmd);
        }

        // 2. units, in id order.
        for i in 0..self.units.len() {
            self.step_unit(i);
        }

        // 3. projectiles, then zones.
        for i in 0..self.projs.len() {
            self.step_proj(i);
        }
        for i in 0..self.zones.len() {
            self.step_zone(i);
        }

        // 4. sweep dead minions, spent clones, finished shots and zones.
        self.units
            .retain(|u| !(u.dead && matches!(u.kind, Kind::Melee | Kind::Caster | Kind::Clone)));
        self.projs.retain(|p| p.travel > 0.0);
        self.zones.retain(|z| z.ttl > 0.0);

        // 5. waves and objectives.
        self.wave_left -= crate::DT;
        if self.wave_left <= 0.0 {
            self.spawn_wave();
            self.wave_left = data::WAVE_EVERY;
        }
        for c in 0..2 {
            if self.court_respawn[c] > 0.0 {
                self.court_respawn[c] -= crate::DT;
                if self.court_respawn[c] <= 0.0 {
                    self.respawn_court(c);
                }
            }
            if self.boon_left[c] > 0.0 {
                self.boon_left[c] -= crate::DT;
                if self.boon_left[c] <= 0.0 {
                    self.boon[c] = 0;
                }
            }
        }
        self.core_regen();

        // 6. win check: a core fell.
        if let Some(lost) = self
            .units
            .iter()
            .find(|u| matches!(u.kind, Kind::CoreBlue | Kind::CoreRed) && u.dead)
            .map(|u| u.team)
        {
            self.winner = 1 - lost;
            self.phase = Phase::Over;
            self.left = data::RESULT_SECS;
            self.log.push(LogEv {
                t: 3,
                a: u32::from(lost),
                b: 0,
                g: 0,
            });
        }
    }

    fn step_unit(&mut self, ui: usize) {
        let kind = self.units[ui].kind;
        if kind == Kind::Champ {
            self.step_champ_timers(ui);
            if self.units[ui].dead {
                self.units[ui].respawn -= crate::DT;
                if self.units[ui].respawn <= 0.0 {
                    self.revive(ui);
                }
                return;
            }
            self.regen_and_buffs(ui);
            if self.units[ui].dead {
                return; // burn killed us mid-tick
            }
            self.step_order(ui);
        } else if matches!(kind, Kind::Melee | Kind::Caster) {
            self.units[ui].atk_cd = (self.units[ui].atk_cd - crate::DT).max(0.0);
            self.regen_and_buffs(ui);
            if self.units[ui].dead {
                return;
            }
            self.step_minion(ui);
        } else if kind == Kind::Clone {
            self.units[ui].atk_cd = (self.units[ui].atk_cd - crate::DT).max(0.0);
            self.units[ui].ttl -= crate::DT;
            let parent = self.units[ui].parent;
            let parent_alive = self
                .units
                .iter()
                .any(|o| o.kind == Kind::Champ && o.slot == parent && !o.dead);
            if self.units[ui].ttl <= 0.0 || !parent_alive {
                self.units[ui].dead = true;
                let (x, z) = (self.units[ui].x, self.units[ui].z);
                self.fx.push(Fx {
                    source: 0,
                    champ: crate::proto::UNKNOWN_PRESENTATION,
                    ability: crate::proto::UNKNOWN_PRESENTATION,
                    k: 5,
                    x,
                    z,
                    x2: x,
                    z2: z,
                    v: 1.0,
                });
                return;
            }
            self.regen_and_buffs(ui);
            self.step_order(ui);
        } else if matches!(kind, Kind::CoreBlue | Kind::CoreRed) && !self.units[ui].dead {
            self.step_core(ui);
        }
    }

    /// Minions absorb core fire before champions; keep shooting the same
    /// valid target within that priority so damage is not spread at random.
    fn step_core(&mut self, ui: usize) {
        self.units[ui].atk_cd = (self.units[ui].atk_cd - crate::DT).max(0.0);
        let core = &self.units[ui];
        let target = self
            .units
            .iter()
            .enumerate()
            .filter(|(_, unit)| {
                !unit.dead
                    && unit.team != core.team
                    && matches!(unit.kind, Kind::Champ | Kind::Melee | Kind::Caster)
                    && dist(core.x, core.z, unit.x, unit.z) <= data::CORE_ATTACK_RANGE
            })
            .min_by(|(_, a), (_, b)| {
                (a.kind == Kind::Champ)
                    .cmp(&(b.kind == Kind::Champ))
                    .then_with(|| (a.id != core.target).cmp(&(b.id != core.target)))
                    .then_with(|| {
                        dist(core.x, core.z, a.x, a.z).total_cmp(&dist(core.x, core.z, b.x, b.z))
                    })
                    .then_with(|| a.id.cmp(&b.id))
            })
            .map(|(index, _)| index);
        if let Some(target) = target {
            self.units[ui].target = self.units[target].id;
            if self.units[ui].atk_cd <= 0.0 {
                self.do_attack(ui, target);
            }
        } else {
            self.units[ui].target = 0;
        }
    }

    fn step_champ_timers(&mut self, ui: usize) {
        let stats = self.champ_stats_of(&self.units[ui]);
        // keep maxima current with whatever levels and items arrived
        let (mh, mm) = self.champ_maxes(&self.units[ui]);
        self.units[ui].max_hp = mh;
        self.units[ui].max_mana = mm;
        for c in 0..4 {
            self.units[ui].cds[c] = (self.units[ui].cds[c] - crate::DT).max(0.0);
        }
        for c in 0..2 {
            self.units[ui].scds[c] = (self.units[ui].scds[c] - crate::DT).max(0.0);
        }
        self.units[ui].atk_cd = (self.units[ui].atk_cd - crate::DT).max(0.0);
        self.units[ui].form = (self.units[ui].form - crate::DT).max(0.0);
        if self.units[ui].form <= 0.0 {
            self.units[ui].tp = 0;
        }
        // passive income continues during the respawn timer
        let gold =
            data::GOLD_PER_SEC * (1.0 + (stats.gold + self.boon_gold(ui)) / 100.0) * crate::DT;
        let team = usize::from(self.units[ui].team);
        self.units[ui].gold_frac += gold;
        if self.units[ui].gold_frac >= 1.0 {
            let whole = self.units[ui].gold_frac as u32;
            self.units[ui].gold_frac -= whole as f32;
            self.units[ui].gold += whole;
            self.earned[team] += whole;
        }
        self.gain_xp(ui, data::XP_PER_SEC * crate::DT);
    }

    /// Shared: burn/regen ticks and buff expiry, for every living unit.
    fn regen_and_buffs(&mut self, ui: usize) {
        if self.units[ui].kind == Kind::Champ && !self.units[ui].dead {
            let stats = self.champ_stats_of(&self.units[ui]);
            let (mx, mm) = (self.units[ui].max_hp, self.units[ui].max_mana);
            let (hp_regen, mana_regen) = if self.in_own_fountain(ui) {
                (mx * 0.06, mm * 0.05)
            } else {
                (mx * 0.0025, mm * (0.004 + 0.004 * stats.mregen / 100.0))
            };
            let u = &mut self.units[ui];
            u.hp = (u.hp + hp_regen * crate::DT).min(mx);
            u.mana = (u.mana + mana_regen * crate::DT).min(mm);
        }
        // Damage may remove a revive mark or clear every buff on death.
        // Iterate a stable copy so a lethal burn cannot invalidate indices.
        let buffs = self.units[ui].buffs.clone();
        for buff in buffs {
            let (k, val, src) = (buff.k, buff.val, buff.src);
            if buff.ttl <= 0.0 {
                continue;
            }
            match k {
                BuffKind::Burn if val > 0.0 => {
                    self.deal_damage(ui, val * crate::DT, 2, src, false);
                }
                BuffKind::Regen => {
                    let mx = self.units[ui].max_hp;
                    let u = &mut self.units[ui];
                    u.hp = (u.hp + val * crate::DT).min(mx);
                }
                BuffKind::ManaRegen => {
                    let mm = self.units[ui].max_mana;
                    let u = &mut self.units[ui];
                    u.mana = (u.mana + val * crate::DT).min(mm);
                }
                _ => {}
            }
            if self.units[ui].dead {
                return;
            }
            if let Some(bb) = self.units[ui]
                .buffs
                .iter_mut()
                .find(|b| b.k == k && b.src == src)
            {
                bb.ttl -= crate::DT;
            }
        }
        self.units[ui].buffs.retain(|b| b.ttl > 0.0);
    }

    fn in_own_fountain(&self, ui: usize) -> bool {
        let u = &self.units[ui];
        let core_x = if u.team == 0 {
            -data::CORE_X
        } else {
            data::CORE_X
        };
        (u.x - core_x).abs() <= data::FOUNTAIN_R && u.z.abs() <= 6.0
    }

    pub(crate) fn boon_haste(&self, ui: usize) -> f32 {
        let t = usize::from(self.units[ui].team);
        if self.boon[t] == 2 { 15.0 } else { 0.0 }
    }

    pub(crate) fn boon_gold(&self, ui: usize) -> f32 {
        self.boon_haste(ui)
    }

    fn revive(&mut self, ui: usize) {
        let (fx, fz) = Self::fountain_pos(self.units[ui].team);
        let u = &mut self.units[ui];
        u.x = fx;
        u.z = fz;
        let u = &mut self.units[ui];
        u.dead = false;
        u.respawn = 0.0;
        u.hp = u.max_hp;
        u.mana = u.max_mana;
        u.buffs.clear();
        u.order = Order::Hold;
        u.target = 0;
        u.atk_cd = 0.0;
        u.dmg_log = [(u8::MAX, 0); 8];
    }

    /// Movement and auto-attacks for champions, clones and ordered units.
    fn step_order(&mut self, ui: usize) {
        match self.units[ui].order {
            Order::Hold => {}
            Order::Move => {
                let (ox, oz) = {
                    let u = &self.units[ui];
                    (u.ox, u.oz)
                };
                self.walk_toward(ui, ox, oz);
            }
            Order::Attack => {
                let target = self.units[ui].target;
                let ti = self.index_of(target).filter(|&i| !self.units[i].dead);
                let Some(ti) = ti else {
                    self.units[ui].order = Order::Hold;
                    return;
                };
                self.step_attack_target(ui, ti);
            }
            Order::AttackMove => {
                if let Some(ti) = self.attack_move_target(ui) {
                    self.units[ui].target = self.units[ti].id;
                    self.step_attack_target(ui, ti);
                } else {
                    self.units[ui].target = 0;
                    self.walk_toward(ui, self.units[ui].ox, self.units[ui].oz);
                }
            }
        }
    }

    /// Keep a valid engagement; otherwise acquire the nearest opponent, using
    /// unit id to resolve ties. Attack-move never initiates a neutral court fight.
    fn attack_move_target(&self, ui: usize) -> Option<usize> {
        let u = &self.units[ui];
        self.units
            .iter()
            .enumerate()
            .filter(|(_, target)| {
                !target.dead
                    && target.team != u.team
                    && matches!(
                        target.kind,
                        Kind::Champ | Kind::Melee | Kind::Caster | Kind::CoreBlue | Kind::CoreRed
                    )
                    && dist(u.x, u.z, target.x, target.z) <= ATTACK_MOVE_RANGE
            })
            .min_by(|(_, a), (_, b)| {
                (a.id != u.target)
                    .cmp(&(b.id != u.target))
                    .then_with(|| dist(u.x, u.z, a.x, a.z).total_cmp(&dist(u.x, u.z, b.x, b.z)))
                    .then_with(|| a.id.cmp(&b.id))
            })
            .map(|(i, _)| i)
    }

    /// An attack releases immediately and pays its existing cooldown. Cast
    /// animation takeover never resets this timer or recalls released shots.
    fn step_attack_target(&mut self, ui: usize, ti: usize) {
        let (ux, uz) = (self.units[ui].x, self.units[ui].z);
        let (tx, tz) = (self.units[ti].x, self.units[ti].z);
        let reach = self.range_of(ui) + self.units[ti].kind.hit_r();
        if dist(ux, uz, tx, tz) > reach {
            self.walk_toward(ui, tx, tz);
        } else {
            self.face_to(ui, tx, tz);
            if self.units[ui].atk_cd <= 0.0 {
                self.do_attack(ui, ti);
            }
        }
    }

    fn step_minion(&mut self, ui: usize) {
        let (x, z, team) = {
            let u = &self.units[ui];
            (u.x, u.z, u.team)
        };
        let mut best: Option<(usize, f32)> = None;
        for i in 0..self.units.len() {
            let o = &self.units[i];
            if o.dead || !hostile(team, o.kind, o.team) || o.kind.is_objective() {
                continue;
            }
            let d = dist(x, z, o.x, o.z);
            if d <= 8.0 && best.is_none_or(|(_, bd)| d < bd) {
                best = Some((i, d));
            }
        }
        if let Some((ti, d)) = best {
            let (tx, tz) = (self.units[ti].x, self.units[ti].z);
            if d <= 1.9 + self.range_of(ui) - 1.6 {
                self.face_to(ui, tx, tz);
                if self.units[ui].atk_cd <= 0.0 {
                    self.do_attack(ui, ti);
                }
            } else {
                self.walk_toward(ui, tx, tz);
            }
            return;
        }
        let (core_x, core_kind) = if team == 0 {
            (data::CORE_X, Kind::CoreRed)
        } else {
            (-data::CORE_X, Kind::CoreBlue)
        };
        if let Some(ci) = self.units.iter().position(|o| o.kind == core_kind)
            && !self.units[ci].dead
            && dist(x, z, self.units[ci].x, self.units[ci].z) <= 4.2
        {
            let (tx, tz) = (self.units[ci].x, self.units[ci].z);
            self.face_to(ui, tx, tz);
            if self.units[ui].atk_cd <= 0.0 {
                self.do_attack(ui, ci);
            }
            return;
        }
        let lane_z = self.units[ui].lane_offset();
        let (tx, tz) = if (z - lane_z).abs() > 0.4 {
            (core_x * 0.5, lane_z)
        } else {
            (core_x, lane_z)
        };
        self.walk_toward(ui, tx, tz);
    }

    fn walk_toward(&mut self, ui: usize, tx: f32, tz: f32) {
        let speed = self.speed_of(ui);
        if speed <= 0.1 {
            return;
        }
        let (x, z) = (self.units[ui].x, self.units[ui].z);
        let (dx, dz) = dir_to(x, z, tx, tz);
        let d = dist(x, z, tx, tz);
        if d < 0.35 {
            if self.units[ui].order == Order::Move {
                self.units[ui].order = Order::Hold;
            }
            return;
        }
        let step = (speed * crate::DT).min(d);
        let nx = (x + dx * step).clamp(-FIELD_X, FIELD_X);
        let nz = (z + dz * step).clamp(-data::FIELD_Z, data::FIELD_Z);
        let u = &mut self.units[ui];
        u.x = nx;
        u.z = nz;
        u.facing = dz.atan2(dx);
    }

    fn face_to(&mut self, ui: usize, tx: f32, tz: f32) {
        let (x, z) = (self.units[ui].x, self.units[ui].z);
        let (dx, dz) = dir_to(x, z, tx, tz);
        self.units[ui].facing = dz.atan2(dx);
    }

    #[must_use]
    pub fn speed_of(&self, ui: usize) -> f32 {
        let u = &self.units[ui];
        if has_buff(u, BuffKind::Root) {
            return 0.0;
        }
        match u.kind {
            Kind::Champ | Kind::Clone => {
                data::champ_stats(u.def, u.level).ms
                    * MS_SCALE
                    * Self::ms_mult(u, u.kind == Kind::Champ)
            }
            Kind::Melee | Kind::Caster => 300.0 * MS_SCALE * Self::ms_mult(u, false),
            _ => 0.0,
        }
    }

    /// Move-speed percent total for a unit (runes, items, buffs).
    fn ms_pct(u: &Unit, champ: bool) -> f32 {
        let mut pct = 0.0;
        if champ && u.kind == Kind::Champ {
            pct += data::rune_stats(&u.runes).ms;
            pct += item_sum(&u.items, |s| s.ms);
        }
        for b in &u.buffs {
            match b.k {
                BuffKind::Ms => pct += b.val,
                BuffKind::Slow => pct -= b.val,
                BuffKind::Exhaust => pct -= 30.0,
                _ => {}
            }
        }
        pct
    }

    fn ms_mult(u: &Unit, champ: bool) -> f32 {
        (1.0 + Self::ms_pct(u, champ) / 100.0).max(0.05)
    }

    #[must_use]
    pub fn range_of(&self, ui: usize) -> f32 {
        let u = &self.units[ui];
        match u.kind {
            Kind::Champ | Kind::Clone => data::CHAMPS[usize::from(u.def.min(4))].range,
            Kind::Melee => 1.6,
            Kind::Caster => 7.0,
            Kind::CoreBlue | Kind::CoreRed => data::CORE_ATTACK_RANGE,
            _ => 0.0,
        }
    }

    /// One auto-attack from `ui` at `ti`, paid for by `atk_cd`.
    #[allow(
        clippy::too_many_lines,
        reason = "One attack resolves stats, crit, cooldown and its melee/projectile effect in a fixed order."
    )]
    fn do_attack(&mut self, ui: usize, ti: usize) {
        let kind = self.units[ui].kind;
        let champ = self.presentation_champ(self.units[ui].id);
        let reach_is_melee = self.range_of(ui) < 2.5;
        let (aid, team) = (self.units[ui].id, self.units[ui].team);
        let mut stats = if kind == Kind::Champ {
            self.champ_stats_of(&self.units[ui])
        } else if kind == Kind::Clone {
            let p = self.units[ui].parent;
            let pct = self.units[ui].clone_dmg;
            let mut s = self
                .units
                .iter()
                .find(|o| o.kind == Kind::Champ && o.slot == p)
                .map(|o| self.champ_stats_of(o))
                .unwrap_or_default();
            s.ad *= pct;
            s
        } else if matches!(kind, Kind::CoreBlue | Kind::CoreRed) {
            data::Stats {
                ad: data::CORE_ATTACK_DAMAGE,
                ..data::Stats::ZERO
            }
        } else {
            let waves = (self.tick / (60 * 120)) as f32;
            let scale = 1.0 + 0.15 * waves;
            let mut s = data::Stats::ZERO;
            s.ad = if kind == Kind::Caster { 13.0 } else { 9.0 } * scale;
            s
        };
        let target_is_objective = self.units[ti].kind.is_objective();
        if kind == Kind::Clone {
            stats.crit = 0.0; // holograms do not roll crits
        }
        let fire = self.units[ui]
            .buffs
            .iter()
            .find(|b| b.k == BuffKind::Fire)
            .map(|b| (b.val, b.aux));
        let mut dmg = stats.ad;
        let crit = stats.crit > 0.0
            && rng::unit(self.seed, self.tick, u64::from(aid), 7) * 100.0 < stats.crit;
        if crit {
            dmg *= stats.critd / 100.0;
        }
        if let Some((pct, _)) = fire {
            dmg += stats.ad * pct / 100.0;
        }
        let aspd_scale = 1.0
            / (1.0
                + if kind == Kind::Champ {
                    stats.aspd / 100.0
                } else {
                    0.0
                });
        self.units[ui].atk_cd = self.atk_cd_of(ui) * aspd_scale;
        // item burn rides the hit: emberbrand-style dots
        let item_burn = if kind == Kind::Champ {
            let ap = stats.ap;
            self.units[ui]
                .items
                .iter()
                .filter_map(|&it| data::item(it))
                .map(|d| d.burn + d.burn_ap * ap)
                .fold(0.0, f32::max)
        } else {
            0.0
        };
        let burn = item_burn.max(fire.map_or(0.0, |(_, aux)| aux));
        if target_is_objective || reach_is_melee {
            self.deal_damage(ti, dmg, 0, aid, crit);
            if let Some(b) = self.units.get_mut(ti)
                && burn > 0.0
                && b.kind == Kind::Champ
            {
                add_buff(b, BuffKind::Burn, 2.0, burn, aid);
            }
            let (sx, sz) = (self.units[ui].x, self.units[ui].z);
            let (tx, tz) = (self.units[ti].x, self.units[ti].z);
            self.fx.push(Fx {
                source: aid,
                champ,
                ability: 4,
                k: 0,
                x: sx,
                z: sz,
                x2: tx,
                z2: tz,
                v: 4.0 + f32::from(crit),
            });
        } else {
            let (sx, sz) = (self.units[ui].x, self.units[ui].z);
            let (tx, tz) = (self.units[ti].x, self.units[ti].z);
            let (dx, dz) = dir_to(sx, sz, tx, tz);
            self.projs.push(Proj {
                id: self.next_id,
                owner: aid,
                champ,
                ability: 4,
                team,
                kind: ProjKind::Auto,
                x: sx,
                z: sz,
                dx,
                dz,
                speed: 18.0,
                travel: dist(sx, sz, tx, tz) + 1.5,
                dmg,
                crit,
                burn,
                slow_pct: 0.0,
                slow_ttl: 0.0,
                pierce: 0,
                hook_rank: 0,
                homing: self.units[ti].id,
                hit: [0; 4],
                nhit: 0,
            });
            self.next_id += 1;
            // Ranged champions need an attack-start event as well as their
            // later impact. This adds no projectile, damage or cooldown work.
            if champ != proto::UNKNOWN_PRESENTATION {
                self.fx.push(Fx {
                    source: aid,
                    champ,
                    ability: 4,
                    k: 0,
                    x: sx,
                    z: sz,
                    x2: tx,
                    z2: tz,
                    v: 4.0 + f32::from(crit),
                });
            }
        }
    }

    pub(crate) fn presentation_champ(&self, owner: u32) -> u8 {
        self.index_of(owner)
            .filter(|&i| matches!(self.units[i].kind, Kind::Champ | Kind::Clone))
            .map_or(proto::UNKNOWN_PRESENTATION, |i| self.units[i].def)
    }

    fn atk_cd_of(&self, ui: usize) -> f32 {
        let u = &self.units[ui];
        match u.kind {
            Kind::Champ | Kind::Clone => data::CHAMPS[usize::from(u.def.min(4))].atk_cd,
            Kind::Melee => 1.1,
            Kind::Caster => 1.2,
            Kind::CoreBlue | Kind::CoreRed => data::CORE_ATTACK_CD,
            _ => 1.0,
        }
    }

    // ------------------------------------------------------------------
    // projectiles and zones
    // ------------------------------------------------------------------

    #[allow(
        clippy::too_many_lines,
        reason = "Projectile movement, collision and hit effects form one ordered tick transition."
    )]
    fn step_proj(&mut self, pi: usize) {
        if self.projs[pi].travel <= 0.0 {
            return;
        }
        let (mut x, mut z) = (self.projs[pi].x, self.projs[pi].z);
        let mut dx = self.projs[pi].dx;
        let mut dz = self.projs[pi].dz;
        let homing = self.projs[pi].homing;
        if homing != 0 {
            if let Some(ti) = self.index_of(homing).filter(|&i| !self.units[i].dead) {
                let (tx, tz) = (self.units[ti].x, self.units[ti].z);
                (dx, dz) = dir_to(x, z, tx, tz);
            } else {
                self.projs[pi].travel = 0.0;
                return;
            }
        }
        let speed = self.projs[pi].speed;
        let step = speed * crate::DT;
        x += dx * step;
        z += dz * step;
        self.projs[pi].x = x;
        self.projs[pi].z = z;
        self.projs[pi].dx = dx;
        self.projs[pi].dz = dz;
        self.projs[pi].travel -= step;

        let team = self.projs[pi].team;
        let mut hit_i: Option<usize> = None;
        for i in 0..self.units.len() {
            let o = &self.units[i];
            if o.dead || !o.kind.hittable_by_shot() || o.team == team {
                continue;
            }
            if homing != 0 && o.id != homing {
                continue;
            }
            if self.projs[pi].hit.contains(&o.id) {
                continue;
            }
            if dist(x, z, o.x, o.z) <= o.kind.hit_r() + 0.35 {
                hit_i = Some(i);
                break;
            }
        }
        let Some(ti) = hit_i else { return };

        let (champ, ability) = (self.projs[pi].champ, self.projs[pi].ability);

        let (dmg, crit, burn, pk, pierce, slow, slow_ttl, hook_rank, owner) = {
            let p = &self.projs[pi];
            (
                p.dmg,
                p.crit,
                p.burn,
                p.kind,
                p.pierce,
                p.slow_pct,
                p.slow_ttl,
                p.hook_rank,
                p.owner,
            )
        };
        // where the shot's caster stands, for hook pulls
        let (ox, oz) = self
            .index_of(owner)
            .map_or((x, z), |i| (self.units[i].x, self.units[i].z));
        if pk == ProjKind::Hook {
            self.deal_damage(ti, dmg, 1, owner, false);
            let root = 0.6 + 0.1 * f32::from(hook_rank);
            if self.units[ti].kind == Kind::Champ {
                let (dx2, dz2) = dir_to(ox, oz, self.units[ti].x, self.units[ti].z);
                let u = &mut self.units[ti];
                u.x = (ox + dx2 * 2.0).clamp(-FIELD_X, FIELD_X);
                u.z = (oz + dz2 * 2.0).clamp(-data::FIELD_Z, data::FIELD_Z);
                add_buff(u, BuffKind::Root, root, 0.0, owner);
            } else {
                add_buff(&mut self.units[ti], BuffKind::Slow, 1.0, 40.0, owner);
            }
            self.fx.push(Fx {
                source: owner,
                champ,
                ability,
                k: 10,
                x: ox,
                z: oz,
                x2: self.units[ti].x,
                z2: self.units[ti].z,
                v: 0.0,
            });
            self.projs[pi].travel = 0.0;
        } else {
            let as_spell = pk != ProjKind::Auto;
            self.deal_damage(ti, dmg, u8::from(as_spell), owner, crit);
            if burn > 0.0 && self.units[ti].kind == Kind::Champ {
                add_buff(&mut self.units[ti], BuffKind::Burn, 2.0, burn, owner);
            }
            if slow > 0.0 {
                add_buff(&mut self.units[ti], BuffKind::Slow, slow_ttl, slow, owner);
            }
            self.fx.push(Fx {
                source: owner,
                champ,
                ability,
                k: 0,
                x: self.units[ti].x,
                z: self.units[ti].z,
                x2: 0.0,
                z2: 0.0,
                v: f32::from(crit) + if as_spell { 2.0 } else { 0.0 },
            });
            if pk == ProjKind::Bolt && pierce > 0 {
                let tid = self.units[ti].id;
                let p = &mut self.projs[pi];
                if p.nhit < 4 {
                    p.hit[p.nhit] = tid;
                    p.nhit += 1;
                }
                p.pierce -= 1;
                if p.pierce == 0 {
                    p.travel = 0.0;
                }
            } else {
                self.projs[pi].travel = 0.0;
            }
        }
    }

    #[allow(
        clippy::while_float,
        reason = "The half-second zone accumulator intentionally preserves existing f32 timing and catch-up order."
    )]
    fn step_zone(&mut self, zi: usize) {
        if self.zones[zi].ttl <= 0.0 {
            return;
        }
        let attach = self.zones[zi].attach;
        let (mut x, mut z) = (self.zones[zi].x, self.zones[zi].z);
        if attach != 0 {
            match self.index_of(attach) {
                Some(ai) if !self.units[ai].dead => {
                    x = self.units[ai].x;
                    z = self.units[ai].z;
                    self.zones[zi].x = x;
                    self.zones[zi].z = z;
                }
                _ => {
                    self.zones[zi].ttl = 0.0;
                    return;
                }
            }
        }
        self.zones[zi].ttl -= crate::DT;
        let (owner, team, zk, r, dmg, root, slow, detonate) = {
            let z = &self.zones[zi];
            (
                z.owner, z.team, z.zk, z.r, z.dmg, z.root, z.slow, z.detonate,
            )
        };
        let (champ, ability) = (self.zones[zi].champ, self.zones[zi].ability);
        match zk {
            ZoneKind::Trap => {
                for i in 0..self.units.len() {
                    if self.hitable_enemy(i, team, x, z, r) {
                        self.deal_damage(i, dmg, 1, owner, true);
                        add_buff(&mut self.units[i], BuffKind::Root, root, 0.0, owner);
                        self.zones[zi].ttl = 0.0;
                        self.fx.push(Fx {
                            source: 0,
                            champ,
                            ability,
                            k: 2,
                            x,
                            z,
                            x2: x,
                            z2: z,
                            v: r,
                        });
                        break;
                    }
                }
            }
            ZoneKind::Tornado | ZoneKind::Stasis | ZoneKind::Shroud => {
                let mut tick_left = self.zones[zi].tick_t - crate::DT;
                while tick_left <= 0.0 {
                    for i in 0..self.units.len() {
                        if self.hitable_enemy(i, team, x, z, r) {
                            let dealt = self.deal_damage(i, dmg, 1, owner, true);
                            if zk == ZoneKind::Shroud
                                && let Some(oi) = self.index_of(owner)
                            {
                                let mx = self.units[oi].max_hp;
                                let half = dealt * 0.5;
                                let u = &mut self.units[oi];
                                u.hp = (u.hp + half).min(mx);
                            }
                            let live = &mut self.units[i];
                            if slow > 0.0 {
                                add_buff(live, BuffKind::Slow, 0.6, slow, owner);
                            }
                            if zk == ZoneKind::Stasis {
                                add_buff(live, BuffKind::Root, 0.6, 0.0, owner);
                            }
                        }
                    }
                    tick_left += 0.5;
                }
                self.zones[zi].tick_t = tick_left;
                if self.zones[zi].ttl <= 0.0 && zk == ZoneKind::Stasis && detonate > 0.0 {
                    for i in 0..self.units.len() {
                        if self.hitable_enemy(i, team, x, z, r) {
                            self.deal_damage(i, detonate, 1, owner, true);
                        }
                    }
                    self.fx.push(Fx {
                        source: 0,
                        champ,
                        ability,
                        k: 2,
                        x,
                        z,
                        x2: x,
                        z2: z,
                        v: r,
                    });
                }
            }
        }
    }

    /// Can a zone/beam from `owner` on `team` hurt unit `i` at radius `r`?
    pub(crate) fn hitable_enemy(&self, i: usize, team: u8, x: f32, z: f32, r: f32) -> bool {
        let target = &self.units[i];
        if target.dead {
            return false;
        }
        if target.kind == Kind::Clone {
            return false;
        }
        if matches!(
            target.kind,
            Kind::CourtN | Kind::CourtS | Kind::CoreBlue | Kind::CoreRed
        ) {
            return false;
        }
        if target.team == team {
            return false;
        }
        dist(x, z, target.x, target.z) <= r + target.kind.hit_r()
    }

    // ------------------------------------------------------------------
    // damage and death
    // ------------------------------------------------------------------

    /// Apply `raw` damage from unit `src` to unit `ti`. `source_kind`:
    /// 0 auto-attack, 1 ability (spell multipliers apply), 2 dot/true.
    /// Returns what actually landed after immunities, shields and DR.
    #[allow(
        clippy::too_many_lines,
        reason = "Damage resolution keeps source modifiers, shields, assist credit and lethal effects in their deterministic order."
    )]
    pub fn deal_damage(
        &mut self,
        ti: usize,
        raw: f32,
        source_kind: u8,
        src: u32,
        _crit: bool,
    ) -> f32 {
        if raw <= 0.0 || !raw.is_finite() {
            return 0.0;
        }
        let (is_dead, kind, vteam) = {
            let t = &self.units[ti];
            (t.dead, t.kind, t.team)
        };
        if is_dead || kind == Kind::Clone {
            return 0.0;
        }
        if matches!(kind, Kind::CourtN | Kind::CourtS) {
            let from_champ = self
                .units
                .iter()
                .any(|o| o.id == src && (o.kind == Kind::Champ || o.kind == Kind::Clone));
            if !from_champ {
                return 0.0;
            }
        }
        let mut dmg = raw;
        // source multipliers: spell amp, damage amp, exhaustion, boon north
        if let Some(si) = self.index_of(src)
            && self.units[si].team != vteam
        {
            if self.units[si].kind == Kind::Champ && source_kind == 1 {
                let st = self.champ_stats_of(&self.units[si]);
                dmg *= 1.0 + st.spell / 100.0;
            }
            if self.units[si]
                .buffs
                .iter()
                .any(|b| b.k == BuffKind::Exhaust)
            {
                dmg *= 0.6;
            }
            if self.boon[usize::from(self.units[si].team)] == 1 {
                dmg *= 1.12;
            }
            if let Some(a) = self.units[si]
                .buffs
                .iter()
                .find(|b| b.k == BuffKind::DmgAmp)
            {
                dmg *= 1.0 + a.val / 100.0;
            }
        }
        // True damage bypasses reduction, but immunity and shields still
        // protect against it.
        if kind == Kind::Champ && source_kind != 2 {
            let dr = self.champ_stats_of(&self.units[ti]).dr;
            dmg *= 1.0 - dr / 100.0;
        }
        if self.units[ti]
            .buffs
            .iter()
            .any(|b| b.k == BuffKind::Immune && b.ttl > 0.0)
        {
            return 0.0;
        }
        let mut rest = dmg;
        for b in 0..self.units[ti].buffs.len() {
            if rest <= 0.0 {
                break;
            }
            if self.units[ti].buffs[b].k == BuffKind::Shield {
                let eat = self.units[ti].buffs[b].val.min(rest);
                self.units[ti].buffs[b].val -= eat;
                rest -= eat;
                if self.units[ti].buffs[b].val <= 0.0 {
                    self.units[ti].buffs[b].ttl = 0.0;
                }
            }
        }
        if rest <= 0.0 {
            return 0.0;
        }
        if source_kind == 1 && kind == Kind::Champ {
            let slows = self.index_of(src).is_some_and(|si| {
                self.units[si]
                    .items
                    .iter()
                    .filter_map(|&item| data::item(item))
                    .any(|item| item.spell_slow)
            });
            if slows {
                add_buff(&mut self.units[ti], BuffKind::Slow, 1.0, 20.0, src);
            }
        }
        // credit: the champion behind the hit (a clone credits its parent)
        let credit_slot = self
            .index_of(src)
            .filter(|&i| {
                matches!(self.units[i].kind, Kind::Champ | Kind::Clone)
                    && self.units[i].team != vteam
            })
            .map(|i| self.units[i].slot);
        if let Some(sl) = credit_slot {
            let u = &mut self.units[ti];
            if let Some(entry) = u.dmg_log.iter_mut().find(|(slot, _)| *slot == sl) {
                entry.1 = self.tick;
            } else {
                u.dmg_log[u.dmg_next % 8] = (sl, self.tick);
                u.dmg_next = u.dmg_next.wrapping_add(1);
            }
        }
        let dealt = rest.min(self.units[ti].hp.max(0.0));
        let u = &mut self.units[ti];
        u.hp -= rest;
        if u.hp <= 0.0 {
            self.on_death(ti, src, credit_slot);
        }
        dealt
    }

    /// The victim at `ti` has reached 0 hp. `src` landed the blow.
    #[allow(
        clippy::too_many_lines,
        reason = "The exhaustive unit-kind dispatch keeps revive, bounty, experience and objective transitions together at the lethal-hit boundary."
    )]
    fn on_death(&mut self, ti: usize, src: u32, killer_slot: Option<u8>) {
        let (kind, team) = (self.units[ti].kind, self.units[ti].team);
        let (x, z) = (self.units[ti].x, self.units[ti].z);
        match kind {
            Kind::Champ => {
                // the hallow mark eats the killing blow once
                if let Some(mi) = self.units[ti]
                    .buffs
                    .iter()
                    .position(|b| b.k == BuffKind::Revive && b.ttl > 0.0)
                {
                    let pct = self.units[ti].buffs[mi].val;
                    self.units[ti].buffs.remove(mi);
                    let mx = self.units[ti].max_hp;
                    let u = &mut self.units[ti];
                    u.hp = mx * pct / 100.0;
                    u.dmg_log = [(u8::MAX, 0); 8];
                    self.fx.push(Fx {
                        source: 0,
                        champ: data::HALLOW,
                        ability: 3,
                        k: 9,
                        x,
                        z,
                        x2: x,
                        z2: z,
                        v: 3.0,
                    });
                    self.log.push(LogEv {
                        t: 5,
                        a: self.units[ti].id,
                        b: 0,
                        g: 0,
                    });
                    return;
                }
                let vl = self.units[ti].level;
                self.units[ti].dead = true;
                self.units[ti].hp = 0.0;
                self.units[ti].form = 0.0;
                self.units[ti].tp = 0;
                self.units[ti].respawn = (5.0 + 1.6 * f32::from(vl)).min(25.0);
                self.units[ti].buffs.clear();
                self.units[ti].order = Order::Hold;
                self.units[ti].target = 0;
                self.fx.push(Fx {
                    source: 0,
                    champ: crate::proto::UNKNOWN_PRESENTATION,
                    ability: crate::proto::UNKNOWN_PRESENTATION,
                    k: 5,
                    x,
                    z,
                    x2: x,
                    z2: z,
                    v: 0.0,
                });
                // bounty: killer 70%, assists split 30%. The assist list is
                // the victim's own damage log, not anybody else's.
                let bounty = 280 + 40 * u32::from(vl);
                let mut assists: Vec<u8> = self.units[ti]
                    .dmg_log
                    .iter()
                    .filter(|(sl, tk)| {
                        *sl != u8::MAX
                            && self.tick.saturating_sub(*tk) <= 8 * 60
                            && Some(*sl) != killer_slot
                    })
                    .map(|(sl, _)| *sl)
                    .collect();
                assists.sort_unstable();
                assists.dedup();
                self.units[ti].dmg_log = [(u8::MAX, 0); 8];
                let mut paid = 0u32;
                if let Some(ks) = killer_slot {
                    let share = if assists.is_empty() {
                        bounty
                    } else {
                        (bounty * 7 / 10).max(120)
                    };
                    self.pay_gold_to_slot(ks, share);
                    paid += share;
                }
                if !assists.is_empty() {
                    let each =
                        bounty.saturating_sub(paid) / u32::try_from(assists.len()).unwrap_or(1);
                    for a in &assists {
                        self.pay_gold_to_slot(*a, each);
                    }
                }
                if !self.first_blood {
                    self.first_blood = true;
                    if let Some(ks) = killer_slot {
                        self.pay_gold_to_slot(ks, 100);
                    }
                    self.log.push(LogEv {
                        t: 1,
                        a: self.units[ti].id,
                        b: 0,
                        g: 0,
                    });
                }
                self.kills[usize::from(1 - team)] += 1;
                // XP: killer full, allies nearby 60%
                let xp = 150.0 + 20.0 * f32::from(vl);
                if let Some(ks) = killer_slot
                    && let Some(ki) = self.champ_by_slot(ks)
                {
                    self.gain_xp(ki, xp);
                }
                let ids: Vec<usize> = self
                    .units
                    .iter()
                    .enumerate()
                    .filter(|(_, o)| {
                        o.kind == Kind::Champ
                            && o.team != team
                            && !o.dead
                            && dist(o.x, o.z, x, z) <= 14.0
                            && !assists.contains(&o.slot)
                            && Some(o.slot) != killer_slot
                    })
                    .map(|(i, _)| i)
                    .collect();
                for i in ids {
                    self.gain_xp(i, xp * 0.6);
                }
                for a in &assists {
                    if let Some(ai) = self.champ_by_slot(*a)
                        && !self.units[ai].dead
                    {
                        self.gain_xp(ai, xp * 0.6);
                    }
                }
                let victim_id = self.units[ti].id;
                self.log.push(LogEv {
                    t: 0,
                    a: src,
                    b: victim_id,
                    g: bounty,
                });
            }
            Kind::Melee | Kind::Caster => {
                self.units[ti].dead = true;
                self.fx.push(Fx {
                    source: 0,
                    champ: crate::proto::UNKNOWN_PRESENTATION,
                    ability: crate::proto::UNKNOWN_PRESENTATION,
                    k: 5,
                    x,
                    z,
                    x2: x,
                    z2: z,
                    v: 0.0,
                });
                let last = if self.units[ti].kind == Kind::Caster {
                    30
                } else {
                    25
                };
                if let Some(ks) = killer_slot {
                    self.pay_gold_to_slot(ks, last);
                    if let Some(ki) = self.champ_by_slot(ks) {
                        self.gain_xp(
                            ki,
                            if self.units[ti].kind == Kind::Caster {
                                26.0
                            } else {
                                20.0
                            },
                        );
                    }
                }
                let near: Vec<usize> = self
                    .units
                    .iter()
                    .enumerate()
                    .filter(|(_, o)| {
                        o.kind == Kind::Champ
                            && o.team != team
                            && !o.dead
                            && Some(o.slot) != killer_slot
                            && dist(o.x, o.z, x, z) <= 12.0
                    })
                    .map(|(i, _)| i)
                    .collect();
                let xp = if self.units[ti].kind == Kind::Caster {
                    26.0
                } else {
                    20.0
                };
                for i in near {
                    self.gain_xp(i, xp * 0.6);
                }
            }
            Kind::Clone => {
                self.units[ti].dead = true;
            }
            Kind::CourtN | Kind::CourtS => {
                let c = usize::from(kind != Kind::CourtN);
                self.units[ti].dead = true;
                self.units[ti].hp = 0.0;
                self.court_respawn[c] = data::COURT_RESPAWN;
                // the court belongs to nobody (its team is 2); the team
                // that took it down wears the boon
                let winners = killer_slot
                    .and_then(|s| self.champ_by_slot(s))
                    .map_or(0, |i| self.units[i].team);
                self.boon[usize::from(winners)] = if kind == Kind::CourtN { 1 } else { 2 };
                self.boon_left[usize::from(winners)] = data::BOON_SECS;
                self.fx.push(Fx {
                    source: 0,
                    champ: crate::proto::UNKNOWN_PRESENTATION,
                    ability: crate::proto::UNKNOWN_PRESENTATION,
                    k: 2,
                    x,
                    z,
                    x2: x,
                    z2: z,
                    v: 3.0,
                });
                self.log.push(LogEv {
                    t: 2,
                    a: u32::from(winners),
                    b: u32::from(kind.code()),
                    g: 0,
                });
                let allies: Vec<usize> = self
                    .units
                    .iter()
                    .enumerate()
                    .filter(|(_, o)| o.kind == Kind::Champ && o.team == winners && !o.dead)
                    .map(|(i, _)| i)
                    .collect();
                for i in allies {
                    self.pay_gold(i, 150);
                    self.gain_xp(i, 220.0);
                }
            }
            Kind::CoreBlue | Kind::CoreRed => {
                self.units[ti].dead = true;
                self.units[ti].hp = 0.0;
                self.fx.push(Fx {
                    source: 0,
                    champ: crate::proto::UNKNOWN_PRESENTATION,
                    ability: crate::proto::UNKNOWN_PRESENTATION,
                    k: 2,
                    x,
                    z,
                    x2: x,
                    z2: z,
                    v: 8.0,
                });
                let allies: Vec<usize> = self
                    .units
                    .iter()
                    .enumerate()
                    .filter(|(_, o)| o.kind == Kind::Champ && o.team == 1 - team && !o.dead)
                    .map(|(i, _)| i)
                    .collect();
                for i in allies {
                    self.pay_gold(i, 250);
                }
            }
        }
    }

    fn pay_gold_to_slot(&mut self, slot: u8, amount: u32) {
        if let Some(i) = self.champ_by_slot(slot) {
            self.pay_gold(i, amount);
        }
    }

    fn pay_gold(&mut self, ui: usize, amount: u32) {
        let mult = 1.0 + (self.champ_stats_of(&self.units[ui]).gold + self.boon_gold(ui)) / 100.0;
        let g = ((amount as f32) * mult) as u32;
        let team = usize::from(self.units[ui].team);
        self.units[ui].gold += g;
        self.earned[team] += g;
        let (x, z) = (self.units[ui].x, self.units[ui].z);
        self.fx.push(Fx {
            source: 0,
            champ: crate::proto::UNKNOWN_PRESENTATION,
            ability: crate::proto::UNKNOWN_PRESENTATION,
            k: 7,
            x,
            z,
            x2: x,
            z2: z,
            v: 0.0,
        });
    }

    fn gain_xp(&mut self, ui: usize, amount: f32) {
        {
            let u = &self.units[ui];
            if u.kind != Kind::Champ || u.dead || u.level >= data::MAX_LEVEL {
                return;
            }
        }
        let (mh0, mm0) = self.champ_maxes(&self.units[ui]);
        let mut leveled = false;
        {
            let u = &mut self.units[ui];
            u.xp += amount;
            while u.level < data::MAX_LEVEL && u.xp >= data::xp_needed(u.level) {
                u.xp -= data::xp_needed(u.level);
                u.level += 1;
                u.points += 1;
                leveled = true;
            }
            if u.level >= data::MAX_LEVEL {
                u.xp = 0.0;
            }
        }
        if leveled {
            // the level itself heals the new maximum, and says so
            let (mh1, mm1) = self.champ_maxes(&self.units[ui]);
            let u = &mut self.units[ui];
            u.max_hp = mh1;
            u.hp = (u.hp + (mh1 - mh0)).min(mh1);
            u.max_mana = mm1;
            u.mana = (u.mana + (mm1 - mm0)).min(mm1);
            let (x, z) = (u.x, u.z);
            self.fx.push(Fx {
                source: 0,
                champ: crate::proto::UNKNOWN_PRESENTATION,
                ability: crate::proto::UNKNOWN_PRESENTATION,
                k: 6,
                x,
                z,
                x2: x,
                z2: z,
                v: f32::from(u.level),
            });
        }
    }

    fn core_regen(&mut self) {
        for i in 0..self.units.len() {
            if !matches!(self.units[i].kind, Kind::CoreBlue | Kind::CoreRed) || self.units[i].dead {
                continue;
            }
            let team = self.units[i].team;
            let x = self.units[i].x;
            let z = self.units[i].z;
            let threatened = self.units.iter().any(|o| {
                !o.dead
                    && o.team != team
                    && !matches!(o.kind, Kind::Clone)
                    && dist(x, z, o.x, o.z) <= 18.0
            });
            if !threatened {
                let mx = self.units[i].max_hp;
                let u = &mut self.units[i];
                u.hp = (u.hp + 0.4 * crate::DT).min(mx);
            }
        }
    }

    fn respawn_court(&mut self, c: usize) {
        let want = if c == 0 { Kind::CourtN } else { Kind::CourtS };
        if let Some(ci) = self.units.iter().position(|u| u.kind == want) {
            let u = &mut self.units[ci];
            u.dead = false;
            u.hp = u.max_hp;
        }
    }

    fn spawn_wave(&mut self) {
        for team in 0..2 {
            let alive = self
                .units
                .iter()
                .filter(|u| matches!(u.kind, Kind::Melee | Kind::Caster) && u.team == team)
                .count();
            if alive >= data::MINION_CAP {
                continue;
            }
            let waves = (self.tick / (60 * 120)) as f32;
            let scale = 1.0 + 0.15 * waves;
            let core_x = if team == 0 {
                -data::CORE_X + 4.0
            } else {
                data::CORE_X - 4.0
            };
            for (i, caster) in [(0u32, false), (1, false), (2, false), (3, true)] {
                if alive + i as usize >= data::MINION_CAP {
                    break;
                }
                let kind = if caster { Kind::Caster } else { Kind::Melee };
                let mut m = Unit::blank(self.next_id, kind, team);
                self.next_id += 1;
                m.x = core_x;
                m.z = ((i as f32) - 1.5) * 1.2;
                m.max_hp = if caster { 80.0 } else { 130.0 } * scale;
                m.hp = m.max_hp;
                m.facing = if team == 0 { 0.0 } else { std::f32::consts::PI };
                self.units.push(m);
            }
        }
    }

    // ------------------------------------------------------------------
    // stats
    // ------------------------------------------------------------------

    #[must_use]
    pub fn champ_stats_of(&self, u: &Unit) -> data::Stats {
        let mut s = data::champ_stats(u.def, u.level);
        s.add(&data::rune_stats(&u.runes));
        for &it in &u.items {
            if let Some(def) = data::item(it) {
                s.add(&def.flat);
            }
        }
        s.clamp_odds();
        s
    }

    #[must_use]
    pub fn champ_maxes(&self, u: &Unit) -> (f32, f32) {
        let s = self.champ_stats_of(u);
        (s.hp.max(50.0), s.mana.max(50.0))
    }
}

/// Sum one stat across the six item slots.
fn item_sum(items: &[u16; 6], f: impl Fn(&data::Stats) -> f32) -> f32 {
    let mut total = 0.0;
    for &it in items {
        if let Some(def) = data::item(it) {
            total += f(&def.flat);
        }
    }
    total
}

/// A normalized direction from (x,z) toward (tx,tz); zero-length input
/// keeps facing the way it was (returned as (0,1), which nobody aims at).
#[must_use]
pub fn dir_to(x: f32, z: f32, tx: f32, tz: f32) -> (f32, f32) {
    let dx = tx - x;
    let dz = tz - z;
    let d = (dx * dx + dz * dz).sqrt();
    if d < 1e-6 {
        return (0.0, 1.0);
    }
    (dx / d, dz / d)
}

#[must_use]
pub fn dist(x: f32, z: f32, tx: f32, tz: f32) -> f32 {
    ((tx - x) * (tx - x) + (tz - z) * (tz - z)).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn duel() -> Match {
        let mut m = Match::new(1, 777);
        m.roster[0].handle = "a".into();
        m.roster[1].handle = "b".into();
        m.set_pick(0, data::SWARM, 0, 1, [5, 2, 0]);
        m.set_pick(1, data::KNIGHT, 2, 3, [1, 3, 4]);
        m.start();
        m
    }

    fn visual_duel(def: u8) -> Match {
        let mut m = duel();
        let caster = m.champ_by_slot(0).unwrap();
        let target = m.champ_by_slot(1).unwrap();
        let u = &mut m.units[caster];
        u.def = def;
        u.level = data::MAX_LEVEL;
        u.ranks = [3; 4];
        u.mana = 10_000.0;
        u.x = 0.0;
        u.z = 0.0;
        m.units[target].x = 4.0;
        m.units[target].z = 0.0;
        m.units[target].hp = 100_000.0;
        m.fx.clear();
        m
    }

    #[test]
    fn all_twenty_accepted_abilities_publish_their_cast_identity() {
        for champ in 0..5 {
            for ability in 0..4 {
                let mut m = visual_duel(champ);
                let caster = m.champ_by_slot(0).unwrap();
                let source = m.units[caster].id;
                m.cast(caster, ability, 4.0, 0.0);
                assert!(m.units[caster].cds[usize::from(ability)] > 0.0);
                let casts: Vec<_> = m.fx.iter().filter(|f| f.k == 13).collect();
                assert_eq!(casts.len(), 1, "champ {champ} ability {ability}");
                let event = casts[0];
                assert_eq!((event.champ, event.ability), (champ, ability));
                assert_eq!(event.source, source);
                assert!(event.x.abs() < f32::EPSILON && event.z.abs() < f32::EPSILON);
                assert!((event.x2 - 4.0).abs() < f32::EPSILON && event.z2.abs() < f32::EPSILON);
                for fx in m.fx.iter().filter(|f| !matches!(f.k, 5..=7)) {
                    assert_eq!((fx.champ, fx.ability), (champ, ability));
                }
                for p in &m.projs {
                    assert_eq!((p.champ, p.ability), (champ, ability));
                }
                for z in &m.zones {
                    assert_eq!((z.champ, z.ability), (champ, ability));
                }
                let proto::S2C::State { fx, projs, .. } = m.snapshot() else {
                    unreachable!()
                };
                assert!(fx.iter().any(|f| {
                    f.k == 13 && f.champ == champ && f.ability == ability && f.source == source
                }));
                assert!(projs.iter().all(|p| p.champ == champ));
            }
        }
    }

    #[test]
    fn all_five_auto_attacks_and_impacts_keep_the_shooters_identity() {
        for champ in 0..5 {
            let mut m = visual_duel(champ);
            let caster = m.champ_by_slot(0).unwrap();
            let target = m.champ_by_slot(1).unwrap();
            let source = m.units[caster].id;
            m.do_attack(caster, target);
            let start = m.fx.iter().find(|f| f.k == 0).expect("attack-start event");
            assert_eq!((start.champ, start.ability), (champ, 4));
            assert_eq!(start.source, source);
            assert_eq!(start.v as u8 & 4, 4);
            assert!(start.x.abs() < f32::EPSILON && (start.x2 - 4.0).abs() < f32::EPSILON);
            if !m.projs.is_empty() {
                assert_eq!((m.projs[0].champ, m.projs[0].ability), (champ, 4));
                m.fx.clear();
                for _ in 0..60 {
                    m.step_proj(0);
                }
                let impact = m.fx.iter().find(|f| f.k == 0).expect("projectile impact");
                assert_eq!((impact.champ, impact.ability), (champ, 4));
                assert_eq!(impact.source, source);
                assert_eq!(impact.v as u8 & 4, 0);
                assert!((impact.x - 4.0).abs() < f32::EPSILON);
            }
        }
    }

    #[test]
    fn an_auto_aimed_at_the_origin_is_distinct_from_a_point_impact() {
        let mut m = visual_duel(data::SWARM);
        let caster = m.champ_by_slot(0).unwrap();
        let target = m.champ_by_slot(1).unwrap();
        m.units[caster].x = -4.0;
        m.units[target].x = 0.0;
        m.do_attack(caster, target);
        let start = m.fx.iter().find(|f| f.k == 0).unwrap();
        assert!(start.x2.abs() < f32::EPSILON && start.z2.abs() < f32::EPSILON);
        assert_eq!(start.v as u8 & 4, 4);
        m.fx.clear();
        for _ in 0..60 {
            m.step_proj(0);
        }
        let impact = m.fx.iter().find(|f| f.k == 0).unwrap();
        assert!(impact.x.abs() < f32::EPSILON && impact.z.abs() < f32::EPSILON);
        assert_eq!(impact.v as u8 & 4, 0);
    }

    #[test]
    fn expired_clone_projectiles_keep_identity_in_snapshots_and_hits() {
        let mut m = visual_duel(data::SWARM);
        let caster = m.champ_by_slot(0).unwrap();
        m.cast(caster, 3, 4.0, 0.0);
        let clone = m.units.iter().position(|u| u.kind == Kind::Clone).unwrap();
        let clone_id = m.units[clone].id;
        let target = m.champ_by_slot(1).unwrap();
        m.do_attack(clone, target);
        let start = m.fx.iter().find(|f| f.k == 0 && f.ability == 4).unwrap();
        assert_eq!(start.source, clone_id);
        assert_ne!(start.source, m.units[caster].id);
        m.projs.retain(|p| p.owner == clone_id);
        assert!(m.projs.iter().any(|p| p.ability == 3));
        assert!(m.projs.iter().any(|p| p.ability == 4));
        m.units.retain(|u| u.kind != Kind::Clone);
        m.fx.clear();
        let proto::S2C::State { projs, .. } = m.snapshot() else {
            unreachable!()
        };
        assert!(projs.iter().all(|p| p.champ == data::SWARM));
        for _ in 0..120 {
            for pi in 0..m.projs.len() {
                m.step_proj(pi);
            }
        }
        assert!(m.fx.iter().any(|f| {
            f.k == 0 && f.champ == data::SWARM && f.ability == 3 && f.source == clone_id
        }));
        assert!(m.fx.iter().any(|f| {
            f.k == 0 && f.champ == data::SWARM && f.ability == 4 && f.source == clone_id
        }));
    }

    #[test]
    fn rejected_casts_are_silent_and_knight_blinks_keep_r_identity() {
        let mut m = visual_duel(data::SWARM);
        let caster = m.champ_by_slot(0).unwrap();
        let target = m.champ_by_slot(1).unwrap();
        m.units[target].x = 60.0;
        m.cast(caster, 0, 4.0, 0.0); // No drone target: refund.
        assert!(m.fx.is_empty());
        m.units[caster].mana = 0.0;
        m.cast(caster, 1, 4.0, 0.0);
        assert!(m.fx.is_empty());

        let mut m = visual_duel(data::KNIGHT);
        let caster = m.champ_by_slot(0).unwrap();
        m.cast(caster, 3, 4.0, 0.0);
        let mana = m.units[caster].mana;
        let cooldown = m.units[caster].cds[3];
        m.fx.clear();
        m.cast(caster, 3, 6.0, 1.0);
        assert_eq!(m.units[caster].tp, 2);
        assert!((m.units[caster].mana - mana).abs() < f32::EPSILON);
        assert!((m.units[caster].cds[3] - cooldown).abs() < f32::EPSILON);
        assert!(m.fx.iter().any(|f| {
            f.k == 13 && f.champ == data::KNIGHT && f.ability == 3 && f.source == m.units[caster].id
        }));
    }

    #[test]
    fn delayed_traps_and_revives_keep_the_original_ability_identity() {
        let mut m = visual_duel(data::TESSERA);
        let caster = m.champ_by_slot(0).unwrap();
        m.cast(caster, 1, 4.0, 0.0);
        m.fx.clear();
        m.step_zone(0);
        assert!(
            m.fx.iter()
                .any(|f| f.k == 2 && f.champ == data::TESSERA && f.ability == 1)
        );

        let mut m = visual_duel(data::HALLOW);
        let caster = m.champ_by_slot(0).unwrap();
        let attacker = m.champ_by_slot(1).unwrap();
        m.cast(caster, 3, 0.0, 0.0);
        m.fx.clear();
        m.units[caster].hp = 1.0;
        m.deal_damage(caster, 10.0, 0, m.units[attacker].id, false);
        assert!(
            m.fx.iter()
                .any(|f| f.k == 9 && f.champ == data::HALLOW && f.ability == 3)
        );
        assert!(!m.units[caster].dead);
    }

    #[test]
    fn a_started_duel_has_the_world() {
        let m = duel();
        assert_eq!(m.phase, Phase::Live);
        // two champs, two courts, two cores
        assert_eq!(m.units.iter().filter(|u| u.kind == Kind::Champ).count(), 2);
        assert!(
            m.units
                .iter()
                .any(|u| u.kind == Kind::CoreBlue && u.hp == data::CORE_HP)
        );
        assert!(m.units.iter().any(|u| u.kind == Kind::CourtN));
    }

    #[test]
    fn waves_appear_and_march() {
        let mut m = duel();
        for _ in 0..(60 * 12) {
            m.step();
        }
        let blue: Vec<&Unit> = m
            .units
            .iter()
            .filter(|u| u.kind == Kind::Melee && u.team == 0)
            .collect();
        assert!(!blue.is_empty(), "a wave should have spawned by 12 s");
        assert!(blue[0].x > -data::CORE_X + 6.0, "the wave marched off base");
    }

    #[test]
    fn picks_are_unique_within_each_team() {
        let mut m = Match::new(3, 5);
        m.set_pick(0, data::SWARM, 0, 1, [0, 1, 2]);
        m.set_pick(1, data::SWARM, 0, 1, [0, 1, 2]);
        assert!(!m.roster[1].picked, "the duplicate must be refused");
        m.set_pick(1, data::KNIGHT, 0, 1, [0, 1, 2]);
        assert!(m.roster[1].picked);
        m.set_pick(3, data::SWARM, 0, 1, [0, 1, 2]);
        assert!(m.roster[3].picked, "opposing teams may mirror a champion");
    }

    #[test]
    fn nobody_picking_still_starts_with_distinct_champs() {
        let mut m = Match::new(3, 4242);
        m.start();
        let a = m.roster[0].champ;
        let b = m.roster[1].champ;
        assert_ne!(a, b, "fallback picks must not collide");
        assert_eq!(m.units.iter().filter(|u| u.kind == Kind::Champ).count(), 6);
    }

    #[test]
    fn the_core_falls_the_match_over() {
        let mut m = duel();
        // A champion can land the finishing blow on a damaged core.
        m.wave_left = 1e9;
        m.units
            .iter_mut()
            .find(|unit| unit.kind == Kind::CoreRed)
            .unwrap()
            .hp = 50.0;
        let blue = m.champ_by_slot(0).unwrap();
        m.units[blue].x = data::CORE_X - 2.0;
        m.units[blue].z = 0.0;
        for _ in 0..60 * 60 {
            m.command(
                0,
                Cmd::Attack {
                    target: m.units.iter().find(|u| u.kind == Kind::CoreRed).unwrap().id,
                },
            );
            m.step();
            if m.phase == Phase::Over {
                break;
            }
        }
        assert_eq!(m.phase, Phase::Over);
        assert_eq!(m.winner, 0);
    }
}
