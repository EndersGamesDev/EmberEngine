//! Breach-12's server-owned eight-pellet resource and collision contract.
use super::*;

fn arena(seed: u64) -> Sim {
    let mut level = Level::from_seed(seed);
    level.arena_half = 80.0;
    level.obstacles.clear();
    level.pads.clear();
    level.supplies.clear();
    let mut sim = Sim::from_level(&level, seed, GameMode::Ffa);
    sim.add_player(0);
    sim.players[0].pos = [0.0, 0.0];
    grant(&mut sim.players[0], SHOTGUN);
    sim.players[0].cooldown = 0.0;
    sim
}

fn duel(seed: u64, distance: f32, shield: bool, cover: bool) -> Sim {
    let mut sim = arena(seed);
    sim.add_player(1);
    sim.players[1].pos = [distance, 0.0];
    if cover {
        sim.obstacles.push(Obstacle::boxed(
            Cover::Wall,
            [distance * 0.5, -3.0],
            [distance * 0.5 + 0.2, 3.0],
            0.0,
            3.0,
        ));
    }
    let pitch = ((1.0 - EYE_STAND) / (distance - 0.2)).atan();
    for tick in 0..8 {
        sim.step(&|id| PlayerIn {
            fire: id == 0 && tick == 0,
            aim: if id == 0 { [1.0, 0.0] } else { [-1.0, 0.0] },
            pitch: if id == 0 { pitch } else { 0.0 },
            shield: id == 1 && shield,
            ..Default::default()
        });
    }
    sim
}

#[test]
fn one_shell_spawns_eight_unique_deterministic_projectiles_and_one_recoil() {
    let mut a = arena(31);
    let mut b = arena(31);
    for sim in [&mut a, &mut b] {
        sim.step(&|_| PlayerIn {
            fire: true,
            aim: [1.0, 0.0],
            ..Default::default()
        });
    }
    assert_eq!(a.bullets, b.bullets);
    assert_eq!(a.bullets.len(), usize::from(SHOTGUN_PELLETS));
    assert_eq!(a.players[0].ammo, 5);
    assert_eq!(a.players[0].reserve, 24);
    assert_eq!(a.players[0].fired, 1);
    assert_eq!(a.players[0].bloom, weapon_stats(SHOTGUN).bloom);
    assert_eq!(a.players[0].cooldown, 0.85);
    let ids: std::collections::BTreeSet<_> = a.bullets.iter().map(|b| b.projectile_id).collect();
    assert_eq!(ids.len(), 8);
    assert!(ids.iter().all(|id| id >> 4 == projectile_id(1, 0, 0) >> 4));
    assert_eq!(a.players[0].inventory[7].ammo, 5);
    a.step(&|_| PlayerIn {
        fire: true,
        aim: [1.0, 0.0],
        ..Default::default()
    });
    assert_eq!(a.players[0].ammo, 5, "held fire cannot skip the pump cycle");
}

#[test]
fn batch_capacity_never_fires_half_a_shell_or_charges_for_a_refused_shot() {
    let mut sim = arena(31);
    let p = sim.players[0].clone();
    launch(
        &p,
        &weapon_stats(SHOTGUN),
        false,
        true,
        31,
        0,
        0,
        &mut sim.bullets,
    );
    sim.bullets.truncate(3);
    sim.step(&|_| PlayerIn {
        fire: true,
        aim: [1.0, 0.0],
        ..Default::default()
    });
    assert_eq!(sim.players[0].ammo, 6);
    assert_eq!(sim.players[0].fired, 0);
    assert_eq!(sim.bullets.len(), 3);
}

#[test]
fn projectile_identity_is_unique_and_json_number_exact() {
    let mut ids = std::collections::BTreeSet::new();
    for tick in [0, 1, (1 << 40) - 1] {
        for owner in 0..=255 {
            for pellet in 0..SHOTGUN_PELLETS {
                let id = projectile_id(tick, owner, pellet);
                assert!(id < (1 << 52));
                assert!(ids.insert(id));
            }
        }
    }
}

