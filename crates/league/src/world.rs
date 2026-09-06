//! The client's view of the world and its handshake with the page.
//!
//! `World` is what both game modes converge on: online it is fed by
//! snapshots; local it is rebuilt from the authoritative sim every frame.
//! The renderer and the page's `state_json` read only this — nothing
//! downstream knows whether the match runs in-process or on a host across
//! a tunnel.

use league_core::data;
use league_core::proto::{self, BuffSnap, ChampView, Phase, ProjSnap, SlotInfo, UnitSnap, ZoneSnap};

/// A unit as the renderer and the page need it.
#[derive(Clone, Copy, Debug)]
pub struct UnitLite {
    pub id: u32,
    pub k: u8,
    pub t: u8,
    /// Roster seat for champions; `u8::MAX` for everything else.
    pub slot: u8,
    pub x: f32,
    pub z: f32,
    pub fa: f32,
    pub hp: f32,
    pub mh: f32,
    pub mn: f32,
    pub mm: f32,
    pub dead: bool,
    pub colour: [f32; 3],
}

/// A transient effect with its remaining life, in seconds.
#[derive(Clone, Copy, Debug)]
pub struct FxLite {
    pub k: u8,
    pub x: f32,
    pub z: f32,
    pub x2: f32,
    pub z2: f32,
    pub v: f32,
    pub life: f32,
    pub left: f32,
}

/// One line of the kill feed, already text — the world knows handles.
#[derive(Clone, Debug)]
pub struct FeedLine {
    pub text: String,
    pub left: f32,
}

/// The zones currently on the field, for the renderer: (kind, x, z, r,
/// spin). kind 0 tornado / 1 trap / 2 stasis / 3 shroud.
pub type ZoneLite = (u8, f32, f32, f32, f32);

#[derive(Default)]
pub struct World {
    pub tick: u64,
    pub secs: f32,
    pub units: Vec<UnitLite>,
    pub champs: Vec<ChampView>,
    pub buffs: Vec<BuffSnap>,
    pub zones: Vec<ZoneLite>,
    pub projs: Vec<ProjSnap>,
    pub fx: Vec<FxLite>,
    pub feed: Vec<FeedLine>,
    pub kills: [u16; 2],
    pub boon: [u8; 2],
    pub boon_left: [f32; 2],
    pub court_respawn: [f32; 2],
    pub roster: Vec<SlotInfo>,
    pub my_slot: u8,
    pub mode: u8,
    pub phase: Phase,
    pub left: f32,
    pub winner: u8,
    pub connected: bool,
    pub notice: Option<String>,
    /// Camera focus, smoothed toward your champion.
    pub cam: (f32, f32),
    /// The page asked for the shop panel (B); the page reads this and
    /// shows it; buying itself is a page command back through `cmd_json`.
    pub shop_open: bool,
}

impl World {
    #[must_use]
    pub fn new(mode: u8) -> Self {
        Self {
            mode,
            phase: Phase::Select,
            cam: (-40.0, 0.0),
            ..Self::default()
        }
    }

    /// Drain the transient lists' clocks.
    pub fn tick_clocks(&mut self, dt: f32) {
        for f in &mut self.fx {
            f.left -= dt;
        }
        self.fx.retain(|f| f.left > 0.0);
        for line in &mut self.feed {
            line.left -= dt;
        }
        self.feed.retain(|l| l.left > 0.0);
    }

    pub fn push_fx(&mut self, mut f: FxLite) {
        f.life = match f.k {
            1 | 8 | 10 => 0.35,
            2 | 3 | 5 => 0.55,
            _ => 0.3,
        };
        f.left = f.life;
        if self.fx.len() > 160 {
            self.fx.remove(0);
        }
        self.fx.push(f);
    }

    /// Replace the unit list from a snapshot's flat rows.
    pub fn set_units(&mut self, rows: &[UnitSnap]) {
        self.units.clear();
        for r in rows {
            let colour = if r.k == 0 || r.k == 3 {
                data::CHAMPS[usize::from(r.def.min(4))].colour
            } else {
                [0.0, 0.0, 0.0] // the renderer tints the rest by team
            };
            self.units.push(UnitLite {
                id: r.id,
                k: r.k,
                t: r.t,
                slot: r.slot,
                x: r.x,
                z: r.z,
                fa: r.fa,
                hp: r.hp,
                mh: f32::from(r.mh),
                mn: f32::from(r.mn),
                mm: f32::from(r.mm),
                dead: r.dead,
                colour,
            });
        }
    }

