#![allow(clippy::float_cmp)]

use league_core::data;
use league_core::proto::{Cmd, Phase};
use league_core::sim::{Buff, BuffKind, Kind, Match, Order};

fn game(size: u8) -> Match {
    let mut game = Match::new(size, 777);
    for slot in 0..size * 2 {
        game.join(&format!("player-{slot}"));
        game.set_pick(slot, slot % 5, 0, 1, [0, 1, 2]);
    }
    game.start();
    game.wave_left = 1e9;
    for unit in &mut game.units {
        if unit.kind == Kind::Champ {
            unit.runes = [u8::MAX; 3];
        }
    }
    game.step();
    game
}

const fn buff(kind: BuffKind, ttl: f32, val: f32, src: u32) -> Buff {
    Buff {
        k: kind,
        ttl,
        val,
        aux: 0.0,
        src,
    }
}

#[test]
fn level_one_can_learn_and_cast_an_ability() {
    let mut game = game(1);
    assert_eq!(game.units[0].level, 1);
    assert_eq!(game.units[0].points, 1);
    game.command(0, Cmd::Rank { slot: 1 });
    game.command(
        0,
        Cmd::Cast {
            slot: 1,
            x: 0.0,
            z: 0.0,
        },
    );
    game.step();
    assert_eq!(game.units[0].ranks, [0, 1, 0, 0]);
    assert_eq!(game.units[0].points, 0);
    assert!(game.units[0].cds[1] > 0.0);
    assert!(game.units[0].mana < game.units[0].max_mana);
}

#[test]
fn ultimate_unlocks_at_six_and_levels_grant_points() {
    let mut game = game(1);
    game.command(0, Cmd::Rank { slot: 3 });
    game.step();
    assert_eq!(game.units[0].ranks[3], 0);
    assert_eq!(game.units[0].points, 1);
    game.units[0].xp = data::xp_needed(1);
    game.step();
    assert_eq!(game.units[0].level, 2);
    assert_eq!(game.units[0].points, 2);
    game.units[0].level = 6;
    game.command(0, Cmd::Rank { slot: 3 });
    game.step();
    assert_eq!(game.units[0].ranks[3], 1);
}

#[test]
fn percentage_stats_retain_the_values_advertised_by_items_and_runes() {
    let mut game = game(1);
    game.units[0].runes = [0, 5, 6];
    game.units[0].items = [5, 6, 0, 0, 0, 0];
    let stats = game.champ_stats_of(&game.units[0]);
    assert_eq!(stats.aspd, 7.0);
    assert_eq!(stats.haste, 17.0);
    assert_eq!(stats.crit, 13.0);
    assert_eq!(stats.critd, 191.0);
    assert_eq!(stats.ms, data::CHAMPS[0].ms);
}

#[test]
fn ability_haste_reduces_cooldown_once_and_counts_down_while_dead() {
    let mut game = game(1);
    game.units[0].runes = [5, u8::MAX, u8::MAX];
    game.units[0].items[0] = 5;
    game.units[0].ranks[1] = 1;
    game.command(
        0,
        Cmd::Cast {
            slot: 1,
            x: 0.0,
            z: 0.0,
        },
    );
    game.step();
    let cast_cd = data::CHAMPS[0].w.cd[0] / 1.17;
    assert!((game.units[0].cds[1] - (cast_cd - league_core::DT)).abs() < 0.001);
    game.units[0].dead = true;
    game.units[0].respawn = 20.0;
    let before = game.units[0].cds[1];
    for _ in 0..60 {
        game.step();
    }
    assert!((game.units[0].cds[1] - (before - 1.0)).abs() < 0.001);
}

