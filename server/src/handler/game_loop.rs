use crate::model::components::{Enemy, Position, ShapeComponent, Target};
use crate::model::shape::Shape;
use crate::ServerState;
use bevy_ecs::prelude::{Entity, With, Without};
use std::collections::HashMap;
use std::time::Duration;

const TICK_RATE: f32 = 30.0;
const SPEED: f32 = 100.0; // pixels per second
const MELEE_RANGE: f32 = 20.0;


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
                    let mut commands = Vec::new();

                    // --- TARGETING SYSTEM ---
                    let enemy_positions: Vec<(Entity, Position)> = lobby.game_state.world
                        .query_filtered::<(Entity, &Position), With<Enemy>>()
                        .iter(&lobby.game_state.world)
                        .map(|(entity, pos)| (entity, Position { x: pos.x, y: pos.y }))
                        .collect();

                    if !enemy_positions.is_empty() {
                        let mut query = lobby.game_state.world.query_filtered::<(Entity, &Position), (Without<Enemy>, Without<Target>)>();
                        for (unit_entity, unit_pos) in query.iter(&lobby.game_state.world) {
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

                    // Apply targeting commands
                    for (entity, target) in commands {
                        lobby.game_state.world.entity_mut(entity).insert(target);
                    }

                    // --- MOVEMENT & COLLISION SYSTEM ---
                    let positions: HashMap<Entity, Position> = lobby.game_state.world
                        .query::<(Entity, &Position)>()
                        .iter(&lobby.game_state.world)
                        .map(|(e, p)| (e, Position { x: p.x, y: p.y }))
                        .collect();

                    let mut query = lobby.game_state.world.query_filtered::<(Entity, &mut Position, &Target), Without<Enemy>>();
                    for (_entity, mut unit_pos, target) in query.iter_mut(&mut lobby.game_state.world) {
                        if let Some(target_pos) = positions.get(&target.0) {
                            let distance = ((unit_pos.x - target_pos.x).powi(2) + (unit_pos.y - target_pos.y).powi(2)).sqrt();
                            
                            // Steering force (towards target)
                            let steering_x = target_pos.x - unit_pos.x;
                            let steering_y = target_pos.y - unit_pos.y;



                            // Combine forces
                            let final_x = steering_x;
                            let final_y = steering_y;

                            if distance > 0.0 { // Avoid division by zero if unit is exactly on target
                                let mut scaled_speed = SPEED;
                                if distance < MELEE_RANGE {
                                    // Scale speed down as it gets closer to MELEE_RANGE
                                    scaled_speed = SPEED * (distance / MELEE_RANGE);
                                    if scaled_speed < 1.0 { scaled_speed = 0.0; } // Stop if very close
                                }

                                let norm = (final_x.powi(2) + final_y.powi(2)).sqrt();
                                if norm > 0.0 {
                                    let move_x = (final_x / norm) * scaled_speed * (1.0 / TICK_RATE);
                                    let move_y = (final_y / norm) * scaled_speed * (1.0 / TICK_RATE);
                                    unit_pos.x += move_x;
                                    unit_pos.y += move_y;
                                }
                            }
                        }
                    }
                }
            }
            lobby.broadcast_gamestate();
        } else {
            // Lobby no longer exists, stop the loop
            break;
        }
    }
}
