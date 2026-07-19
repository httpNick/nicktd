//! Whole-game balance simulations driving the real tick + message handlers.
use crate::handler::game_loop::{build_main_schedule, run_tick, TICK_RATE};
use crate::handler::in_game::handle_client_message;
use crate::model::game_state::GamePhase;
use crate::model::lobby::Lobby;
use crate::model::messages::{ClientMessage, PlaceMessage};
use crate::model::player::Player;
use crate::model::shape::Shape;

const R: i64 = 1; // rusher
const D: i64 = 2; // defender

/// Diagnostic helper (not part of the brief's spec): report each king's remaining HP.
fn king_hps(lobby: &mut Lobby) -> Vec<(i64, f32)> {
    let mut query = lobby
        .game_state
        .world
        .query::<(&crate::model::components::King, &crate::model::components::PlayerIdComponent, &crate::model::components::Health)>();
    query
        .iter(&lobby.game_state.world)
        .map(|(_, pid, health)| (pid.0, health.current))
        .collect()
}

fn ticks(lobby: &mut Lobby, schedule: &mut bevy_ecs::schedule::Schedule, n: u32) {
    for _ in 0..n {
        run_tick(lobby, schedule, 1.0 / TICK_RATE);
    }
}

/// Runs ticks until the phase changes away from `from`, with a safety bound.
fn tick_past_phase(
    lobby: &mut Lobby,
    schedule: &mut bevy_ecs::schedule::Schedule,
    from: GamePhase,
) {
    for _ in 0..20_000 {
        if lobby.game_state.phase != from {
            return;
        }
        run_tick(lobby, schedule, 1.0 / TICK_RATE);
    }
    panic!("phase never left {:?} within 20k ticks", from);
}

#[test]
fn wave_one_all_in_rush_is_survivable_and_net_negative() {
    let mut lobby = Lobby::new();
    lobby.players.push(Player::new(R, "rusher".into(), 100));
    lobby.players.push(Player::new(D, "defender".into(), 100));

    let mut schedule = build_main_schedule();
    // A few build ticks so workers/kings spawn.
    ticks(&mut lobby, &mut schedule, 5);
    assert_eq!(lobby.game_state.phase, GamePhase::Build);

    // Rusher: max Scouts + one tower with the leftovers.
    loop {
        let cost = lobby.players[0].next_send_costs[0];
        // Keep 25g for one tower.
        if lobby.players[0].gold < cost + 25 {
            break;
        }
        let out = handle_client_message(
            &mut lobby,
            R,
            ClientMessage::SendUnit { shape: Shape::Square },
        );
        assert!(matches!(out, crate::handler::in_game::MessageOutcome::Handled));
    }
    let rush_sends = lobby.players[0].sends_this_wave[0];
    assert!(rush_sends >= 4, "rusher should afford several scouts, got {}", rush_sends);
    handle_client_message(
        &mut lobby,
        R,
        ClientMessage::Place(PlaceMessage { shape: Shape::Square, row: 1, col: 4 }),
    );

    // Defender: four towers across the lane.
    for col in [2u32, 4, 6, 8] {
        handle_client_message(
            &mut lobby,
            D,
            ClientMessage::Place(PlaceMessage { shape: Shape::Square, row: 1, col }),
        );
    }

    // Wave 1 combat.
    handle_client_message(&mut lobby, R, ClientMessage::SkipToCombat);
    tick_past_phase(&mut lobby, &mut schedule, GamePhase::Build);
    assert_eq!(lobby.game_state.phase, GamePhase::Combat);
    tick_past_phase(&mut lobby, &mut schedule, GamePhase::Combat);

    // PROPERTY 1: the rush did not end the game on wave 1.
    let diag_king_hps = king_hps(&mut lobby);
    eprintln!(
        "[diag] after wave 1 combat: phase={:?} wave={} winner={:?} R(gold={} income={}) D(gold={} income={}) king_hps={:?}",
        lobby.game_state.phase,
        lobby.game_state.wave_number,
        lobby.winner_id,
        lobby.players[0].gold,
        lobby.players[0].income,
        lobby.players[1].gold,
        lobby.players[1].income,
        diag_king_hps
    );
    assert_ne!(
        lobby.game_state.phase,
        GamePhase::GameOver,
        "wave-1 all-in must not kill a defended king"
    );

    // Play waves 2 and 3 with no further actions.
    for _ in 0..2 {
        handle_client_message(&mut lobby, R, ClientMessage::SkipToCombat);
        tick_past_phase(&mut lobby, &mut schedule, GamePhase::Build);
        if lobby.game_state.phase == GamePhase::GameOver {
            break;
        }
        tick_past_phase(&mut lobby, &mut schedule, GamePhase::Combat);
    }

    // PROPERTY 2: after 3 waves the defender is economically ahead.
    // Compare gold + income over the remaining 9 waves.
    let remaining = 9u32;
    let r = &lobby.players[0];
    let d = &lobby.players[1];
    let r_worth = r.gold + r.income * remaining;
    let d_worth = d.gold + d.income * remaining;
    assert!(
        d_worth > r_worth,
        "defender must out-economy the rusher: D={} (g{} i{}) vs R={} (g{} i{})",
        d_worth, d.gold, d.income, r_worth, r.gold, r.income
    );
}