#[test]
fn recent_assists_survive_repeated_hits_and_receive_gold() {
    let mut game = game(3);
    let helper = game.units[0].id;
    let killer = game.units[1].id;
    let helper_gold = game.units[0].gold;
    let killer_gold = game.units[1].gold;
    game.deal_damage(3, 10.0, 0, helper, false);
    for _ in 0..20 {
        game.deal_damage(3, 1.0, 0, killer, false);
    }
    game.deal_damage(3, 10000.0, 0, killer, false);
    assert!(game.units[3].dead);
    assert_eq!(game.units[0].gold - helper_gold, 96);
    assert_eq!(game.units[1].gold - killer_gold, 324);
    assert_eq!(game.kills, [1, 0]);
}

#[test]
fn unassisted_kill_pays_the_full_bounty_and_clears_demon_form() {
    let mut game = game(1);
    let killer = game.units[0].id;
    game.units[1].form = 10.0;
    game.units[1].tp = 3;
    let gold = game.units[0].gold;
    game.deal_damage(1, 10000.0, 0, killer, false);
    assert_eq!(game.units[0].gold - gold, 420);
    assert_eq!(game.units[1].hp, 0.0);
    assert_eq!(game.units[1].tp, 0);
    assert_eq!(game.units[1].form, 0.0);
}

#[test]
fn lethal_burn_with_other_buffs_does_not_panic_or_heal_a_corpse() {
    let mut game = game(1);
    let killer = game.units[0].id;
    game.units[1].hp = 1.0;
    game.units[1].buffs = vec![
        buff(BuffKind::Burn, 2.0, 10000.0, killer),
        buff(BuffKind::Regen, 2.0, 20.0, killer),
    ];
    game.step();
    assert!(game.units[1].dead);
    assert_eq!(game.units[1].hp, 0.0);
    assert!(game.units[1].buffs.is_empty());
}

#[test]
fn burn_can_consume_a_revive_mark_without_invalidating_the_buff_loop() {
    let mut game = game(1);
    let killer = game.units[0].id;
    game.units[1].hp = 1.0;
    game.units[1].buffs = vec![
        buff(BuffKind::Revive, 5.0, 30.0, killer),
        buff(BuffKind::Burn, 2.0, 1000.0, killer),
    ];
    game.step();
    assert!(!game.units[1].dead);
    assert!(game.units[1].hp > 100.0);
    assert!(!game.units[1].buffs.iter().any(|b| b.k == BuffKind::Revive));
    assert_eq!(game.kills, [0, 0]);
}

#[test]
fn every_fallback_squad_has_unique_teammates_and_three_distinct_runes() {
    for seed in 0..100 {
        let mut game = Match::new(3, seed);
        game.start();
        for team in 0..2 {
            let picks: Vec<_> = game
                .roster
                .iter()
                .filter(|r| r.team == team)
                .map(|r| r.champ)
                .collect();
            assert!(picks[0] != picks[1] && picks[1] != picks[2] && picks[0] != picks[2]);
        }
        for roster in game.roster {
            let [a, b, c] = roster.runes;
            assert!(a < 8 && b < 8 && c < 8);
            assert!(a != b && b != c && a != c);
        }
    }
}

#[test]
fn leaving_selection_releases_the_champion_pick() {
    let mut game = Match::new(3, 12);
    game.join("first");
    game.join("second");
    game.set_pick(0, data::SWARM, 0, 1, [0, 1, 2]);
    game.leave(0);
    game.set_pick(1, data::SWARM, 0, 1, [0, 1, 2]);
    assert!(game.roster[1].picked);
    assert!(!game.roster[0].picked);
}

#[test]
fn all_twenty_champion_abilities_have_executable_ranked_casts() {
    for champion in 0..5 {
        for slot in 0..4 {
            let mut game = game(1);
            game.units[0].def = champion;
            game.units[0].level = 12;
            game.units[0].ranks = [3; 4];
            game.units[0].mana = 1000.0;
            game.units[0].x = 0.0;
            game.units[0].z = 0.0;
            game.units[1].x = 4.0;
            game.units[1].z = 0.0;
            game.units[1].hp = 10000.0;
            game.command(
                0,
                Cmd::Cast {
                    slot,
                    x: 4.0,
                    z: 0.0,
                },
            );
            game.step();
            assert!(
                game.units[0].cds[usize::from(slot)] > 0.0,
                "champion {champion}, ability {slot}"
            );
            assert!(
                game.units
                    .iter()
                    .all(|u| u.hp.is_finite() && u.x.is_finite() && u.z.is_finite())
            );
        }
    }
}

