//! Regression gates for the authoritative Killshot v30 contract.
use super::*;

fn fixture(kind: SupplyKind) -> Sim {
    let mut level = Level::from_seed(30);
    level.obstacles.clear();
    level.pads.clear();
    level.supplies = vec![SupplySpawn {
        pos: [0.0, 0.0],
        kind,
    }];
    let mut sim = Sim::from_level(&level, 30, GameMode::Ffa);
    sim.add_player(0);
    sim.players[0].pos = [0.0, 0.0];
    sim
}

#[test]
fn inventory_rounds_survive_switches_reload_cancellation_and_empty_slots() {
    let mut sim = fixture(SupplyKind::Health);
    let p = &mut sim.players[0];
    p.ammo = 2;
    grant(p, 3);
    p.ammo = 7;
    p.reserve = 11;
    p.reload_t = 0.7;
    p.shield_state.fire_lock = 0.3;
    p.parkour.velocity = [7.0, 0.0];
    assert!(select_weapon(p, 1));
    assert_eq!((p.ammo, p.reserve), (2, RESERVE_INFINITE));
    assert_eq!(
        p.inventory[2],
        WeaponSlot {
            weapon: 3,
            ammo: 7,
            reserve: 11
        }
    );
    assert_eq!(p.reload_t, 0.0);
    assert_eq!(p.shield_state.fire_lock, 0.3);
    assert_eq!(p.parkour.velocity, [7.0, 0.0]);
    assert!(select_weapon(p, 3));
    assert_eq!((p.ammo, p.reserve), (7, 11));
    for invalid in [0, 2, 8, 9, 255] {
        assert!(!select_weapon(p, invalid));
        assert_eq!((p.weapon, p.ammo, p.reserve), (3, 7, 11));
    }
    assert!(
        p.inventory[7..]
            .iter()
            .all(|slot| *slot == WeaponSlot::EMPTY)
    );
}

#[test]
fn duplicate_weapon_pickup_is_not_a_free_refill_or_reload_reset() {
    let mut sim = fixture(SupplyKind::Health);
    let p = &mut sim.players[0];
    assert!(collect_weapon(p, 7));
    p.ammo = 0;
    p.reserve = 0;
    p.reload_t = 0.5;
    assert!(!collect_weapon(p, 7));
    assert_eq!((p.ammo, p.reserve, p.reload_t), (0, 0, 0.5));
    assert_eq!(
        p.inventory_snapshot()[6],
        WeaponSlot {
            weapon: 7,
            ammo: 0,
            reserve: 0
        }
    );
}

#[test]
fn newly_collected_gun_does_not_shorten_prior_weapon_trigger_recovery() {
    let mut sim = fixture(SupplyKind::Health);
    let p = &mut sim.players[0];
    p.cooldown = 1.4;
    p.shield_state.fire_lock = 0.3;
    assert!(collect_weapon(p, 7));
    assert_eq!(p.cooldown, 1.4);
    assert_eq!(p.shield_state.fire_lock, 0.3);
}

#[test]
fn fully_owned_weapon_pad_is_not_consumed_and_another_player_can_take_it() {
    let mut sim = fixture(SupplyKind::Health);
    for weapon in 1..=WEAPON_COUNT {
        grant(&mut sim.players[0], weapon);
    }
    sim.pads.push(Pad {
        pos: [0.0, 0.0],
        respawn_t: 0.0,
    });
    sim.players[0].ammo = 0;
    sim.players[0].reserve = 0;
    sim.step(&|_| PlayerIn::default());
    assert_eq!(sim.pads[0].respawn_t, 0.0);
    assert_eq!((sim.players[0].ammo, sim.players[0].reserve), (0, 0));
    sim.add_player(1);
    sim.players[1].pos = [0.0, 0.0];
    sim.step(&|_| PlayerIn::default());
    assert!(sim.players[1].weapon > SIDEARM);
    assert_eq!(sim.pads[0].respawn_t, PAD_RESPAWN_SECS);
}

