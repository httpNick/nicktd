use crate::model::{
    components::{MiningTimer, PlayerIdComponent, Position, TargetPositions, Worker, WorkerState},
    game_state::DeltaTime,
    player::Players,
};
use bevy_ecs::prelude::{Commands, Entity, Query, Res, ResMut, With};

pub const WORKER_SPEED: f32 = 50.0;

pub const VEIN_POSITIONS: [Position; 2] = [
    Position { x: 700.0, y: 50.0 },
    Position { x: 700.0, y: 350.0 },
];
pub const CART_POSITIONS: [Position; 2] = [
    Position { x: 700.0, y: 250.0 },
    Position { x: 700.0, y: 550.0 },
];

/// Bevy system: moves workers along their vein→cart route and awards gold on deposit.
/// Requires `DeltaTime` and `Players` resources to be present in the world.
pub fn update_workers(
    mut commands: Commands,
    mut worker_query: Query<
        (
            Entity,
            &mut Position,
            &mut WorkerState,
            Option<&mut MiningTimer>,
            &PlayerIdComponent,
            &TargetPositions,
        ),
        With<Worker>,
    >,
    time: Res<DeltaTime>,
    mut players: ResMut<Players>,
) {
    let tick_delta = time.0;
    let mut deposit_pids: Vec<i64> = Vec::new();

    for (entity, mut pos, mut state, timer_opt, player_id, targets) in worker_query.iter_mut() {
        match *state {
            WorkerState::MovingToVein => {
                let target = targets.vein;
                let dx = target.x - pos.x;
                let dy = target.y - pos.y;
                let dist = (dx * dx + dy * dy).sqrt();
                let move_dist = WORKER_SPEED * tick_delta;

                if dist <= move_dist {
                    pos.x = target.x;
                    pos.y = target.y;
                    *state = WorkerState::Mining;
                    commands.entity(entity).insert(MiningTimer(10.0));
                } else {
                    pos.x += (dx / dist) * move_dist;
                    pos.y += (dy / dist) * move_dist;
                }
            }
            WorkerState::Mining => {
                if let Some(mut timer) = timer_opt {
                    timer.0 -= tick_delta;
                    if timer.0 <= 0.0 {
                        *state = WorkerState::MovingToCart;
                        commands.entity(entity).remove::<MiningTimer>();
                    }
                }
            }
            WorkerState::MovingToCart => {
                let target = targets.cart;
                let dx = target.x - pos.x;
                let dy = target.y - pos.y;
                let dist = (dx * dx + dy * dy).sqrt();
                let move_dist = WORKER_SPEED * tick_delta;

                if dist <= move_dist {
                    pos.x = target.x;
                    pos.y = target.y;
                    *state = WorkerState::MovingToVein;
                    deposit_pids.push(player_id.0);
                } else {
                    pos.x += (dx / dist) * move_dist;
                    pos.y += (dy / dist) * move_dist;
                }
            }
        }
    }

    for pid in deposit_pids {
        if let Some(player) = players.0.iter_mut().find(|p| p.id == pid) {
            player.gold += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::game_state::GamePhase;
    use crate::model::player::Player;
    use bevy_ecs::prelude::World;
    use bevy_ecs::system::RunSystemOnce;

    fn setup_world_with_player(player_id: i64) -> World {
        let mut world = World::new();
        world.insert_resource(DeltaTime(1.0 / 30.0));
        world.insert_resource(Players(vec![Player::new(player_id, "test".to_string(), 0)]));
        world
    }

    // --- Task 4 TDD: verify update_workers works as a Bevy system ---

    #[test]
    fn update_workers_moves_worker_via_system() {
        let mut world = setup_world_with_player(1);

        let targets = TargetPositions {
            vein: VEIN_POSITIONS[0],
            cart: CART_POSITIONS[0],
        };
        let worker = world
            .spawn((
                CART_POSITIONS[0], // start at cart
                Worker,
                WorkerState::MovingToVein,
                PlayerIdComponent(1),
                targets,
            ))
            .id();

        world.run_system_once(update_workers).unwrap();

        let pos = world.entity(worker).get::<Position>().unwrap();
        assert!(
            pos.y < CART_POSITIONS[0].y,
            "Worker should move north (decrease Y) towards the vein"
        );
    }

    #[test]
    fn update_workers_awards_gold_to_players_resource() {
        let mut world = setup_world_with_player(1);

        let targets = TargetPositions {
            vein: VEIN_POSITIONS[0],
            cart: CART_POSITIONS[0],
        };
        // Start at cart position, moving to vein — teleport to cart in MovingToCart state
        let _worker = world
            .spawn((
                CART_POSITIONS[0], // already at cart
                Worker,
                WorkerState::MovingToCart,
                PlayerIdComponent(1),
                targets,
            ))
            .id();

        world.run_system_once(update_workers).unwrap();

        let gold = world.resource::<Players>().0[0].gold;
        assert_eq!(
            gold, 1,
            "Depositing at cart should award 1 gold via Players resource"
        );
    }

    // --- Existing behavior preserved ---

    #[test]
    fn worker_behavior_cycle() {
        let mut world = setup_world_with_player(1);

        let targets = TargetPositions {
            vein: VEIN_POSITIONS[0],
            cart: CART_POSITIONS[0],
        };

        let worker = world
            .spawn((
                targets.cart,
                Worker,
                WorkerState::MovingToVein,
                PlayerIdComponent(1),
                targets,
            ))
            .id();

        // 1. Check Movement
        world.run_system_once(update_workers).unwrap();
        let pos = world.entity(worker).get::<Position>().unwrap();
        assert!(
            pos.y < CART_POSITIONS[0].y,
            "Worker should move North (decrease Y)"
        );

        // 2. Check Arrival at Vein -> Mining
        world.entity_mut(worker).insert(VEIN_POSITIONS[0]);
        world.run_system_once(update_workers).unwrap();

        let state = world.entity(worker).get::<WorkerState>().unwrap();
        assert_eq!(*state, WorkerState::Mining);
        let timer = world.entity(worker).get::<MiningTimer>();
        assert!(timer.is_some());

        // 3. Check Mining Timer Expire -> MovingToCart
        world.entity_mut(worker).insert(MiningTimer(0.0));
        world.run_system_once(update_workers).unwrap();

        let state = world.entity(worker).get::<WorkerState>().unwrap();
        assert_eq!(*state, WorkerState::MovingToCart);

        // 4. Check Arrival at Cart -> Deposit
        world.entity_mut(worker).insert(CART_POSITIONS[0]);
        world.run_system_once(update_workers).unwrap();

        let state = world.entity(worker).get::<WorkerState>().unwrap();
        assert_eq!(*state, WorkerState::MovingToVein);

        let gold = world.resource::<Players>().0[0].gold;
        assert_eq!(gold, 1);
    }

    #[test]
    fn test_workers_active_in_all_phases() {
        for phase in [GamePhase::Build, GamePhase::Combat, GamePhase::Victory] {
            let mut world = World::new();
            world.insert_resource(DeltaTime(1.0 / 30.0));
            world.insert_resource(Players(vec![Player::new(1, "test".to_string(), 100)]));

            let initial_pos = Position { x: 700.0, y: 250.0 };
            let targets = TargetPositions {
                vein: VEIN_POSITIONS[0],
                cart: CART_POSITIONS[0],
            };

            let worker = world
                .spawn((
                    initial_pos,
                    Worker,
                    WorkerState::MovingToVein,
                    PlayerIdComponent(1),
                    targets,
                ))
                .id();

            // Phase doesn't gate this system; workers always run
            let _ = phase;
            world.run_system_once(update_workers).unwrap();

            let current_pos = world.entity(worker).get::<Position>().unwrap();
            assert!(
                current_pos.y < initial_pos.y,
                "Worker should move during phase {:?}",
                phase
            );
        }
    }
}