#[test]
fn swarm_hologram_attacks_then_expires_after_five_seconds() {
    let mut game = game(1);
    game.units[0].x = 0.0;
    game.units[1].x = 5.0;
    game.units[0].ranks[2] = 1;
    let target = game.units[1].id;
    game.command(0, Cmd::Attack { target });
    game.command(
        0,
        Cmd::Cast {
            slot: 2,
            x: 5.0,
            z: 0.0,
        },
    );
    game.step();
    let clone = game.units.iter().find(|u| u.kind == Kind::Clone).unwrap();
    assert_eq!(clone.order, Order::Attack);
    assert_eq!(clone.target, target);
    for _ in 0..301 {
        game.step();
    }
    assert!(game.units.iter().all(|u| u.kind != Kind::Clone));
}

#[test]
fn knight_demon_form_doubles_damage_and_allows_exactly_three_blinks() {
    let mut game = game(1);
    game.units[0].def = data::KNIGHT;
    game.units[0].level = 6;
    game.units[0].ranks[3] = 1;
    game.units[0].x = 0.0;
    game.command(
        0,
        Cmd::Cast {
            slot: 3,
            x: 0.0,
            z: 0.0,
        },
    );
    game.step();
    let source = game.units[0].id;
    assert_eq!(game.deal_damage(1, 10.0, 0, source, false), 20.0);
    for aim in [40.0, -40.0, 15.0] {
        game.command(
            0,
            Cmd::Cast {
                slot: 3,
                x: aim,
                z: 0.0,
            },
        );
        game.step();
        assert_eq!(game.units[0].x, aim);
    }
    assert_eq!(game.units[0].tp, 0);
    game.command(
        0,
        Cmd::Cast {
            slot: 3,
            x: 20.0,
            z: 0.0,
        },
    );
    game.step();
    assert_eq!(game.units[0].x, 15.0);
}

#[test]
fn hallow_mark_only_guards_the_requested_five_second_window() {
    let mut game = game(1);
    game.units[0].def = data::HALLOW;
    game.units[0].level = 12;
    game.units[0].ranks[3] = 3;
    game.command(
        0,
        Cmd::Cast {
            slot: 3,
            x: 0.0,
            z: 0.0,
        },
    );
    game.step();
    assert!(game.units[0].buffs.iter().any(|b| b.k == BuffKind::Revive));
    for _ in 0..301 {
        game.step();
    }
    let source = game.units[1].id;
    game.deal_damage(0, 10000.0, 0, source, false);
    assert!(game.units[0].dead);
}

#[test]
fn knight_guard_blocks_damage_for_two_seconds_then_expires() {
    let mut game = game(1);
    game.units[0].def = data::KNIGHT;
    game.units[0].ranks[1] = 1;
    game.units[0].x = 0.0;
    game.command(
        0,
        Cmd::Cast {
            slot: 1,
            x: 0.0,
            z: 0.0,
        },
    );
    game.step();
    let source = game.units[1].id;
    assert_eq!(game.deal_damage(0, 1000.0, 0, source, false), 0.0);
    for _ in 0..121 {
        game.step();
    }
    assert!(game.deal_damage(0, 100.0, 0, source, false) > 0.0);
}