    /// Persistent effects arrive in snapshots, so late joiners see fields
    /// and projectiles already in flight as well as newly cast flashes.
    pub fn set_zones(&mut self, zones: &[ZoneSnap]) {
        let spin = (self.tick as f32 * 0.35) % std::f32::consts::TAU;
        self.zones = zones.iter().map(|z| (z.k, z.x, z.z, z.r, spin)).collect();
    }

    #[must_use]
    pub fn my_team(&self) -> u8 {
        self.roster
            .iter()
            .find(|r| r.slot == self.my_slot)
            .map_or(0, |r| r.team)
    }

    /// Your own champion's row, if it exists.
    #[must_use]
    pub fn my_unit(&self) -> Option<&UnitLite> {
        self.units
            .iter()
            .find(|u| u.k == 0 && u.slot == self.my_slot)
    }

    /// Move the camera toward your champion.
    pub fn follow(&mut self, dt: f32) {
        let target = self
            .my_unit()
            .map_or((self.cam.0, self.cam.1), |u| (u.x, u.z));
        let k = (dt * 6.0).min(1.0);
        self.cam.0 += (target.0 - self.cam.0) * k;
        self.cam.1 += (target.1 - self.cam.1) * k;
    }

    /// The JSON the page polls every frame: the HUD, the draft screen and
    /// the minimap all read the same truth from here.
    #[must_use]
    pub fn state_json(&self) -> String {
        use serde_json::json;
        let me = self.champs.iter().find(|c| c.slot == self.my_slot).map(|c| {
            let u = self.my_unit();
            let roster = self.roster.iter().find(|r| r.slot == self.my_slot);
            let def = roster.map_or(0, |r| r.champ.min(4));
            let mut stats = data::champ_stats(def, c.level);
            if let Some(r) = roster {
                stats.add(&data::rune_stats(&r.runes));
            }
            for item in c.items.iter().filter_map(|id| data::item(*id)) {
                stats.add(&item.flat);
            }
            stats.clamp_odds();
            json!({
                "slot": c.slot, "uid": u.map_or(0, |u| u.id), "champ": def, "team": c.team,
                "alive": c.alive, "resp": c.resp,
                "hp": u.map_or(0.0, |u| u.hp), "mh": u.map_or(0.0, |u| u.mh),
                "mn": u.map_or(0.0, |u| u.mn), "mm": u.map_or(0.0, |u| u.mm),
                "x": u.map_or(0.0, |u| u.x), "z": u.map_or(0.0, |u| u.z),
                "lv": c.level, "g": c.gold, "pt": c.points,
                "rk": c.ranks, "cd": c.cds, "scd": c.scds,
                "items": c.items, "charges": c.charges, "d": c.d, "f": c.f,
                "stats": {"ad": stats.ad, "ap": stats.ap, "haste": stats.haste,
                    "crit": stats.crit, "critd": stats.critd,
                    "attackSpeed": (1.0 + stats.aspd) / data::CHAMPS[usize::from(def)].atk_cd}
            })
        });
        let units: Vec<_> = self.units.iter().map(|u| {
            let fraction = if u.mh > 0.0 { (u.hp / u.mh).clamp(0.0, 1.0) * 100.0 } else { 0.0 };
            json!([u.k, u.t, u.x, u.z, fraction, u.slot])
        }).collect();
        let cores: Vec<_> = self.units.iter().filter(|u| u.k == 6 || u.k == 7)
            .map(|u| json!({"t": u.t, "hp": u.hp, "mh": u.mh})).collect();
        json!({
            "phase": self.phase, "left": self.left, "mode": self.mode,
            "slot": self.my_slot, "me": me, "secs": self.secs,
            "kills": self.kills, "boon": self.boon, "boonLeft": self.boon_left,
            "court": self.court_respawn, "winner": self.winner,
            "connected": self.connected, "notice": self.notice, "shop": self.shop_open,
            "roster": self.roster, "champs": self.champs, "units": units,
            "feed": self.feed.iter().map(|l| &l.text).collect::<Vec<_>>(),
            "buffs": self.buffs.iter().map(|b| json!([b.u, b.k, b.ttl])).collect::<Vec<_>>(),
            "cores": cores
        }).to_string()
    }
}

