use crate::{database, model::{components::PlayerIdComponent, game_state::GameState}, routes::ws::broadcast_lobby_status, state::ServerState};
use bevy_ecs::prelude::Entity;

pub async fn remove_player_from_lobby(
    lobby_id: usize,
    player_id: i64,
    server_state: &ServerState,
) {
    {
        let mut lobbies = server_state.lobbies.lock().await;
        if let Some(lobby) = lobbies.get_mut(lobby_id) {
            // Despawn all entities belonging to this player
            let mut entities_to_despawn = Vec::new();
            {
                let mut query = lobby.game_state.world.query::<(Entity, &PlayerIdComponent)>();
                for (entity, owner) in query.iter(&lobby.game_state.world) {
                    if owner.0 == player_id {
                        entities_to_despawn.push(entity);
                    }
                }
            }
            for entity in entities_to_despawn {
                let _ = lobby.game_state.world.despawn(entity);
            }

            lobby.players.retain(|p| p.id != player_id);
            if lobby.players.is_empty() {
                lobby.game_state = GameState::new();
            }
        }
    }
    broadcast_lobby_status(server_state).await;
}

pub async fn cleanup(
    lobby_id: usize,
    player_id: i64,
    server_state: &ServerState,
) {
    remove_player_from_lobby(lobby_id, player_id, server_state).await;
    if let Err(e) = database::clear_session(&server_state.db_pool, player_id).await {
        log::error!("Failed to clear session for player {}: {}", player_id, e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::player::Player;
    use crate::model::components::Position;
    use crate::state::ServerStateData;
    use sqlx::sqlite::SqlitePoolOptions;

    #[tokio::test]
    async fn test_remove_player_despawns_entities() {
        // Mock DB pool (won't actually be used by remove_player_from_lobby)
        let db_pool = SqlitePoolOptions::new().connect("sqlite::memory:").await.unwrap();
        let server_state = ServerStateData::new(db_pool);
        let player_id = 123;
        
        {
            let mut lobbies = server_state.lobbies.lock().await;
            let lobby = &mut lobbies[0];
            lobby.players.push(Player { id: player_id, username: "test".into(), gold: 100 });
            lobby.players.push(Player { id: 456, username: "other".into(), gold: 100 }); // Keep lobby alive
            
            // Spawn an entity for the player
            lobby.game_state.world.spawn((
                Position { x: 100.0, y: 100.0 },
                PlayerIdComponent(player_id),
            ));
            
            // Spawn an entity for another player
            lobby.game_state.world.spawn((
                Position { x: 200.0, y: 200.0 },
                PlayerIdComponent(456),
            ));
        }

        remove_player_from_lobby(0, player_id, &server_state).await;

        {
            let mut lobbies = server_state.lobbies.lock().await;
            let lobby = &mut lobbies[0];
            
            let mut query = lobby.game_state.world.query::<&PlayerIdComponent>();
            let owners: Vec<i64> = query.iter(&lobby.game_state.world).map(|o| o.0).collect();
            
            assert!(!owners.contains(&player_id), "Player's entity should be despawned");
            assert!(owners.contains(&456), "Other player's entity should remain");
            assert_eq!(lobby.players.len(), 1, "Player should be removed from lobby list");
        }
    }

    #[tokio::test]
    async fn test_cleanup_despawns_entities() {
        let db_pool = SqlitePoolOptions::new().connect("sqlite::memory:").await.unwrap();
        let server_state = ServerStateData::new(db_pool);
        let player_id = 789;
        
        {
            let mut lobbies = server_state.lobbies.lock().await;
            let lobby = &mut lobbies[0];
            lobby.players.push(Player { id: player_id, username: "test".into(), gold: 100 });
            
            lobby.game_state.world.spawn((
                Position { x: 100.0, y: 100.0 },
                PlayerIdComponent(player_id),
            ));
        }

        cleanup(0, player_id, &server_state).await;

        {
            let mut lobbies = server_state.lobbies.lock().await;
            let lobby = &mut lobbies[0];
            
            let mut query = lobby.game_state.world.query::<&PlayerIdComponent>();
            let owners: Vec<i64> = query.iter(&lobby.game_state.world).map(|o| o.0).collect();
            
            assert!(!owners.contains(&player_id), "Player's entity should be despawned during cleanup");
        }
    }
}