#[test]
fn ammo_cannot_refill_inactive_guns_and_cooldown_rearms_only_after_thirty_seconds() {
    let mut sim = fixture(SupplyKind::Ammo);
    grant(&mut sim.players[0], 7);
    sim.players[0].ammo = 0;
    sim.players[0].reserve = 0;
    assert!(select_weapon(&mut sim.players[0], SIDEARM));
    sim.step(&|_| PlayerIn::default());
    assert_eq!(sim.players[0].inventory[6].reserve, 0);
    assert_eq!(sim.supplies[0].respawn_t, 0.0);
    assert!(select_weapon(&mut sim.players[0], 7));
    sim.step(&|_| PlayerIn::default());
    assert_eq!(sim.players[0].reserve, weapon_stats(7).reserve);
    assert_eq!(sim.players[0].ammo, 0);
    sim.players[0].pos = [5.0, 0.0];
    for _ in 0..1799 {
        sim.step(&|_| PlayerIn::default());
    }
    assert!(sim.supplies[0].respawn_t > 0.0);
    for _ in 0..3 {
        sim.step(&|_| PlayerIn::default());
    }
    assert_eq!(sim.supplies[0].respawn_t, 0.0);
}

#[test]
fn reload_for_each_weapon_counts_down_and_pays_only_at_completion() {
    for weapon in 1..=WEAPON_COUNT {
        let mut sim = fixture(SupplyKind::Health);
        grant(&mut sim.players[0], weapon);
        sim.players[0].ammo = 0;
        let before = sim.players[0].reserve;
        sim.step(&|_| PlayerIn {
            reload: true,
            ..Default::default()
        });
        let duration = weapon_stats(weapon).reload;
        assert_eq!(sim.players[0].reload_t, duration);
        let mut previous = duration;
        for _ in 0..300 {
            sim.step(&|_| PlayerIn::default());
            let p = &sim.players[0];
            assert!(p.reload_t >= 0.0 && p.reload_t < previous);
            assert_eq!(p.inventory_snapshot()[usize::from(weapon - 1)].ammo, p.ammo);
            if p.reload_t == 0.0 {
                let paid = weapon_stats(weapon).mag.min(before);
                assert_eq!(p.ammo, paid);
                assert_eq!(
                    p.reserve,
                    if before == RESERVE_INFINITE {
                        before
                    } else {
                        before - paid
                    }
                );
                break;
            }
            assert_eq!((p.ammo, p.reserve), (0, before));
            previous = p.reload_t;
        }
        assert_eq!(sim.players[0].reload_t, 0.0);
    }
}

#[test]
fn health_supply_caps_healing_and_never_pays_full_health_or_corpses() {
    let mut sim = fixture(SupplyKind::Health);
    sim.step(&|_| PlayerIn::default());
    assert_eq!(sim.supplies[0].respawn_t, 0.0);
    sim.players[0].hp = 4;
    sim.step(&|_| PlayerIn::default());
    assert_eq!(sim.players[0].hp, MAX_HP);
    assert_eq!(sim.supplies[0].respawn_t, SUPPLY_RESPAWN_SECS);
    sim.players[0].hp = 1;
    sim.step(&|_| PlayerIn::default());
    assert_eq!(sim.players[0].hp, 1, "the cooling box cannot pay twice");
    sim.supplies[0].respawn_t = 0.0;
    sim.players[0].alive = false;
    sim.players[0].respawn_in = 10.0;
    sim.step(&|_| PlayerIn::default());
    assert_eq!(sim.players[0].hp, 1);
    assert_eq!(sim.supplies[0].respawn_t, 0.0);
}

#[test]
fn ammo_supply_restores_only_active_finite_reserve_never_magazines() {
    for weapon in 2..=WEAPON_COUNT {
        let mut sim = fixture(SupplyKind::Ammo);
        grant(&mut sim.players[0], weapon);
        sim.players[0].ammo = 0;
        sim.players[0].reserve = 0;
        sim.step(&|_| PlayerIn::default());
        let p = &sim.players[0];
        assert_eq!(
            (p.weapon, p.ammo, p.reserve),
            (weapon, 0, weapon_stats(weapon).reserve)
        );
        assert_eq!(
            p.reload_t, 0.0,
            "pickup does not finish/start a free reload"
        );
        assert_eq!(
            p.inventory_snapshot()[usize::from(weapon - 1)].reserve,
            p.reserve
        );
        assert_eq!(sim.supplies[0].respawn_t, SUPPLY_RESPAWN_SECS);
        sim.step(&|_| PlayerIn::default());
        assert_eq!(sim.players[0].reload_t, weapon_stats(weapon).reload);
    }
    let mut sim = fixture(SupplyKind::Ammo);
    sim.players[0].ammo = 1;
    sim.step(&|_| PlayerIn::default());
    assert_eq!(
        sim.supplies[0].respawn_t, 0.0,
        "pistol does not consume an ammo box"
    );
    assert_eq!(sim.players[0].ammo, 1);
}

