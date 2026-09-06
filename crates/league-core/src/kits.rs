//! The five kits: every Q/W/E/R and the four summoner spells, as `impl
//! Match` methods on the machinery in `sim`. Split out because twenty
//! abilities and the whole 60 Hz world do not fit one screen.
//!
//! Numbers come from `data`; this file pays costs, sets cooldowns, and
//! pushes beams, drones, hooks, zones and buffs into the world. A cast is
//! all-or-nothing: `cast` refuses and refunds when the kit needs a target
//! that is not there.

use crate::data;
use crate::sim::{
    self, BuffKind, Kind, Match, Proj, ProjKind, Unit, Zone, ZoneKind, add_buff, dist,
};
use crate::proto::Fx;

/// What a hit is, for the multiplier pipeline in `Match::deal_damage`.
const SPELL: u8 = 1;

impl Match {
    fn ab_cost(def: &data::ChampDef, ab: u8, rank: usize) -> (f32, f32) {
        let r = rank.clamp(1, 3) - 1;
        match ab {
            0 => (def.q.mana[r], def.q.cd[r]),
            1 => (def.w.mana[r], def.w.cd[r]),
            2 => (def.e.mana[r], def.e.cd[r]),
            _ => (def.r.mana[r], def.r.cd[r]),
        }
    }

    /// Cast one ability at the world point under the cursor. `ui` is the
    /// unit index into `self.units`.
    pub(crate) fn cast(&mut self, ui: usize, ab: u8, ax: f32, az: f32) {
        let u = &self.units[ui];
        debug_assert!(matches!(u.kind, Kind::Champ));
        let a = usize::from(ab);
        // Demon-form blinks are a free re-cast of R while the form holds.
        if ab == 3 && u.def == data::KNIGHT && u.form > 0.0 && u.tp > 0 {
            let (x0, z0) = (u.x, u.z);
            let (dx, dz) = sim::dir_to(x0, z0, ax, az);
            let d = dist(x0, z0, ax, az).min(12.0);
            let u = &mut self.units[ui];
            u.x = (x0 + dx * d).clamp(-sim::FIELD_X, sim::FIELD_X);
            u.z = (z0 + dz * d).clamp(-data::FIELD_Z, data::FIELD_Z);
            u.tp -= 1;
            let (x, z) = (u.x, u.z);
            self.fx.push(Fx {
                k: 8,
                x,
                z,
                x2: x,
                z2: z,
                v: 0.0,
            });
            return;
        }
        let rank = usize::from(u.ranks[a]);
        if rank == 0
            || u.dead
            || u.cds[a] > 0.0
            || (ab == 3 && u.level < data::R_LEVELS[(rank - 1).min(2)])
        {
            return;
        }
        let (mana_cost, cd_base) = Self::ab_cost(&data::CHAMPS[usize::from(u.def)], ab, rank);
        if u.mana < mana_cost {
            return;
        }
        let def = u.def;
        let stats = self.champ_stats_of(&self.units[ui]);
        let haste_scale = 100.0 / (100.0 + stats.haste + self.boon_haste(ui));
        {
            let u = &mut self.units[ui];
            u.mana -= mana_cost;
            u.cds[a] = cd_base * haste_scale;
        }
        match (def, ab) {
            (data::SWARM, 0) => self.swarm_q(ui, rank, &stats),
            (data::SWARM, 1) => self.swarm_w(ui, rank, &stats, ax, az),
            (data::SWARM, 2) => self.swarm_e(ui, rank),
            (data::SWARM, 3) => self.swarm_r(ui, rank, &stats, ax, az),
            (data::KNIGHT, 0) => self.knight_q(ui, rank, &stats, ax, az),
            (data::KNIGHT, 1) => self.knight_w(ui, rank, &stats),
            (data::KNIGHT, 2) => self.knight_e(ui, rank),
            (data::KNIGHT, 3) => self.knight_r(ui, rank),
            (data::HALLOW, 0) => self.hallow_q(ui, rank, &stats, ax, az),
            (data::HALLOW, 1) => self.hallow_w(ui, ax, az),
            (data::HALLOW, 2) => self.hallow_e(ui, rank, &stats, ax, az),
            (data::HALLOW, 3) => self.hallow_r(ui, rank, ax, az),
            (data::MAW, 0) => self.maw_q(ui, rank, &stats, ax, az),
            (data::MAW, 1) => self.maw_w(ui, rank, &stats),
            (data::MAW, 2) => self.maw_e(ui, rank, &stats, ax, az),
            (data::MAW, 3) => self.maw_r(ui, rank, &stats),
            (data::TESSERA, 0) => self.tessera_q(ui, rank, &stats, ax, az),
            (data::TESSERA, 1) => self.tessera_w(ui, rank, &stats, ax, az),
            (data::TESSERA, 2) => self.tessera_e(ui, rank, ax, az),
            (data::TESSERA, 3) => self.tessera_r(ui, rank, &stats, ax, az),
            _ => {}
        }
    }

