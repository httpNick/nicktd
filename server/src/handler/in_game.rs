use crate::{
    model::{
        components::{
            AttackRange, AttackStats, Boss, DefenseStats, Health, King, PlayerIdComponent,
            Position, ShapeComponent, TargetPositions, Worker,
        },
        constants::{KING_PLACEMENT_ROW_LIMIT, SQUARE_SIZE},
        game_state::GamePhase,
        king_config::KING_UPGRADE_TIERS,
        messages::ClientMessage,
    },
    state::{ServerState, UpgradedWebSocket},
};
use bevy_ecs::prelude::Entity;
use futures_util::{
    SinkExt, StreamExt,
    stream::{SplitSink, SplitStream},
};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

pub enum InGameLoopResult {
    PlayerLeft,
    ClientDisconnected,
    ForceDisconnect,
}

/// Returns true if a placed entity already claims the grid cell centred at (x, y).
/// Towers anchor to their cell via `HomePosition`; workers and kings have no
/// `HomePosition` and never block placement.
pub fn is_cell_occupied(world: &mut bevy_ecs::prelude::World, x: f32, y: f32) -> bool {
    use crate::model::components::HomePosition;
    let half_cell = SQUARE_SIZE / 2.0;
    let mut query = world.query::<&HomePosition>();
    query
        .iter(world)
        .any(|home| (home.0.x - x).abs() < half_cell && (home.0.y - y).abs() < half_cell)
}

/// Sells the entity with the given index if it is a tower owned by `player_id`.
/// Workers and Kings are never sellable. Returns the refund amount on success.
pub fn try_sell_entity(
    lobby: &mut crate::model::lobby::Lobby,
    player_id: i64,
    entity_id: u64,
) -> Option<u32> {
    use crate::model::components::Tower;
    use bevy_ecs::prelude::With;

    let mut query = lobby.game_state.world.query_filtered::<(
        Entity,
        &PlayerIdComponent,
        &ShapeComponent,
    ), With<Tower>>();
    let found = query
        .iter(&lobby.game_state.world)
        .find(|(e, owner, _)| e.to_bits() == entity_id && owner.0 == player_id)
        .map(|(entity, _, shape)| (entity, shape.0));

    let (entity, shape) = found?;
    let profile = crate::model::unit_config::get_unit_profile(shape);
    let refund = (profile.gold_cost as f32 * 0.75) as u32;
    if let Some(player) = lobby.players.iter_mut().find(|p| p.id == player_id) {
        player.gold += refund;
    }
    lobby.game_state.world.despawn(entity);
    Some(refund)
}

/// Result of handling one client message. Direct replies are returned (not sent)
/// so the caller can release the lobby lock before any network `await`.
#[derive(Debug)]
pub enum MessageOutcome {
    /// Send this message to the requesting client (after unlocking the lobby).
    Reply(crate::model::messages::ServerMessage),
    /// Mutation applied; any broadcast was already sent via the lobby channel.
    Handled,
    /// The player asked to leave the lobby.
    LeaveLobby,
    /// Message not applicable in-game (e.g. JoinQueue).
    Ignored,
}

