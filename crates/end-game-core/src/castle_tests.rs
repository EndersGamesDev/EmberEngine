use super::*;
use enemies::{CastleEventKind, EnemyAttack, EnemyKind, EnemyPhase};
use quest::{QuestKind, SITES};

fn castle(position: Vec3) -> Dungeon {
    Dungeon {
        stage: 5,
        exit_open: 1.,
        gate_open: 1.,
        warden_health: 0.,
        position,
        enemies: Vec::new(),
        ..Dungeon::default()
    }
}
fn with_enemy(kind: EnemyKind, attack: EnemyAttack) -> Dungeon {
    let mut s = castle(Vec3::new(0., 0., -30.));
    let mut e = Enemy::new(0, kind, Vec3::new(0., 0., -31.4));
    e.alerted = true;
    e.attack = attack;
    e.start_attack(s.position);
    e.attack = attack;
    s.enemies.push(e);
    s
}
fn ticks(s: &mut Dungeon, n: usize, input: Controls) {
    for _ in 0..n {
        s.tick(input);
    }
}
fn position_at(s: &mut Dungeon, kind: QuestKind) {
    let site = SITES.iter().find(|q| q.kind == kind).unwrap();
    s.position = match kind {
        QuestKind::Inscription => Vec3::new(0., 0., -23.),
        QuestKind::Sun => Vec3::new(-7., 6., -77.9),
        QuestKind::Wolf => Vec3::new(-24., 14., -82.),
        QuestKind::Bell => Vec3::new(21., 22., -97.),
        QuestKind::SallyPort => Vec3::new(-24., 6., -80.),
        QuestKind::Shrine(_) => Vec3::new(
            site.position.x,
            site.position.y - 0.8,
            site.position.z + 0.4,
        ),
    };
    s.grounded = true;
    s.velocity_y = 0.;
}

