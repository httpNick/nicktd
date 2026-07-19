use crate::model::{lobby::Lobby, player::Player};
use crate::state::{QueuedPlayer, ServerState};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use tokio::sync::{Mutex, oneshot};

pub enum JoinQueueOutcome {
    /// Stored as the waiter; the receiver fires with the match_id when paired.
    Waiting(oneshot::Receiver<u64>),
    /// Paired immediately with the player who was waiting.
    Matched(u64),
}

/// Pairs the caller with the waiting player, or stores the caller as the waiter.
/// All pairing happens under the queue mutex, so double-pairing is impossible.
/// A waiter whose oneshot receiver has been dropped (connection died) is
/// discarded rather than matched. Re-queueing by the same account replaces the
/// old entry (its receiver gets Err, which the old task treats as a no-op).
pub async fn join_queue(
    state: &ServerState,
    account_id: i64,
    username: String,
) -> JoinQueueOutcome {
    let mut queue = state.queue.lock().await;
    match queue.take() {
        Some(waiter) if waiter.account_id != account_id && !waiter.match_tx.is_closed() => {
            let match_id = create_match(
                state,
                (waiter.account_id, waiter.username.clone()),
                (account_id, username.clone()),
            )
            .await;
            if waiter.match_tx.send(match_id).is_err() {
                // Receiver dropped between the is_closed check and the send:
                // destroy the ghost match and make the joiner the new waiter.
                state.matches.write().await.remove(&match_id);
                let (tx, rx) = oneshot::channel();
                *queue = Some(QueuedPlayer {
                    account_id,
                    username,
                    match_tx: tx,
                });
                return JoinQueueOutcome::Waiting(rx);
            }
            JoinQueueOutcome::Matched(match_id)
        }
        _ => {
            // Queue empty, same-account re-queue, or dead waiter: become the waiter.
            let (tx, rx) = oneshot::channel();
            *queue = Some(QueuedPlayer {
                account_id,
                username,
                match_tx: tx,
            });
            JoinQueueOutcome::Waiting(rx)
        }
    }
}

/// Removes this account's queue entry. Returns false when the account is not
/// the current waiter — including when a pairing in flight already took the
/// entry (the caller's oneshot will fire; the match wins).
pub async fn leave_queue(state: &ServerState, account_id: i64) -> bool {
    let mut queue = state.queue.lock().await;
    if queue.as_ref().is_some_and(|w| w.account_id == account_id) {
        *queue = None;
        true
    } else {
        false
    }
}

/// Creates a lobby containing both players, registers it in `matches`, and
/// returns the new match_id. The game loop is spawned here from Task 3 onward.
pub async fn create_match(state: &ServerState, p1: (i64, String), p2: (i64, String)) -> u64 {
    let match_id = state.next_match_id.fetch_add(1, Ordering::Relaxed);
    let mut lobby = Lobby::new();
    lobby.players.push(Player::new(p1.0, p1.1, 100));
    lobby.players.push(Player::new(p2.0, p2.1, 100));
    state
        .matches
        .write()
        .await
        .insert(match_id, Arc::new(Mutex::new(lobby)));
    tokio::spawn(crate::handler::game_loop::run_game_loop(
        state.clone(),
        match_id,
    ));
    match_id
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::ServerStateData;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn test_state() -> crate::state::ServerState {
        let db_pool = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();
        ServerStateData::new(db_pool)
    }

    #[tokio::test]
    async fn first_joiner_waits() {
        let state = test_state().await;
        let outcome = join_queue(&state, 1, "a".into()).await;
        assert!(matches!(outcome, JoinQueueOutcome::Waiting(_)));
        assert!(state.queue.lock().await.is_some());
        assert!(state.matches.read().await.is_empty());
    }

    #[tokio::test]
    async fn second_joiner_pairs_with_waiter() {
        let state = test_state().await;
        let JoinQueueOutcome::Waiting(rx) = join_queue(&state, 1, "a".into()).await else {
            panic!("first joiner must wait");
        };
        let JoinQueueOutcome::Matched(match_id) = join_queue(&state, 2, "b".into()).await else {
            panic!("second joiner must match");
        };
        assert_eq!(
            rx.await.unwrap(),
            match_id,
            "waiter must be notified with the same match_id"
        );
        assert!(
            state.queue.lock().await.is_none(),
            "queue must be empty after pairing"
        );

        let matches = state.matches.read().await;
        let lobby = matches.get(&match_id).unwrap().lock().await;
        assert_eq!(lobby.players.len(), 2);
        let ids: Vec<i64> = lobby.players.iter().map(|p| p.id).collect();
        assert!(ids.contains(&1) && ids.contains(&2));
    }

    #[tokio::test]
    async fn rejoin_by_same_account_replaces_entry_not_self_match() {
        let state = test_state().await;
        let _rx1 = join_queue(&state, 1, "a".into()).await;
        let outcome = join_queue(&state, 1, "a".into()).await;
        assert!(
            matches!(outcome, JoinQueueOutcome::Waiting(_)),
            "same account re-queue must wait, never self-match"
        );
        assert!(state.matches.read().await.is_empty());
    }

    #[tokio::test]
    async fn dead_waiter_is_discarded_joiner_becomes_waiter() {
        let state = test_state().await;
        let JoinQueueOutcome::Waiting(rx) = join_queue(&state, 1, "a".into()).await else {
            panic!();
        };
        drop(rx); // waiter's connection died; oneshot receiver gone
        let outcome = join_queue(&state, 2, "b".into()).await;
        assert!(
            matches!(outcome, JoinQueueOutcome::Waiting(_)),
            "joiner must become the new waiter, not match a dead one"
        );
        assert!(
            state.matches.read().await.is_empty(),
            "no ghost match may be created"
        );
    }

    #[tokio::test]
    async fn leave_queue_removes_own_entry() {
        let state = test_state().await;
        let _rx = join_queue(&state, 1, "a".into()).await;
        assert!(leave_queue(&state, 1).await);
        assert!(state.queue.lock().await.is_none());
        // Leaving when not queued is a no-op returning false.
        assert!(!leave_queue(&state, 1).await);
    }

    #[tokio::test]
    async fn leave_queue_does_not_remove_someone_else() {
        let state = test_state().await;
        let _rx = join_queue(&state, 1, "a".into()).await;
        assert!(!leave_queue(&state, 2).await);
        assert!(
            state.queue.lock().await.is_some(),
            "player 1 must still be queued"
        );
    }

    #[tokio::test]
    async fn create_match_assigns_unique_ids() {
        let state = test_state().await;
        let a = create_match(&state, (1, "a".into()), (2, "b".into())).await;
        let b = create_match(&state, (3, "c".into()), (4, "d".into())).await;
        assert_ne!(a, b);
        assert_eq!(state.matches.read().await.len(), 2);
    }
}