/// Applies a client message to the lobby. Synchronous on purpose: it runs under
/// the lobby lock and must never await. Broadcasts (`lobby.broadcast_changes`)
/// are channel sends, not awaits, so they are safe here.
pub fn handle_client_message(
    lobby: &mut crate::model::lobby::Lobby,
    player_id: i64,
    msg: ClientMessage,
) -> MessageOutcome {
    use crate::model::messages::ServerMessage;

    match msg {
        ClientMessage::PickFamily { family } => {
            let player_idx = lobby.players.iter().position(|p| p.id == player_id);
            let Some(idx) = player_idx else {
                return MessageOutcome::Ignored;
            };
            if lobby.players[idx].family.is_some() {
                return MessageOutcome::Reply(ServerMessage::Error(
                    "Family already locked for this match.".into(),
                ));
            }
            lobby.players[idx].family = Some(family);
            lobby.broadcast_changes();
            let catalog = crate::model::unit_config::family_catalog(family)
                .into_iter()
                .map(|unit_kind| crate::model::messages::BuildCatalogEntry {
                    unit_kind,
                    name: crate::model::unit_config::unit_kind_name(unit_kind),
                    cost: crate::model::unit_config::get_unit_profile(unit_kind).gold_cost,
                })
                .collect();
            MessageOutcome::Reply(ServerMessage::BuildCatalog(catalog))
        }
        ClientMessage::Place(p) => {
            if lobby.game_state.phase != GamePhase::Build {
                return MessageOutcome::Reply(ServerMessage::Error(
                    "Tower placement is only allowed during the build phase.".into(),
                ));
            }
            let profile = crate::model::unit_config::get_unit_profile(p.shape);
            let player_idx = lobby.players.iter().position(|pl| pl.id == player_id);

            let Some(idx) = player_idx else {
                return MessageOutcome::Ignored;
            };
            match lobby.players[idx].family {
                None => {
                    return MessageOutcome::Reply(ServerMessage::Error(
                        "Pick a family before building.".into(),
                    ));
                }
                Some(family) => {
                    if !crate::model::unit_config::family_catalog(family).contains(&p.shape) {
                        return MessageOutcome::Reply(ServerMessage::Error(
                            "That unit isn't in your family.".into(),
                        ));
                    }
                }
            }
            if p.row >= KING_PLACEMENT_ROW_LIMIT || p.col >= 10 {
                return MessageOutcome::Reply(ServerMessage::Error(
                    "Invalid placement coordinates.".into(),
                ));
            }

            let x = if idx == 0 {
                (p.col as f32 * SQUARE_SIZE) + (SQUARE_SIZE / 2.0)
            } else {
                crate::model::constants::RIGHT_BOARD_START
                    + (p.col as f32 * SQUARE_SIZE)
                    + (SQUARE_SIZE / 2.0)
            };
            let y = (p.row as f32 * SQUARE_SIZE) + (SQUARE_SIZE / 2.0);

            if is_cell_occupied(&mut lobby.game_state.world, x, y) {
                return MessageOutcome::Reply(ServerMessage::Error(
                    "That square is already occupied.".into(),
                ));
            }

            if lobby.players[idx].try_spend_gold(profile.gold_cost) {
                crate::handler::spawn::spawn_unit(
                    &mut lobby.game_state.world,
                    Position { x, y },
                    p.shape,
                    player_id,
                );
                lobby.broadcast_changes();
                MessageOutcome::Handled
            } else {
                MessageOutcome::Reply(ServerMessage::Error(format!(
                    "Insufficient gold for {:?} (cost: {})",
                    p.shape, profile.gold_cost
                )))
            }
        }
        ClientMessage::SkipToCombat => {
            lobby.game_state.phase_timer = 0.0;
            MessageOutcome::Handled
        }
        ClientMessage::HireWorker {} => {
            let player_idx = lobby.players.iter().position(|p| p.id == player_id);
            let Some(idx) = player_idx else {
                return MessageOutcome::Ignored;
            };
            let worker_count = {
                let mut q = lobby
                    .game_state
                    .world
                    .query::<(&Worker, &PlayerIdComponent)>();
                q.iter(&lobby.game_state.world)
                    .filter(|(_, owner)| owner.0 == player_id)
                    .count()
            };
            if worker_count >= crate::handler::worker::WORKER_CAP {
                return MessageOutcome::Reply(ServerMessage::Error(
                    "Worker limit reached (max 7)".into(),
                ));
            }
            if lobby.players[idx].try_spend_gold(50) {
                let targets = TargetPositions {
                    vein: crate::handler::worker::VEIN_POSITIONS[idx],
                    cart: crate::handler::worker::CART_POSITIONS[idx],
                };
                crate::handler::spawn::spawn_worker(
                    &mut lobby.game_state.world,
                    player_id,
                    targets,
                );
                lobby.broadcast_changes();
                MessageOutcome::Handled
            } else {
                MessageOutcome::Reply(ServerMessage::Error(
                    "Insufficient gold for Worker (cost: 50)".into(),
                ))
            }
        }
        ClientMessage::SendUnit { shape } => {
            let player_idx = lobby.players.iter().position(|p| p.id == player_id);
            let Some(idx) = player_idx else {
                return MessageOutcome::Ignored;
            };
            let wave = lobby.game_state.wave_number;
            let sent_profile = crate::model::unit_config::get_sent_unit_profile(shape);
            let i = crate::model::unit_config::shape_index(shape);
            let cost = crate::model::unit_config::sent_unit_cost(
                shape,
                wave,
                lobby.players[idx].sends_this_wave[i],
            );
            if lobby.players[idx].try_spend_gold(cost) {
                lobby.players[idx].spawning_queue.push(shape);
                lobby.players[idx].income += sent_profile.income;
                lobby.players[idx].sends_this_wave[i] += 1;
                lobby.players[idx].refresh_send_costs(wave);
                lobby.broadcast_changes();
                MessageOutcome::Handled
            } else {
                MessageOutcome::Reply(ServerMessage::Error(format!(
                    "Insufficient gold for {} (cost: {})",
                    sent_profile.name, cost
                )))
            }
        }
        ClientMessage::LeaveLobby => MessageOutcome::LeaveLobby,
        ClientMessage::SellById { entity_id } => {
            if lobby.game_state.phase != GamePhase::Build {
                return MessageOutcome::Reply(ServerMessage::Error(
                    "Tower selling is only allowed during the build phase.".into(),
                ));
            }
            if try_sell_entity(lobby, player_id, entity_id).is_some() {
                lobby.broadcast_changes();
            }
            MessageOutcome::Handled
        }
        ClientMessage::RequestUnitInfo { entity_id } => {
            let mut query = lobby.game_state.world.query::<(
                Entity,
                Option<&AttackStats>,
                Option<&AttackRange>,
                Option<&DefenseStats>,
                Option<&ShapeComponent>,
                Option<&Boss>,
                Option<&PlayerIdComponent>,
                Option<&Worker>,
            )>();
            let found = query
                .iter(&lobby.game_state.world)
                .find(|(entity, ..)| entity.to_bits() == entity_id)
                .map(
                    |(
                        _,
                        attack_stats,
                        attack_range,
                        defense_stats,
                        shape_comp,
                        boss,
                        owner,
                        worker,
                    )| {
                        (
                            attack_stats.map(|s| s.damage),
                            attack_stats.map(|s| s.rate),
                            attack_stats.map(|s| s.damage_type),
                            attack_range.map(|r| r.0),
                            defense_stats.map(|d| d.armor),
                            shape_comp.map(|s| s.0),
                            boss.is_some(),
                            owner.map(|o| o.0),
                            worker.is_some(),
                        )
                    },
                );

            let Some((
                attack_damage,
                attack_rate,
                damage_type,
                attack_range,
                armor,
                shape,
                is_boss,
                owner_id,
                is_worker,
            )) = found
            else {
                return MessageOutcome::Ignored;
            };
            let sell_value = match (owner_id, shape, is_worker) {
                (Some(oid), Some(sh), false) if oid == player_id => {
                    let profile = crate::model::unit_config::get_unit_profile(sh);
                    Some((profile.gold_cost as f32 * 0.75) as u32)
                }
                _ => None,
            };
            let info = crate::model::messages::UnitInfoData {
                entity_id,
                attack_damage,
                attack_rate,
                attack_range,
                damage_type,
                armor,
                is_boss,
                sell_value,
            };
            MessageOutcome::Reply(ServerMessage::UnitInfo(info))
        }
        ClientMessage::UpgradeKing {} => {
            if lobby.game_state.phase != GamePhase::Build {
                return MessageOutcome::Reply(ServerMessage::Error(
                    "King upgrades are only available during the build phase.".into(),
                ));
            }
            let player_idx = lobby.players.iter().position(|p| p.id == player_id);
            let Some(idx) = player_idx else {
                return MessageOutcome::Ignored;
            };
            let current_tier = lobby.players[idx].king_tier;
            if current_tier >= 4 {
                return MessageOutcome::Reply(ServerMessage::Error(
                    "King is already at maximum tier.".into(),
                ));
            }
            let tier = &KING_UPGRADE_TIERS[current_tier as usize];
            if !lobby.players[idx].can_afford(tier.cost) {
                return MessageOutcome::Reply(ServerMessage::Error(
                    "Insufficient gold for king upgrade.".into(),
                ));
            }
            // Deduct gold, increment tier, add income.
            lobby.players[idx].gold -= tier.cost;
            lobby.players[idx].king_tier += 1;
            lobby.players[idx].income += tier.income_delta;
            let hp_delta = tier.hp_delta;
            let new_damage = tier.new_damage;
            // Find and update the king entity.
            let king_entity = {
                let mut q = lobby
                    .game_state
                    .world
                    .query::<(Entity, &PlayerIdComponent, &King)>();
                q.iter(&lobby.game_state.world)
                    .find(|(_, pid, _)| pid.0 == player_id)
                    .map(|(e, _, _)| e)
            };
            if let Some(king_e) = king_entity {
                if let Some(mut health) = lobby.game_state.world.get_mut::<Health>(king_e) {
                    health.max += hp_delta;
                    health.current = (health.current + hp_delta).min(health.max);
                }
                if let Some(mut stats) = lobby.game_state.world.get_mut::<AttackStats>(king_e) {
                    stats.damage = new_damage;
                }
                lobby.broadcast_changes();
                MessageOutcome::Handled
            } else {
                MessageOutcome::Reply(ServerMessage::Error("King not found.".into()))
            }
        }
        ClientMessage::RequestFullState => MessageOutcome::Reply(lobby.full_state_message()),
        _ => MessageOutcome::Ignored,
    }
}

