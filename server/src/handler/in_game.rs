use crate::{
    model::{
        components::{PlayerIdComponent, Position, TargetPositions},
        constants::SQUARE_SIZE,
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
                                    ClientMessage::Sell(s) => {
                                        let player_idx = lobby.players.iter().position(|pl| pl.id == player_id);
                                        let mut sell_data: Option<(Entity, crate::model::shape::Shape)> = None;

                                        if let Some(idx) = player_idx {
                                            if s.row >= 10 || s.col >= 10 { continue; }

                                            let x_min = if idx == 0 {
                                                s.col as f32 * SQUARE_SIZE
                                            } else {
                                                crate::model::constants::RIGHT_BOARD_START + (s.col as f32 * SQUARE_SIZE)
                                            };
                                            let x_max = x_min + SQUARE_SIZE;
                                            let y_min = s.row as f32 * SQUARE_SIZE;
                                            let y_max = y_min + SQUARE_SIZE;

                                            let mut query = lobby.game_state.world.query::<(Entity, &Position, &PlayerIdComponent, &crate::model::components::ShapeComponent)>();
                                            for (entity, position, owner, shape) in query.iter(&lobby.game_state.world) {
                                                if position.x >= x_min && position.x < x_max && position.y >= y_min && position.y < y_max && owner.0 == player_id {
                                                    sell_data = Some((entity, shape.0));
                                                    break;
                                                }
                                            }

                                            if let Some((entity, shape)) = sell_data {
                                                let profile = crate::model::unit_config::get_unit_profile(shape);
                                                let refund = (profile.gold_cost as f32 * 0.75) as u32;

                                                if let Some(player) = lobby.players.iter_mut().find(|p| p.id == player_id) {
                                                    player.add_gold(refund);
                                                }

                                                lobby.game_state.world.despawn(entity);
                                                lobby.broadcast_gamestate();
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
    use crate::model::components::ShapeComponent;
    use crate::model::lobby::Lobby;
    use crate::model::messages::{PlaceMessage, SellMessage};
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
    fn test_unit_selling_refunds_gold() {
        let mut lobby = Lobby::new();
        let player_id = 1;
        lobby.players.push(Player {
            id: player_id,
            username: "p1".into(),
            gold: 0,
        });

        // Manually spawn a Square (cost 25)
        let x = SQUARE_SIZE / 2.0;
        let y = SQUARE_SIZE / 2.0;
        let _ = crate::handler::spawn::spawn_unit(
            &mut lobby.game_state.world,
            Position { x, y },
            Shape::Square,
            player_id,
        );

        let s = SellMessage { row: 0, col: 0 };

        // --- SIMULATED logic ---
        let mut sell_data = None;
        let x_min = s.col as f32 * SQUARE_SIZE;
        let x_max = x_min + SQUARE_SIZE;
        let y_min = s.row as f32 * SQUARE_SIZE;
        let y_max = y_min + SQUARE_SIZE;

        let mut query =
            lobby
                .game_state
                .world
                .query::<(Entity, &Position, &PlayerIdComponent, &ShapeComponent)>();
        for (e, position, owner, shape) in query.iter(&lobby.game_state.world) {
            if position.x >= x_min
                && position.x < x_max
                && position.y >= y_min
                && position.y < y_max
                && owner.0 == player_id
            {
                sell_data = Some((e, shape.0));
                break;
            }
        }

        if let Some((e, shape)) = sell_data {
            let profile = crate::model::unit_config::get_unit_profile(shape);
            let refund = (profile.gold_cost as f32 * 0.75) as u32; // 25 * 0.75 = 18.75 -> 18
            let player = lobby
                .players
                .iter_mut()
                .find(|p| p.id == player_id)
                .unwrap();
            player.add_gold(refund);
            lobby.game_state.world.despawn(e);
        }
        // --- END SIMULATED logic ---

        assert_eq!(
            lobby.players[0].gold, 18,
            "Refund for Square (25g) should be 18g (75%)"
        );
        let mut query = lobby.game_state.world.query::<&ShapeComponent>();
        assert_eq!(
            query.iter(&lobby.game_state.world).count(),
            0,
            "Entity should be despawned"
        );
    }
}
