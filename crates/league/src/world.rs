//! The client's view of the world and its handshake with the page.
//!
//! `World` is what both game modes converge on: online it is fed by
//! snapshots; local it is rebuilt from the authoritative sim every frame.
//! The renderer and the page's `state_json` read only this — nothing
//! downstream knows whether the match runs in-process or on a host across
//! a tunnel.

use league_core::data;
use league_core::proto::{self, BuffSnap, ChampView, Phase, SlotInfo, UnitSnap};

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
        let me = match self.champs.iter().find(|c| c.slot == self.my_slot) {
            Some(c) => {
                let u = self.my_unit();
                let (hp, mh, mn, mm) = u.map_or((0.0, 0.0, 0.0, 0.0), |u| (u.hp, u.mh, u.mn, u.mm));
                let uid = u.map_or(0, |u| u.id);
                format!(
                    "{{\"slot\":{},\"uid\":{},\"alive\":{},\"resp\":{:.1},\"hp\":{hp:.1},\"mh\":{mh:.1},\
                     \"mn\":{mn:.1},\"mm\":{mm:.1},\"lv\":{},\"g\":{},\"pt\":{},\
                     \"rk\":[{},{},{},{}],\"cd\":[{:.1},{:.1},{:.1},{:.1}],\"scd\":[{:.0},{:.0}],\
                     \"items\":[{},{},{},{},{},{}],\"charges\":[{},{},{},{},{},{}],\
                     \"d\":{},\"f\":{}}}",
                    c.slot,
                    uid,
                    c.alive,
                    c.resp,
                    c.level,
                    c.gold,
                    c.points,
                    c.ranks[0],
                    c.ranks[1],
                    c.ranks[2],
                    c.ranks[3],
                    c.cds[0],
                    c.cds[1],
                    c.cds[2],
                    c.cds[3],
                    c.scds[0],
                    c.scds[1],
                    c.items[0],
                    c.items[1],
                    c.items[2],
                    c.items[3],
                    c.items[4],
                    c.items[5],
                    c.charges[0],
                    c.charges[1],
                    c.charges[2],
                    c.charges[3],
                    c.charges[4],
                    c.charges[5],
                    c.d,
                    c.f,
                )
            }
            None => "null".to_string(),
        };
        let roster: Vec<String> = self
            .roster
            .iter()
            .map(|r| {
                format!(
                    "{{\"slot\":{},\"team\":{},\"handle\":\"{}\",\"bot\":{},\"champ\":{},\"picked\":{},\"d\":{},\"f\":{},\"runes\":[{},{},{}],\"connected\":{}}}",
                    r.slot,
                    r.team,
                    json_escape(&r.handle),
                    r.bot,
                    if r.champ == u8::MAX {
                        -1
                    } else {
                        i32::from(r.champ)
                    },
                    r.picked,
                    r.d,
                    r.f,
                    rune_or(&r.runes, 0),
                    rune_or(&r.runes, 1),
                    rune_or(&r.runes, 2),
                    r.connected,
                )
            })
            .collect();
        // the minimap needs the field too, kept tiny: kind, team, x, z,
        // health fraction
        let units: Vec<String> = self
            .units
            .iter()
            .map(|u| {
                let hf = if u.mh > 0.0 {
                    (u.hp / u.mh).clamp(0.0, 1.0) * 100.0
                } else {
                    0.0
                };
                format!(
                    "[{},{},{:.1},{:.1},{:.0}]",
                    u.k,
                    u.t,
                    u.x,
                    u.z,
                    hf
                )
            })
            .collect();
        let feed: Vec<String> = self
            .feed
            .iter()
            .map(|l| format!("\"{}\"", json_escape(&l.text)))
            .collect();
        let buffs: Vec<String> = self
            .buffs
            .iter()
            .map(|b| format!("[{},{},{:.1}]", b.u, b.k, b.ttl))
            .collect();
        let core_hp: Vec<String> = self
            .units
            .iter()
            .filter(|u| u.k == 6 || u.k == 7)
            .map(|u| format!("[{:.0},{:.0}]", u.hp, u.mh))
            .collect();
        format!(
            "{{\"phase\":\"{}\",\"left\":{:.1},\"mode\":{},\"me\":{},\"secs\":{:.0},\
             \"kills\":[{},{}],\"boon\":[{},{}],\"boonLeft\":[{:.1},{:.1}],\
             \"court\":[{:.1},{:.1}],\
             \"winner\":{},\"connected\":{},\"notice\":{},\"shop\":{},\
             \"me\":{me},\"roster\":[{}],\"units\":[{}],\"feed\":[{}],\"buffs\":[{}],\"cores\":[{}]}}",
            match self.phase {
                Phase::Select => "select",
                Phase::Live => "live",
                Phase::Over => "over",
            },
            self.left,
            self.mode,
            self.my_slot,
            self.secs,
            self.kills[0],
            self.kills[1],
            self.boon[0],
            self.boon[1],
            self.boon_left[0],
            self.boon_left[1],
            self.court_respawn[0],
            self.court_respawn[1],
            self.winner,
            self.connected,
            match &self.notice {
                Some(n) => format!("\"{}\"", json_escape(n)),
                None => "null".to_string(),
            },
            self.shop_open,
            roster.join(","),
            units.join(","),
            feed.join(","),
            buffs.join(","),
            core_hp.join(","),
        )
    }
}

