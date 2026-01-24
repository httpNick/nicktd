use crate::{
    model::{
        components::{Enemy, Position, ShapeComponent, Target, Worker, WorkerState, MiningTimer, PlayerIdComponent, TargetPositions},
        shape::Shape,
        lobby::Lobby,
    },
    state::ServerState,
};
use bevy_ecs::prelude::{Entity, With, Without, World};
use std::collections::HashMap;
use std::time::Duration;

const TICK_RATE: f32 = 30.0;
const SPEED: f32 = 100.0; // pixels per second
const MELEE_RANGE: f32 = 20.0;
const WORKER_SPEED: f32 = 50.0;

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

fn update_workers(lobby: &mut Lobby) {
    let mut actions = Vec::new();
    let tick_delta = 1.0 / TICK_RATE;

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
                } else {
                     // Should not happen if logic is correct, but safe fallback or just ignore
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

fn update_targeting(world: &mut World) {
    let mut commands = Vec::new();

    // --- UNIT TARGETING (Units target closest Enemy) ---
    let enemy_positions: Vec<(Entity, Position)> = world
        .query_filtered::<(Entity, &Position), With<Enemy>>()
        .iter(world)
        .map(|(entity, pos)| (entity, Position { x: pos.x, y: pos.y }))
        .collect();

    if !enemy_positions.is_empty() {
        let mut query = world.query_filtered::<(Entity, &Position), (Without<Enemy>, Without<Target>, Without<Worker>)>();
        for (unit_entity, unit_pos) in query.iter(world) {
            let mut closest_enemy: Option<(Entity, f32)> = None;
            for (enemy_entity, enemy_pos) in &enemy_positions {
                let distance_sq = (unit_pos.x - enemy_pos.x).powi(2) + (unit_pos.y - enemy_pos.y).powi(2);
                if closest_enemy.is_none() || distance_sq < closest_enemy.unwrap().1 {
                    closest_enemy = Some((*enemy_entity, distance_sq));
                }
            }
            if let Some((target_entity, _)) = closest_enemy {
                commands.push((unit_entity, Target(target_entity)));
            }
        }
    }

    // --- ENEMY TARGETING (Enemies target closest non-Worker Unit) ---
    let unit_positions: Vec<(Entity, Position)> = world
        .query_filtered::<(Entity, &Position), (Without<Enemy>, Without<Worker>)>()
        .iter(world)
        .map(|(entity, pos)| (entity, Position { x: pos.x, y: pos.y }))
        .collect();

    if !unit_positions.is_empty() {
        let mut query = world.query_filtered::<(Entity, &Position), (With<Enemy>, Without<Target>)>();
        for (enemy_entity, enemy_pos) in query.iter(world) {
            let mut closest_unit: Option<(Entity, f32)> = None;
            for (unit_entity, unit_pos) in &unit_positions {
                let distance_sq = (enemy_pos.x - unit_pos.x).powi(2) + (enemy_pos.y - unit_pos.y).powi(2);
                if closest_unit.is_none() || distance_sq < closest_unit.unwrap().1 {
                    closest_unit = Some((*unit_entity, distance_sq));
                }
            }
            if let Some((target_entity, _)) = closest_unit {
                commands.push((enemy_entity, Target(target_entity)));
            }
        }
    }

    // Apply targeting commands
    for (entity, target) in commands {
        world.entity_mut(entity).insert(target);
    }
}

fn update_combat_movement(world: &mut World) {
    // --- MOVEMENT & COLLISION SYSTEM ---
    let positions: HashMap<Entity, Position> = world
        .query::<(Entity, &Position)>()
        .iter(world)
        .map(|(e, p)| (e, Position { x: p.x, y: p.y }))
        .collect();

    let mut query = world.query_filtered::<(Entity, &mut Position, &Target), Without<Worker>>();
    for (_entity, mut unit_pos, target) in query.iter_mut(world) {
        if let Some(target_pos) = positions.get(&target.0) {
            let distance = ((unit_pos.x - target_pos.x).powi(2) + (unit_pos.y - target_pos.y).powi(2)).sqrt();
            
            // Steering force (towards target)
            let steering_x = target_pos.x - unit_pos.x;
            let steering_y = target_pos.y - unit_pos.y;

            if distance > 0.0 { // Avoid division by zero if unit is exactly on target
                let mut scaled_speed = SPEED;
                if distance < MELEE_RANGE {
                    // Scale speed down as it gets closer to MELEE_RANGE
                    scaled_speed = SPEED * (distance / MELEE_RANGE);
                    if scaled_speed < 1.0 { scaled_speed = 0.0; } // Stop if very close
                }

                let norm = (steering_x.powi(2) + steering_y.powi(2)).sqrt();
                if norm > 0.0 {
                    let move_x = (steering_x / norm) * scaled_speed * (1.0 / TICK_RATE);
                    let move_y = (steering_y / norm) * scaled_speed * (1.0 / TICK_RATE);
                    unit_pos.x += move_x;
                    unit_pos.y += move_y;
                }
            }
        }
    }
}

pub async fn run_game_loop(server_state: ServerState, lobby_id: usize) {
    let mut interval = tokio::time::interval(Duration::from_secs_f32(1.0 / TICK_RATE));

    loop {
        interval.tick().await;
        let mut lobbies = server_state.lobbies.lock().await;
        if let Some(lobby) = lobbies.get_mut(lobby_id) {
            match lobby.game_state.phase {
                crate::model::game_state::GamePhase::Build => {
                    if lobby.is_full() {
                        lobby.game_state.phase_timer -= 1.0 / TICK_RATE;
                        if lobby.game_state.phase_timer <= 0.0 {
                            lobby.game_state.phase = crate::model::game_state::GamePhase::Combat;
                            // Spawn one enemy at the top center
                            lobby.game_state.world.spawn((
                                Position { x: 300.0, y: 30.0 },
                                ShapeComponent(Shape::Triangle),
                                Enemy,
                            ));
                        }
                    }
                }
                crate::model::game_state::GamePhase::Combat => {
                    update_targeting(&mut lobby.game_state.world);
                    update_combat_movement(&mut lobby.game_state.world);
                    update_workers(lobby);
                }
            }
            lobby.broadcast_gamestate();
        } else {
            // Lobby no longer exists, stop the loop
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::components::Worker;

    #[test]
    fn targeting_ignores_workers() {
        let mut world = World::new();
        // Spawn Enemy
        let _enemy = world.spawn((Position { x: 100.0, y: 100.0 }, Enemy)).id();
        // Spawn Worker (close to enemy)
        let worker = world.spawn((Position { x: 105.0, y: 105.0 }, Worker)).id();
        // Spawn Normal Unit (farther)
        let unit = world.spawn((Position { x: 200.0, y: 200.0 })).id();

        update_targeting(&mut world);

        // Assert Worker does NOT have Target
        assert!(world.entity(worker).get::<Target>().is_none(), "Worker should not have target");
        // Assert Unit DOES have Target
        assert!(world.entity(unit).get::<Target>().is_some(), "Unit should have target");
    }

    #[test]
    fn nothing_targets_workers() {
        let mut world = World::new();
        // Spawn Worker (very close to enemy)
        let worker = world.spawn((Position { x: 100.0, y: 100.0 }, Worker)).id();
        // Spawn Enemy
        let enemy = world.spawn((Position { x: 105.0, y: 105.0 }, Enemy)).id();
        // Spawn Normal Unit (farther than worker)
        let unit = world.spawn((Position { x: 200.0, y: 200.0 })).id();

        update_targeting(&mut world);

        // Enemy should NOT target the worker, but SHOULD target the unit
        let target = world.entity(enemy).get::<Target>();
        assert!(target.is_some(), "Enemy should have a target");
        assert_eq!(target.unwrap().0, unit, "Enemy should target unit, not worker");
    }

    #[test]
    fn worker_behavior_cycle() {
        use crate::model::lobby::Lobby;
        use crate::model::player::Player;
        use crate::model::components::{WorkerState, MiningTimer, PlayerIdComponent, TargetPositions};

        let mut lobby = Lobby::new();
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

        // 1. Check Movement
        update_workers(&mut lobby); 
        let pos = lobby.game_state.world.entity(worker).get::<Position>().unwrap();
        assert!(pos.y < CART_POSITIONS[0].y, "Worker should move North (decrease Y)");

        // 2. Check Arrival at Vein -> Mining
        // Teleport to Vein
        lobby.game_state.world.entity_mut(worker).insert(VEIN_POSITIONS[0]);
        update_workers(&mut lobby);
        
        // State should be Mining
        let state = lobby.game_state.world.entity(worker).get::<WorkerState>().unwrap();
        assert_eq!(*state, WorkerState::Mining);
        // Should have MiningTimer
        let timer = lobby.game_state.world.entity(worker).get::<MiningTimer>();
        assert!(timer.is_some());

        // 3. Check Mining Timer Expire -> MovingToCart
        // Set timer to 0
        lobby.game_state.world.entity_mut(worker).insert(MiningTimer(0.0));
        update_workers(&mut lobby);
        
        let state = lobby.game_state.world.entity(worker).get::<WorkerState>().unwrap();
        assert_eq!(*state, WorkerState::MovingToCart);
        
        // 4. Check Arrival at Cart -> Deposit
        // Teleport to Cart
        lobby.game_state.world.entity_mut(worker).insert(CART_POSITIONS[0]);
        update_workers(&mut lobby);
        
        // State should be MovingToVein
        let state = lobby.game_state.world.entity(worker).get::<WorkerState>().unwrap();
        assert_eq!(*state, WorkerState::MovingToVein);
        
        // Player gold should increase
        assert_eq!(lobby.players[0].gold, 1);
    }
}