/// Minimal string escaping for the state JSON: handles go through it, so
/// a hostile name cannot break the page's parser.
#[must_use]
pub fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// The static tables, as JSON, so the draft screen and the shop render
/// from the same numbers the sim runs on.
#[must_use]
pub fn data_json() -> String {
    use serde_json::json;
    let ability = |a: &data::Ability| json!({"name": a.name, "desc": a.desc, "mana": a.mana, "cd": a.cd});
    json!({
        "champs": data::CHAMPS.iter().map(|c| json!({
            "key": c.key, "name": c.name, "title": c.title, "hp": c.hp0, "mana": c.mn0,
            "ms": c.ms, "ad": c.ad0, "ap": c.ap0, "range": c.range, "atkCd": c.atk_cd,
            "style": c.atk_style, "colour": c.colour,
            "q": ability(&c.q), "w": ability(&c.w), "e": ability(&c.e), "r": ability(&c.r)
        })).collect::<Vec<_>>(),
        "spells": data::SPELLS.iter().map(|s| json!({"key": s.key, "name": s.name, "desc": s.desc, "cd": s.cd})).collect::<Vec<_>>(),
        "runes": data::RUNES.iter().map(|r| json!({"key": r.key, "name": r.name, "desc": r.desc})).collect::<Vec<_>>(),
        "items": data::ITEMS.iter().map(|i| json!({"id": i.id, "key": i.key, "name": i.name,
            "cost": i.cost, "tier": i.tier, "desc": i.desc, "charges": i.charges})).collect::<Vec<_>>(),
        "ultimateLevels": data::R_LEVELS
    }).to_string()
}

/// Turn one log event into a feed line, resolving ids to names.
pub fn feed_line(w: &World, ev: &proto::LogEv) -> Option<String> {
    let name_of = |id: u32| -> String {
        w.units.iter().find(|u| u.id == id).map_or_else(
            || "nothing".to_string(),
            |u| match u.k {
                0 | 3 => w
                    .roster
                    .iter()
                    .find(|r| r.slot == u.slot)
                    .map_or_else(|| "a champion".to_string(), |r| r.handle.clone()),
                1 => "a minion".to_string(),
                2 => "a caster".to_string(),
                4 => "the North Court".to_string(),
                5 => "the South Court".to_string(),
                6 | 7 => "a core".to_string(),
                _ => "something".to_string(),
            },
        )
    };
    match ev.t {
        0 => Some(format!(
            "{} slew {} for {}",
            name_of(ev.a),
            name_of(ev.b),
            ev.g
        )),
        1 => Some("first blood".to_string()),
        2 => Some(format!(
            "{} took the {}",
            if ev.a == 0 { "blue" } else { "red" },
            if ev.b == 4 { "North Court" } else { "South Court" },
        )),
        3 => Some(format!(
            "{} lost their core — {} wins",
            if ev.a == 0 { "blue" } else { "red" },
            if ev.a == 0 { "red" } else { "blue" },
        )),
        5 => Some(format!("{} breathed twice", name_of(ev.a))),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_handles_the_nasty_names() {
        assert_eq!(json_escape("a\"b\\c"), "a\\\"b\\\\c");
        assert_eq!(json_escape("\u{1}"), "\\u0001");
    }

    #[test]
    fn data_json_is_parseable() {
        let j = data_json();
        let v: serde_json::Value = serde_json::from_str(&j).expect("data_json must be valid JSON");
        assert_eq!(v["champs"].as_array().unwrap().len(), 5);
        assert_eq!(v["items"].as_array().unwrap().len(), 18);
    }

    #[test]
    fn state_json_is_parseable_even_empty() {
        let w = World::new(1);
        let j = w.state_json();
        let v: serde_json::Value = serde_json::from_str(&j).expect("state_json must be valid JSON");
        assert_eq!(v["phase"].as_str(), Some("select"));
    }
}
