use crate::{
    model::{
        components::{
            AttackRange, AttackStats, Boss, DefenseStats, PlayerIdComponent, Position,
            ShapeComponent, TargetPositions, Worker,
        },
        constants::SQUARE_SIZE,
        game_state::GamePhase,
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

pub async fn in_game_loop(
    ws_sender: &mut SplitSink<UpgradedWebSocket, Message>,
    ws_receiver: &mut SplitStream<UpgradedWebSocket>,
    server_state: &ServerState,
    lobby_id: usize,
    player_id: i64,
    shutdown_rx: &mut mpsc::Receiver<()>,
) -> InGameLoopResult {
    let mut game_rx = {
        let lobbies = server_state.lobbies.lock().await;
        lobbies[lobby_id].tx.subscribe()
    };
    server_state.lobbies.lock().await[lobby_id].broadcast_gamestate();

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
                                let mut lobbies = server_state.lobbies.lock().await;
                                let lobby = &mut lobbies[lobby_id];
                                match client_msg {
                                    ClientMessage::Place(p) => {
                                        if lobby.game_state.phase != GamePhase::Build {
                                            let _ = crate::routes::ws::send_message(ws_sender, crate::model::messages::ServerMessage::Error("Tower placement is only allowed during the build phase.".into())).await;
                                            continue;
                                        }
                                        let profile = crate::model::unit_config::get_unit_profile(p.shape);
                                        let player_idx = lobby.players.iter().position(|pl| pl.id == player_id);

                                        if let Some(idx) = player_idx {
                                            if p.row >= 10 || p.col >= 10 {
                                                let _ = crate::routes::ws::send_message(ws_sender, crate::model::messages::ServerMessage::Error("Invalid placement coordinates.".into())).await;
                                                continue;
                                            }

                                            let x = if idx == 0 {
                                                (p.col as f32 * SQUARE_SIZE) + (SQUARE_SIZE / 2.0)
                                            } else {
                                                crate::model::constants::RIGHT_BOARD_START + (p.col as f32 * SQUARE_SIZE) + (SQUARE_SIZE / 2.0)
                                            };
                                            let y = (p.row as f32 * SQUARE_SIZE) + (SQUARE_SIZE / 2.0);

                                            if lobby.players[idx].try_spend_gold(profile.gold_cost) {
                                                crate::handler::spawn::spawn_unit(
                                                    &mut lobby.game_state.world,
                                                    Position { x, y },
                                                    p.shape,
                                                    player_id,
                                                );
                                                lobby.broadcast_gamestate();
                                            } else {
                                                let error_msg = format!("Insufficient gold for {:?} (cost: {})", p.shape, profile.gold_cost);
                                                let _ = crate::routes::ws::send_message(ws_sender, crate::model::messages::ServerMessage::Error(error_msg)).await;
                                            }
                                        }
                                    }
                                    ClientMessage::SkipToCombat => {
                                        lobby.game_state.phase_timer = 0.0;
                                    }
                                    ClientMessage::HireWorker {} => {
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
                                                lobby.broadcast_gamestate();
                                            } else {
                                                let _ = crate::routes::ws::send_message(ws_sender, crate::model::messages::ServerMessage::Error("Insufficient gold for Worker (cost: 50)".into())).await;
                                            }
                                        }
                                    }
                                    ClientMessage::LeaveLobby => break InGameLoopResult::PlayerLeft,
                                    ClientMessage::SellById { entity_id } => {
                                        if lobby.game_state.phase != GamePhase::Build {
                                            let _ = crate::routes::ws::send_message(ws_sender, crate::model::messages::ServerMessage::Error("Tower selling is only allowed during the build phase.".into())).await;
                                            continue;
                                        }

                                        let mut query = lobby.game_state.world.query::<(Entity, &PlayerIdComponent, &ShapeComponent)>();
                                        let found = query
                                            .iter(&lobby.game_state.world)
                                            .find(|(e, owner, _)| e.index() == entity_id && owner.0 == player_id)
                                            .map(|(entity, _, shape)| (entity, shape.0));

                                        if let Some((entity, shape)) = found {
                                            let profile = crate::model::unit_config::get_unit_profile(shape);
                                            let refund = (profile.gold_cost as f32 * 0.75) as u32;
                                            if let Some(player) = lobby.players.iter_mut().find(|p| p.id == player_id) {
                                                player.gold += refund;
                                            }
                                            lobby.game_state.world.despawn(entity);
                                            lobby.broadcast_gamestate();
                                        }
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
                                            .find(|(entity, ..)| entity.index() == entity_id)
                                            .map(|(_, attack_stats, attack_range, defense_stats, shape_comp, boss, owner, worker)| (
                                                attack_stats.map(|s| s.damage),
                                                attack_stats.map(|s| s.rate),
                                                attack_stats.map(|s| s.damage_type),
                                                attack_range.map(|r| r.0),
                                                defense_stats.map(|d| d.armor),
                                                shape_comp.map(|s| s.0),
                                                boss.is_some(),
                                                owner.map(|o| o.0),
                                                worker.is_some(),
                                            ));

                                        if let Some((attack_damage, attack_rate, damage_type, attack_range, armor, shape, is_boss, owner_id, is_worker)) = found {
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
                                            let _ = crate::routes::ws::send_message(ws_sender, crate::model::messages::ServerMessage::UnitInfo(info)).await;
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    },
                    Some(Err(_)) | None => break InGameLoopResult::ClientDisconnected,
                }
            },
            Ok(msg) = game_rx.recv() => {
                if ws_sender.send(Message::Text(msg.into())).await.is_err() { break InGameLoopResult::ClientDisconnected; }
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
    use crate::model::shape::Shape;

    #[test]
    fn test_unit_placement_restricted_by_player_id() {
        use crate::model::constants::{
            LEFT_BOARD_END, RIGHT_BOARD_END, RIGHT_BOARD_START, SQUARE_SIZE,
        };
        let mut lobby = Lobby::new();

        let p1_id = 1;
        let p2_id = 2;
        lobby.players.push(Player {
            id: p1_id,
            username: "p1".into(),
            gold: 100,
        });
        lobby.players.push(Player {
            id: p2_id,
            username: "p2".into(),
            gold: 100,
        });

        // Player 0 (index 0) is P1. Board is 0-600.
        // Player 1 (index 1) is P2. Board is 800-1400.

        // Valid placements
        let p1_valid = PlaceMessage {
            shape: Shape::Square,
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
            shape: Shape::Square,
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
        lobby.players.push(Player {
            id: player_id,
            username: "p1".into(),
            gold: 100,
        });

        let p = PlaceMessage {
            shape: Shape::Square,
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
        lobby.players.push(Player {
            id: player_id,
            username: "p1".into(),
            gold: 10,
        });

        let p = PlaceMessage {
            shape: Shape::Square,
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
        lobby.players.push(Player {
            id: player_id,
            username: "p1".into(),
            gold: 100,
        });
        lobby.game_state.phase = GamePhase::Combat;

        let p = PlaceMessage {
            shape: Shape::Square,
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
        lobby.players.push(Player {
            id: player_id,
            username: "p1".into(),
            gold: 100,
        });
        // Default phase is Build

        let p = PlaceMessage {
            shape: Shape::Square,
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

    // --- SellById tests ---

    #[test]
    fn test_sell_by_id_refunds_gold_in_build_phase() {
        use crate::model::unit_config::get_unit_profile;

        let mut lobby = Lobby::new();
        let player_id: i64 = 1;
        lobby.players.push(Player {
            id: player_id,
            username: "p1".into(),
            gold: 100,
        });

        let entity = spawn_unit(
            &mut lobby.game_state.world,
            Position { x: 100.0, y: 100.0 },
            Shape::Square,
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
        lobby.players.push(Player {
            id: player_id,
            username: "p1".into(),
            gold: 100,
        });
        lobby.game_state.phase = GamePhase::Combat;

        let entity = spawn_unit(
            &mut lobby.game_state.world,
            Position { x: 100.0, y: 100.0 },
            Shape::Square,
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
        lobby.players.push(Player {
            id: player_1_id,
            username: "p1".into(),
            gold: 100,
        });
        lobby.players.push(Player {
            id: player_2_id,
            username: "p2".into(),
            gold: 100,
        });

        let entity = spawn_unit(
            &mut lobby.game_state.world,
            Position { x: 100.0, y: 100.0 },
            Shape::Square,
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
        lobby.players.push(Player {
            id: player_id,
            username: "p1".into(),
            gold: 100,
        });

        let entity = spawn_unit(
            &mut lobby.game_state.world,
            Position { x: 100.0, y: 100.0 },
            Shape::Circle,
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

        let profile = get_unit_profile(Shape::Circle);
        assert_eq!(shape, Some(Shape::Circle));
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
        lobby.players.push(Player {
            id: owner_id,
            username: "p1".into(),
            gold: 100,
        });

        let entity = spawn_unit(
            &mut lobby.game_state.world,
            Position { x: 100.0, y: 100.0 },
            Shape::Square,
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

    #[test]
    fn hire_worker_accepted_during_combat_phase() {
        let mut lobby = Lobby::new();
        let player_id = 1;
        lobby.players.push(Player {
            id: player_id,
            username: "p1".into(),
            gold: 100,
        });
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
}