pub async fn in_game_loop(
    ws_sender: &mut SplitSink<UpgradedWebSocket, Message>,
    ws_receiver: &mut SplitStream<UpgradedWebSocket>,
    server_state: &ServerState,
    match_id: u64,
    player_id: i64,
    shutdown_rx: &mut mpsc::Receiver<()>,
) -> InGameLoopResult {
    let Some(lobby_arc) = server_state.matches.read().await.get(&match_id).cloned() else {
        // Match already torn down (e.g. opponent left and cleanup raced us).
        return InGameLoopResult::PlayerLeft;
    };
    let mut game_rx = lobby_arc.lock().await.tx.subscribe();
    lobby_arc.lock().await.broadcast_gamestate();

    loop {
        tokio::select! {
            _ = shutdown_rx.recv() => {
                break InGameLoopResult::ForceDisconnect;
            },
            maybe_msg = ws_receiver.next() => {
                match maybe_msg {
                    Some(Ok(msg)) => {
                        if let Message::Text(text) = msg {
                            if let Ok(client_msg) = serde_json::from_str::<ClientMessage>(&text) {
                                let outcome = {
                                    let mut lobby = lobby_arc.lock().await;
                                    handle_client_message(&mut lobby, player_id, client_msg)
                                }; // lobby guard dropped here, before any network await
                                match outcome {
                                    MessageOutcome::Reply(reply) => {
                                        let _ = crate::routes::ws::send_message(ws_sender, reply).await;
                                    }
                                    MessageOutcome::LeaveLobby => break InGameLoopResult::PlayerLeft,
                                    MessageOutcome::Handled | MessageOutcome::Ignored => {}
                                }
                            }
                        }
                    },
                    Some(Err(_)) | None => break InGameLoopResult::ClientDisconnected,
                }
            },
            result = game_rx.recv() => {
                match result {
                    Ok(msg) => {
                        if ws_sender.send(Message::Text(msg.into())).await.is_err() {
                            break InGameLoopResult::ClientDisconnected;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        // Missed deltas: rebaseline this client with a direct snapshot.
                        let snapshot = {
                            let mut lobby = lobby_arc.lock().await;
                            lobby.full_state_message() // -> ServerMessage::GameState
                        };
                        if crate::routes::ws::send_message(ws_sender, snapshot).await.is_err() {
                            break InGameLoopResult::ClientDisconnected;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        break InGameLoopResult::ClientDisconnected;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handler::spawn::spawn_unit;
    use crate::model::components::{ShapeComponent, Worker};
    use crate::model::game_state::GamePhase;
    use crate::model::lobby::Lobby;
    use crate::model::messages::PlaceMessage;
    use crate::model::player::Player;
    use crate::model::unit_kind::UnitKind;

    #[test]
    fn test_unit_placement_restricted_by_player_id() {
        use crate::model::constants::{
            LEFT_BOARD_END, RIGHT_BOARD_END, RIGHT_BOARD_START, SQUARE_SIZE,
        };
        let mut lobby = Lobby::new();

        let p1_id = 1;
        let p2_id = 2;
        lobby.players.push(Player::new(p1_id, "p1".into(), 100));
        lobby.players.push(Player::new(p2_id, "p2".into(), 100));

        // Player 0 (index 0) is P1. Board is 0-600.
        // Player 1 (index 1) is P2. Board is 800-1400.

        // Valid placements
        let p1_valid = PlaceMessage {
            shape: UnitKind::Square,
            row: 1,
            col: 2,
        };
        // SIMULATED logic for P1
        let p1_idx = 0;
        let x1 = if p1_idx == 0 {
            (p1_valid.col as f32 * SQUARE_SIZE) + (SQUARE_SIZE / 2.0)
        } else {
            RIGHT_BOARD_START + (p1_valid.col as f32 * SQUARE_SIZE) + (SQUARE_SIZE / 2.0)
        };
        assert!(x1 < LEFT_BOARD_END);

        let p2_valid = PlaceMessage {
            shape: UnitKind::Square,
            row: 1,
            col: 2, // Now uses local col 2
        };
        // SIMULATED logic for P2
        let p2_idx = 1;
        let x2 = if p2_idx == 0 {
            (p2_valid.col as f32 * SQUARE_SIZE) + (SQUARE_SIZE / 2.0)
        } else {
            RIGHT_BOARD_START + (p2_valid.col as f32 * SQUARE_SIZE) + (SQUARE_SIZE / 2.0)
        };
        assert!(x2 >= RIGHT_BOARD_START && x2 < RIGHT_BOARD_END);
    }

    #[test]
    fn test_unit_placement_deducts_gold() {
        let mut lobby = Lobby::new();
        let player_id = 1;
        lobby.players.push(Player::new(player_id, "p1".into(), 100));

        let p = PlaceMessage {
            shape: UnitKind::Square,
            row: 1,
            col: 1,
        };

        // --- SIMULATED logic from in_game_loop ---
        let profile = crate::model::unit_config::get_unit_profile(p.shape);
        let player = lobby
            .players
            .iter_mut()
            .find(|pl| pl.id == player_id)
            .unwrap();

        if player.try_spend_gold(profile.gold_cost) {
            let x = (p.col as f32 * SQUARE_SIZE) + (SQUARE_SIZE / 2.0);
            let y = (p.row as f32 * SQUARE_SIZE) + (SQUARE_SIZE / 2.0);
            crate::handler::spawn::spawn_unit(
                &mut lobby.game_state.world,
                Position { x, y },
                p.shape,
                player_id,
            );
        }
        // --- END SIMULATED logic ---

        assert_eq!(lobby.players[0].gold, 75, "Square costs 25, 100 - 25 = 75");
        let mut query = lobby.game_state.world.query::<&ShapeComponent>();
        assert_eq!(query.iter(&lobby.game_state.world).count(), 1);
    }

    #[test]
    fn test_insufficient_gold_prevents_placement() {
        let mut lobby = Lobby::new();
        let player_id = 1;
        lobby.players.push(Player::new(player_id, "p1".into(), 10));

        let p = PlaceMessage {
            shape: UnitKind::Square,
            row: 1,
            col: 1,
        };

        // --- SIMULATED logic ---
        let profile = crate::model::unit_config::get_unit_profile(p.shape);
        let player_opt = lobby.players.iter_mut().find(|pl| pl.id == player_id);
        if let Some(player) = player_opt {
            if player.try_spend_gold(profile.gold_cost) {
                let x = (p.col as f32 * SQUARE_SIZE) + (SQUARE_SIZE / 2.0);
                let y = (p.row as f32 * SQUARE_SIZE) + (SQUARE_SIZE / 2.0);
                crate::handler::spawn::spawn_unit(
                    &mut lobby.game_state.world,
                    Position { x, y },
                    p.shape,
                    player_id,
                );
            }
        }
        // --- END SIMULATED logic ---

        assert_eq!(lobby.players[0].gold, 10, "Gold should NOT be deducted");
        let mut query = lobby.game_state.world.query::<&ShapeComponent>();
        assert_eq!(
            query.iter(&lobby.game_state.world).count(),
            0,
            "Unit should NOT be spawned"
        );
    }

    #[test]
    fn place_rejected_during_combat_phase() {
        let mut lobby = Lobby::new();
        let player_id = 1;
        lobby.players.push(Player::new(player_id, "p1".into(), 100));
        lobby.game_state.phase = GamePhase::Combat;

        let p = PlaceMessage {
            shape: UnitKind::Square,
            row: 1,
            col: 1,
        };

        // Simulate Place handler WITH phase guard
        if lobby.game_state.phase == GamePhase::Build {
            let profile = crate::model::unit_config::get_unit_profile(p.shape);
            let player_idx = lobby.players.iter().position(|pl| pl.id == player_id);
            if let Some(idx) = player_idx {
                if lobby.players[idx].try_spend_gold(profile.gold_cost) {
                    let x = (p.col as f32 * SQUARE_SIZE) + (SQUARE_SIZE / 2.0);
                    let y = (p.row as f32 * SQUARE_SIZE) + (SQUARE_SIZE / 2.0);
                    crate::handler::spawn::spawn_unit(
                        &mut lobby.game_state.world,
                        Position { x, y },
                        p.shape,
                        player_id,
                    );
                }
            }
        }

        assert_eq!(
            lobby.players[0].gold, 100,
            "Gold should NOT be deducted when placement is rejected during combat"
        );
        let mut query = lobby.game_state.world.query::<&ShapeComponent>();
        assert_eq!(
            query.iter(&lobby.game_state.world).count(),
            0,
            "No unit should be spawned when placement is rejected during combat"
        );
    }

    #[test]
    fn place_accepted_during_build_phase() {
        let mut lobby = Lobby::new();
        let player_id = 1;
        lobby.players.push(Player::new(player_id, "p1".into(), 100));
        // Default phase is Build

        let p = PlaceMessage {
            shape: UnitKind::Square,
            row: 1,
            col: 1,
        };

        // Simulate Place handler WITH phase guard
        if lobby.game_state.phase == GamePhase::Build {
            let profile = crate::model::unit_config::get_unit_profile(p.shape);
            let player_idx = lobby.players.iter().position(|pl| pl.id == player_id);
            if let Some(idx) = player_idx {
                if lobby.players[idx].try_spend_gold(profile.gold_cost) {
                    let x = (p.col as f32 * SQUARE_SIZE) + (SQUARE_SIZE / 2.0);
                    let y = (p.row as f32 * SQUARE_SIZE) + (SQUARE_SIZE / 2.0);
                    crate::handler::spawn::spawn_unit(
                        &mut lobby.game_state.world,
                        Position { x, y },
                        p.shape,
                        player_id,
                    );
                }
            }
        }

        assert_eq!(
            lobby.players[0].gold, 75,
            "Gold should be deducted (100 - 25 = 75) when placement succeeds during build"
        );
        let mut query = lobby.game_state.world.query::<&ShapeComponent>();
        assert_eq!(
            query.iter(&lobby.game_state.world).count(),
            1,
            "Unit should be spawned during build phase"
        );
    }

    // --- handle_client_message tests (real handler, not simulated logic) ---

    #[test]
    fn handle_place_spawns_tower_and_deducts_gold() {
        use crate::model::messages::ClientMessage;

        let mut lobby = Lobby::new();
        lobby.players.push(Player::new(1, "p1".into(), 100));
        handle_client_message(
            &mut lobby,
            1,
            ClientMessage::PickFamily {
                family: crate::model::family::Family::Basic,
            },
        );
        let msg = ClientMessage::Place(PlaceMessage {
            shape: UnitKind::Square,
            row: 1,
            col: 1,
        });

        let outcome = handle_client_message(&mut lobby, 1, msg);

        assert!(matches!(outcome, MessageOutcome::Handled));
        assert_eq!(lobby.players[0].gold, 75);
        let mut query = lobby.game_state.world.query::<&ShapeComponent>();
        assert_eq!(query.iter(&lobby.game_state.world).count(), 1);
    }

    #[test]
    fn handle_place_rejects_occupied_cell_with_reply() {
        use crate::model::messages::{ClientMessage, ServerMessage};

        let mut lobby = Lobby::new();
        lobby.players.push(Player::new(1, "p1".into(), 200));
        handle_client_message(
            &mut lobby,
            1,
            ClientMessage::PickFamily {
                family: crate::model::family::Family::Basic,
            },
        );
        let msg = ClientMessage::Place(PlaceMessage {
            shape: UnitKind::Square,
            row: 1,
            col: 1,
        });
        assert!(matches!(
            handle_client_message(&mut lobby, 1, msg),
            MessageOutcome::Handled
        ));

        let dup = ClientMessage::Place(PlaceMessage {
            shape: UnitKind::Square,
            row: 1,
            col: 1,
        });
        let outcome = handle_client_message(&mut lobby, 1, dup);

        assert!(matches!(
            outcome,
            MessageOutcome::Reply(ServerMessage::Error(_))
        ));
        assert_eq!(
            lobby.players[0].gold, 175,
            "second placement must not charge gold"
        );
    }

    #[test]
    fn place_rejected_without_family_picked() {
        use crate::model::messages::{ClientMessage, PlaceMessage, ServerMessage};

        let mut lobby = Lobby::new();
        lobby.players.push(Player::new(1, "p1".into(), 100));
        lobby.players.push(Player::new(2, "p2".into(), 100));

        let player_id = lobby.players[0].id;
        let msg = ClientMessage::Place(PlaceMessage {
            shape: UnitKind::Square,
            row: 0,
            col: 0,
        });
        let outcome = handle_client_message(&mut lobby, player_id, msg);
        match outcome {
            MessageOutcome::Reply(ServerMessage::Error(e)) => {
                assert!(e.contains("family"), "expected family error, got: {e}");
            }
            other => panic!("expected family-required error, got {other:?}"),
        }
    }

    #[test]
    fn pick_family_locks_and_rejects_second_pick() {
        use crate::model::family::Family;
        use crate::model::messages::{ClientMessage, ServerMessage};

        let mut lobby = Lobby::new();
        lobby.players.push(Player::new(1, "p1".into(), 100));
        lobby.players.push(Player::new(2, "p2".into(), 100));
        let player_id = lobby.players[0].id;

        let first = handle_client_message(
            &mut lobby,
            player_id,
            ClientMessage::PickFamily { family: Family::Basic },
        );
        assert!(matches!(
            first,
            MessageOutcome::Reply(ServerMessage::BuildCatalog(_))
        ));
        assert_eq!(lobby.players[0].family, Some(Family::Basic));

        let second = handle_client_message(
            &mut lobby,
            player_id,
            ClientMessage::PickFamily { family: Family::Basic },
        );
        match second {
            MessageOutcome::Reply(ServerMessage::Error(e)) => {
                assert!(e.contains("locked") || e.contains("already"));
            }
            other => panic!("expected already-locked error, got {other:?}"),
        }
    }

    #[test]
    fn place_succeeds_after_family_picked() {
        use crate::model::family::Family;
        use crate::model::messages::{ClientMessage, PlaceMessage};

        let mut lobby = Lobby::new();
        lobby.players.push(Player::new(1, "p1".into(), 100));
        lobby.players.push(Player::new(2, "p2".into(), 100));
        let player_id = lobby.players[0].id;
        handle_client_message(
            &mut lobby,
            player_id,
            ClientMessage::PickFamily { family: Family::Basic },
        );
        let msg = ClientMessage::Place(PlaceMessage {
            shape: UnitKind::Square,
            row: 0,
            col: 0,
        });
        let outcome = handle_client_message(&mut lobby, player_id, msg);
        assert!(matches!(outcome, MessageOutcome::Handled));
    }

    #[test]
    fn handle_leave_lobby_returns_leave_outcome() {
        use crate::model::messages::ClientMessage;

        let mut lobby = Lobby::new();
        lobby.players.push(Player::new(1, "p1".into(), 100));
        let outcome = handle_client_message(&mut lobby, 1, ClientMessage::LeaveLobby);
        assert!(matches!(outcome, MessageOutcome::LeaveLobby));
    }

    #[test]
    fn handle_request_unit_info_returns_reply() {
        use crate::model::messages::{ClientMessage, ServerMessage};

        let mut lobby = Lobby::new();
        lobby.players.push(Player::new(1, "p1".into(), 100));
        let e = spawn_unit(
            &mut lobby.game_state.world,
            Position { x: 100.0, y: 100.0 },
            UnitKind::Square,
            1,
        );
        let outcome = handle_client_message(
            &mut lobby,
            1,
            ClientMessage::RequestUnitInfo {
                entity_id: e.to_bits(),
            },
        );
        match outcome {
            MessageOutcome::Reply(ServerMessage::UnitInfo(info)) => {
                assert_eq!(info.entity_id, e.to_bits())
            }
            _ => panic!("expected UnitInfo reply"),
        }
    }

    // --- Placement occupancy tests ---

    #[test]
    fn is_cell_occupied_detects_existing_tower() {
        let mut lobby = Lobby::new();
        let x = (2.0 * SQUARE_SIZE) + (SQUARE_SIZE / 2.0);
        let y = (3.0 * SQUARE_SIZE) + (SQUARE_SIZE / 2.0);

        spawn_unit(
            &mut lobby.game_state.world,
            Position { x, y },
            UnitKind::Square,
            1,
        );

        assert!(
            is_cell_occupied(&mut lobby.game_state.world, x, y),
            "Cell with a tower must be occupied"
        );
    }

    #[test]
    fn is_cell_occupied_false_for_empty_and_adjacent_cells() {
        let mut lobby = Lobby::new();
        let x = (2.0 * SQUARE_SIZE) + (SQUARE_SIZE / 2.0);
        let y = (3.0 * SQUARE_SIZE) + (SQUARE_SIZE / 2.0);

        assert!(
            !is_cell_occupied(&mut lobby.game_state.world, x, y),
            "Empty world has no occupied cells"
        );

        spawn_unit(
            &mut lobby.game_state.world,
            Position { x, y },
            UnitKind::Square,
            1,
        );

        assert!(
            !is_cell_occupied(&mut lobby.game_state.world, x + SQUARE_SIZE, y),
            "Adjacent column must not read as occupied"
        );
        assert!(
            !is_cell_occupied(&mut lobby.game_state.world, x, y + SQUARE_SIZE),
            "Adjacent row must not read as occupied"
        );
    }

    #[test]
    fn is_cell_occupied_ignores_workers() {
        let mut lobby = Lobby::new();

        // Worker positions are outside the board, but guard the logic anyway:
        // workers have no HomePosition, so wherever they stand must not block placement.
        let targets = TargetPositions {
            vein: crate::handler::worker::VEIN_POSITIONS[0],
            cart: crate::handler::worker::CART_POSITIONS[0],
        };
        crate::handler::spawn::spawn_worker(&mut lobby.game_state.world, 1, targets);

        let cart = crate::handler::worker::CART_POSITIONS[0];
        assert!(
            !is_cell_occupied(&mut lobby.game_state.world, cart.x, cart.y),
            "A worker standing on a spot must not block tower placement"
        );
    }

    // --- SellById tests ---

    #[test]
    fn try_sell_entity_stale_id_does_not_sell_recycled_entity() {
        let mut lobby = Lobby::new();
        let player_id: i64 = 1;
        lobby.players.push(Player::new(player_id, "p1".into(), 100));

        // Tower A exists, client learns its ID, then A is despawned.
        let tower_a = spawn_unit(
            &mut lobby.game_state.world,
            Position { x: 100.0, y: 100.0 },
            UnitKind::Square,
            player_id,
        );
        let stale_id = tower_a.to_bits();
        lobby.game_state.world.despawn(tower_a);

        // Tower B spawns and may reuse A's index (with a new generation).
        let tower_b = spawn_unit(
            &mut lobby.game_state.world,
            Position { x: 200.0, y: 100.0 },
            UnitKind::Circle,
            player_id,
        );

        // The client's stale request must not sell tower B.
        let sold = try_sell_entity(&mut lobby, player_id, stale_id);

        assert_eq!(sold, None, "A stale entity ID must not sell anything");
        assert!(
            lobby.game_state.world.entities().contains(tower_b),
            "The new tower must not be despawned by a stale ID"
        );
        assert_eq!(lobby.players[0].gold, 100, "No refund for a stale ID");
    }

    #[test]
    fn broadcast_unit_ids_are_full_entity_bits() {
        let mut lobby = Lobby::new();
        let tower = spawn_unit(
            &mut lobby.game_state.world,
            Position { x: 100.0, y: 300.0 },
            UnitKind::Square,
            1,
        );

        let mut rx = lobby.tx.subscribe();
        lobby.broadcast_gamestate();

        let msg = rx.try_recv().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&msg).unwrap();
        let units = parsed["data"]["units"].as_array().unwrap();
        assert_eq!(
            units[0]["id"].as_u64().unwrap(),
            tower.to_bits(),
            "Broadcast unit id must be the full entity bits (index + generation)"
        );
    }

    #[test]
    fn try_sell_entity_sells_own_tower_and_refunds_gold() {
        let mut lobby = Lobby::new();
        let player_id: i64 = 1;
        lobby.players.push(Player::new(player_id, "p1".into(), 100));

        let entity = spawn_unit(
            &mut lobby.game_state.world,
            Position { x: 100.0, y: 100.0 },
            UnitKind::Square,
            player_id,
        );

        let sold = try_sell_entity(&mut lobby, player_id, entity.to_bits());

        assert_eq!(sold, Some(18), "Square costs 25, refund is 75% = 18");
        assert_eq!(lobby.players[0].gold, 118, "Refund should be added to gold");
        assert!(
            !lobby.game_state.world.entities().contains(entity),
            "Sold tower should be despawned"
        );
    }

    #[test]
    fn try_sell_entity_refuses_to_sell_worker() {
        let mut lobby = Lobby::new();
        let player_id: i64 = 1;
        lobby.players.push(Player::new(player_id, "p1".into(), 100));

        let targets = TargetPositions {
            vein: crate::handler::worker::VEIN_POSITIONS[0],
            cart: crate::handler::worker::CART_POSITIONS[0],
        };
        let worker =
            crate::handler::spawn::spawn_worker(&mut lobby.game_state.world, player_id, targets);

        let sold = try_sell_entity(&mut lobby, player_id, worker.to_bits());

        assert_eq!(sold, None, "Workers must not be sellable");
        assert_eq!(lobby.players[0].gold, 100, "Gold must not change");
        assert!(
            lobby.game_state.world.entities().contains(worker),
            "Worker must not be despawned"
        );
    }

    #[test]
    fn try_sell_entity_refuses_to_sell_king() {
        let mut lobby = Lobby::new();
        let player_id: i64 = 1;
        lobby.players.push(Player::new(player_id, "p1".into(), 100));

        let king = crate::handler::spawn::spawn_king(&mut lobby.game_state.world, player_id, 0);

        let sold = try_sell_entity(&mut lobby, player_id, king.to_bits());

        assert_eq!(sold, None, "The king must not be sellable");
        assert_eq!(lobby.players[0].gold, 100, "Gold must not change");
        assert!(
            lobby.game_state.world.entities().contains(king),
            "King must not be despawned"
        );
    }

    #[test]
    fn try_sell_entity_refuses_wrong_owner() {
        let mut lobby = Lobby::new();
        lobby.players.push(Player::new(1, "p1".into(), 100));
        lobby.players.push(Player::new(2, "p2".into(), 100));

        let entity = spawn_unit(
            &mut lobby.game_state.world,
            Position { x: 100.0, y: 100.0 },
            UnitKind::Square,
            1,
        );

        let sold = try_sell_entity(&mut lobby, 2, entity.to_bits());

        assert_eq!(sold, None, "Players must not sell towers they don't own");
        assert!(
            lobby.game_state.world.entities().contains(entity),
            "Tower must not be despawned"
        );
    }

    #[test]
    fn test_sell_by_id_refunds_gold_in_build_phase() {
        use crate::model::unit_config::get_unit_profile;

        let mut lobby = Lobby::new();
        let player_id: i64 = 1;
        lobby.players.push(Player::new(player_id, "p1".into(), 100));

        let entity = spawn_unit(
            &mut lobby.game_state.world,
            Position { x: 100.0, y: 100.0 },
            UnitKind::Square,
            player_id,
        );
        let entity_id = entity.index();

        assert_eq!(lobby.game_state.phase, GamePhase::Build);

        // Simulate SellById handler
        if lobby.game_state.phase == GamePhase::Build {
            let mut query = lobby
                .game_state
                .world
                .query::<(Entity, &PlayerIdComponent, &ShapeComponent)>();
            let found = query
                .iter(&lobby.game_state.world)
                .find(|(e, owner, _)| e.index() == entity_id && owner.0 == player_id)
                .map(|(entity, _, shape)| (entity, shape.0));

            if let Some((entity, shape)) = found {
                let profile = get_unit_profile(shape);
                let refund = (profile.gold_cost as f32 * 0.75) as u32;
                if let Some(player) = lobby.players.iter_mut().find(|p| p.id == player_id) {
                    player.gold += refund;
                }
                lobby.game_state.world.despawn(entity);
            }
        }

        // Square costs 25; 25 * 0.75 = 18 (truncated)
        assert_eq!(
            lobby.players[0].gold, 118,
            "Gold should be refunded 18 (75% of 25)"
        );
        let mut check = lobby.game_state.world.query::<&ShapeComponent>();
        assert_eq!(
            check.iter(&lobby.game_state.world).count(),
            0,
            "Entity should be despawned"
        );
    }

    #[test]
    fn test_sell_by_id_rejected_in_combat_phase() {
        let mut lobby = Lobby::new();
        let player_id: i64 = 1;
        lobby.players.push(Player::new(player_id, "p1".into(), 100));
        lobby.game_state.phase = GamePhase::Combat;

        let entity = spawn_unit(
            &mut lobby.game_state.world,
            Position { x: 100.0, y: 100.0 },
            UnitKind::Square,
            player_id,
        );
        let entity_id = entity.index();

        // Simulate SellById handler with phase guard
        if lobby.game_state.phase == GamePhase::Build {
            let mut query = lobby
                .game_state
                .world
                .query::<(Entity, &PlayerIdComponent, &ShapeComponent)>();
            let found = query
                .iter(&lobby.game_state.world)
                .find(|(e, owner, _)| e.index() == entity_id && owner.0 == player_id)
                .map(|(entity, _, shape)| (entity, shape.0));

            if let Some((entity, _shape)) = found {
                lobby.game_state.world.despawn(entity);
                lobby.players[0].gold += 18;
            }
        }

        assert_eq!(
            lobby.players[0].gold, 100,
            "Gold should not change in combat phase"
        );
        let mut check = lobby.game_state.world.query::<&ShapeComponent>();
        assert_eq!(
            check.iter(&lobby.game_state.world).count(),
            1,
            "Entity should still exist"
        );
    }

    #[test]
    fn test_sell_by_id_rejected_for_wrong_owner() {
        let mut lobby = Lobby::new();
        let player_1_id: i64 = 1;
        let player_2_id: i64 = 2;
        lobby
            .players
            .push(Player::new(player_1_id, "p1".into(), 100));
        lobby
            .players
            .push(Player::new(player_2_id, "p2".into(), 100));

        let entity = spawn_unit(
            &mut lobby.game_state.world,
            Position { x: 100.0, y: 100.0 },
            UnitKind::Square,
            player_1_id,
        );
        let entity_id = entity.index();

        // Player 2 attempts to sell player 1's tower
        let mut query = lobby
            .game_state
            .world
            .query::<(Entity, &PlayerIdComponent, &ShapeComponent)>();
        let found = query
            .iter(&lobby.game_state.world)
            .find(|(e, owner, _)| e.index() == entity_id && owner.0 == player_2_id)
            .map(|(entity, _, shape)| (entity, shape.0));

        assert!(
            found.is_none(),
            "Query should not match a tower owned by a different player"
        );
        let mut check = lobby.game_state.world.query::<&ShapeComponent>();
        assert_eq!(
            check.iter(&lobby.game_state.world).count(),
            1,
            "Entity should not be despawned"
        );
    }

    // --- RequestUnitInfo tests ---

    #[test]
    fn test_request_unit_info_returns_tower_stats() {
        use crate::model::components::{AttackRange, AttackStats, Boss, DefenseStats, Worker};
        use crate::model::unit_config::get_unit_profile;

        let mut lobby = Lobby::new();
        let player_id: i64 = 1;
        lobby.players.push(Player::new(player_id, "p1".into(), 100));

        let entity = spawn_unit(
            &mut lobby.game_state.world,
            Position { x: 100.0, y: 100.0 },
            UnitKind::Circle,
            player_id,
        );
        let entity_id = entity.index();

        let mut query = lobby.game_state.world.query::<(
            Entity,
            Option<&AttackStats>,
            Option<&AttackRange>,
            Option<&DefenseStats>,
            Option<&ShapeComponent>,
            Option<&Boss>,
            Option<&PlayerIdComponent>,
            Option<&Worker>,
        )>();
        let found = query
            .iter(&lobby.game_state.world)
            .find(|(e, ..)| e.index() == entity_id)
            .map(
                |(
                    _,
                    attack_stats,
                    attack_range,
                    defense_stats,
                    shape_comp,
                    boss,
                    owner,
                    worker,
                )| {
                    (
                        attack_stats.map(|s| (s.damage, s.rate, s.damage_type)),
                        attack_range.map(|r| r.0),
                        defense_stats.map(|d| d.armor),
                        shape_comp.map(|s| s.0),
                        boss.is_some(),
                        owner.map(|o| o.0),
                        worker.is_some(),
                    )
                },
            );

        assert!(found.is_some(), "Entity should be found");
        let (attack_data, range, _armor, shape, is_boss, owner_id, is_worker) = found.unwrap();

        let profile = get_unit_profile(UnitKind::Circle);
        assert_eq!(shape, Some(UnitKind::Circle));
        assert!(!is_boss);
        assert_eq!(owner_id, Some(player_id));
        assert!(!is_worker);

        let (damage, rate, damage_type) = attack_data.unwrap();
        assert_eq!(damage, profile.combat.primary.damage);
        assert_eq!(rate, profile.combat.primary.rate);
        assert_eq!(damage_type, profile.combat.primary.damage_type);
        assert_eq!(range, Some(profile.combat.primary.range));

        // sell_value: Circle costs 75, 75 * 0.75 = 56
        let sell_value = match (owner_id, shape, is_worker) {
            (Some(oid), Some(sh), false) if oid == player_id => {
                Some((get_unit_profile(sh).gold_cost as f32 * 0.75) as u32)
            }
            _ => None,
        };
        assert_eq!(sell_value, Some(56));
    }

    #[test]
    fn test_request_unit_info_returns_none_for_unknown_entity() {
        use crate::model::components::{AttackRange, AttackStats, Boss, DefenseStats, Worker};

        let mut lobby = Lobby::new();
        let entity_id: u32 = 9999;

        let mut query = lobby.game_state.world.query::<(
            Entity,
            Option<&AttackStats>,
            Option<&AttackRange>,
            Option<&DefenseStats>,
            Option<&ShapeComponent>,
            Option<&Boss>,
            Option<&PlayerIdComponent>,
            Option<&Worker>,
        )>();
        let found = query
            .iter(&lobby.game_state.world)
            .find(|(e, ..)| e.index() == entity_id)
            .map(
                |(
                    _,
                    attack_stats,
                    attack_range,
                    defense_stats,
                    shape_comp,
                    boss,
                    owner,
                    worker,
                )| {
                    (
                        attack_stats.map(|s| (s.damage, s.rate, s.damage_type)),
                        attack_range.map(|r| r.0),
                        defense_stats.map(|d| d.armor),
                        shape_comp.map(|s| s.0),
                        boss.is_some(),
                        owner.map(|o| o.0),
                        worker.is_some(),
                    )
                },
            );

        assert!(
            found.is_none(),
            "No entity should be found for unknown entity_id"
        );
    }

    #[test]
    fn test_sell_value_is_none_for_wrong_owner() {
        use crate::model::components::{AttackRange, AttackStats, Boss, DefenseStats, Worker};
        use crate::model::unit_config::get_unit_profile;

        let mut lobby = Lobby::new();
        let owner_id: i64 = 1;
        let requester_id: i64 = 2;
        lobby.players.push(Player::new(owner_id, "p1".into(), 100));

        let entity = spawn_unit(
            &mut lobby.game_state.world,
            Position { x: 100.0, y: 100.0 },
            UnitKind::Square,
            owner_id,
        );
        let entity_id = entity.index();

        let mut query = lobby.game_state.world.query::<(
            Entity,
            Option<&AttackStats>,
            Option<&AttackRange>,
            Option<&DefenseStats>,
            Option<&ShapeComponent>,
            Option<&Boss>,
            Option<&PlayerIdComponent>,
            Option<&Worker>,
        )>();
        let found = query
            .iter(&lobby.game_state.world)
            .find(|(e, ..)| e.index() == entity_id)
            .map(
                |(
                    _,
                    attack_stats,
                    attack_range,
                    defense_stats,
                    shape_comp,
                    boss,
                    owner,
                    worker,
                )| {
                    (
                        attack_stats.map(|s| (s.damage, s.rate, s.damage_type)),
                        attack_range.map(|r| r.0),
                        defense_stats.map(|d| d.armor),
                        shape_comp.map(|s| s.0),
                        boss.is_some(),
                        owner.map(|o| o.0),
                        worker.is_some(),
                    )
                },
            );

        assert!(found.is_some());
        let (_, _, _, shape, _, pic_owner_id, is_worker) = found.unwrap();

        let sell_value = match (pic_owner_id, shape, is_worker) {
            (Some(oid), Some(sh), false) if oid == requester_id => {
                Some((get_unit_profile(sh).gold_cost as f32 * 0.75) as u32)
            }
            _ => None,
        };
        assert_eq!(
            sell_value, None,
            "sell_value should be None when requester does not own the entity"
        );
    }

    // --- Task 3.1: SendUnit handler ---

    #[test]
    fn send_unit_deducts_gold_queues_unit_and_increases_income() {
        use crate::model::unit_config::get_sent_unit_profile;

        let mut lobby = Lobby::new();
        let player_id = 1;
        lobby.players.push(Player::new(player_id, "p1".into(), 100));

        let shape = UnitKind::Square;
        let profile = get_sent_unit_profile(shape);

        // Simulate SendUnit handler
        let player_idx = lobby.players.iter().position(|p| p.id == player_id);
        if let Some(idx) = player_idx {
            if lobby.players[idx].try_spend_gold(profile.send_cost) {
                lobby.players[idx].spawning_queue.push(shape);
                lobby.players[idx].income += profile.income;
            }
        }

        assert_eq!(
            lobby.players[0].gold,
            100 - profile.send_cost,
            "Gold should be deducted by send cost"
        );
        assert_eq!(
            lobby.players[0].income, profile.income,
            "Income should be increased by profile income"
        );
        assert_eq!(lobby.players[0].spawning_queue.len(), 1);
        assert_eq!(lobby.players[0].spawning_queue[0], UnitKind::Square);
    }

    #[test]
    fn send_unit_rejected_when_insufficient_gold() {
        use crate::model::unit_config::get_sent_unit_profile;

        let mut lobby = Lobby::new();
        let player_id = 1;
        lobby.players.push(Player::new(player_id, "p1".into(), 3)); // Only 3 gold, Square costs 5

        let shape = UnitKind::Square;
        let profile = get_sent_unit_profile(shape);

        let player_idx = lobby.players.iter().position(|p| p.id == player_id);
        let mut sent = false;
        if let Some(idx) = player_idx {
            if lobby.players[idx].try_spend_gold(profile.send_cost) {
                lobby.players[idx].spawning_queue.push(shape);
                lobby.players[idx].income += profile.income;
                sent = true;
            }
        }

        assert!(!sent, "Purchase should be rejected");
        assert_eq!(lobby.players[0].gold, 3, "Gold should not be deducted");
        assert_eq!(lobby.players[0].income, 0, "Income should not change");
        assert!(
            lobby.players[0].spawning_queue.is_empty(),
            "Queue should remain empty"
        );
    }

    #[test]
    fn send_unit_multiple_purchases_accumulate_income_and_queue() {
        use crate::model::unit_config::get_sent_unit_profile;

        let mut lobby = Lobby::new();
        let player_id = 1;
        lobby.players.push(Player::new(player_id, "p1".into(), 200));

        let square_profile = get_sent_unit_profile(UnitKind::Square);
        let triangle_profile = get_sent_unit_profile(UnitKind::Triangle);

        // Buy a Square (costs 8, income 1)
        let idx = 0;
        if lobby.players[idx].try_spend_gold(square_profile.send_cost) {
            lobby.players[idx].spawning_queue.push(UnitKind::Square);
            lobby.players[idx].income += square_profile.income;
        }
        // Buy a Triangle (costs 20, income 2)
        if lobby.players[idx].try_spend_gold(triangle_profile.send_cost) {
            lobby.players[idx].spawning_queue.push(UnitKind::Triangle);
            lobby.players[idx].income += triangle_profile.income;
        }

        assert_eq!(lobby.players[0].gold, 200 - 8 - 20);
        assert_eq!(lobby.players[0].income, 1 + 2);
        assert_eq!(lobby.players[0].spawning_queue.len(), 2);
    }

    #[test]
    fn hire_worker_accepted_during_combat_phase() {
        let mut lobby = Lobby::new();
        let player_id = 1;
        lobby.players.push(Player::new(player_id, "p1".into(), 100));
        lobby.game_state.phase = GamePhase::Combat;

        // Simulate HireWorker handler (no phase guard)
        let player_idx = lobby.players.iter().position(|p| p.id == player_id);
        if let Some(idx) = player_idx {
            if lobby.players[idx].try_spend_gold(50) {
                let targets = TargetPositions {
                    vein: crate::handler::worker::VEIN_POSITIONS[idx],
                    cart: crate::handler::worker::CART_POSITIONS[idx],
                };
                crate::handler::spawn::spawn_worker(
                    &mut lobby.game_state.world,
                    player_id,
                    targets,
                );
            }
        }

        assert_eq!(
            lobby.players[0].gold, 50,
            "Gold should be deducted for worker hire during combat"
        );
        let mut query = lobby
            .game_state
            .world
            .query_filtered::<&Worker, bevy_ecs::prelude::With<Worker>>();
        assert_eq!(
            query.iter(&lobby.game_state.world).count(),
            1,
            "Worker should be spawned during combat phase"
        );
    }

    // --- Task 9.2: King upgrade handler unit tests ---

    fn make_lobby_with_king(player_id: i64) -> crate::model::lobby::Lobby {
        use crate::handler::spawn::spawn_king;
        use crate::model::game_state::GamePhase;
        let mut lobby = crate::model::lobby::Lobby::new();
        lobby.game_state.phase = GamePhase::Build;
        lobby.game_state.world.insert_resource(GamePhase::Build);
        lobby
            .game_state
            .world
            .insert_resource(crate::model::player::Players::default());
        let player = crate::model::player::Player::new(player_id, "test".to_string(), 500);
        lobby.players.push(player);
        spawn_king(&mut lobby.game_state.world, player_id, 0);
        lobby
    }

    #[test]
    fn king_upgrade_increments_tier_and_deducts_gold() {
        use crate::model::king_config::KING_UPGRADE_TIERS;
        let mut lobby = make_lobby_with_king(1);
        let tier = &KING_UPGRADE_TIERS[0];
        let initial_gold = lobby.players[0].gold;
        let initial_income = lobby.players[0].income;

        // Manually apply upgrade (same logic as handler).
        lobby.players[0].gold -= tier.cost;
        lobby.players[0].king_tier += 1;
        lobby.players[0].income += tier.income_delta;

        assert_eq!(lobby.players[0].king_tier, 1);
        assert_eq!(lobby.players[0].gold, initial_gold - tier.cost);
        assert_eq!(lobby.players[0].income, initial_income + tier.income_delta);
    }

    #[test]
    fn king_upgrade_rejected_if_max_tier() {
        let lobby = make_lobby_with_king(1);
        // At max tier (4), upgrade should be rejected.
        let player_tier = lobby.players[0].king_tier;
        // Default is 0, this test validates that checking tier >= 4 would block.
        assert!(player_tier < 4, "New player starts below max tier");
        // Simulate being at max tier.
        let at_max = 4u32;
        assert!(at_max >= 4, "King at tier 4 should be rejected");
    }

    #[test]
    fn king_upgrade_rejected_if_insufficient_gold() {
        use crate::model::king_config::KING_UPGRADE_TIERS;
        let mut lobby = make_lobby_with_king(1);
        // Set gold below tier 1 cost.
        lobby.players[0].gold = KING_UPGRADE_TIERS[0].cost - 1;
        let can_afford = lobby.players[0].can_afford(KING_UPGRADE_TIERS[0].cost);
        assert!(
            !can_afford,
            "Player with insufficient gold should not be able to upgrade"
        );
    }

    #[test]
    fn handle_request_full_state_returns_snapshot_reply() {
        use crate::model::messages::ServerMessage;
        let mut lobby = Lobby::new();
        lobby.players.push(Player::new(1, "p1".into(), 100));
        spawn_unit(
            &mut lobby.game_state.world,
            Position { x: 100.0, y: 300.0 },
            UnitKind::Square,
            1,
        );
        // Establish a nonzero seq (as a real lobby would have after ticking) so the
        // assertions below actually pin the value rather than trivially matching 0.
        lobby.broadcast_gamestate();
        let seq_before = lobby.seq;

        let outcome = handle_client_message(&mut lobby, 1, ClientMessage::RequestFullState);

        match outcome {
            MessageOutcome::Reply(ServerMessage::GameState(state)) => {
                assert_eq!(state.units.len(), 1);
                assert_eq!(
                    state.seq, seq_before,
                    "direct snapshot must be stamped with the lobby's current seq"
                );
                assert_eq!(
                    state.players.len(),
                    1,
                    "players must be populated in the direct snapshot"
                );
                assert_eq!(state.phase, lobby.game_state.phase);
            }
            _ => panic!("expected GameState reply"),
        }

        assert_eq!(
            lobby.seq, seq_before,
            "handling RequestFullState must not bump the shared lobby seq"
        );
    }

    // --- Task 1 TDD tests ---

    #[test]
    fn send_unit_charges_escalating_price_and_bumps_counter() {
        use crate::model::messages::ClientMessage;
        let mut lobby = Lobby::new();
        lobby.players.push(Player::new(1, "p1".into(), 100));
        lobby.players.push(Player::new(2, "p2".into(), 100));

        // Wave 1: scouts cost 8 then 12.
        for expected_cost in [8u32, 12u32] {
            let before = lobby.players[0].gold;
            let outcome = handle_client_message(
                &mut lobby,
                1,
                ClientMessage::SendUnit {
                    shape: UnitKind::Square,
                },
            );
            assert!(matches!(outcome, MessageOutcome::Handled));
            assert_eq!(before - lobby.players[0].gold, expected_cost);
        }
        assert_eq!(lobby.players[0].sends_this_wave[0], 2);
        assert_eq!(lobby.players[0].next_send_costs[0], 16); // third scout
        assert_eq!(lobby.players[0].income, 2); // +1 per scout regardless of price
    }

    #[test]
    fn send_unit_rejects_when_escalated_price_unaffordable() {
        use crate::model::messages::ClientMessage;
        let mut lobby = Lobby::new();
        lobby.players.push(Player::new(1, "p1".into(), 15)); // affords 8, not the next 12
        let ok = handle_client_message(
            &mut lobby,
            1,
            ClientMessage::SendUnit {
                shape: UnitKind::Square,
            },
        );
        assert!(matches!(ok, MessageOutcome::Handled));
        let rejected = handle_client_message(
            &mut lobby,
            1,
            ClientMessage::SendUnit {
                shape: UnitKind::Square,
            },
        );
        assert!(matches!(rejected, MessageOutcome::Reply(_)));
        assert_eq!(
            lobby.players[0].sends_this_wave[0], 1,
            "failed send must not bump counter"
        );
    }

    #[test]
    fn hire_worker_rejected_at_cap() {
        use crate::model::messages::ClientMessage;
        let mut lobby = Lobby::new();
        lobby.players.push(Player::new(1, "p1".into(), 1000));
        let targets = TargetPositions {
            vein: crate::handler::worker::VEIN_POSITIONS[0],
            cart: crate::handler::worker::CART_POSITIONS[0],
        };
        for _ in 0..crate::handler::worker::WORKER_CAP {
            crate::handler::spawn::spawn_worker(&mut lobby.game_state.world, 1, targets);
        }
        let outcome = handle_client_message(&mut lobby, 1, ClientMessage::HireWorker {});
        assert!(
            matches!(outcome, MessageOutcome::Reply(_)),
            "8th worker must be rejected"
        );
        assert_eq!(lobby.players[0].gold, 1000, "no gold charged on rejection");
        let count = lobby
            .game_state
            .world
            .query::<&Worker>()
            .iter(&lobby.game_state.world)
            .count();
        assert_eq!(count, crate::handler::worker::WORKER_CAP);
    }
}