#[test]
fn authored_roster_has_eight_normals_and_one_boss_in_clear_grounded_positions() {
    let enemies = enemies::authored_enemies();
    assert_eq!(enemies.len(), 9);
    assert_eq!(
        enemies
            .iter()
            .filter(|e| e.kind == EnemyKind::Cyclops)
            .count(),
        1
    );
    for e in enemies {
        assert!(
            !layout::castle().occupied(e.position.to_array(), e.kind.height(), e.kind.radius()),
            "spawn {}",
            e.id
        );
        assert_eq!(
            layout::castle().support_below(
                e.position.x,
                e.position.z,
                e.position.y + 0.001,
                e.kind.radius()
            ),
            Some(e.position.y)
        );
    }
}
#[test]
fn each_enemy_attack_has_one_contact_after_a_readable_windup_and_long_recovery() {
    for attack in [
        EnemyAttack::Slash,
        EnemyAttack::Thrust,
        EnemyAttack::Chop,
        EnemyAttack::Sweep,
        EnemyAttack::Slam,
    ] {
        assert!(attack.windup_time() >= 0.45);
        assert!(attack.contact_time() > attack.windup_time());
        assert!(attack.duration() - attack.follow_end() >= 0.7);
        let kind = if matches!(attack, EnemyAttack::Sweep | EnemyAttack::Slam) {
            EnemyKind::Cyclops
        } else {
            EnemyKind::SwordSoldier
        };
        let mut s = with_enemy(kind, attack);
        s.quest.checkpoint = true;
        let contact_ticks = (attack.contact_time() / STEP).ceil() as usize;
        ticks(&mut s, contact_ticks - 1, Controls::default());
        assert_eq!(s.health, 100., "early {:?}", attack);
        s.tick(Controls::default());
        assert_eq!(s.health, 100. - attack.damage(), "contact {:?}", attack);
        let health = s.health;
        ticks(&mut s, 25, Controls::default());
        assert_eq!(s.health, health, "duplicate {:?}", attack);
    }
}
#[test]
fn every_guardable_castle_weapon_uses_its_cost_and_never_health_on_a_ready_front_guard() {
    for attack in [
        EnemyAttack::Slash,
        EnemyAttack::Thrust,
        EnemyAttack::Chop,
        EnemyAttack::Sweep,
    ] {
        let mut s = with_enemy(EnemyKind::SwordSoldier, attack);
        s.guard.amount = 1.;
        ticks(
            &mut s,
            (attack.contact_time() / STEP).ceil() as usize,
            Controls {
                block: true,
                ..Controls::default()
            },
        );
        assert_eq!(s.health, 100.);
        assert_eq!(s.guard.block_event, 1);
        assert!((s.stamina - (100. - attack.block_cost())).abs() < 0.01);
    }
}
#[test]
fn low_stamina_break_back_facing_and_boss_slam_are_distinct_from_a_block() {
    let mut s = with_enemy(EnemyKind::SwordSoldier, EnemyAttack::Slash);
    s.guard.amount = 1.;
    s.stamina = 1.;
    ticks(
        &mut s,
        34,
        Controls {
            block: true,
            ..Controls::default()
        },
    );
    assert_eq!(s.health, 88.);
    assert_eq!(s.guard.break_event, 1);
    let mut s = with_enemy(EnemyKind::SwordSoldier, EnemyAttack::Slash);
    s.yaw = std::f32::consts::PI;
    s.guard.amount = 1.;
    ticks(
        &mut s,
        34,
        Controls {
            block: true,
            ..Controls::default()
        },
    );
    assert_eq!(s.health, 88.);
    assert_eq!(s.guard.block_event, 0);
    let mut s = with_enemy(EnemyKind::Cyclops, EnemyAttack::Slam);
    s.guard.amount = 1.;
    ticks(
        &mut s,
        77,
        Controls {
            block: true,
            ..Controls::default()
        },
    );
    assert_eq!(s.health, 66.);
    assert_eq!(s.guard.block_event, 0);
}
#[test]
fn dodge_at_slam_contact_and_leaving_locked_thrust_direction_avoid_damage() {
    let mut s = with_enemy(EnemyKind::Cyclops, EnemyAttack::Slam);
    ticks(&mut s, 73, Controls::default());
    s.tick(Controls {
        dodge: true,
        movement: Vec2::X,
        ..Controls::default()
    });
    ticks(&mut s, 5, Controls::default());
    assert_eq!(s.health, 100.);
    let mut s = with_enemy(EnemyKind::SpearSoldier, EnemyAttack::Thrust);
    let yaw = s.enemies[0].yaw;
    ticks(&mut s, 20, Controls::default());
    s.position = Vec3::new(1.5, 0., -31.4);
    ticks(&mut s, 30, Controls::default());
    assert_eq!(s.enemies[0].yaw, yaw);
    assert_eq!(s.health, 100.);
}
#[test]
fn castle_weapons_respect_solid_los_and_height() {
    let mut s = with_enemy(EnemyKind::SwordSoldier, EnemyAttack::Slash);
    s.position = Vec3::new(0., 6., -31.4);
    ticks(&mut s, 34, Controls::default());
    assert_eq!(s.health, 100.);
    assert!(!enemies::line_clear(
        Vec3::new(13., 1., -35.),
        Vec3::new(15., 1., -35.),
        &[]
    ));
    assert!(!enemies::line_clear(
        Vec3::new(-24., 7., -75.),
        Vec3::new(-26., 7., -75.),
        &[]
    ));
    assert!(enemies::line_clear(
        Vec3::new(0., 1., -30.),
        Vec3::new(0., 1., -40.),
        &[]
    ));
}
#[test]
fn contact_selects_only_nearest_visible_enemy_and_commits_once() {
    let mut s = castle(Vec3::new(0., 0., -30.));
    s.enemies = vec![
        Enemy::new(0, EnemyKind::SwordSoldier, Vec3::new(0., 0., -31.4)),
        Enemy::new(1, EnemyKind::SwordSoldier, Vec3::new(0.8, 0., -32.)),
    ];
    s.tick(Controls {
        attack: true,
        ..Controls::default()
    });
    ticks(&mut s, 22, Controls::default());
    assert_eq!(s.enemies[0].health, 42.);
    assert_eq!(s.enemies[1].health, 70.);
    assert_eq!(s.combat.impact_kind, Some(ImpactKind::Enemy));
    ticks(&mut s, 12, Controls::default());
    assert_eq!(s.enemies[0].health, 42.);
}
#[test]
fn heavy_hits_cancel_normal_windups_and_dead_pose_retains_interrupted_weight() {
    let mut e = Enemy::new(0, EnemyKind::HollowAxeKnight, Vec3::ZERO);
    e.start_attack(Vec3::NEG_Z);
    e.elapsed = 0.6;
    e.take_hit(20., true);
    assert_eq!(e.phase, EnemyPhase::Staggered);
    e.elapsed = 0.2;
    let before = e.attack_pose().unwrap();
    e.take_hit(999., false);
    let after = e.attack_pose().unwrap();
    assert_eq!(before, after);
    assert_eq!(e.phase, EnemyPhase::Dead);
}
#[test]
fn encounter_director_commits_at_most_one_attacker_and_rotates_turns() {
    let mut s = castle(Vec3::new(0., 0., -30.));
    s.quest.checkpoint = true;
    s.enemies = vec![
        Enemy::new(0, EnemyKind::SwordSoldier, Vec3::new(-0.8, 0., -31.)),
        Enemy::new(1, EnemyKind::SwordSoldier, Vec3::new(0.8, 0., -31.)),
    ];
    for _ in 0..700 {
        s.health = 100.;
        s.tick(Controls::default());
        assert!(
            s.enemies
                .iter()
                .filter(|e| e.phase == EnemyPhase::Attacking)
                .count()
                <= 1
        );
        for e in &s.enemies {
            assert!(!layout::castle().occupied(
                e.position.to_array(),
                e.kind.height(),
                e.kind.radius()
            ));
        }
    }
    assert!(s.enemies.iter().all(|e| e.attack_event > 0));
}
#[test]
fn boss_alternates_patterns_enters_phase_two_once_and_drops_the_crown() {
    let mut s = castle(Vec3::new(0., 0., -42.));
    let mut boss = Enemy::new(8, EnemyKind::Cyclops, Vec3::new(0., 0., -44.));
    boss.start_attack(s.position);
    assert_eq!(boss.attack, EnemyAttack::Sweep);
    boss.start_attack(s.position);
    assert_eq!(boss.attack, EnemyAttack::Slam);
    s.enemies.push(boss);
    s.enemies[0].health = 181.;
    // Past its windup, so the cut lands rather than being turned aside.
    s.enemies[0].elapsed = EnemyAttack::Slam.windup_time();
    s.strike_contact(StrikeKind::Cut);
    assert!(s.enemies[0].phase_two);
    let phase_events = s
        .castle_events
        .events()
        .filter(|e| e.kind == CastleEventKind::BossPhaseTwo)
        .count();
    assert_eq!(phase_events, 1);
    s.strike_contact(StrikeKind::Cut);
    assert_eq!(
        s.castle_events
            .events()
            .filter(|e| e.kind == CastleEventKind::BossPhaseTwo)
            .count(),
        1
    );
    s.enemies[0].health = 1.;
    s.strike_contact(StrikeKind::Cut);
    assert!(s.quest.boss_defeated);
    assert!(s.crown_position().is_some());
    assert!(!s.quest.seal);
    s.combat.cancel();
    s.position = Vec3::new(0., 0., -43.);
    s.interact();
    assert!(s.quest.seal);
    assert!(s.crown_position().is_none());
}
#[test]
fn clue_is_rereadable_order_is_enforced_and_incorrect_future_seal_resets_safely() {
    let mut s = castle(Vec3::ZERO);
    position_at(&mut s, QuestKind::Inscription);
    s.interact();
    s.interact();
    assert!(s.quest.inscription_read);
    assert_eq!(
        s.castle_events
            .events()
            .filter(|e| e.kind == CastleEventKind::Clue)
            .count(),
        1
    );
    position_at(&mut s, QuestKind::Wolf);
    s.interact();
    assert_eq!(s.quest.sequence, 0);
    assert!(s.message.contains("Begin again"));
    position_at(&mut s, QuestKind::Sun);
    assert!(!s.prompt().is_empty());
    s.interact();
    assert_eq!(s.quest.sequence, 1);
    position_at(&mut s, QuestKind::Bell);
    s.interact();
    assert_eq!(s.quest.sequence, 0);
    for kind in [QuestKind::Sun, QuestKind::Wolf, QuestKind::Bell] {
        position_at(&mut s, kind);
        s.interact();
    }
    assert_eq!(s.quest.sequence, 3);
    assert!(
        s.quest
            .journal()
            .iter()
            .any(|s| s.contains("Sun at the dry"))
    );
}
#[test]
fn healing_shrines_are_finite_need_injury_and_cannot_be_double_spent() {
    let mut s = castle(Vec3::ZERO);
    position_at(&mut s, QuestKind::Shrine(0));
    s.interact();
    assert_eq!(s.quest.healing_left(), 3);
    s.health = 25.;
    s.interact();
    assert_eq!(s.health, 100.);
    assert_eq!(s.quest.healing_left(), 2);
    s.health = 25.;
    s.interact();
    assert_eq!(s.health, 25.);
    assert_eq!(s.quest.healing_left(), 2);
}
#[test]
fn checkpoint_retains_defeats_clues_seals_and_spent_shrines_but_restores_survivors() {
    let mut s = castle(quest::CHECKPOINT);
    s.tick(Controls::default());
    assert!(s.quest.checkpoint);
    s.enemies = vec![
        Enemy::new(0, EnemyKind::SwordSoldier, Vec3::new(0., 0., -18.)),
        Enemy::new(1, EnemyKind::SpearSoldier, Vec3::new(5., 0., -29.)),
    ];
    s.enemies[0].take_hit(999., true);
    s.enemies[1].health = 5.;
    s.quest.sequence = 2;
    s.quest.seal = true;
    s.quest.shrines_used = 3;
    s.health = 0.;
    s.tick(Controls::default());
    assert_eq!(s.position, quest::CHECKPOINT);
    assert_eq!(s.health, 100.);
    assert_eq!(s.stamina, 100.);
    assert_eq!(s.stage, 5);
    assert!(!s.enemies[0].alive());
    assert_eq!(s.enemies[1].health, 75.);
    assert_eq!(s.quest.sequence, 2);
    assert!(s.quest.seal);
    assert_eq!(s.quest.shrines_used, 3);
}
#[test]
fn final_gate_requires_both_keys_has_physical_clearance_and_only_walking_out_finishes() {
    let mut s = castle(Vec3::new(-24., 6., -80.));
    s.quest.checkpoint = true;
    s.interact();
    assert!(!s.quest.gate_unlocked);
    s.quest.sequence = 3;
    s.interact();
    assert!(!s.quest.gate_unlocked);
    s.quest.seal = true;
    s.interact();
    assert!(s.quest.gate_unlocked);
    assert!(!s.finished());
    assert!(layout::occupied_with(
        layout::castle(),
        &[s.quest.gate_bounds()],
        [-25.3, 6., -80.],
        1.7,
        0.22
    ));
    ticks(&mut s, 120, Controls::default());
    assert!(!s.finished());
    assert!(!layout::occupied_with(
        layout::castle(),
        &[s.quest.gate_bounds()],
        [-25.3, 6., -80.],
        1.7,
        0.22
    ));
    s.yaw = 0.;
    ticks(
        &mut s,
        75,
        Controls {
            movement: Vec2::NEG_X,
            ..Controls::default()
        },
    );
    assert!(s.position.x < -26. && s.position.x > -28.);
    assert!(!s.finished());
    ticks(
        &mut s,
        55,
        Controls {
            movement: Vec2::X,
            ..Controls::default()
        },
    );
    assert!(s.position.x > -25.6);
    assert!(!s.finished());
    ticks(
        &mut s,
        180,
        Controls {
            movement: Vec2::NEG_X,
            ..Controls::default()
        },
    );
    assert!(s.finished());
    assert_eq!(
        s.castle_events
            .events()
            .filter(|e| e.kind == CastleEventKind::Escaped)
            .count(),
        1
    );
}
#[test]
fn serial_event_history_is_bounded_and_keeps_audio_keys_and_time() {
    let mut events = CastleEvents::default();
    for i in 0..40 {
        events.emit(CastleEventKind::EnemyHit, i as f32 * STEP, Vec3::ZERO);
    }
    let rows: Vec<_> = events.events().collect();
    assert_eq!(rows.len(), 24);
    assert_eq!(rows[0].id, 17);
    assert_eq!(rows[23].id, 40);
    assert_eq!(CastleEventKind::BossAwaken.key(), "boss_intro");
    assert_eq!(CastleEventKind::Clue.key(), "escape_clue");
    assert_eq!(CastleEventKind::Escaped.key(), "escape_ending");
}

