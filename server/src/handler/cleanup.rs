use crate::{
    database,
    model::{components::PlayerIdComponent, game_state::GamePhase},
    state::ServerState,
};
use bevy_ecs::prelude::Entity;

pub async fn remove_player_from_match(match_id: u64, player_id: i64, server_state: &ServerState) {
    let Some(lobby_arc) = server_state.matches.read().await.get(&match_id).cloned() else {
        return; // match already torn down
    };
    let now_empty = {
        let mut lobby = lobby_arc.lock().await;
        let game_in_progress = lobby.is_full()
            && lobby.game_state.phase != GamePhase::GameOver
            && lobby.game_state.phase != GamePhase::Victory;

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
            lobby.broadcast_gamestate();
        }

        lobby.players.is_empty()
    }; // lobby guard dropped BEFORE taking the matches write lock (rule 3)

    if now_empty {
        server_state.matches.write().await.remove(&match_id);
    }
}

pub async fn cleanup(match_id: u64, player_id: i64, server_state: &ServerState) {
    remove_player_from_match(match_id, player_id, server_state).await;
    if let Err(e) = database::clear_session(&server_state.db_pool, player_id).await {
        log::error!("Failed to clear session for player {}: {}", player_id, e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handler::matchmaking::create_match;
    use crate::model::components::Position;
    use crate::state::ServerStateData;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn state_with_match(p1: i64, p2: i64) -> (crate::state::ServerState, u64) {
        let db_pool = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let state = ServerStateData::new(db_pool);
        let match_id = create_match(&state, (p1, "p1".into()), (p2, "p2".into())).await;
        (state, match_id)
    }

    async fn lock_lobby(
        state: &crate::state::ServerState,
        match_id: u64,
    ) -> std::sync::Arc<tokio::sync::Mutex<crate::model::lobby::Lobby>> {
        state.matches.read().await.get(&match_id).unwrap().clone()
    }

    #[tokio::test]
    async fn test_remove_player_despawns_entities() {
        let player_id = 123;
        let (state, match_id) = state_with_match(player_id, 456).await;

        {
            let arc = lock_lobby(&state, match_id).await;
            let mut lobby = arc.lock().await;

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

        remove_player_from_match(match_id, player_id, &state).await;

        {
            let arc = lock_lobby(&state, match_id).await;
            let mut lobby = arc.lock().await;

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
    async fn leaving_mid_game_declares_remaining_player_winner() {
        use crate::model::game_state::GamePhase;
        let (state, match_id) = state_with_match(1, 2).await;
        {
            let arc = lock_lobby(&state, match_id).await;
            let mut lobby = arc.lock().await;
            lobby.game_state.phase = GamePhase::Combat;
        }

        remove_player_from_match(match_id, 1, &state).await;

        let arc = lock_lobby(&state, match_id).await;
        let lobby = arc.lock().await;
        assert_eq!(lobby.game_state.phase, GamePhase::GameOver);
        assert_eq!(lobby.winner_id, Some(2));
    }

    #[tokio::test]
    async fn match_entry_removed_when_last_player_leaves() {
        let (state, match_id) = state_with_match(1, 2).await;

        remove_player_from_match(match_id, 1, &state).await;
        assert!(
            state.matches.read().await.contains_key(&match_id),
            "match must survive while one player remains"
        );

        remove_player_from_match(match_id, 2, &state).await;
        assert!(
            !state.matches.read().await.contains_key(&match_id),
            "match must be destroyed when the last player leaves"
        );
    }

    #[tokio::test]
    async fn removing_from_nonexistent_match_is_noop() {
        let db_pool = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let state = ServerStateData::new(db_pool);
        // Must not panic.
        remove_player_from_match(999, 1, &state).await;
    }

    #[tokio::test]
    async fn test_leaving_after_game_over_does_not_change_winner() {
        use crate::model::game_state::GamePhase;
        let winner_id = 1;
        let loser_id = 2;
        let (state, match_id) = state_with_match(winner_id, loser_id).await;

        {
            let arc = lock_lobby(&state, match_id).await;
            let mut lobby = arc.lock().await;
            lobby.game_state.phase = GamePhase::GameOver;
            lobby.winner_id = Some(winner_id);
        }

        // The loser leaves after the game already ended.
        remove_player_from_match(match_id, loser_id, &state).await;

        {
            let arc = lock_lobby(&state, match_id).await;
            let lobby = arc.lock().await;
            assert_eq!(
                lobby.winner_id,
                Some(winner_id),
                "Winner must not change when the loser leaves a finished game"
            );
        }
    }

    #[tokio::test]
    async fn test_leaving_pre_game_match_declares_no_winner() {
        use crate::model::game_state::GamePhase;
        let (state, match_id) = state_with_match(1, 2).await;
        // Hold the Arc so the Lobby stays inspectable even after the map entry is
        // removed when the last player leaves.
        let arc = lock_lobby(&state, match_id).await;
        {
            let mut lobby = arc.lock().await;
            // Simulate a not-yet-full match: only one player seated, no game started.
            lobby.players.retain(|p| p.id == 1);
        }

        remove_player_from_match(match_id, 1, &state).await;

        let lobby = arc.lock().await;
        assert_eq!(lobby.winner_id, None, "No winner without an active game");
        assert_ne!(
            lobby.game_state.phase,
            GamePhase::GameOver,
            "A not-full match should not be forced into GameOver on leave"
        );
    }

    #[tokio::test]
    async fn test_cleanup_despawns_entities() {
        let player_id = 789;
        let (state, match_id) = state_with_match(player_id, 2).await;

        {
            let arc = lock_lobby(&state, match_id).await;
            let mut lobby = arc.lock().await;
            lobby.game_state.world.spawn((
                Position { x: 100.0, y: 100.0 },
                PlayerIdComponent(player_id),
            ));
        }

        cleanup(match_id, player_id, &state).await;

        {
            let arc = lock_lobby(&state, match_id).await;
            let mut lobby = arc.lock().await;

            let mut query = lobby.game_state.world.query::<&PlayerIdComponent>();
            let owners: Vec<i64> = query.iter(&lobby.game_state.world).map(|o| o.0).collect();

            assert!(
                !owners.contains(&player_id),
                "Player's entity should be despawned during cleanup"
            );
        }
    }
}