fn rune_or(runes: &[u8; 3], i: usize) -> i32 {
    if runes[i] == u8::MAX {
        -1
    } else {
        i32::from(runes[i])
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
    let mut s = String::from("{\"champs\":[");
    let champs: Vec<String> = data::CHAMPS
        .iter()
        .map(|c| {
            format!(
                "{{\"key\":\"{}\",\"name\":\"{}\",\"title\":\"{}\",\"hp\":{},\"mana\":{},\"ms\":{},\
                 \"ad\":{},\"ap\":{},\"range\":{},\"atkCd\":{},\"style\":{},\
                 \"colour\":[{:.2},{:.2},{:.2}],\
                 \"q\":{{\"name\":\"{}\",\"desc\":\"{}\"}},\"w\":{{\"name\":\"{}\",\"desc\":\"{}\"}},\
                 \"e\":{{\"name\":\"{}\",\"desc\":\"{}\"}},\"r\":{{\"name\":\"{}\",\"desc\":\"{}\"}}}}",
                c.key,
                json_escape(c.name),
                json_escape(c.title),
                c.hp0,
                c.mn0,
                c.ms,
                c.ad0,
                c.ap0,
                c.range,
                c.atk_cd,
                c.atk_style,
                c.colour[0],
                c.colour[1],
                c.colour[2],
                json_escape(c.q.name),
                json_escape(c.q.desc),
                json_escape(c.w.name),
                json_escape(c.w.desc),
                json_escape(c.e.name),
                json_escape(c.e.desc),
                json_escape(c.r.name),
                json_escape(c.r.desc),
            )
        })
        .collect();
    s.push_str(&champs.join(","));
    s.push_str("],\"spells\":[");
    let spells: Vec<String> = data::SPELLS
        .iter()
        .map(|sp| {
            format!(
                "{{\"key\":\"{}\",\"name\":\"{}\",\"desc\":\"{}\",\"cd\":{}}}",
                sp.key,
                json_escape(sp.name),
                json_escape(sp.desc),
                sp.cd
            )
        })
        .collect();
    s.push_str(&spells.join(","));
    s.push_str("],\"runes\":[");
    let runes: Vec<String> = data::RUNES
        .iter()
        .map(|r| {
            format!(
                "{{\"key\":\"{}\",\"name\":\"{}\",\"desc\":\"{}\"}}",
                r.key,
                json_escape(r.name),
                json_escape(r.desc)
            )
        })
        .collect();
    s.push_str(&runes.join(","));
    s.push_str("],\"items\":[");
    let items: Vec<String> = data::ITEMS
        .iter()
        .map(|i| {
            format!(
                "{{\"id\":{},\"key\":\"{}\",\"name\":\"{}\",\"cost\":{},\"tier\":{},\"desc\":\"{}\"}}",
                i.id,
                i.key,
                json_escape(i.name),
                i.cost,
                i.tier,
                json_escape(i.desc)
            )
        })
        .collect();
    s.push_str(&items.join(","));
    s.push_str("]}");
    s
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
#[test]
fn debug_col() {
    let j = crate::world::data_json();
    let chars: Vec<char> = j.chars().collect();
    let end = chars.len().min(6307);
    let start = 6100usize.min(chars.len());
    println!("LEN {} SEG [{}]", chars.len(), chars[start..end].iter().collect::<String>());
}