fn walk(s: &mut Dungeon, target: [f32; 3]) {
    s.yaw = 0.;
    for _ in 0..5000 {
        let d = Vec2::new(target[0] - s.position.x, target[2] - s.position.z);
        if d.length() < 0.025 {
            assert!(
                (s.position.y - target[1]).abs() < 0.28,
                "floor {:?} -> {target:?}",
                s.position
            );
            return;
        }
        let input = Vec2::new(d.x, -d.y).normalize_or_zero() * (d.length() / (2.1 * STEP)).min(1.);
        s.tick(Controls {
            movement: input,
            ..Controls::default()
        });
        assert!(
            s.health > 0. && s.position.y > -1.,
            "route left the floor {:?}",
            s.position
        );
        if s.finished() {
            return;
        }
    }
    panic!("blocked at {:?} toward {target:?}", s.position);
}

#[test]
fn full_quest_walk_reaches_each_visible_seal_and_returns_down_the_same_stairs_to_escape() {
    // Combat has separate contact/guard tests. This starts with defeated normal
    // guards and makes the final boss kill at its actual hall drop location.
    let mut s = castle(quest::CHECKPOINT);
    walk(&mut s, [0., 0., -23.]);
    s.interact();
    assert!(s.quest.inscription_read);
    walk(&mut s, [0., 0., -42.]);
    let mut boss = Enemy::new(8, EnemyKind::Cyclops, Vec3::new(0., 0., -44.));
    boss.health = 1.;
    s.enemies.push(boss);
    s.tick(Controls {
        attack: true,
        ..Controls::default()
    });
    ticks(&mut s, 70, Controls::default());
    assert!(s.quest.boss_defeated);
    walk(&mut s, [0., 0., -43.]);
    s.interact();
    assert!(s.quest.seal);
    walk(&mut s, [0., 0., -53.5]);
    walk(&mut s, [0., 6., -65.]);
    walk(&mut s, [-7., 6., -77.9]);
    assert!(!layout::castle().occupied(s.position.to_array(), 1.7, 0.22));
    assert_eq!(s.quest_site().map(|s| s.kind), Some(QuestKind::Sun));
    s.interact();
    assert_eq!(s.quest.sequence, 1);
    for p in [
        [0., 6., -77.9],
        [0., 6., -87.5],
        [13.5, 6., -87.5],
        [16., 6., -87.5],
        [18.5, 6., -87.5],
        [18.5, 10., -97.],
        [21.75, 10., -97.],
        [21.75, 14., -87.5],
        [24., 14., -87.5],
        [24., 14., -65.],
        [-24., 14., -65.],
        [-24., 14., -82.],
    ] {
        walk(&mut s, p);
    }
    assert_eq!(s.quest_site().map(|s| s.kind), Some(QuestKind::Wolf));
    s.interact();
    assert_eq!(s.quest.sequence, 2);
    for p in [
        [-24., 14., -101.],
        [24., 14., -101.],
        [24., 14., -87.5],
        [18.5, 14., -87.5],
        [18.5, 18., -97.],
        [21.75, 18., -97.],
        [21.75, 22., -87.5],
        [16., 22., -87.5],
        [16., 22., -97.],
        [21., 22., -97.],
    ] {
        walk(&mut s, p);
    }
    s.interact();
    assert_eq!(s.quest.sequence, 3);
    assert!(!s.finished());
    for p in [
        [16., 22., -97.],
        [16., 22., -87.5],
        [21.75, 22., -87.5],
        [21.75, 18., -97.],
        [18.5, 18., -97.],
        [18.5, 14., -87.5],
        [21.75, 14., -87.5],
        [21.75, 10., -97.],
        [18.5, 10., -97.],
        [18.5, 6., -87.5],
        [16., 6., -87.5],
        [13.5, 6., -87.5],
        [0., 6., -87.5],
        [-20., 6., -87.5],
        [-24., 6., -80.],
    ] {
        walk(&mut s, p);
    }
    s.interact();
    assert!(s.quest.gate_unlocked);
    ticks(&mut s, 120, Controls::default());
    walk(&mut s, [-29., 6., -80.]);
    assert!(s.finished());
    assert_eq!(s.explored_count(), 7);
}

