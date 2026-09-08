use super::*;

fn explorer(position: Vec3) -> Dungeon {
    Dungeon {
        stage: 5,
        exit_open: 1.0,
        warden_health: 0.0,
        position,
        ..Dungeon::default()
    }
}
fn walk_to(s: &mut Dungeon, target: [f32; 3]) {
    s.yaw = 0.0;
    for _ in 0..6000 {
        let delta = Vec2::new(target[0] - s.position.x, target[2] - s.position.z);
        if delta.length() < 0.025 {
            assert!(
                (s.position.y - target[1]).abs() < 0.28,
                "wrong floor {:?} -> {:?}",
                s.position,
                target
            );
            return;
        }
        let movement =
            Vec2::new(delta.x, -delta.y).normalize() * (delta.length() / (2.1 * STEP)).min(1.0);
        s.tick(Controls {
            movement,
            ..Controls::default()
        });
        assert!(!s.finished());
        assert!(s.position.is_finite());
        assert!(
            !layout::castle().occupied(
                s.position.to_array(),
                layout::PLAYER_HEIGHT,
                layout::PLAYER_RADIUS
            ),
            "body inside shared solid at {:?}",
            s.position
        );
    }
    panic!(
        "integrated route blocked at {:?}, target {:?}",
        s.position, target
    );
}

#[test]
fn exit_requires_sword_warden_death_and_a_separate_chain_contact() {
    let mut s = Dungeon {
        stage: 4,
        position: Vec3::new(0.0, 0.0, -5.4),
        ..Dungeon::default()
    };
    s.strike_contact(StrikeKind::Cut);
    assert_eq!(s.stage, 4);
    assert_eq!(s.exit_open, 0.0);
    s.warden = Vec2::new(0.0, -6.0);
    s.warden_health = 1.0;
    s.strike_contact(StrikeKind::Cut);
    assert_eq!(s.warden_health, 0.0);
    assert_eq!(s.stage, 4, "one sample must not kill and break the chain");
    s.stage = 3;
    s.strike_contact(StrikeKind::Cut);
    assert_eq!(s.stage, 3, "unequipped contact cannot unlock");
    s.stage = 4;
    s.strike_contact(StrikeKind::Cut);
    assert_eq!(s.stage, 5);
    assert_eq!(s.exit_open, 0.0);
    let frozen = s.combat.hitstop_left;
    s.tick(Controls::default());
    assert!(s.combat.hitstop_left < frozen);
    assert_eq!(s.exit_open, 0.0);
    for _ in 0..100 {
        s.tick(Controls::default());
    }
    assert_eq!(s.exit_open, 1.0);
    assert!(s.escaped());
    assert!(!s.finished());
}

#[test]
fn closed_and_partly_raised_far_gate_block_until_the_body_fits() {
    let mut s = Dungeon {
        stage: 4,
        warden_health: 0.0,
        position: Vec3::new(0.0, 0.0, -5.4),
        ..Dungeon::default()
    };
    for _ in 0..100 {
        s.tick(Controls {
            movement: Vec2::Y,
            ..Controls::default()
        });
    }
    assert!(s.position.z >= -6.63 - 0.001);
    assert_eq!(s.exit_open, 0.0);
    s.stage = 5;
    for _ in 0..20 {
        s.tick(Controls {
            movement: Vec2::Y,
            ..Controls::default()
        });
    }
    assert!(s.exit_open > 0.0 && s.exit_open * EXIT_GATE_LIFT < layout::PLAYER_HEIGHT);
    assert!(s.position.z >= -6.63 - 0.001);
    for _ in 0..120 {
        s.tick(Controls {
            movement: Vec2::Y,
            ..Controls::default()
        });
    }
    assert!(s.position.z < -8.0);
    assert!(s.grounded);
    assert_eq!(s.position.y, 0.0);
    walk_to(&mut s, [0.0, 0.0, -6.3]);
    assert_eq!(s.location(), "Basement");
}

#[test]
fn complete_castle_route_and_perimeter_are_walkable_in_both_directions() {
    let mut s = explorer(Vec3::from_array(layout::ROUTE[0]));
    for (i, p) in layout::ROUTE.iter().enumerate().skip(1) {
        walk_to(&mut s, *p);
        if i == 10 {
            for p in [
                [24., 14., -87.5],
                [24., 14., -65.],
                [-24., 14., -65.],
                [-24., 14., -101.],
                [24., 14., -101.],
                [24., 14., -87.5],
                [21.75, 14., -87.5],
            ] {
                walk_to(&mut s, p);
            }
        }
    }
    assert_eq!(s.location(), "Tower summit");
    assert_eq!(s.explored_count(), TOTAL_EXPLORE_COUNT);
    assert!(s.grounded);
    for p in layout::ROUTE.iter().rev().skip(1) {
        walk_to(&mut s, *p);
    }
    assert_eq!(s.location(), "Lower passage");
    assert_eq!(s.position.y, 0.0);
    let count = s.explored_count();
    walk_to(&mut s, [0., 0., -6.3]);
    assert_eq!(s.location(), "Basement");
    assert_eq!(s.explored_count(), count);
    assert!(!s.finished());
}

#[test]
fn jump_guard_and_footsteps_use_the_elevated_support_surface() {
    let mut s = explorer(Vec3::new(0.0, 6.0, -70.0));
    s.tick(Controls {
        jump: true,
        ..Controls::default()
    });
    assert!(s.position.y > 6.0 && !s.grounded);
    s.tick(Controls {
        block: true,
        ..Controls::default()
    });
    assert_eq!(s.guard.amount, 0.0);
    for _ in 0..80 {
        s.tick(Controls::default());
    }
    assert_eq!(s.position.y, 6.0);
    assert!(s.grounded);
    for _ in 0..12 {
        s.tick(Controls {
            block: true,
            ..Controls::default()
        });
    }
    assert!(s.guard.ready());
    let steps = s.footsteps;
    let stamina = s.stamina;
    for _ in 0..60 {
        s.tick(Controls {
            movement: Vec2::Y,
            block: true,
            ..Controls::default()
        });
    }
    assert!(s.footsteps > steps);
    assert_eq!(s.stamina, stamina);
    assert_eq!(s.position.y, 6.0);
    assert_eq!(s.location(), "Backyard garden");
}

#[test]
fn standing_under_wallwalk_never_selects_its_overhead_floor() {
    let mut s = explorer(Vec3::new(24., 6., -80.));
    for _ in 0..120 {
        s.tick(Controls::default());
    }
    assert_eq!(s.position.y, 6.);
    assert!(s.grounded);
    assert_eq!(s.location(), "Backyard garden");
}

#[test]
fn an_out_of_world_fall_uses_the_existing_respawn_path() {
    let mut s = explorer(Vec3::new(40., -7.9, -90.));
    s.velocity_y = -12.;
    s.grounded = false;
    let life = s.dialogue.life;
    s.tick(Controls::default());
    assert_eq!(s.stage, 0);
    assert_eq!(s.health, 100.);
    assert_eq!(s.position, Dungeon::default().position);
    assert_eq!(s.dialogue.life, life + 1);
    assert_eq!(s.explored_count(), 0);
}
