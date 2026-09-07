use league_core::data;
use league_core::proto::{C2S, Cmd};
use league_core::sim::{Kind, Match, Order, ProjKind};

fn duel(champ: u8) -> Match {
    let mut game = Match::new(1, 777);
    game.join("player");
    game.join("opponent");
    game.set_pick(0, champ, 0, 1, [0, 1, 2]);
    game.set_pick(1, data::KNIGHT, 0, 1, [0, 1, 2]);
    game.start();
    game.wave_left = 1e9;
    for (index, unit) in game.units.iter_mut().take(2).enumerate() {
        unit.x = if index == 0 { 0.0 } else { 4.0 };
        unit.z = 0.0;
        unit.runes = [u8::MAX; 3];
        unit.hp = 100_000.0;
        unit.max_hp = unit.hp;
    }
    game.fx.clear();
    game
}

fn near(actual: f32, expected: f32) {
    assert!((actual - expected).abs() < 0.001, "{actual} != {expected}");
}

fn add_unit(game: &mut Match, kind: Kind, team: u8, x: f32, z: f32) -> usize {
    let mut unit = game.units[1].clone();
    unit.id = game.next_id;
    game.next_id += 1;
    unit.kind = kind;
    unit.team = team;
    unit.x = x;
    unit.z = z;
    if kind == Kind::Clone {
        unit.parent = 0;
        unit.ttl = 60.0;
    }
    let index = game.units.len();
    game.units.push(unit);
    index
}

fn auto_starts(game: &Match, champ: u8) -> usize {
    game.fx
        .iter()
        .filter(|f| f.k == 0 && f.champ == champ && f.ability == 4 && (4.0..8.0).contains(&f.v))
        .count()
}

