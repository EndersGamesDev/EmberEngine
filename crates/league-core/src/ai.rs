//! Bot players: the same `Cmd` stream a human produces, decided from the
//! snapshot of the world a tick before it is applied.
//!
//! Deterministic by construction — every choice reads `rng::hash(tick,
//! slot, salt)`, and the bot owns no state across ticks. That keeps a
//! three-second decision delay (bots act on a fresh roll every ~1.2 s, like
//! a slow human) and a re-run of a replay agreeing with itself.

use crate::data;
use crate::proto::Cmd;
use crate::rng;
use crate::sim::{self, Kind, Match};

/// One bot's turn of the mind, appended to the match's command queue.
#[allow(
    clippy::too_many_lines,
    reason = "The ordered decision ladder returns one command; keeping priorities together makes deterministic bot behavior auditable."
)]
pub fn think(m: &Match, slot: u8) -> Option<Cmd> {
    let ui = m.champ_by_slot(slot)?;
    let u = m.units.get(ui)?;
    if u.dead {
        return None;
    }
    let tick = m.tick;
    let who = u64::from(slot);

    // 1. skill points, spent with a fixed kit priority: R when it unlocks,
    //    else the lowest-ranked of the main three.
    if u.points > 0 {
        let want = if u.ranks[3] < 3 && u.level >= data::R_LEVELS[usize::from(u.ranks[3])] {
            3
        } else {
            [0usize, 1, 2]
                .into_iter()
                .filter(|&a| u.ranks[a] < 3)
                .min_by_key(|&a| u.ranks[a])
                .unwrap_or(0) as u8
        };
        return Some(Cmd::Rank { slot: want });
    }

    // 2. low health: run home; in the fountain, spend the purse.
    let home_x = if u.team == 0 {
        -data::CORE_X + 4.0
    } else {
        data::CORE_X - 4.0
    };
    let at_home = (u.x - home_x).abs() <= 3.0 && u.z.abs() <= 6.0;
    if at_home && let Some(item) = shop_pick(m, ui) {
        return Some(Cmd::Buy { item });
    }
    // Preserve a retreat until the fountain has repaired the champion.
    // Crossing the initial threshold through lane regeneration must not
    // immediately send a nearly dead bot back into core fire.
    let returning_home =
        u.order == sim::Order::Move && (u.ox - home_x).abs() < 0.01 && u.oz.abs() < 0.01;
    let recovering = (returning_home || at_home) && u.hp < u.max_hp * 0.75;
    if u.hp < u.max_hp * 0.28 || recovering {
        let enemies_nea = (0..m.units.len()).any(|i| {
            let o = &m.units[i];
            o.kind == Kind::Champ
                && o.team != u.team
                && !o.dead
                && sim::dist(u.x, u.z, o.x, o.z) < 14.0
        });
        if let Some(flash) = index_of_flash(u)
            .filter(|&slot| u.hp < u.max_hp * 0.15 && enemies_nea && u.scds[slot] <= 0.0)
        {
            let away = if u.team == 0 { u.x - 7.0 } else { u.x + 7.0 };
            return Some(Cmd::Spell {
                slot: flash as u8,
                x: away,
                z: u.z,
            });
        }
        let in_fountain = (u.x - home_x).abs() <= data::FOUNTAIN_R + 1.0 && u.z.abs() <= 6.0;
        if in_fountain && let Some(item) = shop_pick(m, ui) {
            return Some(Cmd::Buy { item });
        }
        return Some(Cmd::Move { x: home_x, z: 0.0 });
    }

    // 3. fight: the nearest enemy champion within a leashed window.
    let mut foe: Option<(usize, f32)> = None;
    for i in 0..m.units.len() {
        let o = &m.units[i];
        if o.kind != Kind::Champ || o.dead || o.team == u.team {
            continue;
        }
        let d = sim::dist(u.x, u.z, o.x, o.z);
        if d <= 10.0 + m.units[ui].kind.hit_r() && foe.is_none_or(|(_, bd)| d < bd) {
            foe = Some((i, d));
        }
    }
    if let Some((fi, _)) = foe {
        let (fx, fz) = (m.units[fi].x, m.units[fi].z);
        let fid = m.units[fi].id;
        // abilities, gated by a fresh roll about every half second
        for ab in 0..4u8 {
            let demon_blink = ab == 3 && u.def == data::KNIGHT && u.form > 0.0 && u.tp > 0;
            let ready = u.ranks[usize::from(ab)] > 0
                && (u.cds[usize::from(ab)] <= 0.0 || demon_blink)
                && !(ab == 3 && u.level < data::R_LEVELS[usize::from(u.ranks[3].max(1) - 1)])
                && (ab != 3 || u.form <= 0.0 || u.tp > 0);
            if ready && rng::unit(m.seed, tick / 30, who * 4 + u64::from(ab), 11) > 0.62 {
                return Some(Cmd::Cast {
                    slot: ab,
                    x: fx,
                    z: fz,
                });
            }
        }
        let reach = m.range_of(ui) + m.units[fi].kind.hit_r();
        if sim::dist(u.x, u.z, fx, fz) > reach {
            return Some(Cmd::Attack { target: fid });
        }
        return Some(Cmd::Attack { target: fid });
    }

    // Clear the wave so minions can escort the push and pay experience.
    if let Some(minion) = m
        .units
        .iter()
        .filter(|o| {
            matches!(o.kind, Kind::Melee | Kind::Caster)
                && !o.dead
                && o.team != u.team
                && sim::dist(u.x, u.z, o.x, o.z) <= 10.0
        })
        .min_by(|a, b| sim::dist(u.x, u.z, a.x, a.z).total_cmp(&sim::dist(u.x, u.z, b.x, b.z)))
    {
        return Some(Cmd::Attack { target: minion.id });
    }

    // 4. no champion around: take a court if the lane fight is near one,
    //    else hit the enemy core if we stand at it, else push.
    for c in 0..2 {
        if m.court_respawn[c] > 0.0 {
            continue;
        }
        let [cx, cz] = data::COURT_POS[c];
        if sim::dist(u.x, u.z, cx, cz) <= 12.0
            && let Some(ci) = m
                .units
                .iter()
                .position(|o| o.kind == if c == 0 { Kind::CourtN } else { Kind::CourtS } && !o.dead)
        {
            let cid = m.units[ci].id;
            return Some(Cmd::Attack { target: cid });
        }
    }
    let enemy_core_x = -home_x;
    if sim::dist(u.x, u.z, enemy_core_x, 0.0) <= 10.0
        && let Some(ci) = m.units.iter().position(|o| {
            o.kind
                == if u.team == 0 {
                    Kind::CoreRed
                } else {
                    Kind::CoreBlue
                }
                && !o.dead
        })
    {
        let cid = m.units[ci].id;
        return Some(Cmd::Attack { target: cid });
    }
    // push the lane on a slow, tick-indexed wander so five bots do not
    // walk in a single file
    let sway = (rng::unit(m.seed, tick / 90, who, 12) - 0.5) * 6.0;
    Some(Cmd::Move {
        x: (enemy_core_x - 6.0).clamp(-sim::FIELD_X, sim::FIELD_X),
        z: sway,
    })
}