#[test]
fn hallow_support_abilities_target_the_ally_under_the_cursor() {
    let mut game = game(3);
    game.units[0].def = data::HALLOW;
    game.units[0].ranks = [1, 1, 1, 0];
    game.units[0].x = 0.0;
    game.units[1].x = 5.0;
    game.units[1].hp = 100.0;
    game.units[0].mana = 400.0;
    game.units[0].hp = 100.0;
    let normal_speed = game.speed_of(1);
    for slot in 0..3 {
        game.command(
            0,
            Cmd::Cast {
                slot,
                x: 5.0,
                z: 0.0,
            },
        );
    }
    game.step();
    assert!(game.units[1].hp > 200.0);
    assert!(game.units[0].hp < 110.0);
    assert!((game.speed_of(1) / normal_speed - 1.35).abs() < 0.001);
    assert!(
        game.units[1]
            .buffs
            .iter()
            .any(|b| b.k == BuffKind::Shield && b.val > 0.0)
    );
}

#[test]
fn swarm_ultimate_splits_into_four_and_fires_four_drone_volleys() {
    let mut game = game(1);
    game.units[0].level = 6;
    game.units[0].ranks[3] = 1;
    game.units[0].x = 0.0;
    game.units[1].x = 10.0;
    game.command(
        0,
        Cmd::Cast {
            slot: 3,
            x: 10.0,
            z: 0.0,
        },
    );
    game.step();
    assert_eq!(
        game.units.iter().filter(|u| u.kind == Kind::Clone).count(),
        3
    );
    assert_eq!(
        game.projs
            .iter()
            .filter(|p| p.kind == league_core::sim::ProjKind::Drone)
            .count(),
        12
    );
    assert_eq!(game.fx.iter().filter(|fx| fx.k == 1).count(), 4);
}

#[test]
fn spell_slow_items_and_damage_reduction_apply_their_advertised_effects() {
    let mut game = game(1);
    game.units[0].items[0] = data::ITEMS.iter().find(|item| item.spell_slow).unwrap().id;
    game.units[1].items[0] = data::ITEMS
        .iter()
        .find(|item| item.flat.dr > 0.0)
        .unwrap()
        .id;
    let source = game.units[0].id;
    assert!((game.deal_damage(1, 100.0, 1, source, false) - 92.0).abs() < 0.001);
    assert!(
        game.units[1]
            .buffs
            .iter()
            .any(|b| b.k == BuffKind::Slow && b.val == 20.0)
    );
    assert_eq!(game.deal_damage(1, 100.0, 2, source, false), 100.0);
}

#[test]
fn smite_cannot_hit_a_core_or_a_distant_court_and_refunds_invalid_targets() {
    let mut game = game(1);
    game.units[0].d = data::SPELL_SMITE;
    let core = game
        .units
        .iter()
        .position(|u| u.kind == Kind::CoreRed)
        .unwrap();
    game.units[0].x = data::CORE_X - 3.0;
    game.command(
        0,
        Cmd::Spell {
            slot: 0,
            x: data::CORE_X,
            z: 0.0,
        },
    );
    game.step();
    assert_eq!(game.units[core].hp, data::CORE_HP);
    assert_eq!(game.units[0].scds[0], 0.0);
    game.command(
        0,
        Cmd::Spell {
            slot: 0,
            x: 0.0,
            z: 16.0,
        },
    );
    game.step();
    assert_eq!(game.units[0].scds[0], 0.0);
    assert!(
        game.units
            .iter()
            .any(|u| u.kind == Kind::CourtN && u.hp == data::COURT_HP)
    );
}

#[test]
fn court_capture_pays_the_team_and_grants_the_strategic_boon() {
    let mut game = game(3);
    let source = game.units[0].id;
    let court = game
        .units
        .iter()
        .position(|u| u.kind == Kind::CourtN)
        .unwrap();
    game.deal_damage(court, 2000.0, 0, source, false);
    assert_eq!(game.boon, [1, 0]);
    assert_eq!(game.court_respawn[0], data::COURT_RESPAWN);
    for slot in 0..3 {
        assert_eq!(game.units[slot].gold, data::START_GOLD + 150);
        assert!(game.units[slot].level > 1);
    }
}