#[test]
fn intrinsic_pattern_stays_wide_while_ads_and_crouch_steady_aim() {
    let hip = weapon_spread(SHOTGUN, 0.0, 0.0, false, false, true);
    let ads = weapon_spread(SHOTGUN, 1.0, 0.0, false, false, true);
    let crouch = weapon_spread(SHOTGUN, 1.0, 0.0, true, false, true);
    assert!(hip > ads && ads > crouch && crouch > SHOTGUN_PATTERN_SPREAD);
    let mut sim = arena(31);
    sim.players[0].ads_fraction = 1.0;
    sim.players[0].crouch = true;
    let mut pellets = Vec::new();
    launch(
        &sim.players[0],
        &weapon_stats(SHOTGUN),
        false,
        true,
        31,
        9,
        0,
        &mut pellets,
    );
    let widths: Vec<_> = pellets
        .iter()
        .map(|p| {
            [
                p.vel[1].atan2(p.vel[0]),
                (p.vy / weapon_stats(SHOTGUN).speed).atan(),
            ]
        })
        .collect();
    let widest = widths
        .iter()
        .flat_map(|a| widths.iter().map(move |b| (a[0] - b[0]).hypot(a[1] - b[1])))
        .fold(0.0_f32, f32::max);
    assert!(
        widest > SHOTGUN_PATTERN_SPREAD * 1.9,
        "the spread never becomes a slug"
    );
}

#[test]
fn centred_close_shots_are_lethal_but_distance_dispersion_is_not_a_sniper() {
    let mut close_kills = 0;
    let mut far_survivors = 0;
    for seed in 0..32 {
        let close = duel(seed, 5.0, false, false);
        close_kills += usize::from(!close.players[1].alive);
        assert!(
            close.players[0].score <= 1,
            "one shell cannot score the same victim twice"
        );
        far_survivors += usize::from(duel(seed, 18.0, false, false).players[1].alive);
        assert_eq!(duel(seed, 27.0, false, false).players[1].hp, MAX_HP);
    }
    assert_eq!(close_kills, 32);
    assert!(far_survivors >= 28, "18m survivors: {far_survivors}/32");
}

#[test]
fn every_pellet_stops_at_cover_and_shields_protect_the_holder() {
    for seed in 0..16 {
        assert_eq!(duel(seed, 5.0, false, true).players[1].hp, MAX_HP);
        assert_eq!(duel(seed, 5.0, true, false).players[1].hp, MAX_HP);
    }
}

#[test]
fn one_pellet_head_contact_is_one_damage_not_an_instant_kill() {
    let mut sim = arena(31);
    sim.add_player(1);
    sim.players[1].pos = [3.0, 0.0];
    let mut pellets = Vec::new();
    launch(
        &sim.players[0],
        &weapon_stats(SHOTGUN),
        false,
        true,
        31,
        1,
        0,
        &mut pellets,
    );
    let mut pellet = pellets[0];
    pellet.pos = [0.2, 0.0];
    pellet.y = f32::midpoint(HEAD_TOP_STAND, head_lo(false));
    pellet.vel = [360.0, 0.0];
    pellet.vy = 0.0;
    sim.bullets.push(pellet);
    sim.step(&|_| PlayerIn::default());
    assert_eq!(sim.hits, vec![(0, 1, 1, true)]);
    assert_eq!(sim.players[1].hp, 4);
    assert!(sim.players[1].alive);
    assert_eq!(sim.shots[0].projectile_id, pellet.projectile_id);
}