fn index_of_flash(u: &sim::Unit) -> Option<usize> {
    [u.d, u.f]
        .iter()
        .position(|&spell| spell == data::SPELL_FLASH)
}

/// What a returning bot buys: potions when nearly out of consumables,
/// otherwise the next affordable spike, in a fixed priority per role.
fn shop_pick(m: &Match, ui: usize) -> Option<u16> {
    let u = &m.units[ui];
    let pot = data::item(17).unwrap();
    if u.items
        .iter()
        .zip(&u.charges)
        .any(|(it, c)| *it == pot.id && *c < pot.charges)
    {
        return if u.gold >= pot.cost {
            Some(pot.id)
        } else {
            None
        };
    }
    if !u.items.contains(&0) {
        return None;
    }
    let mage = matches!(u.def, data::SWARM | data::HALLOW | data::TESSERA);
    // a fixed ladder; the first affordable step is the pick
    let ladder: [u16; 9] = if mage {
        [2, 5, 8, 12, 16, 13, 10, 15, 14]
    } else {
        [1, 4, 7, 11, 14, 15, 12, 13, 10]
    };
    for &it in &ladder {
        if u.items.contains(&it) {
            continue;
        }
        let Some(def) = data::item(it) else { continue };
        if u.gold >= def.cost {
            return Some(it);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::Phase;

    #[test]
    fn bots_level_and_push() {
        let mut m = Match::new(1, 99);
        m.start();
        for _ in 0..60 * 90 {
            let mut acts = Vec::new();
            for slot in 0..m.roster.len() as u8 {
                if m.roster[usize::from(slot)].bot
                    && let Some(c) = think(&m, slot)
                {
                    acts.push((slot, c));
                }
            }
            for (slot, c) in acts {
                m.command(slot, c);
            }
            m.step();
            assert_eq!(m.phase, Phase::Live, "the duel must survive 90 s of bots");
        }
        let any_moved = m
            .units
            .iter()
            .any(|u| u.kind == Kind::Champ && u.x.abs() > data::CORE_X - 30.0);
        assert!(any_moved, "bots must push out of the fountain");
    }
}