#[test]
fn enemy_hitstop_freezes_pending_contacts_and_quest_motion() {
    let mut s = with_enemy(EnemyKind::SwordSoldier, EnemyAttack::Slash);
    s.enemies[0].elapsed = EnemyAttack::Slash.contact_time() - STEP;
    s.quest.gate_unlocked = true;
    s.combat.hitstop_left = 6. * STEP;
    let before = s.enemies[0].elapsed;
    ticks(&mut s, 6, Controls::default());
    assert_eq!(s.enemies[0].elapsed, before);
    assert_eq!(s.health, 100.);
    assert_eq!(s.quest.gate_open, 0.);
    s.tick(Controls::default());
    assert_eq!(s.health, 88.);
    assert!(s.quest.gate_open > 0.);
}

#[test]
fn focus_and_crown_cannot_reach_through_a_wall() {
    let mut s = castle(Vec3::new(13.7, 0., -35.));
    s.yaw = std::f32::consts::FRAC_PI_2;
    let mut e = Enemy::new(8, EnemyKind::Cyclops, Vec3::new(15., 0., -35.));
    e.alerted = true;
    s.enemies.push(e);
    assert!(s.focus_enemy().is_none());
    s.enemies[0].take_hit(999., true);
    s.interact();
    assert!(!s.quest.seal);
}