    // --- SW4RM -----------------------------------------------------------

    fn swarm_q(&mut self, ui: usize, rank: usize, stats: &data::Stats) {
        let Some(t) = self.current_target_or_enemy_near(ui) else {
            self.refund(ui, 0);
            return;
        };
        let dmg = 15.0 + 8.0 * ((rank as f32) - 1.0) + 0.22 * stats.ap;
        let (sx, sz, team, owner) = {
            let u = &self.units[ui];
            (u.x, u.z, u.team, u.id)
        };
        for i in 0..3u32 {
            self.push_proj(
                owner,
                team,
                ProjKind::Drone,
                sx,
                sz,
                0.0,
                1.0,
                12.0 + 3.0 * (i as f32),
                30.0,
                dmg,
                false,
                0.0,
                t,
            );
        }
    }

    /// The attack-order target if it still lives, else the enemy champion
    /// nearest the last aim, else nothing.
    fn current_target_or_enemy_near(&self, ui: usize) -> Option<u32> {
        let t = self.units[ui].target;
        if t != 0 {
            if let Some(ti) = self.index_of(t) {
                if !self.units[ti].dead {
                    return Some(t);
                }
            }
        }
        let (x, z) = (self.units[ui].x, self.units[ui].z);
        self.enemy_champ_near(ui, x, z, 14.0)
    }

    fn swarm_w(&mut self, ui: usize, rank: usize, stats: &data::Stats, ax: f32, az: f32) {
        let (sx, sz) = (self.units[ui].x, self.units[ui].z);
        let (dx, dz) = sim::dir_to(sx, sz, ax, az);
        let dmg = 50.0 + 15.0 * ((rank as f32) - 1.0) + 0.65 * stats.ap;
        let (owner, team) = (self.units[ui].id, self.units[ui].team);
        self.beam_by(owner, team, sx, sz, dx, dz, 24.0, 1.4, dmg, 30.0, 1.5);
    }

    fn swarm_e(&mut self, ui: usize, rank: usize) {
        let (px, pz, pteam, pslot, facing) = {
            let u = &self.units[ui];
            (u.x, u.z, u.team, u.slot, u.facing)
        };
        // one hologram at a time: recasting replaces the old one
        self.units
            .retain(|o| !(o.kind == Kind::Clone && o.parent == pslot));
        let mut c = self.spawn_clone(pslot, pteam, 5.0, 0.4 + 0.1 * (rank as f32));
        c.x = px - facing.cos() * 2.5;
        c.z = pz - facing.sin() * 2.5;
        self.units.push(c);
        let id = self.units[ui].id;
        add_buff(&mut self.units[ui], BuffKind::Ms, 1.0, 20.0, id);
        self.fx.push(Fx {
            k: 8,
            x: px,
            z: pz,
            x2: px,
            z2: pz,
            v: 1.0,
        });
    }