#[test]
fn shopping_and_six_inventory_keys_work_without_duplicate_permanents() {
    let mut game = game(1);
    game.command(0, Cmd::Buy { item: 1 });
    game.step();
    assert_eq!(game.units[0].items[0], 1);
    assert_eq!(game.units[0].gold, 150);
    game.units[0].gold = 1000;
    game.command(0, Cmd::Buy { item: 1 });
    game.command(0, Cmd::Buy { item: 17 });
    game.step();
    assert_eq!(game.units[0].items.iter().filter(|&&i| i == 1).count(), 1);
    assert_eq!(game.units[0].items[1], 17);
    game.units[0].x = 0.0;
    game.units[0].hp = 100.0;
    let charges = game.units[0].charges[1];
    game.command(0, Cmd::UseItem { slot: 1 });
    game.step();
    assert_eq!(game.units[0].charges[1], charges - 1);
    for _ in 0..360 {
        game.step();
    }
    assert!(game.units[0].hp > 200.0);
}

#[test]
fn ranged_auto_attacks_follow_the_selected_target_past_other_enemies() {
    let mut game = game(3);
    game.units[0].x = 0.0;
    game.units[0].z = 0.0;
    game.units[3].x = 2.0;
    game.units[3].z = 0.0;
    game.units[4].x = 5.0;
    game.units[4].z = 0.0;
    let target = game.units[4].id;
    let blocker_hp = game.units[3].hp;
    let target_hp = game.units[4].hp;
    game.command(0, Cmd::Attack { target });
    game.step();
    assert!(game.projs.iter().any(|p| p.homing == target));
    game.units[0].order = Order::Hold;
    game.units[4].z = 2.0;
    for _ in 0..40 {
        game.step();
    }
    assert!(game.units[4].hp < target_hp);
    assert!(game.units[3].hp >= blocker_hp);
}

#[test]
fn identical_bot_matches_produce_identical_snapshots() {
    let mut first = Match::new(3, 812);
    let mut second = Match::new(3, 812);
    first.start();
    second.start();
    for tick in 0..60 * 180 {
        for game in [&mut first, &mut second] {
            for slot in 0..6 {
                if let Some(command) = league_core::ai::think(game, slot) {
                    game.command(slot, command);
                }
            }
            game.step();
        }
        if tick % 60 == 0 {
            assert_eq!(
                serde_json::to_string(&first.snapshot()).unwrap(),
                serde_json::to_string(&second.snapshot()).unwrap()
            );
        }
        if first.phase == Phase::Over {
            break;
        }
    }
    assert!(
        first
            .units
            .iter()
            .filter(|u| u.kind == Kind::Champ)
            .any(|u| u.level >= 3)
    );
}

#[test]
fn snapshots_keep_active_projectiles_and_zones_visible_across_broadcasts() {
    let mut game = game(1);
    game.units[0].x = 0.0;
    game.units[1].x = 10.0;
    game.units[0].ranks[0] = 1;
    game.command(
        0,
        Cmd::Cast {
            slot: 0,
            x: 10.0,
            z: 0.0,
        },
    );
    game.step();
    game.units[0].def = data::KNIGHT;
    game.units[0].cds[0] = 0.0;
    game.command(
        0,
        Cmd::Cast {
            slot: 0,
            x: 5.0,
            z: 0.0,
        },
    );
    game.step();
    let snapshot = game.snapshot();
    let league_core::proto::S2C::State { projs, zones, .. } = snapshot else {
        panic!("expected world snapshot");
    };
    assert_eq!(projs.len(), 3);
    assert!(projs.iter().all(|p| p.k == 1 && p.t == 0));
    assert_eq!(zones.len(), 1);
    assert_eq!(zones[0].k, 0);
    let wire = serde_json::to_string(&game.snapshot()).unwrap();
    let league_core::proto::S2C::State {
        projs: next_projs,
        zones: next_zones,
        ..
    } = serde_json::from_str(&wire).unwrap()
    else {
        panic!("expected world snapshot");
    };
    assert_eq!(projs, next_projs);
    assert_eq!(zones, next_zones);
    let mut legacy: serde_json::Value = serde_json::from_str(&wire).unwrap();
    legacy.as_object_mut().unwrap().remove("projs");
    legacy.as_object_mut().unwrap().remove("zones");
    let league_core::proto::S2C::State { projs, zones, .. } =
        serde_json::from_value(legacy).unwrap()
    else {
        panic!("expected world snapshot");
    };
    assert!(projs.is_empty() && zones.is_empty());
}