#[test]
fn focus_prioritizes_a_visible_boss_windup_then_returns_to_the_nearer_guard() {
    let mut s = castle(Vec3::new(0., 0., -30.));
    let mut guard = Enemy::new(0, EnemyKind::SwordSoldier, Vec3::new(0., 0., -31.4));
    let mut boss = Enemy::new(8, EnemyKind::Cyclops, Vec3::new(1., 0., -33.));
    guard.alerted = true;
    boss.alerted = true;
    s.enemies = vec![guard, boss];
    assert_eq!(s.focus_enemy().unwrap().id, 0);
    s.enemies[1].start_attack(s.position);
    s.enemies[1].attack = EnemyAttack::Slam;
    for elapsed in [0., EnemyAttack::Slam.contact_time() - STEP] {
        s.enemies[1].elapsed = elapsed;
        assert_eq!(s.focus_enemy().unwrap().id, 8);
    }
    for elapsed in [
        EnemyAttack::Slam.contact_time(),
        EnemyAttack::Slam.follow_end(),
    ] {
        s.enemies[1].elapsed = elapsed;
        assert_eq!(s.focus_enemy().unwrap().id, 0);
    }
}

#[test]
fn shrine_does_not_steal_committed_sword_motion_and_checkpoint_handles_a_later_void_fall() {
    let mut s = castle(quest::CHECKPOINT);
    s.tick(Controls::default());
    position_at(&mut s, QuestKind::Shrine(1));
    s.health = 20.;
    s.combat.active = Some(Strike::new(StrikeKind::Cut));
    s.interact();
    assert_eq!(s.health, 20.);
    assert_eq!(s.quest.healing_left(), 3);
    s.combat.cancel();
    s.interact();
    assert_eq!(s.health, 100.);
    assert_eq!(s.quest.healing_left(), 2);
    s.position = Vec3::new(40., -7.9, -90.);
    s.velocity_y = -12.;
    s.grounded = false;
    s.tick(Controls::default());
    assert_eq!(s.position, quest::CHECKPOINT);
    assert_eq!(s.quest.healing_left(), 2);
    assert_eq!(s.stage, 5);
}