    fn swarm_r(&mut self, ui: usize, rank: usize, stats: &data::Stats, ax: f32, az: f32) {
        let Some(t) = self.enemy_champ_near(ui, ax, az, 8.0) else {
            self.refund(ui, 3);
            return;
        };
        let (px, pz, pteam, pslot, facing) = {
            let u = &self.units[ui];
            (u.x, u.z, u.team, u.slot, u.facing)
        };
        self.units
            .retain(|o| !(o.kind == Kind::Clone && o.parent == pslot));
        let pct = 0.5 + 0.1 * (rank as f32);
        for i in 0..3 {
            let a = std::f32::consts::TAU * (i as f32) / 3.0;
            let mut c = self.spawn_clone(pslot, pteam, 8.0, pct);
            c.x = px + a.cos() * 2.2;
            c.z = pz + a.sin() * 2.2;
            c.facing = facing;
            c.order = self.units[ui].order;
            c.target = t;
            self.units.push(c);
        }
        let beam_dmg = 50.0 + 15.0 * 2.0 + 0.65 * stats.ap;
        let drone_dmg = 15.0 + 8.0 * 2.0 + 0.22 * stats.ap;
        // you fire first, the copies for their share right behind
        let (tx, tz) = self
            .index_of(t)
            .map_or((ax, az), |i| (self.units[i].x, self.units[i].z));
        let (sx, sz) = (self.units[ui].x, self.units[ui].z);
        let (dx, dz) = sim::dir_to(sx, sz, tx, tz);
        let (owner, team) = (self.units[ui].id, self.units[ui].team);
        self.beam_by(owner, team, sx, sz, dx, dz, 24.0, 1.4, beam_dmg, 30.0, 1.5);
        self.swarm_drones_at(ui, t, drone_dmg);
        let clone_ids: Vec<u32> = self
            .units
            .iter()
            .filter(|o| o.kind == Kind::Clone && o.parent == pslot)
            .map(|o| o.id)
            .collect();
        let mut clone_beams = Vec::new();
        let mut clone_drones = Vec::new();
        for cid in clone_ids {
            let Some(ci) = self.index_of(cid) else { continue };
            let (cx, cz) = (self.units[ci].x, self.units[ci].z);
            let (dx, dz) = sim::dir_to(cx, cz, tx, tz);
            clone_beams.push((cid, pteam, cx, cz, dx, dz));
            clone_drones.push((cid, pteam, cx, cz));
        }
        for (cid, team, cx, cz, dx, dz) in clone_beams {
            self.beam_by(cid, team, cx, cz, dx, dz, 24.0, 1.4, beam_dmg * pct, 30.0, 1.5);
        }
        for (cid, team, cx, cz) in clone_drones {
            for i in 0..3u32 {
                self.push_proj(
                    cid,
                    team,
                    ProjKind::Drone,
                    cx,
                    cz,
                    0.0,
                    1.0,
                    12.0 + 3.0 * (i as f32),
                    30.0,
                    drone_dmg * pct,
                    false,
                    0.0,
                    t,
                );
            }
        }
    }

    fn swarm_drones_at(&mut self, ui: usize, target: u32, dmg: f32) {
        let (sx, sz, team, owner) = {
            let u = &self.units[ui];
            (u.x, u.z, u.team, u.id)
        };
        for i in 0..3u32 {
            self.push_proj(
                owner,
                team,
                ProjKind::Drone,
                sx,
                sz,
                0.0,
                1.0,
                12.0 + 3.0 * (i as f32),
                30.0,
                dmg,
                false,
                0.0,
                target,
            );
        }
    }

    // --- EmberKnight -----------------------------------------------------

    fn knight_q(&mut self, ui: usize, rank: usize, stats: &data::Stats, ax: f32, az: f32) {
        let (sx, sz, team, id) = {
            let u = &self.units[ui];
            (u.x, u.z, u.team, u.id)
        };
        let (dx, dz) = sim::dir_to(sx, sz, ax, az);
        let d = dist(sx, sz, ax, az).min(11.0);
        let (zx, zz) = (sx + dx * d, sz + dz * d);
        let z = Zone {
            id: self.next_id,
            owner: id,
            team,
            zk: ZoneKind::Tornado,
            x: zx,
            z: zz,
            r: 3.0,
            ttl: 3.0 + 0.2 * (rank as f32),
            tick_t: 0.0,
            dmg: 16.0 + 6.0 * ((rank as f32) - 1.0) + 0.30 * stats.ap,
            detonate: 0.0,
            root: 0.0,
            slow: 25.0,
            attach: 0,
        };
        self.next_id += 1;
        self.zones.push(z);
        self.fx.push(Fx {
            k: 3,
            x: zx,
            z: zz,
            x2: zx,
            z2: zz,
            v: 3.0,
        });
    }

    fn knight_w(&mut self, ui: usize, rank: usize, stats: &data::Stats) {
        let id = self.units[ui].id;
        add_buff(&mut self.units[ui], BuffKind::Immune, 2.0, 0.0, id);
        add_buff(
            &mut self.units[ui],
            BuffKind::Shield,
            4.0,
            60.0 + 20.0 * ((rank as f32) - 1.0) + 0.4 * stats.ap,
            id,
        );
        let (x, z) = (self.units[ui].x, self.units[ui].z);
        self.fx.push(Fx {
            k: 12,
            x,
            z,
            x2: x,
            z2: z,
            v: 0.0,
        });
    }

