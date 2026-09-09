use crate::{
    Controls, Dungeon, Guard, STEP, WardenPhase,
    guard::{BLOCK_COST, PARRY_STAMINA_COST, PARRY_WINDOW},
    warden::KNIFE_CONTACT,
};
use glam::{Vec2, Vec3};

fn duel() -> Dungeon {
    let mut game = Dungeon::default();
    game.stage = 4;
    game.position = Vec3::new(-2.9, 0.0, -2.55);
    game.warden_ai.phase = WardenPhase::Attacking;
    game
}

fn contact(game: &mut Dungeon, held: bool) {
    game.warden_ai.elapsed = KNIFE_CONTACT - STEP;
    game.tick(Controls {
        block: held,
        ..Controls::default()
    });
}

#[test]
fn front_guard_catches_one_contact_and_pays_stamina_instead_of_health() {
    let mut game = duel();
    for _ in 0..11 {
        game.tick(Controls {
            block: true,
            ..Controls::default()
        });
    }
    assert!(game.guard.ready());
    contact(&mut game, true);
    assert_eq!(game.health, 100.0);
    assert_eq!(game.stamina, 72.0);
    assert_eq!(game.guard.block_event, 1);
    assert_eq!(
        game.guard.parry_event, 0,
        "a held guard blocks, it does not parry"
    );
    assert_eq!(game.warden_ai.hit_event, 0);
    assert!(game.combat.hitstop_left > 0.0);
    for _ in 0..30 {
        game.tick(Controls {
            block: true,
            ..Controls::default()
        });
    }
    assert_eq!(game.guard.block_event, 1);
    assert_eq!(game.stamina, 72.0);
    assert_eq!(game.health, 100.0);
}

#[test]
fn guard_does_not_cover_back_sides_or_a_view_aimed_away() {
    for (yaw, pitch) in [(std::f32::consts::PI, 0.0), (1.57, 0.0), (0.0, 1.15)] {
        let mut game = duel();
        game.guard.amount = 1.0;
        game.yaw = yaw;
        game.pitch = pitch;
        contact(&mut game, true);
        assert_eq!(game.health, 85.0);
        assert_eq!(game.guard.block_event, 0);
        assert_eq!(game.warden_ai.hit_event, 1);
    }
}

#[test]
fn late_released_unequipped_and_airborne_guards_do_not_stop_the_knife() {
    for case in 0..4 {
        let mut game = duel();
        game.guard.amount = if case == 0 { 0.0 } else { 1.0 };
        if case == 2 {
            game.stage = 3;
        }
        if case == 3 {
            game.position.y = 0.1;
        }
        // Already holding: this is a guard that cannot block, not a parry.
        game.block_held = true;
        contact(&mut game, case != 1);
        assert_eq!(game.health, 85.0, "case {case}");
        assert_eq!(game.guard.block_event, 0);
    }
}

#[test]
fn insufficient_stamina_breaks_once_and_cannot_immediately_rearm() {
    let mut game = duel();
    game.guard.amount = 1.0;
    game.stamina = BLOCK_COST - 1.0;
    game.block_held = true;
    contact(&mut game, true);
    assert_eq!(game.health, 85.0);
    assert_eq!(game.stamina, 0.0);
    assert_eq!(game.guard.break_event, 1);
    assert!(!game.guard.ready());
    for _ in 0..45 {
        game.tick(Controls {
            block: true,
            ..Controls::default()
        });
        assert!(!game.guard.ready());
    }
    assert_eq!(game.guard.break_event, 1);
    assert!(game.stamina > 0.0);
}

#[test]
fn exact_cost_still_defends_and_lowering_recovers_stamina() {
    let mut game = duel();
    game.guard.amount = 1.0;
    game.stamina = BLOCK_COST;
    game.block_held = true;
    contact(&mut game, true);
    assert_eq!(game.health, 100.0);
    assert_eq!(game.stamina, 0.0);
    assert_eq!(game.guard.break_event, 0);
    for _ in 0..30 {
        game.tick(Controls::default());
    }
    assert_eq!(game.guard.amount, 0.0);
    assert!(game.stamina > 0.0);
}

#[test]
fn guard_slows_movement_prevents_new_strikes_and_waits_for_committed_combos() {
    let mut game = duel();
    game.warden_health = 0.0;
    game.stamina = 50.0;
    let start = game.position;
    for _ in 0..60 {
        game.tick(Controls {
            movement: Vec2::X,
            sprint: true,
            attack: true,
            block: true,
            ..Controls::default()
        });
    }
    assert!((game.position.distance(start) - 1.0).abs() < 0.001);
    assert_eq!(game.stamina, 50.0);
    assert_eq!(game.combat.swing_event, 0);
    let mut game = duel();
    game.warden_health = 0.0;
    for _ in 0..3 {
        game.tick(Controls {
            attack: true,
            ..Controls::default()
        });
    }
    for _ in 0..220 {
        game.tick(Controls {
            block: true,
            ..Controls::default()
        });
        if !game.combat.finished() {
            assert!(!game.guard.ready());
        }
    }
    assert_eq!(game.combat.swing_event, 3);
    assert!(game.guard.ready());
}