#[test]
fn tapping_guard_as_a_castle_weapon_lands_deflects_it_and_staggers_the_attacker() {
    let mut s = with_enemy(EnemyKind::SwordSoldier, EnemyAttack::Slash);
    while s.enemies[0].elapsed + STEP + 0.00001 < EnemyAttack::Slash.contact_time() {
        s.tick(Controls::default());
    }
    // The next tick is the one the blade lands on: tap guard into it.
    s.tick(Controls {
        block: true,
        ..Controls::default()
    });
    assert!(s.enemies[0].contact_done);
    assert_eq!(s.guard.parry_event, 1);
    assert_eq!(s.guard.block_event, 0);
    assert_eq!(s.health, 100.);
    assert!((s.stamina - (100. - guard::PARRY_STAMINA_COST)).abs() < 0.01);
    assert_eq!(s.enemies[0].phase, EnemyPhase::Staggered);
}

#[test]
fn a_sword_soldier_turns_a_cut_aside_only_during_its_windup() {
    let mut e = Enemy::new(0, EnemyKind::SwordSoldier, Vec3::ZERO);
    e.start_attack(Vec3::new(0., 0., 2.));
    assert!(e.can_parry());
    e.elapsed = EnemyAttack::Slash.windup_time();
    assert!(!e.can_parry(), "a committed swing cannot also parry");
    e.elapsed = 0.;
    e.parry_player();
    assert_eq!(e.parry_event, 1);
    assert_eq!(e.phase, EnemyPhase::Hunting);
    assert!(!e.can_parry(), "one deflection buys the player a window");
}