    fn knight_e(&mut self, ui: usize, rank: usize) {
        let id = self.units[ui].id;
        let ap = self.champ_stats_of(&self.units[ui]).ap;
        let (val, aux) = (25.0 + 5.0 * (rank as f32), 12.0 + 6.0 * (rank as f32) + 0.2 * ap);
        let u = &mut self.units[ui];
        if let Some(b) = u.buffs.iter_mut().find(|b| b.k == BuffKind::Fire) {
            b.ttl = 4.0;
            b.val = val;
            b.aux = aux;
        } else {
            u.buffs.push(sim::Buff {
                k: BuffKind::Fire,
                ttl: 4.0,
                val,
                aux,
                src: id,
            });
        }
    }

    fn knight_r(&mut self, ui: usize, rank: usize) {
        let dur = 10.0 + 2.0 * (rank as f32);
        let id = self.units[ui].id;
        let u = &mut self.units[ui];
        u.tp = 3;
        u.form = dur;
        u.buffs.retain(|b| b.k != BuffKind::DmgAmp);
        add_buff(u, BuffKind::DmgAmp, dur, 100.0, id);
        let (x, z) = (u.x, u.z);
        self.fx.push(Fx {
            k: 2,
            x,
            z,
            x2: x,
            z2: z,
            v: 3.5,
        });
    }

    // --- The Hallow One ----------------------------------------------------

    fn hallow_q(&mut self, ui: usize, rank: usize, stats: &data::Stats, ax: f32, az: f32) {
        let Some(ti) = self.ally_near(ui, ax, az) else {
            return;
        };
        let amount = 80.0 + 30.0 * ((rank as f32) - 1.0) + 0.7 * stats.ap;
        let mx = self.units[ti].max_hp;
        let u = &mut self.units[ti];
        u.hp = (u.hp + amount).min(mx);
        let (x, z) = (u.x, u.z);
        self.fx.push(Fx {
            k: 9,
            x,
            z,
            x2: x,
            z2: z,
            v: 0.0,
        });
    }

    fn hallow_w(&mut self, ui: usize, ax: f32, az: f32) {
        let Some(ti) = self.ally_near(ui, ax, az) else {
            return;
        };
        let id = self.units[ti].id;
        add_buff(&mut self.units[ti], BuffKind::Ms, 3.0, 35.0, id);
        let (x, z) = (self.units[ti].x, self.units[ti].z);
        self.fx.push(Fx {
            k: 9,
            x,
            z,
            x2: x,
            z2: z,
            v: 1.0,
        });
    }

    fn hallow_e(&mut self, ui: usize, rank: usize, stats: &data::Stats, ax: f32, az: f32) {
        let Some(ti) = self.ally_near(ui, ax, az) else {
            return;
        };
        let src = self.units[ui].id;
        add_buff(
            &mut self.units[ti],
            BuffKind::Shield,
            4.0,
            60.0 + 25.0 * ((rank as f32) - 1.0) + 0.55 * stats.ap,
            src,
        );
        let (x, z) = (self.units[ti].x, self.units[ti].z);
        self.fx.push(Fx {
            k: 12,
            x,
            z,
            x2: x,
            z2: z,
            v: 0.0,
        });
    }

    fn hallow_r(&mut self, ui: usize, rank: usize, ax: f32, az: f32) {
        let Some(ti) = self.ally_near(ui, ax, az) else {
            return;
        };
        let src = self.units[ui].id;
        add_buff(
            &mut self.units[ti],
            BuffKind::Revive,
            5.0 + (rank as f32),
            30.0 + 10.0 * ((rank as f32) - 1.0),
            src,
        );
        let (x, z) = (self.units[ti].x, self.units[ti].z);
        self.fx.push(Fx {
            k: 9,
            x,
            z,
            x2: x,
            z2: z,
            v: 2.0,
        });
    }

    // --- Bog Maw -----------------------------------------------------------