#[test]
fn shotgun_pattern_passes_teammates_without_damage_or_reflection() {
    let mut sim = arena(31);
    sim.mode = GameMode::Tdm;
    sim.add_player(1);
    sim.players[1].team = sim.players[0].team;
    sim.players[1].pos = [3.0, 0.0];
    sim.step(&|id| PlayerIn {
        fire: id == 0,
        shield: id == 1,
        aim: if id == 0 { [1.0, 0.0] } else { [-1.0, 0.0] },
        ..Default::default()
    });
    assert_eq!(sim.hits, []);
    assert_eq!(sim.players[1].hp, MAX_HP);
    assert!(sim.bullets.iter().all(|p| p.owner == 0));
}

#[test]
fn shield_reflection_keeps_pellet_identity_and_credits_the_reflector_once() {
    let mut sim = arena(31);
    sim.add_player(1);
    sim.players[1].pos = [5.0, 0.0];
    sim.players[0].hp = 1;
    let mut pellets = Vec::new();
    launch(
        &sim.players[0],
        &weapon_stats(SHOTGUN),
        false,
        true,
        31,
        1,
        0,
        &mut pellets,
    );
    let mut pellet = pellets[0];
    pellet.pos = [0.2, 0.0];
    pellet.y = 1.0;
    pellet.vel = [360.0, 0.0];
    pellet.vy = 0.0;
    sim.bullets.push(pellet);
    let mut events = Vec::new();
    let mut kills = Vec::new();
    for _ in 0..4 {
        sim.step(&|id| PlayerIn {
            shield: id == 1,
            aim: if id == 1 { [-1.0, 0.0] } else { [1.0, 0.0] },
            ..Default::default()
        });
        events.extend(sim.shots.iter().copied());
        kills.extend(sim.events.iter().copied());
    }
    assert_eq!(events.len(), 2);
    assert_eq!((events[0].hit, events[0].owner), (SHOT_SHIELD, 0));
    assert_eq!((events[1].hit, events[1].owner), (SHOT_BODY, 1));
    assert!(
        events
            .iter()
            .all(|event| event.projectile_id == pellet.projectile_id)
    );
    assert_eq!(kills, vec![(1, 0)]);
    assert_eq!(sim.players[1].score, 1);
    assert_eq!(sim.players[1].hp, MAX_HP);
}

#[test]
fn shotgun_slot_eight_custom_start_and_loot_are_authoritative() {
    assert_eq!(SHOTGUN, 8);
    assert!(LOOT_POOL.contains(&SHOTGUN));
    assert!((0..100).any(|tick| loot_roll(31, tick, 0, 1) == SHOTGUN));
    let mut sim = arena(31);
    sim.starting_weapon = SHOTGUN;
    sim.add_player(1);
    let p = &mut sim.players[1];
    assert_eq!((p.weapon, p.ammo, p.reserve), (SHOTGUN, 6, 24));
    assert_eq!(p.inventory[7].weapon, SHOTGUN);
    assert_eq!(p.inventory[8], WeaponSlot::EMPTY);
    assert!(select_weapon(p, SIDEARM));
    assert!(select_weapon(p, SHOTGUN));
    assert!(!collect_weapon(p, SHOTGUN));
    p.alive = false;
    p.respawn_in = 0.0;
    sim.step(&|_| PlayerIn::default());
    assert_eq!(sim.players[1].weapon, SHOTGUN);
}

#[test]
fn buckshot_has_exact_twenty_five_metre_horizontal_lifetime() {
    let mut sim = arena(31);
    let mut ended = Vec::new();
    for tick in 0..8 {
        sim.step(&|_| PlayerIn {
            fire: tick == 0,
            aim: [1.0, 0.0],
            ..Default::default()
        });
        ended.extend(sim.shots.iter().copied());
    }
    assert_eq!(ended.len(), 8);
    assert_eq!(sim.bullets, []);
    for shot in ended {
        assert_eq!(shot.hit, SHOT_EXPIRED);
        let distance = (shot.to[0] - shot.from[0]).hypot(shot.to[2] - shot.from[2]);
        assert!((distance - SHOTGUN_RANGE).abs() < 0.001, "{distance}");
    }
}