#[test]
fn attack_move_round_trips_sanitizes_and_clamps_to_the_playable_field() {
    let cmd = C2S::Cmd(Cmd::AttackMove { x: 12.5, z: -3.25 });
    let json = serde_json::to_string(&cmd).unwrap();
    assert_eq!(json, r#"{"t":"cmd","a":"attack_move","x":12.5,"z":-3.25}"#);
    assert_eq!(serde_json::from_str::<C2S>(&json).unwrap(), cmd);
    let mut game = duel(data::SWARM);
    game.command(
        0,
        Cmd::AttackMove {
            x: f32::NAN,
            z: f32::INFINITY,
        },
    );
    game.step();
    near(game.units[0].ox, 0.0);
    near(game.units[0].oz, 0.0);
    game.command(0, Cmd::AttackMove { x: 1e9, z: -1e9 });
    game.step();
    near(game.units[0].ox, league_core::sim::FIELD_X);
    near(game.units[0].oz, -data::FIELD_Z);
}

#[test]
fn ordinary_move_walks_past_enemies_but_attack_move_engages_them() {
    let mut walking = duel(data::SWARM);
    let mut attacking = walking.clone();
    walking.command(0, Cmd::Move { x: 20.0, z: 0.0 });
    attacking.command(0, Cmd::AttackMove { x: 20.0, z: 0.0 });
    walking.step();
    attacking.step();
    assert!(walking.units[0].x > 0.0);
    assert_eq!(auto_starts(&walking, data::SWARM), 0);
    near(attacking.units[0].x, 0.0);
    assert_eq!(attacking.units[0].target, attacking.units[1].id);
    assert_eq!(auto_starts(&attacking, data::SWARM), 1);
    assert_eq!(attacking.projs.len(), 1);
}

#[test]
fn attack_move_acquires_nearest_opponents_and_keeps_a_valid_engagement() {
    let mut game = duel(data::SWARM);
    game.units[1].x = 40.0;
    // All three closer bodies are ineligible for automatic acquisition.
    add_unit(&mut game, Kind::Champ, 0, 0.5, 0.0);
    add_unit(&mut game, Kind::CourtN, 2, 1.0, 0.0);
    add_unit(&mut game, Kind::Clone, 1, 1.5, 0.0);
    let first = add_unit(&mut game, Kind::Champ, 1, 0.0, 4.0);
    let second = add_unit(&mut game, Kind::Champ, 1, 0.0, -4.0);
    let dead = add_unit(&mut game, Kind::Champ, 1, 0.0, 0.25);
    game.units[dead].dead = true;
    game.units[dead].respawn = 30.0;
    game.command(0, Cmd::AttackMove { x: 20.0, z: 0.0 });
    game.step();
    assert_eq!(
        game.units[0].target, game.units[first].id,
        "lowest id wins a tie"
    );
    game.units[second].z = -0.5;
    game.step();
    assert_eq!(
        game.units[0].target, game.units[first].id,
        "do not split an ongoing attack"
    );
    game.units[first].z = 20.0;
    game.step();
    assert_eq!(
        game.units[0].target, game.units[second].id,
        "reacquire after the old target escapes"
    );
}

#[test]
fn attack_move_resumes_its_destination_after_a_kill_or_escape() {
    for killed in [false, true] {
        let mut game = duel(data::SWARM);
        game.command(0, Cmd::AttackMove { x: 20.0, z: 0.0 });
        game.step();
        if killed {
            let source = game.units[0].id;
            game.deal_damage(1, 1e9, 0, source, false);
        } else {
            game.units[1].x = 40.0;
        }
        game.step();
        assert_eq!(game.units[0].order, Order::AttackMove);
        assert_eq!(game.units[0].target, 0);
        assert!(game.units[0].x > 0.0);
        near(game.units[0].ox, 20.0);
    }
}

#[test]
fn attack_move_waits_at_destination_and_new_explicit_orders_replace_it() {
    let mut game = duel(data::SWARM);
    game.units[1].x = 40.0;
    game.command(0, Cmd::AttackMove { x: 0.5, z: 0.0 });
    for _ in 0..30 {
        game.step();
    }
    assert_eq!(game.units[0].order, Order::AttackMove);
    assert!(game.units[0].x <= 0.5);
    let arrived = game.units[0].x;
    game.units[1].x = 4.0;
    game.step();
    assert_eq!(auto_starts(&game, data::SWARM), 1);
    near(game.units[0].x, arrived);
    game.command(0, Cmd::Move { x: -20.0, z: 0.0 });
    game.step();
    assert_eq!(game.units[0].order, Order::Move);
    assert!(game.units[0].x < arrived);
    // Neutral courts are still intentionally attackable with right-click.
    let court = game
        .units
        .iter()
        .find(|u| u.kind == Kind::CourtN)
        .unwrap()
        .id;
    game.command(0, Cmd::Attack { target: court });
    game.step();
    assert_eq!(game.units[0].order, Order::Attack);
    assert_eq!(game.units[0].target, court);
}

#[test]
fn attack_move_fights_minions_and_cores_but_obeys_melee_reach() {
    for kind in [Kind::Melee, Kind::Caster, Kind::CoreRed] {
        let mut game = duel(data::KNIGHT);
        game.units[1].kind = kind;
        game.units[1].x = 7.0;
        let target = game.units[1].id;
        game.command(0, Cmd::AttackMove { x: 20.0, z: 0.0 });
        game.step();
        assert_eq!(game.units[0].target, target);
        assert!(game.units[0].x > 0.0);
        assert_eq!(
            auto_starts(&game, data::KNIGHT),
            0,
            "melee must approach first"
        );
        for _ in 0..90 {
            game.step();
        }
        assert!(auto_starts(&game, data::KNIGHT) > 0);
    }
}

#[test]
fn repeated_attack_move_input_cannot_bypass_any_champions_attack_cooldown() {
    for champ in 0..5 {
        let mut direct = duel(champ);
        direct.units[1].x = 2.0;
        let mut moving = direct.clone();
        direct.command(
            0,
            Cmd::Attack {
                target: direct.units[1].id,
            },
        );
        for _ in 0..180 {
            moving.command(0, Cmd::AttackMove { x: 20.0, z: 0.0 });
            direct.step();
            moving.step();
            near(moving.units[0].atk_cd, direct.units[0].atk_cd);
            near(moving.units[1].hp, direct.units[1].hp);
            assert_eq!(auto_starts(&moving, champ), auto_starts(&direct, champ));
        }
        assert!(auto_starts(&moving, champ) >= 2);
    }
}

#[test]
fn all_twenty_casts_preserve_paid_attack_cooldown_and_released_projectiles() {
    for champ in 0..5 {
        for ability in 0..4 {
            let mut game = duel(champ);
            game.units[1].x = 2.0;
            game.units[0].level = data::MAX_LEVEL;
            game.units[0].ranks = [3; 4];
            game.command(
                0,
                Cmd::Attack {
                    target: game.units[1].id,
                },
            );
            game.step();
            assert_eq!(auto_starts(&game, champ), 1);
            let cooldown = game.units[0].atk_cd;
            let launched = game
                .projs
                .iter()
                .find(|p| p.kind == ProjKind::Auto)
                .cloned();
            game.fx.clear();
            game.command(
                0,
                Cmd::Cast {
                    slot: ability,
                    x: 2.0,
                    z: 0.0,
                },
            );
            game.step();
            assert!(
                game.fx
                    .iter()
                    .any(|f| f.k == 13 && f.champ == champ && f.ability == ability)
            );
            near(game.units[0].atk_cd, cooldown - league_core::DT);
            // SW4RM clones can legitimately attack; the original cannot restart.
            assert!(!game.fx.iter().any(|f| {
                f.k == 0
                    && f.ability == 4
                    && (4.0..8.0).contains(&f.v)
                    && f.x.abs() < 0.001
                    && f.z.abs() < 0.001
            }));
            assert_eq!(game.units[0].order, Order::Attack);
            if let Some(launched) = launched {
                let shot = game
                    .projs
                    .iter()
                    .find(|p| p.id == launched.id)
                    .expect("released shot survives casting");
                near(shot.dmg, launched.dmg);
                near(shot.speed, launched.speed);
                assert!(shot.travel < launched.travel);
            }
        }
    }
}

#[test]
fn casts_keep_movement_intent_and_rejections_do_not_announce_animation_takeover() {
    let mut game = duel(data::SWARM);
    game.units[0].ranks[1] = 1;
    game.command(0, Cmd::Move { x: -20.0, z: 0.0 });
    game.command(
        0,
        Cmd::Cast {
            slot: 1,
            x: 0.0,
            z: 15.0,
        },
    );
    game.step();
    assert!(game.units[0].x < 0.0);
    assert_eq!(game.units[0].order, Order::Move);
    assert!(game.fx.iter().any(|f| f.k == 13));
    let mana = game.units[0].mana;
    let cooldown = game.units[0].cds[1];
    game.fx.clear();
    game.command(
        0,
        Cmd::Cast {
            slot: 1,
            x: 0.0,
            z: 15.0,
        },
    );
    game.step();
    assert!(!game.fx.iter().any(|f| f.k == 13));
    assert!(game.units[0].mana >= mana, "rejection cannot spend mana");
    near(game.units[0].cds[1], cooldown - league_core::DT);
}

#[test]
fn a_released_ranged_auto_still_hits_after_an_ability_takes_over_its_animation() {
    let mut baseline = duel(data::SWARM);
    baseline.units[0].ranks[1] = 1;
    baseline.command(
        0,
        Cmd::Attack {
            target: baseline.units[1].id,
        },
    );
    baseline.step();
    let mut casting = baseline.clone();
    casting.command(
        0,
        Cmd::Cast {
            slot: 1,
            x: 0.0,
            z: 15.0,
        },
    );
    for _ in 0..20 {
        baseline.step();
        casting.step();
    }
    assert!(casting.projs.is_empty());
    assert!(casting.units[1].hp < casting.units[1].max_hp);
    near(casting.units[1].hp, baseline.units[1].hp);
    near(casting.units[0].atk_cd, baseline.units[0].atk_cd);
    assert_eq!(auto_starts(&casting, data::SWARM), 1);
}

#[test]
fn tessera_gear_shot_stops_at_one_enemy_and_has_no_builtin_slow() {
    let mut game = duel(data::TESSERA);
    let behind = add_unit(&mut game, Kind::Champ, 1, 8.0, 0.0);
    game.units[0].ranks[0] = 1;
    game.command(
        0,
        Cmd::Cast {
            slot: 0,
            x: 12.0,
            z: 0.0,
        },
    );
    for _ in 0..30 {
        game.step();
    }
    assert!(game.projs.is_empty());
    assert!(game.units[1].hp < game.units[1].max_hp);
    near(game.units[behind].hp, game.units[behind].max_hp);
    assert!(
        !game.units[1]
            .buffs
            .iter()
            .any(|b| b.k == league_core::sim::BuffKind::Slow)
    );
}