    fn maw_q(&mut self, ui: usize, rank: usize, stats: &data::Stats, ax: f32, az: f32) {
        let (sx, sz, team, id) = {
            let u = &self.units[ui];
            (u.x, u.z, u.team, u.id)
        };
        let (dx, dz) = sim::dir_to(sx, sz, ax, az);
        let dmg = 40.0 + 12.0 * ((rank as f32) - 1.0) + 0.5 * stats.ap + 0.35 * stats.ad;
        let mut p = self.blank_proj(id, team, ProjKind::Hook, sx, sz, dx, dz, 20.0, 11.0, dmg);
        p.hook_rank = rank as u8;
        self.projs.push(p);
    }

    fn maw_w(&mut self, ui: usize, rank: usize, stats: &data::Stats) {
        let (id, team, x, z) = {
            let u = &self.units[ui];
            (u.id, u.team, u.x, u.z)
        };
        let z = Zone {
            id: self.next_id,
            owner: id,
            team,
            zk: ZoneKind::Shroud,
            x,
            z,
            r: 3.5,
            ttl: 4.0 + 0.5 * (rank as f32),
            tick_t: 0.0,
            dmg: 10.0 + 4.0 * ((rank as f32) - 1.0) + 0.15 * stats.ap,
            detonate: 0.0,
            root: 0.0,
            slow: 0.0,
            attach: id,
        };
        self.next_id += 1;
        self.zones.push(z);
    }

    fn maw_e(&mut self, ui: usize, rank: usize, stats: &data::Stats, ax: f32, az: f32) {
        let (sx, sz) = (self.units[ui].x, self.units[ui].z);
        let (dx, dz) = sim::dir_to(sx, sz, ax, az);
        let d = dist(sx, sz, ax, az).min(5.5 + 0.5 * (rank as f32));
        let nx = (sx + dx * d).clamp(-sim::FIELD_X, sim::FIELD_X);
        let nz = (sz + dz * d).clamp(-data::FIELD_Z, data::FIELD_Z);
        {
            let u = &mut self.units[ui];
            u.x = nx;
            u.z = nz;
        }
        let (id, team) = (self.units[ui].id, self.units[ui].team);
        let dmg = 20.0 + 8.0 * ((rank as f32) - 1.0) + 0.30 * stats.ap;
        for i in 0..self.units.len() {
            if self.hitable_enemy(i, team, nx, nz, 2.5) {
                self.deal_damage(i, dmg, SPELL, id, true);
                add_buff(&mut self.units[i], BuffKind::Slow, 1.0, 20.0, id);
            }
        }
        self.fx.push(Fx {
            k: 8,
            x: sx,
            z: sz,
            x2: nx,
            z2: nz,
            v: 0.0,
        });
        self.fx.push(Fx {
            k: 2,
            x: nx,
            z: nz,
            x2: nx,
            z2: nz,
            v: 2.5,
        });
    }

    fn maw_r(&mut self, ui: usize, rank: usize, stats: &data::Stats) {
        let (id, x, z, team) = {
            let u = &self.units[ui];
            (u.id, u.x, u.z, u.team)
        };
        let dmg = 90.0 + 35.0 * ((rank as f32) - 1.0) + 0.8 * stats.ap;
        let root = 1.0 + 0.15 * (rank as f32);
        for i in 0..self.units.len() {
            if self.hitable_enemy(i, team, x, z, 6.5) {
                self.deal_damage(i, dmg, SPELL, id, true);
                add_buff(&mut self.units[i], BuffKind::Root, root, 0.0, id);
                add_buff(&mut self.units[i], BuffKind::Slow, 3.0, 30.0, id);
            }
        }
        self.fx.push(Fx {
            k: 2,
            x,
            z,
            x2: x,
            z2: z,
            v: 6.5,
        });
    }

    // --- Tessera -----------------------------------------------------------

    fn tessera_q(&mut self, ui: usize, rank: usize, stats: &data::Stats, ax: f32, az: f32) {
        let (sx, sz, team, id) = {
            let u = &self.units[ui];
            (u.x, u.z, u.team, u.id)
        };
        let (dx, dz) = sim::dir_to(sx, sz, ax, az);
        let mut p = self.blank_proj(
            id,
            team,
            ProjKind::Bolt,
            sx,
            sz,
            dx,
            dz,
            26.0,
            16.0,
            40.0 + 14.0 * ((rank as f32) - 1.0) + 0.55 * stats.ap,
        );
        p.pierce = 1;
        self.projs.push(p);
    }