#[test]
fn full_clean_game_reaches_victory_at_wave_12() {
    // Both players defend identically and never send; game must reach Victory.
    let mut lobby = Lobby::new();
    lobby.players.push(Player::new(1, "a".into(), 100));
    lobby.players.push(Player::new(2, "b".into(), 100));
    let mut schedule = build_main_schedule();
    ticks(&mut lobby, &mut schedule, 5);
    for player in [1i64, 2] {
        for col in [2u32, 4, 6, 8] {
            handle_client_message(
                &mut lobby,
                player,
                ClientMessage::Place(PlaceMessage { shape: Shape::Square, row: 1, col }),
            );
        }
    }
    // EXPERIMENT: each build phase, both players reinvest — greedily buy Square
    // towers (25g) into successive free cells. Models a real player instead of
    // one who banks gold forever.
    let mut next_slot: [usize; 2] = [0, 0];
    let slot_to_cell = |slot: usize| -> (u32, u32) {
        let row = 2 + (slot / 10) as u32; // rows 2.. below the original row-1 line
        let col = (slot % 10) as u32;
        (row, col)
    };
    for _ in 0..12 {
        if lobby.game_state.phase == GamePhase::Victory
            || lobby.game_state.phase == GamePhase::GameOver
        {
            break;
        }
        for (pi, pid) in [1i64, 2].iter().enumerate() {
            while lobby.players[pi].gold >= 25 && next_slot[pi] < 60 {
                let (row, col) = slot_to_cell(next_slot[pi]);
                next_slot[pi] += 1;
                handle_client_message(
                    &mut lobby,
                    *pid,
                    ClientMessage::Place(PlaceMessage { shape: Shape::Square, row, col }),
                );
            }
        }
        handle_client_message(&mut lobby, 1, ClientMessage::SkipToCombat);
        tick_past_phase(&mut lobby, &mut schedule, GamePhase::Build);
        if lobby.game_state.phase == GamePhase::Combat {
            tick_past_phase(&mut lobby, &mut schedule, GamePhase::Combat);
        }
        let diag_king_hps = king_hps(&mut lobby);
        eprintln!(
            "[diag] wave loop iter end: phase={:?} wave={} winner={:?} P1(gold={} income={}) P2(gold={} income={}) king_hps={:?}",
            lobby.game_state.phase,
            lobby.game_state.wave_number,
            lobby.winner_id,
            lobby.players[0].gold,
            lobby.players[0].income,
            lobby.players[1].gold,
            lobby.players[1].income,
            diag_king_hps
        );
    }
    assert_eq!(
        lobby.game_state.phase,
        GamePhase::Victory,
        "clean defended game must reach Victory (wave 12 cleared); ended wave {}",
        lobby.game_state.wave_number
    );
}

/// Fairness regression test: two players with byte-identical builds must see
/// identical economy and (very close to identical) king HP at every wave end.
/// This does NOT assert who wins or that the game reaches Victory (balance is
/// tuned separately) — only that outcomes are symmetric between mirrored boards
/// through wave 4, per the asymmetry diagnosis
/// (see .superpowers/sdd/asymmetry-diagnosis.md).
///
/// Gold is asserted with exact equality: it is fully explained by the diagnosed
/// bug (co-located wave spawns triggering the entity-index-dependent scatter
/// tiebreaker in `combat.rs`) and the spawn-offset fix below closes it exactly
/// through wave 4 in this scenario.
///
/// King HP is asserted with a small tolerance rather than exact equality. Root
/// cause instrumentation (see asymmetry-fix-report.md) traced a residual,
/// much-smaller divergence to a *different*, structural source: mirrored boards
/// use absolute world x coordinates of different magnitude (~300 left vs ~1100
/// right), so f32 position-integration rounding differs by board and, over
/// enough ticks, chaotic amplification in the nonlinear repulsion/collision
/// feedback (`update_combat_movement`) can flip a genuinely-tied targeting
/// decision. Confirmed NOT to be the diagnosed scatter tiebreaker (it never
/// fires post-fix — instrumented with zero hits) and NOT resolved by widening
/// the targeting tie-break epsilon (tested up to 100 px^2 with no effect on the
/// outcome). A full fix requires board-relative physics coordinates, which is
/// out of scope for this fix. The tolerance is small enough to still catch the
/// original systemic bug (which produced a 130+ HP gap and a king death by
/// wave 5) while not flaking on this residual, order-of-magnitude-smaller
/// effect.
const KING_HP_SYMMETRY_TOLERANCE: f32 = 50.0;