#[test]
fn supply_eligibility_skips_full_player_and_single_payout_is_stable() {
    let mut sim = fixture(SupplyKind::Health);
    sim.add_player(1);
    sim.add_player(2);
    for p in &mut sim.players {
        p.pos = [0.0, 0.0];
    }
    sim.players[1].hp = 1;
    sim.players[2].hp = 1;
    sim.step(&|_| PlayerIn::default());
    assert_eq!(
        sim.players.iter().map(|p| p.hp).collect::<Vec<_>>(),
        vec![5, 3, 1]
    );
    assert_eq!(sim.supplies[0].respawn_t, SUPPLY_RESPAWN_SECS);
}

#[test]
fn supplies_do_not_pay_through_a_wall_roof_or_round_pause() {
    for reason in 0..3 {
        let mut sim = fixture(SupplyKind::Health);
        sim.players[0].hp = 1;
        match reason {
            0 => {
                sim.players[0].pos = [-0.8, 0.0];
                sim.obstacles.push(Obstacle::boxed(
                    Cover::Wall,
                    [-0.4, -1.0],
                    [-0.2, 1.0],
                    0.0,
                    2.5,
                ));
            }
            1 => {
                sim.players[0].y = 3.0;
                sim.obstacles.push(Obstacle::boxed(
                    Cover::Roof,
                    [-2.0, -2.0],
                    [2.0, 2.0],
                    2.5,
                    3.0,
                ));
            }
            _ => sim.round_pause = 2.0,
        }
        sim.step(&|_| PlayerIn::default());
        assert_eq!(sim.players[0].hp, 1, "blocked reason {reason}");
        assert_eq!(sim.supplies[0].respawn_t, 0.0);
    }
}

#[test]
fn selected_start_applies_to_join_death_and_round_restart_without_loot_carry() {
    for weapon in 1..=WEAPON_COUNT {
        let mut sim = fixture(SupplyKind::Health);
        sim.starting_weapon = weapon;
        sim.add_player(1);
        let p = &mut sim.players[1];
        assert_eq!(p.weapon, weapon);
        assert_eq!(p.inventory[0].weapon, SIDEARM);
        let extra = if weapon == 7 { 6 } else { 7 };
        collect_weapon(p, extra);
        p.alive = false;
        p.respawn_in = 0.0;
        sim.step(&|_| PlayerIn::default());
        let p = &sim.players[1];
        assert_eq!(p.weapon, weapon);
        assert_eq!(p.inventory[usize::from(extra - 1)].weapon, 0);
        assert_eq!(
            (p.ammo, p.reserve),
            (weapon_stats(weapon).mag, weapon_stats(weapon).reserve)
        );
        sim.restart_round();
        assert_eq!(sim.players[1].weapon, weapon);
        assert!(sim.supplies.iter().all(|s| s.respawn_t == 0.0));
    }
}

#[test]
fn all_authored_supplies_are_clear_accessible_and_off_spawn_points() {
    for level in [Level::trench_city(), Level::freight_yard(), Level::harbor()] {
        for supply in &level.supplies {
            let p = supply.pos;
            assert!(
                p.iter()
                    .all(|v| v.is_finite() && v.abs() < level.arena_half - 2.0)
            );
            assert!(
                !blocked_in(p, 0.0, PLAYER_R, &level.obstacles, level.arena_half),
                "blocked supply {supply:?}"
            );
            assert!(
                level
                    .spawns
                    .iter()
                    .all(|s| (p[0] - s[0]).hypot(p[1] - s[1]) > 3.0)
            );
            let exits = [[2.0, 0.0], [-2.0, 0.0], [0.0, 2.0], [0.0, -2.0]]
                .iter()
                .filter(|d| {
                    let q = [p[0] + d[0], p[1] + d[1]];
                    !blocked_in(q, 0.0, PLAYER_R, &level.obstacles, level.arena_half)
                        && !segment_hits_cover(
                            [p[0], 0.6, p[1]],
                            [q[0], 0.6, q[1]],
                            &level.obstacles,
                        )
                })
                .count();
            assert!(exits >= 2, "supply needs two open approaches: {supply:?}");
        }
        assert!(level.supplies.iter().any(|s| s.kind == SupplyKind::Health));
        assert!(level.supplies.iter().any(|s| s.kind == SupplyKind::Ammo));
    }
}
