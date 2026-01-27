use crate::model::{
    components::{Position, Worker, WorkerState, MiningTimer, PlayerIdComponent, TargetPositions},
    lobby::Lobby,
};
use bevy_ecs::prelude::{Entity, With};

pub const WORKER_SPEED: f32 = 50.0;

pub const VEIN_POSITIONS: [Position; 2] = [
    Position { x: 700.0, y: 50.0 },
    Position { x: 700.0, y: 350.0 },
];
pub const CART_POSITIONS: [Position; 2] = [
    Position { x: 700.0, y: 250.0 },
    Position { x: 700.0, y: 550.0 },
];

enum WorkerAction {
    StartMining(Entity),
    FinishMining(Entity),
    DepositAndRestart(Entity, i64),
}

pub fn update_workers(lobby: &mut Lobby, tick_delta: f32) {
    if matches!(lobby.game_state.phase, crate::model::game_state::GamePhase::Build) {
        return;
    }

    let mut actions = Vec::new();

    let mut query = lobby.game_state.world.query_filtered::<(Entity, &mut Position, &mut WorkerState, Option<&mut MiningTimer>, &PlayerIdComponent, &TargetPositions), With<Worker>>();

    for (entity, mut pos, mut state, mut timer_opt, player_id, targets) in query.iter_mut(&mut lobby.game_state.world) {
        match *state {
            WorkerState::MovingToVein => {
                let target = targets.vein;
                let dx = target.x - pos.x;
                let dy = target.y - pos.y;
                let dist = (dx*dx + dy*dy).sqrt();
                let move_dist = WORKER_SPEED * tick_delta;

                if dist <= move_dist {
                    pos.x = target.x;
                    pos.y = target.y;
                    *state = WorkerState::Mining;
                    actions.push(WorkerAction::StartMining(entity));
                } else {
                    pos.x += (dx/dist) * move_dist;
                    pos.y += (dy/dist) * move_dist;
                }
            }
            WorkerState::Mining => {
                if let Some(ref mut timer) = timer_opt {
                    timer.0 -= tick_delta;
                    if timer.0 <= 0.0 {
                        *state = WorkerState::MovingToCart;
                        actions.push(WorkerAction::FinishMining(entity));
                    }
                }
            }
            WorkerState::MovingToCart => {
                let target = targets.cart;
                let dx = target.x - pos.x;
                let dy = target.y - pos.y;
                let dist = (dx*dx + dy*dy).sqrt();
                let move_dist = WORKER_SPEED * tick_delta;

                if dist <= move_dist {
                    pos.x = target.x;
                    pos.y = target.y;
                    *state = WorkerState::MovingToVein;
                    actions.push(WorkerAction::DepositAndRestart(entity, player_id.0));
                } else {
                    pos.x += (dx/dist) * move_dist;
                    pos.y += (dy/dist) * move_dist;
                }
            }
        }
    }

    for action in actions {
        match action {
            WorkerAction::StartMining(e) => {
                lobby.game_state.world.entity_mut(e).insert(MiningTimer(10.0));
            }
            WorkerAction::FinishMining(e) => {
                lobby.game_state.world.entity_mut(e).remove::<MiningTimer>();
            }
            WorkerAction::DepositAndRestart(_e, pid) => {
                if let Some(player) = lobby.players.iter_mut().find(|p| p.id == pid) {
                    player.gold += 1;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::player::Player;

    #[test]
    fn worker_behavior_cycle() {
        let mut lobby = Lobby::new();
        lobby.game_state.phase = crate::model::game_state::GamePhase::Combat;
        // Add player
        lobby.players.push(Player { id: 1, username: "test".to_string(), gold: 0 });
        
        let targets = TargetPositions {
            vein: VEIN_POSITIONS[0],
            cart: CART_POSITIONS[0],
        };

        // Spawn Worker
        let worker = lobby.game_state.world.spawn((
            targets.cart,
            Worker,
            WorkerState::MovingToVein,
            PlayerIdComponent(1),
            targets,
        )).id();

        let tick_delta = 1.0 / 30.0;

        // 1. Check Movement
        update_workers(&mut lobby, tick_delta); 
        let pos = lobby.game_state.world.entity(worker).get::<Position>().unwrap();
        assert!(pos.y < CART_POSITIONS[0].y, "Worker should move North (decrease Y)");

        // 2. Check Arrival at Vein -> Mining
        // Teleport to Vein
        lobby.game_state.world.entity_mut(worker).insert(VEIN_POSITIONS[0]);
        update_workers(&mut lobby, tick_delta);
        
        // State should be Mining
        let state = lobby.game_state.world.entity(worker).get::<WorkerState>().unwrap();
        assert_eq!(*state, WorkerState::Mining);
        // Should have MiningTimer
        let timer = lobby.game_state.world.entity(worker).get::<MiningTimer>();
        assert!(timer.is_some());

        // 3. Check Mining Timer Expire -> MovingToCart
        // Set timer to 0
        lobby.game_state.world.entity_mut(worker).insert(MiningTimer(0.0));
        update_workers(&mut lobby, tick_delta);
        
        let state = lobby.game_state.world.entity(worker).get::<WorkerState>().unwrap();
        assert_eq!(*state, WorkerState::MovingToCart);
        
        // 4. Check Arrival at Cart -> Deposit
        // Teleport to Cart
        lobby.game_state.world.entity_mut(worker).insert(CART_POSITIONS[0]);
        update_workers(&mut lobby, tick_delta);
        
        // State should be MovingToVein
        let state = lobby.game_state.world.entity(worker).get::<WorkerState>().unwrap();
        assert_eq!(*state, WorkerState::MovingToVein);
        
        // Player gold should increase
        assert_eq!(lobby.players[0].gold, 1);
    }

    #[test]
    fn test_workers_stay_idle_during_build_phase() {
        let mut lobby = Lobby::new();
        lobby.players.push(Player { id: 1, username: "test".to_string(), gold: 100 });
        
        // Initial state is Build phase
        assert!(matches!(lobby.game_state.phase, crate::model::game_state::GamePhase::Build));

        let initial_pos = Position { x: 700.0, y: 250.0 };
        let targets = TargetPositions {
            vein: VEIN_POSITIONS[0],
            cart: CART_POSITIONS[0],
        };

        // Spawn Worker
        let worker = lobby.game_state.world.spawn((
            initial_pos,
            Worker,
            WorkerState::MovingToVein,
            PlayerIdComponent(1),
            targets,
        )).id();

        let tick_delta = 1.0 / 30.0;
        update_workers(&mut lobby, tick_delta);

        let current_pos = lobby.game_state.world.entity(worker).get::<Position>().unwrap();
        assert_eq!(current_pos.x, initial_pos.x);
        assert_eq!(current_pos.y, initial_pos.y, "Worker should not move during Build phase");
        
        let state = lobby.game_state.world.entity(worker).get::<WorkerState>().unwrap();
        assert_eq!(*state, WorkerState::MovingToVein, "Worker state should not change during Build phase");
    }
}