    fn tessera_w(&mut self, ui: usize, rank: usize, stats: &data::Stats, ax: f32, az: f32) {
        let (sx, sz, team, id) = {
            let u = &self.units[ui];
            (u.x, u.z, u.team, u.id)
        };
        let held = self
            .zones
            .iter()
            .filter(|z| z.owner == id && z.zk == ZoneKind::Trap)
            .count();
        if held >= 3 {
            self.refund(ui, 1);
            return;
        }
        let (dx, dz) = sim::dir_to(sx, sz, ax, az);
        let d = dist(sx, sz, ax, az).min(10.0);
        let (zx, zz) = (sx + dx * d, sz + dz * d);
        let z = Zone {
            id: self.next_id,
            owner: id,
            team,
            zk: ZoneKind::Trap,
            x: zx,
            z: zz,
            r: 2.4,
            ttl: 40.0,
            tick_t: 0.0,
            dmg: 25.0 + 10.0 * ((rank as f32) - 1.0) + 0.35 * stats.ap,
            detonate: 0.0,
            root: 0.8,
            slow: 0.0,
            attach: 0,
        };
        self.next_id += 1;
        self.zones.push(z);
        self.fx.push(Fx {
            k: 4,
            x: zx,
            z: zz,
            x2: zx,
            z2: zz,
            v: 2.4,
        });
    }

    fn tessera_e(&mut self, ui: usize, rank: usize, ax: f32, az: f32) {
        let (sx, sz) = (self.units[ui].x, self.units[ui].z);
        let d = dist(sx, sz, ax, az).min(4.5 + 0.5 * (rank as f32));
        let (dx, dz) = sim::dir_to(sx, sz, ax, az);
        {
            let u = &mut self.units[ui];
            u.x = (sx + dx * d).clamp(-sim::FIELD_X, sim::FIELD_X);
            u.z = (sz + dz * d).clamp(-data::FIELD_Z, data::FIELD_Z);
        }
        let id = self.units[ui].id;
        add_buff(&mut self.units[ui], BuffKind::Ms, 1.5, 25.0, id);
        let (nx, nz) = (self.units[ui].x, self.units[ui].z);
        self.fx.push(Fx {
            k: 8,
            x: sx,
            z: sz,
            x2: nx,
            z2: nz,
            v: 0.0,
        });
    }

    fn tessera_r(&mut self, ui: usize, rank: usize, stats: &data::Stats, ax: f32, az: f32) {
        let (sx, sz, team, id) = {
            let u = &self.units[ui];
            (u.x, u.z, u.team, u.id)
        };
        let (dx, dz) = sim::dir_to(sx, sz, ax, az);
        let d = dist(sx, sz, ax, az).min(12.0);
        let (zx, zz) = (sx + dx * d, sz + dz * d);
        let z = Zone {
            id: self.next_id,
            owner: id,
            team,
            zk: ZoneKind::Stasis,
            x: zx,
            z: zz,
            r: 4.5,
            ttl: 2.5 + 0.5 * (rank as f32),
            tick_t: 0.0,
            dmg: 35.0 + 20.0 * ((rank as f32) - 1.0) + 0.35 * stats.ap,
            detonate: 80.0 + 25.0 * ((rank as f32) - 1.0) + 0.5 * stats.ap,
            root: 0.0,
            slow: 0.0,
            attach: 0,
        };
        self.next_id += 1;
        self.zones.push(z);
        self.fx.push(Fx {
            k: 3,
            x: zx,
            z: zz,
            x2: zx,
            z2: zz,
            v: 4.5,
        });
    }

    // --- summoner spells ----------------------------------------------------