#[test]
fn impact_freezes_guard_motion_and_respawn_clears_old_feedback() {
    let mut game = duel();
    game.guard.amount = 1.0;
    contact(&mut game, true);
    let frozen = game.guard.clone();
    let time = game.time;
    game.tick(Controls::default());
    assert_eq!(game.guard, frozen);
    assert_eq!(game.time, time);
    game.combat.hitstop_left = 0.0;
    game.health = 0.0;
    game.tick(Controls::default());
    assert_eq!(game.guard, Guard::default());
    assert_eq!(game.stage, 0);
}

#[test]
fn parry_window_allows_deflection_with_stamina_cost() {
    let mut guard = Guard::default();
    guard.amount = 1.0;
    let mut stamina = 100.0;

    guard.parry_window_left = PARRY_WINDOW;
    let contact = guard.try_parry(&mut stamina, Vec3::X, 22.0);
    assert_eq!(contact, crate::GuardContact::Parried);
    assert_eq!(stamina, 78.0);
    assert_eq!(guard.parry_event, 1);
    assert_eq!(guard.parry_cooldown, 0.50);
    assert!(guard.parry_window_left < 0.001);
}

#[test]
fn parry_fails_without_window_or_stamina() {
    let mut guard = Guard::default();
    guard.amount = 1.0;
    let mut stamina = 100.0;

    guard.parry_window_left = 0.0;
    let contact = guard.try_parry(&mut stamina, Vec3::X, 22.0);
    assert_eq!(contact, crate::GuardContact::Blocked);
    assert_eq!(stamina, 78.0);

    guard.parry_window_left = PARRY_WINDOW;
    guard.parry_cooldown = 0.50;
    guard.amount = 1.0;
    let contact = guard.try_parry(&mut stamina, Vec3::X, 22.0);
    assert_eq!(contact, crate::GuardContact::Blocked);
    assert!(stamina <= 56.0);
    assert!(stamina >= 55.0);
}

#[test]
fn parry_cooldown_prevents_immediate_parrys() {
    let mut guard = Guard::default();
    guard.amount = 1.0;
    let mut stamina = 100.0;

    guard.parry_window_left = PARRY_WINDOW;
    let contact = guard.try_parry(&mut stamina, Vec3::X, 22.0);
    assert_eq!(contact, crate::GuardContact::Parried);
    assert_eq!(stamina, 78.0);
    assert_eq!(guard.parry_cooldown, 0.50);
    assert_eq!(guard.parry_event, 1);

    for _ in 0..20 {
        guard.tick(false, false, true);
    }
    assert!(guard.parry_cooldown > 0.0);

    guard.parry_window_left = PARRY_WINDOW;
    guard.amount = 1.0;
    let contact = guard.try_parry(&mut stamina, Vec3::X, 22.0);
    assert_eq!(contact, crate::GuardContact::Blocked);
    assert!(stamina <= 56.0);
    assert!(stamina >= 55.0);
    assert_eq!(guard.parry_event, 1);
}

#[test]
fn a_guard_tapped_as_the_knife_lands_parries_it_and_staggers_him() {
    let mut game = duel();
    // Guard down through the windup: the tap on the contact frame is the parry.
    for _ in 0..11 {
        game.tick(Controls::default());
    }
    assert!(!game.guard.ready());
    contact(&mut game, true);
    assert_eq!(game.guard.parry_event, 1);
    assert_eq!(game.guard.block_event, 0);
    assert_eq!(game.health, 100.0);
    assert_eq!(game.stamina, 100.0 - PARRY_STAMINA_COST);
    assert_eq!(game.warden_ai.phase, WardenPhase::Staggered);
    assert!(game.guard.parry_cooldown > 0.0);
}

#[test]
fn the_parry_window_shuts_and_the_raised_guard_blocks_at_the_weapon_cost() {
    let mut game = duel();
    for _ in 0..11 {
        game.tick(Controls::default());
    }
    // Tapped far too early: by contact the window is spent and the guard is up.
    for _ in 0..12 {
        game.tick(Controls {
            block: true,
            ..Controls::default()
        });
    }
    assert_eq!(game.guard.parry_window_left, 0.0);
    contact(&mut game, true);
    assert_eq!(game.guard.parry_event, 0);
    assert_eq!(game.guard.block_event, 1);
    assert_eq!(game.stamina, 100.0 - BLOCK_COST);
    assert_eq!(game.health, 100.0);
}

#[test]
fn only_the_rising_edge_of_the_guard_input_opens_the_window() {
    let mut guard = Guard::default();
    guard.tick(true, true, true);
    assert!((guard.parry_window_left - PARRY_WINDOW).abs() < 0.0001);
    let opened = guard.parry_window_left;
    guard.tick(true, false, true);
    assert!(
        guard.parry_window_left < opened,
        "holding must not reopen it"
    );
}