#[test]
fn either_team_can_win_and_result_expiry_preserves_the_finished_match() {
    for size in [1, 3] {
        for winner in [0, 1] {
            let mut game = game(size);
            let loser = 1 - winner;
            let core = game
                .units
                .iter()
                .position(|u| matches!(u.kind, Kind::CoreBlue | Kind::CoreRed) && u.team == loser)
                .unwrap();
            let attacker = game
                .units
                .iter()
                .position(|u| u.kind == Kind::Champ && u.team == winner)
                .unwrap();
            let source = game.units[attacker].id;
            game.deal_damage(core, data::CORE_HP + 1.0, 0, source, false);
            game.step();
            assert_eq!(game.phase, Phase::Over);
            assert_eq!(game.winner, winner);
            assert_eq!(game.left, data::RESULT_SECS);
            assert!(game.units[core].dead);
            assert_eq!(game.units[core].hp, 0.0);
            let champion = &game.units[attacker];
            let frozen = (
                champion.x,
                champion.z,
                champion.hp,
                champion.gold,
                champion.level,
            );
            let slot = champion.slot;
            for _ in 0..60 * 13 {
                game.command(slot, Cmd::Move { x: 0.0, z: 0.0 });
                game.command(slot, Cmd::Buy { item: 1 });
                game.step();
            }
            assert!(
                game.left <= 0.0,
                "the server must observe the expired result timer"
            );
            assert_eq!(
                game.phase,
                Phase::Over,
                "the server owns lobby recreation after expiry"
            );
            assert_eq!(game.winner, winner);
            let champion = &game.units[attacker];
            assert_eq!(
                (
                    champion.x,
                    champion.z,
                    champion.hp,
                    champion.gold,
                    champion.level
                ),
                frozen
            );
            assert!(game.pending.is_empty());
            assert!(game.units[core].dead && game.units[core].hp == 0.0);
        }
    }
}

#[test]
#[allow(clippy::print_stdout)]
fn bot_duels_and_squads_reach_a_destroyed_core() {
    for size in [1, 3] {
        for seed in [99, 777, 812, 42, 5] {
            let mut game = Match::new(size, seed);
            game.start();
            for tick in 0..60 * 60 * 30 {
                for slot in 0..size * 2 {
                    if let Some(command) = league_core::ai::think(&game, slot) {
                        game.command(slot, command);
                    }
                }
                game.step();
                if tick % 3 == 0 {
                    game.fx.clear();
                    game.log.clear();
                }
                if game.phase == Phase::Over {
                    break;
                }
            }
            let cores: Vec<_> = game
                .units
                .iter()
                .filter(|u| matches!(u.kind, Kind::CoreBlue | Kind::CoreRed))
                .map(|u| (u.team, u.hp))
                .collect();
            let champions: Vec<_> = game
                .units
                .iter()
                .filter(|u| u.kind == Kind::Champ)
                .map(|u| (u.slot, u.def, u.x, u.hp, u.level, u.points, u.ranks))
                .collect();
            assert_eq!(
                game.phase,
                Phase::Over,
                "{size}v{size} seed {seed} stalemated at 30 minutes; cores={cores:?}, kills={:?}, champions={champions:?}",
                game.kills
            );
            assert!(
                game.units
                    .iter()
                    .any(|u| matches!(u.kind, Kind::CoreBlue | Kind::CoreRed)
                        && u.dead
                        && u.team == 1 - game.winner)
            );
            println!(
                "{size}v{size} seed {seed}: winner {}, game time {:.1} s",
                game.winner,
                f64::from(u32::try_from(game.tick).unwrap()) / 60.0
            );
        }
    }
}