#[test]
fn a_spear_soldier_and_a_hollow_knight_never_parry() {
    for kind in [EnemyKind::SpearSoldier, EnemyKind::HollowAxeKnight] {
        let mut e = Enemy::new(0, kind, Vec3::ZERO);
        e.start_attack(Vec3::new(0., 0., 2.));
        assert!(!e.can_parry(), "{kind:?} has no blade to turn a cut with");
    }
}

#[test]
fn a_cut_into_a_sword_soldiers_windup_is_deflected_without_a_wound() {
    let mut s = with_enemy(EnemyKind::SwordSoldier, EnemyAttack::Slash);
    let health = s.enemies[0].health;
    let reaction = HitReaction {
        id: 1,
        time: 0.,
        zone: HitZone::LeftTorso,
        point: Vec3::ZERO,
        direction: Vec3::Z,
        strength: 1.,
        elapsed: 0.,
        origin: [Vec3::ZERO; 5],
    };
    assert!(s.enemies[0].can_parry());
    s.apply_enemy_sword_hit(0, StrikeKind::Cut, Vec3::ZERO, reaction);
    assert_eq!(s.enemies[0].health, health, "a parried cut draws no blood");
    assert_eq!(s.enemies[0].parry_event, 1);
    assert_eq!(s.enemies[0].hit_event, 0);
    assert!(
        s.castle_events
            .events()
            .any(|e| e.kind == CastleEventKind::EnemyParry)
    );
}