    /// D/F. Aim is the cursor point; flash and exhaust read it.
    pub(crate) fn cast_spell(&mut self, ui: usize, sp: u8, ax: f32, az: f32) {
        let s = usize::from(sp);
        let (spell, level, team, id, x, z) = {
            let u = &self.units[ui];
            let spell = if s == 0 { u.d } else { u.f };
            (spell, u.level, u.team, u.id, u.x, u.z)
        };
        if spell >= 4 || self.units[ui].dead || self.units[ui].scds[s] > 0.0 {
            return;
        }
        self.units[ui].scds[s] = data::SPELLS[usize::from(spell)].cd;
        match spell {
            data::SPELL_FLASH => {
                let (dx, dz) = sim::dir_to(x, z, ax, az);
                let d = dist(x, z, ax, az).min(7.0);
                {
                    let u = &mut self.units[ui];
                    u.x = (x + dx * d).clamp(-sim::FIELD_X, sim::FIELD_X);
                    u.z = (z + dz * d).clamp(-data::FIELD_Z, data::FIELD_Z);
                }
                let (nx, nz) = (self.units[ui].x, self.units[ui].z);
                self.fx.push(Fx {
                    k: 8,
                    x,
                    z,
                    x2: nx,
                    z2: nz,
                    v: 0.0,
                });
            }
            data::SPELL_HEAL => {
                for i in 0..self.units.len() {
                    if self.units[i].kind == Kind::Champ
                        && !self.units[i].dead
                        && self.units[i].team == team
                        && dist(x, z, self.units[i].x, self.units[i].z) <= 8.0
                    {
                        let missing = self.units[i].max_hp - self.units[i].hp;
                        let amount = 80.0 + missing * 0.1;
                        let mx = self.units[i].max_hp;
                        let u = &mut self.units[i];
                        u.hp = (u.hp + amount).min(mx);
                        let (hx, hz) = (u.x, u.z);
                        self.fx.push(Fx {
                            k: 9,
                            x: hx,
                            z: hz,
                            x2: hx,
                            z2: hz,
                            v: 0.0,
                        });
                    }
                }
            }
            data::SPELL_SMITE => {
                let mut target: Option<usize> = None;
                let mut bd = f32::MAX;
                for i in 0..self.units.len() {
                    let o = &self.units[i];
                    if o.dead || o.team == team {
                        continue;
                    }
                    if matches!(o.kind, Kind::Champ | Kind::Clone) {
                        continue;
                    }
                    let d = dist(ax, az, o.x, o.z);
                    if d <= 5.0 && d < bd {
                        bd = d;
                        target = Some(i);
                    }
                }
                if let Some(ti) = target {
                    let dealt = self.deal_damage(ti, 350.0 + 30.0 * (level as f32), 2, id, false);
                    if dealt > 0.0 {
                        let (tx, tz) = (self.units[ti].x, self.units[ti].z);
                        self.fx.push(Fx {
                            k: 2,
                            x: tx,
                            z: tz,
                            x2: tx,
                            z2: tz,
                            v: 1.2,
                        });
                    }
                } else if let Some(tid) = self.enemy_champ_near(ui, ax, az, 3.5) {
                    if let Some(ti) = self.index_of(tid) {
                        self.deal_damage(ti, 80.0 + 15.0 * (level as f32), 2, id, false);
                        add_buff(&mut self.units[ti], BuffKind::Slow, 1.0, 30.0, id);
                    }
                }
            }
            data::SPELL_EXHAUST => {
                if let Some(tid) = self.enemy_champ_near(ui, ax, az, 4.0) {
                    if let Some(ti) = self.index_of(tid) {
                        add_buff(&mut self.units[ti], BuffKind::Exhaust, 3.0, 0.0, id);
                        add_buff(&mut self.units[ti], BuffKind::Slow, 3.0, 30.0, id);
                        let (tx, tz) = (self.units[ti].x, self.units[ti].z);
                        self.fx.push(Fx {
                            k: 11,
                            x: tx,
                            z: tz,
                            x2: tx,
                            z2: tz,
                            v: 0.0,
                        });
                    }
                }
            }
            _ => {}
        }
    }

    // --- helpers ------------------------------------------------------------

    /// Undo a paid cast that found no target. Cooldowns and mana go back;
    /// a fight must never be lost to a click that landed on nothing.
    fn refund(&mut self, ui: usize, ab: u8) {
        let u = &mut self.units[ui];
        let rank = usize::from(u.ranks[usize::from(ab)].max(1));
        let (mana, _) = Self::ab_cost(&data::CHAMPS[usize::from(u.def)], ab, rank);
        u.mana += mana;
        u.cds[usize::from(ab)] = 0.0;
    }

