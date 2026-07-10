use crate::{
    database,
    model::{components::PlayerIdComponent, game_state::GamePhase},
    routes::ws::broadcast_lobby_status,
    state::ServerState,
};
use bevy_ecs::prelude::Entity;

pub async fn remove_player_from_lobby(lobby_id: usize, player_id: i64, server_state: &ServerState) {
    {
        let mut lobbies = server_state.lobbies.lock().await;
        if let Some(lobby) = lobbies.get_mut(lobby_id) {
            // A game is in progress if the lobby was full and no result exists yet.
            let game_in_progress = lobby.is_full()
                && lobby.game_state.phase != GamePhase::GameOver
                && lobby.game_state.phase != GamePhase::Victory;

            // Despawn all entities belonging to this player
            let mut entities_to_despawn = Vec::new();
            {
                let mut query = lobby
                    .game_state
                    .world
                    .query::<(Entity, &PlayerIdComponent)>();
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

            if game_in_progress {
                // Leaving an active game forfeits: the remaining player wins.
                lobby.game_state.phase = GamePhase::GameOver;
                lobby.game_state.world.insert_resource(GamePhase::GameOver);
                lobby.winner_id = lobby.players.first().map(|p| p.id);
                // Bump the generation so the running game loop exits.
                lobby.game_generation += 1;
                lobby.broadcast_gamestate();
            }

            if lobby.players.is_empty() {
                lobby.game_state.reset();
                lobby.winner_id = None;
                lobby.game_generation += 1;
            }
        }
    }
    broadcast_lobby_status(server_state).await;
}

pub async fn cleanup(lobby_id: usize, player_id: i64, server_state: &ServerState) {
    remove_player_from_lobby(lobby_id, player_id, server_state).await;
    if let Err(e) = database::clear_session(&server_state.db_pool, player_id).await {
        log::error!("Failed to clear session for player {}: {}", player_id, e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::components::Position;
    use crate::model::player::Player;
    use crate::state::ServerStateData;
    use sqlx::sqlite::SqlitePoolOptions;

    #[tokio::test]
    async fn test_remove_player_despawns_entities() {
        // Mock DB pool (won't actually be used by remove_player_from_lobby)
        let db_pool = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let server_state = ServerStateData::new(db_pool);
        let player_id = 123;

        {
            let mut lobbies = server_state.lobbies.lock().await;
            let lobby = &mut lobbies[0];
            lobby
                .players
                .push(Player::new(player_id, "test".into(), 100));
            lobby.players.push(Player::new(456, "other".into(), 100)); // Keep lobby alive

            // Spawn an entity for the player
            lobby.game_state.world.spawn((
                Position { x: 100.0, y: 100.0 },
                PlayerIdComponent(player_id),
            ));

            // Spawn an entity for another player
            lobby
                .game_state
                .world
                .spawn((Position { x: 200.0, y: 200.0 }, PlayerIdComponent(456)));
        }

        remove_player_from_lobby(0, player_id, &server_state).await;

        {
            let mut lobbies = server_state.lobbies.lock().await;
            let lobby = &mut lobbies[0];

            let mut query = lobby.game_state.world.query::<&PlayerIdComponent>();
            let owners: Vec<i64> = query.iter(&lobby.game_state.world).map(|o| o.0).collect();

            assert!(
                !owners.contains(&player_id),
                "Player's entity should be despawned"
            );
            assert!(owners.contains(&456), "Other player's entity should remain");
            assert_eq!(
                lobby.players.len(),
                1,
                "Player should be removed from lobby list"
            );
        }
    }

    #[tokio::test]
    async fn test_leaving_mid_game_declares_remaining_player_winner() {
        use crate::model::game_state::GamePhase;

        let db_pool = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let server_state = ServerStateData::new(db_pool);
        let leaver_id = 1;
        let stayer_id = 2;

        let initial_generation;
        {
            let mut lobbies = server_state.lobbies.lock().await;
            let lobby = &mut lobbies[0];
            lobby.players.push(Player::new(leaver_id, "p1".into(), 100));
            lobby.players.push(Player::new(stayer_id, "p2".into(), 100));
            lobby.game_state.phase = GamePhase::Combat;
            initial_generation = lobby.game_generation;
        }

        remove_player_from_lobby(0, leaver_id, &server_state).await;

        {
            let mut lobbies = server_state.lobbies.lock().await;
            let lobby = &mut lobbies[0];
            assert_eq!(
                lobby.game_state.phase,
                GamePhase::GameOver,
                "Game should end when a player leaves mid-game"
            );
            assert_eq!(
                lobby.winner_id,
                Some(stayer_id),
                "The remaining player should be declared the winner"
            );
            assert!(
                lobby.game_generation > initial_generation,
                "Generation should be bumped so the old game loop exits"
            );
        }
    }

    #[tokio::test]
    async fn test_leaving_after_game_over_does_not_change_winner() {
        use crate::model::game_state::GamePhase;

        let db_pool = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let server_state = ServerStateData::new(db_pool);
        let winner_id = 1;
        let loser_id = 2;

        {
            let mut lobbies = server_state.lobbies.lock().await;
            let lobby = &mut lobbies[0];
            lobby.players.push(Player::new(winner_id, "p1".into(), 100));
            lobby.players.push(Player::new(loser_id, "p2".into(), 100));
            lobby.game_state.phase = GamePhase::GameOver;
            lobby.winner_id = Some(winner_id);
        }

        // The loser leaves after the game already ended.
        remove_player_from_lobby(0, loser_id, &server_state).await;

        {
            let mut lobbies = server_state.lobbies.lock().await;
            let lobby = &mut lobbies[0];
            assert_eq!(
                lobby.winner_id,
                Some(winner_id),
                "Winner must not change when the loser leaves a finished game"
            );
        }
    }

    #[tokio::test]
    async fn test_leaving_pre_game_lobby_declares_no_winner() {
        use crate::model::game_state::GamePhase;

        let db_pool = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let server_state = ServerStateData::new(db_pool);

        {
            let mut lobbies = server_state.lobbies.lock().await;
            let lobby = &mut lobbies[0];
            // Only one player waiting; no game has started.
            lobby.players.push(Player::new(1, "p1".into(), 100));
        }

        remove_player_from_lobby(0, 1, &server_state).await;

        {
            let mut lobbies = server_state.lobbies.lock().await;
            let lobby = &mut lobbies[0];
            assert_eq!(lobby.winner_id, None, "No winner without an active game");
            assert_ne!(
                lobby.game_state.phase,
                GamePhase::GameOver,
                "Empty pre-game lobby should not be in GameOver state"
            );
        }
    }

    #[tokio::test]
    async fn test_winner_cleared_when_lobby_empties() {
        use crate::model::game_state::GamePhase;

        let db_pool = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let server_state = ServerStateData::new(db_pool);

        {
            let mut lobbies = server_state.lobbies.lock().await;
            let lobby = &mut lobbies[0];
            lobby.players.push(Player::new(1, "p1".into(), 100));
            lobby.players.push(Player::new(2, "p2".into(), 100));
            lobby.game_state.phase = GamePhase::GameOver;
            lobby.winner_id = Some(1);
        }

        remove_player_from_lobby(0, 1, &server_state).await;
        remove_player_from_lobby(0, 2, &server_state).await;

        {
            let mut lobbies = server_state.lobbies.lock().await;
            let lobby = &mut lobbies[0];
            assert_eq!(
                lobby.winner_id, None,
                "winner_id must be cleared when the lobby resets for a new game"
            );
            assert_eq!(
                lobby.game_state.phase,
                GamePhase::Build,
                "Reset lobby should be back in Build phase"
            );
        }
    }

    #[tokio::test]
    async fn test_cleanup_despawns_entities() {
        let db_pool = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let server_state = ServerStateData::new(db_pool);
        let player_id = 789;

        {
            let mut lobbies = server_state.lobbies.lock().await;
            let lobby = &mut lobbies[0];
            lobby
                .players
                .push(Player::new(player_id, "test".into(), 100));

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

            assert!(
                !owners.contains(&player_id),
                "Player's entity should be despawned during cleanup"
            );
        }
    }
}