#[test]
fn symmetric_builds_produce_symmetric_outcomes() {
    let mut lobby = Lobby::new();
    lobby.players.push(Player::new(1, "a".into(), 100));
    lobby.players.push(Player::new(2, "b".into(), 100));
    let mut schedule = build_main_schedule();
    ticks(&mut lobby, &mut schedule, 5);
    for player in [1i64, 2] {
        for col in [2u32, 4, 6, 8] {
            handle_client_message(
                &mut lobby,
                player,
                ClientMessage::Place(PlaceMessage { shape: Shape::Square, row: 1, col }),
            );
        }
    }

    for wave in 1..=4u32 {
        if lobby.game_state.phase == GamePhase::Victory
            || lobby.game_state.phase == GamePhase::GameOver
        {
            break;
        }
        handle_client_message(&mut lobby, 1, ClientMessage::SkipToCombat);
        tick_past_phase(&mut lobby, &mut schedule, GamePhase::Build);
        if lobby.game_state.phase == GamePhase::Combat {
            tick_past_phase(&mut lobby, &mut schedule, GamePhase::Combat);
        }

        let p1_gold = lobby.players[0].gold;
        let p2_gold = lobby.players[1].gold;
        let mut hps = king_hps(&mut lobby);
        hps.sort_by_key(|(pid, _)| *pid);
        let (p1_king_hp, p2_king_hp) = match hps.as_slice() {
            [(_, h1), (_, h2)] => (*h1, *h2),
            other => panic!("expected exactly 2 kings, got {:?}", other),
        };
        assert_eq!(
            p1_gold, p2_gold,
            "wave {} gold diverged: P1={} P2={}",
            wave, p1_gold, p2_gold
        );
        assert!(
            (p1_king_hp - p2_king_hp).abs() <= KING_HP_SYMMETRY_TOLERANCE,
            "wave {} king HP diverged beyond tolerance: P1={} P2={} (tolerance={})",
            wave, p1_king_hp, p2_king_hp, KING_HP_SYMMETRY_TOLERANCE
        );
    }
}

/// Nick's 2026-07-18 playtest, as a property: P2 spends EVERY wave's gold on
/// Scouts and never builds; P1 builds 4 Squares wave 1 and then stays static
/// (worst-realistic defense). Sustained scout spam must NOT beat even a
/// static defender — mercenaries are income/pressure tools, not an army
/// (Legion TD parity). The defender's king must outlive wave 4 and the
/// rusher must not win the game.
#[test]
fn sustained_scout_spam_does_not_beat_static_defense() {
    let mut lobby = Lobby::new();
    lobby.players.push(Player::new(R, "spammer".into(), 100));
    lobby.players.push(Player::new(D, "static".into(), 100));
    let mut schedule = build_main_schedule();
    ticks(&mut lobby, &mut schedule, 5);

    // Defender: 4 Squares, wave 1, then nothing forever.
    for col in [2u32, 4, 6, 8] {
        handle_client_message(
            &mut lobby,
            D,
            ClientMessage::Place(PlaceMessage { shape: Shape::Square, row: 1, col }),
        );
    }

    for wave in 1..=4u32 {
        // Rusher: all gold into scouts, every build phase.
        while lobby.players[0].gold >= lobby.players[0].next_send_costs[0] {
            handle_client_message(&mut lobby, R, ClientMessage::SendUnit { shape: Shape::Square });
        }
        handle_client_message(&mut lobby, R, ClientMessage::SkipToCombat);
        tick_past_phase(&mut lobby, &mut schedule, GamePhase::Build);
        if lobby.game_state.phase == GamePhase::Combat {
            tick_past_phase(&mut lobby, &mut schedule, GamePhase::Combat);
        }
        let kings = king_hps(&mut lobby);
        eprintln!(
            "[spam] end wave {}: phase={:?} winner={:?} R(gold={} income={}) D(gold={} income={}) kings={:?}",
            wave, lobby.game_state.phase, lobby.winner_id,
            lobby.players[0].gold, lobby.players[0].income,
            lobby.players[1].gold, lobby.players[1].income,
            kings
        );
        if lobby.game_state.phase == GamePhase::GameOver {
            break;
        }
    }

    assert_ne!(
        lobby.winner_id,
        Some(R),
        "sustained scout spam must not beat even a static 4-tower defense (game phase {:?}, wave {})",
        lobby.game_state.phase,
        lobby.game_state.wave_number
    );
    let d_king = king_hps(&mut lobby)
        .into_iter()
        .find(|(id, _)| *id == D)
        .map(|(_, hp)| hp)
        .unwrap_or(0.0);
    assert!(
        d_king > 0.0,
        "static defender's king must outlive 4 waves of scout spam (HP {})",
        d_king
    );
}