    pub(crate) fn enemy_champ_near(&self, ui: usize, ax: f32, az: f32, r: f32) -> Option<u32> {
        let team = self.units[ui].team;
        let mut best: Option<(u32, f32)> = None;
        for o in &self.units {
            if o.kind != Kind::Champ || o.dead || o.team == team {
                continue;
            }
            let d2 = (o.x - ax).powi(2) + (o.z - az).powi(2);
            let rr = (r + o.kind.hit_r()).powi(2);
            if d2 <= rr && best.is_none_or(|(_, bd)| d2 < bd) {
                best = Some((o.id, d2));
            }
        }
        best.map(|(i, _)| i)
    }

    /// The ally champion nearest the aim, yourself always included; in a
    /// duel the nearest ally IS you, which is what the spec asks for.
    pub(crate) fn ally_near(&self, ui: usize, ax: f32, az: f32) -> Option<usize> {
        let team = self.units[ui].team;
        let mut best: Option<(usize, f32)> = None;
        for (i, o) in self.units.iter().enumerate() {
            if o.kind != Kind::Champ || o.team != team {
                continue;
            }
            if i != ui && o.dead {
                continue;
            }
            let d2 = (o.x - ax).powi(2) + (o.z - az).powi(2);
            if best.is_none_or(|(_, bd)| d2 < bd) {
                best = Some((i, d2));
            }
        }
        best.map(|(i, _)| i)
    }

    pub(crate) fn spawn_clone(&mut self, parent: u8, team: u8, ttl: f32, dmg_pct: f32) -> Unit {
        let mut c = Unit::blank(self.next_id, Kind::Clone, team);
        self.next_id += 1;
        c.parent = parent;
        c.slot = parent;
        c.ttl = ttl;
        c.clone_dmg = dmg_pct;
        if let Some(p) = self
            .units
            .iter()
            .find(|o| o.kind == Kind::Champ && o.slot == parent)
        {
            c.def = p.def;
            c.level = p.level;
            c.max_hp = p.max_hp;
            c.hp = p.max_hp;
            c.facing = p.facing;
        }
        c
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn beam_by(
        &mut self,
        owner: u32,
        team: u8,
        sx: f32,
        sz: f32,
        dx: f32,
        dz: f32,
        len: f32,
        width: f32,
        dmg: f32,
        slow: f32,
        slow_ttl: f32,
    ) {
        let ex = sx + dx * len;
        let ez = sz + dz * len;
        for i in 0..self.units.len() {
            let o = &self.units[i];
            if o.dead || o.kind == Kind::Clone || o.kind.is_objective() || o.team == team {
                continue;
            }
            let px = o.x - sx;
            let pz = o.z - sz;
            let along = (px * dx + pz * dz).clamp(0.0, len);
            let cx = sx + dx * along;
            let cz = sz + dz * along;
            let rr = (width + o.kind.hit_r()).powi(2);
            if (o.x - cx).powi(2) + (o.z - cz).powi(2) <= rr {
                self.deal_damage(i, dmg, SPELL, owner, true);
                add_buff(&mut self.units[i], BuffKind::Slow, slow_ttl, slow, owner);
            }
        }
        self.fx.push(Fx {
            k: 1,
            x: sx,
            z: sz,
            x2: ex,
            z2: ez,
            v: 1.0,
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn push_proj(
        &mut self,
        owner: u32,
        team: u8,
        kind: ProjKind,
        x: f32,
        z: f32,
        dx: f32,
        dz: f32,
        speed: f32,
        travel: f32,
        dmg: f32,
        crit: bool,
        burn: f32,
        homing: u32,
    ) {
        let mut p = self.blank_proj(owner, team, kind, x, z, dx, dz, speed, travel, dmg);
        p.crit = crit;
        p.burn = burn;
        p.homing = homing;
        self.projs.push(p);
    }

    pub(crate) fn blank_proj(
        &mut self,
        owner: u32,
        team: u8,
        kind: ProjKind,
        x: f32,
        z: f32,
        dx: f32,
        dz: f32,
        speed: f32,
        travel: f32,
        dmg: f32,
    ) -> Proj {
        let p = Proj {
            id: self.next_id,
            owner,
            team,
            kind,
            x,
            z,
            dx,
            dz,
            speed,
            travel,
            dmg,
            crit: false,
            burn: 0.0,
            slow_pct: 0.0,
            slow_ttl: 0.0,
            pierce: 0,
            hook_rank: 0,
            homing: 0,
            hit: [0; 4],
            nhit: 0,
        };
        self.next_id += 1;
        p
    }
}

