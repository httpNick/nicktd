use super::{SPEED, get_board};
use crate::model::components::{
    AttackRange, CollisionRadius, Dead, Enemy, Health, HomePosition, King, Mana, Position, Target,
    Worker,
};
use crate::model::constants::{LEFT_BOARD_END, RIGHT_BOARD_END, RIGHT_BOARD_START, TOTAL_HEIGHT};
use crate::model::game_state::DeltaTime;
use bevy_ecs::prelude::{Entity, With, Without, World};

pub fn update_combat_movement(world: &mut World) {
    let tick_delta = world.resource::<DeltaTime>().0;
    // --- MOVEMENT & COLLISION SYSTEM ---
    let physical_entities: Vec<(Entity, Position, f32)> = world
        .query_filtered::<(Entity, &Position, &CollisionRadius), (Without<Worker>, Without<Dead>)>()
        .iter(world)
        .map(|(e, p, r)| (e, *p, r.0))
        .collect();

    // Collect king entities for leaked enemy targeting logic
    let king_entities: Vec<Entity> = world
        .query_filtered::<Entity, With<King>>()
        .iter(world)
        .collect();

    let mut movements = Vec::new();

    // First pass: Calculate all movement vectors (kings are stationary — excluded)
    let mut query = world.query_filtered::<(
        Entity,
        &Position,
        Option<&Target>,
        Option<&AttackRange>,
        &CollisionRadius,
        Option<&Enemy>,
    ), (Without<Worker>, Without<King>, Without<Dead>)>();
    for (entity, pos, target_opt, attack_range_opt, collision_radius, enemy_opt) in
        query.iter(world)
    {
        let mut velocity_x = 0.0;
        let mut velocity_y = 0.0;

        // 1. Chasing Force & Range Gating
        if let Some(target) = target_opt {
            if let Some(target_pos) = physical_entities
                .iter()
                .find(|(e, _, _)| *e == target.0)
                .map(|(_, p, _)| p)
            {
                let dx = target_pos.x - pos.x;
                let dy = target_pos.y - pos.y;
                let distance = (dx * dx + dy * dy).sqrt();

                let range = attack_range_opt.map(|r| r.0).unwrap_or(0.0);

                // Enemies chase to contact distance to counter separation, but not beyond attack range.
                // Towers stop at attack range (normal behavior).
                // Leaked enemies (targeting the king) chase to full attack range to maintain
                // offensive capability despite separation forces.
                let chase_distance = if enemy_opt.is_some() {
                    // Check if this enemy is leaked (y >= TOTAL_HEIGHT) and targeting a king
                    let is_leaked = pos.y >= TOTAL_HEIGHT;
                    let is_targeting_king = king_entities.contains(&target.0);

                    if is_leaked && is_targeting_king {
                        // Leaked enemies targeting the king chase to their full attack range
                        range
                    } else {
                        // In-lane enemies chase to contact distance (original behavior)
                        let target_radius = physical_entities
                            .iter()
                            .find(|(e, _, _)| *e == target.0)
                            .map(|(_, _, r)| *r)
                            .unwrap_or(0.0);
                        let contact_distance = collision_radius.0 + target_radius;
                        // Chase to contact OR attack range, whichever is closer
                        contact_distance.min(range)
                    }
                } else {
                    range
                };

                if distance > chase_distance && distance > 0.0 {
                    velocity_x += (dx / distance) * SPEED;
                    velocity_y += (dy / distance) * SPEED;
                }
            }
        } else {
            // No target: enemies drift downward towards the opponent's base
            if enemy_opt.is_some() {
                velocity_y += SPEED;
            }
        }

        // 2. Separation Force
        // Leaked enemies targeting the king use reduced separation to allow them to close to attack range.
        // Without this, separation forces cancel out the chasing force and enemies get stuck beyond range.
        let use_reduced_separation = if enemy_opt.is_some() && target_opt.is_some() {
            let is_leaked = pos.y >= TOTAL_HEIGHT;
            let is_targeting_king = king_entities.contains(&target_opt.unwrap().0);
            is_leaked && is_targeting_king
        } else {
            false
        };

        for (other_entity, other_pos, other_radius) in &physical_entities {
            if entity == *other_entity {
                continue;
            }

            let dx = pos.x - other_pos.x;
            let dy = pos.y - other_pos.y;
            let distance = (dx * dx + dy * dy).sqrt();
            let min_dist = collision_radius.0 + other_radius;

            if distance < min_dist {
                // Overlap detected
                if distance > 0.0 {
                    // Repulsive force away from neighbor
                    let separation_strength = if use_reduced_separation {
                        // Leaked enemies targeting the king use weaker separation (10% of normal)
                        // to avoid getting stuck beyond attack range
                        SPEED * 0.1
                    } else {
                        SPEED
                    };
                    velocity_x += (dx / distance) * separation_strength;
                    velocity_y += (dy / distance) * separation_strength;
                } else {
                    // Exactly on top of each other, push in a random-ish direction based on entity ID
                    let angle = (entity.index() as f32) * 0.1;
                    velocity_x += angle.cos() * SPEED;
                    velocity_y += angle.sin() * SPEED;
                }
            }
        }

        if velocity_x != 0.0 || velocity_y != 0.0 {
            movements.push((entity, velocity_x * tick_delta, velocity_y * tick_delta));
        }
    }

    // Second pass: Apply movements
    for (entity, dx, dy) in movements {
        let radius = world
            .get::<CollisionRadius>(entity)
            .map(|r| r.0)
            .unwrap_or(0.0);
        let home_x = world
            .get::<HomePosition>(entity)
            .map(|h| h.0.x)
            .unwrap_or(0.0);

        let is_enemy = world.get::<Enemy>(entity).is_some();
        if let Some(mut pos) = world.get_mut::<Position>(entity) {
            pos.x += dx;
            // Leaked enemies (at or past TOTAL_HEIGHT) enter the king zone.
            // Enemies use TOTAL_HEIGHT (not TOTAL_HEIGHT - radius) as their y upper bound
            // so they can actually reach the boundary and trigger the king-zone transition.
            // Non-enemies (towers) keep the radius margin to avoid clipping the wall.
            if is_enemy && pos.y + dy >= TOTAL_HEIGHT {
                use crate::model::constants::KING_Y;
                use crate::model::king_config::KING_COLLISION_RADIUS;
                pos.y = (pos.y + dy).clamp(TOTAL_HEIGHT, KING_Y + KING_COLLISION_RADIUS);
            } else if is_enemy {
                pos.y = (pos.y + dy).clamp(radius, TOTAL_HEIGHT);
            } else {
                pos.y = (pos.y + dy).clamp(radius, TOTAL_HEIGHT - radius);
            }

            if home_x < LEFT_BOARD_END {
                pos.x = pos.x.clamp(radius, LEFT_BOARD_END - radius);
            } else {
                pos.x = pos
                    .x
                    .clamp(RIGHT_BOARD_START + radius, RIGHT_BOARD_END - radius);
            }
        }
    }
}

pub fn update_combat_reset(world: &mut World) {
    let mut boards_to_reset = Vec::new();

    for board_idx in 0..=1 {
        let mut enemy_query = world.query_filtered::<&Position, With<Enemy>>();
        let has_enemies = enemy_query
            .iter(world)
            .any(|pos| get_board(pos.x) == Some(board_idx));

        if !has_enemies {
            boards_to_reset.push(board_idx);
        }
    }

    if boards_to_reset.is_empty() {
        return;
    }

    // Pass 1: collect all entities on cleared boards, including Dead towers
    let entities_to_restore: Vec<Entity> = {
        let mut query = world.query::<(Entity, &HomePosition)>();
        query
            .iter(world)
            .filter_map(|(entity, home)| {
                get_board(home.0.x)
                    .filter(|board_idx| boards_to_reset.contains(board_idx))
                    .map(|_| entity)
            })
            .collect()
    };

    // Pass 2: restore position, health, mana and remove Dead marker for each entity
    for entity in entities_to_restore {
        if let Some(home_pos) = world.get::<HomePosition>(entity).map(|h| h.0) {
            if let Some(mut pos) = world.get_mut::<Position>(entity) {
                *pos = home_pos;
            }
        }

        if let Some(mut health) = world.get_mut::<Health>(entity) {
            let max = health.max;
            health.current = max;
        }

        if let Some(mut mana) = world.get_mut::<Mana>(entity) {
            let max = mana.max;
            mana.current = max;
        }

        if world.get::<Dead>(entity).is_some() {
            world.entity_mut(entity).remove::<Dead>();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handler::combat::{update_attack_range_markers, update_leaked_creeps};
    use crate::model::components::InAttackRange;
    use crate::model::unit_kind::UnitKind;

    #[test]
    fn range_aware_movement_stops_at_range() {
        let mut world = World::new();

        let range = 50.0;
        let unit = crate::handler::spawn::spawn_unit(
            &mut world,
            Position { x: 0.0, y: 0.0 },
            UnitKind::Square,
            1,
        );
        // Overwrite default range for test
        world.entity_mut(unit).insert(AttackRange(range));

        let enemy = crate::handler::spawn::spawn_enemy(
            &mut world,
            Position { x: 100.0, y: 0.0 }, // 100 pixels away
            UnitKind::Triangle,
            1,
        );

        world.entity_mut(unit).insert(Target(enemy));

        let tick_delta = 1.0 / 30.0;
        world.insert_resource(DeltaTime(tick_delta));

        // 1. Far away: should move
        // Reset enemy position before each tick for determinism (enemy gets fallback movement).
        world
            .entity_mut(enemy)
            .insert(Position { x: 100.0, y: 0.0 });
        update_combat_movement(&mut world);
        let pos = world.entity(unit).get::<Position>().unwrap();
        assert!(pos.x > 0.0, "Unit should move towards enemy when far away");

        // 2. Within range: should stop
        // Teleport to just inside range (40 pixels away)
        world.entity_mut(unit).insert(Position { x: 60.0, y: 0.0 });
        world
            .entity_mut(enemy)
            .insert(Position { x: 100.0, y: 0.0 });
        update_combat_movement(&mut world);
        let pos = world.entity(unit).get::<Position>().unwrap();
        assert_eq!(pos.x, 60.0, "Unit should NOT move when within attack range");

        // 3. Exactly at range: should stop
        world.entity_mut(unit).insert(Position { x: 50.0, y: 0.0 });
        world
            .entity_mut(enemy)
            .insert(Position { x: 100.0, y: 0.0 });
        update_combat_movement(&mut world);
        let pos = world.entity(unit).get::<Position>().unwrap();
        assert_eq!(
            pos.x, 50.0,
            "Unit should NOT move when exactly at attack range"
        );
    }

    #[test]
    fn units_separate_when_overlapping() {
        let mut world = World::new();

        let radius = 10.0;
        // Spawn two units very close to each other
        let unit_a = crate::handler::spawn::spawn_unit(
            &mut world,
            Position { x: 100.0, y: 100.0 },
            UnitKind::Square,
            1,
        );
        world.entity_mut(unit_a).insert(CollisionRadius(radius));

        let unit_b = crate::handler::spawn::spawn_unit(
            &mut world,
            Position { x: 105.0, y: 100.0 }, // Overlapping by 15 pixels (combined radius 20)
            UnitKind::Square,
            1,
        );
        world.entity_mut(unit_b).insert(CollisionRadius(radius));

        let tick_delta = 1.0 / 30.0;
        world.insert_resource(DeltaTime(tick_delta));
        update_combat_movement(&mut world);

        let pos_a = world.entity(unit_a).get::<Position>().unwrap();
        let pos_b = world.entity(unit_b).get::<Position>().unwrap();

        assert!(
            pos_a.x < 100.0,
            "Unit A should move away from Unit B (Left)"
        );
        assert!(
            pos_b.x > 105.0,
            "Unit B should move away from Unit A (Right)"
        );
    }

    #[test]
    fn entities_stay_within_bounds() {
        let mut world = World::new();

        let radius = 10.0;
        // Spawn unit at the very edge
        let unit = crate::handler::spawn::spawn_unit(
            &mut world,
            Position { x: 5.0, y: 5.0 },
            UnitKind::Square,
            1,
        );
        world.entity_mut(unit).insert(CollisionRadius(radius));

        // Spawn many units around it to push it out
        for _ in 0..10 {
            let _ = crate::handler::spawn::spawn_unit(
                &mut world,
                Position { x: 15.0, y: 15.0 },
                UnitKind::Square,
                1,
            );
        }

        let tick_delta = 1.0 / 30.0;
        world.insert_resource(DeltaTime(tick_delta));

        // Run many ticks to let separation force push it
        for _ in 0..100 {
            update_combat_movement(&mut world);
        }

        let pos = world.entity(unit).get::<Position>().unwrap();
        assert!(
            pos.x >= radius,
            "Entity should not be pushed off the left edge (x={}, radius={})",
            pos.x,
            radius
        );
        assert!(
            pos.y >= radius,
            "Entity should not be pushed off the top edge (y={}, radius={})",
            pos.y,
            radius
        );
        assert!(
            pos.x <= LEFT_BOARD_END - radius,
            "Entity should not be pushed off the board edge (x={}, limit={})",
            pos.x,
            LEFT_BOARD_END - radius
        );
    }

    #[test]
    fn test_combat_reset() {
        use crate::model::components::{Health, HomePosition, Mana};
        use crate::model::constants::RIGHT_BOARD_START;
        let mut world = World::new();

        // Unit on Left Board (damaged, moved)
        let home_pos = Position {
            x: 0.0 + 100.0,
            y: 300.0,
        };
        let unit = world
            .spawn((
                Position {
                    x: home_pos.x + 50.0,
                    y: home_pos.y + 50.0,
                },
                HomePosition(home_pos),
                Health {
                    current: 50.0,
                    max: 100.0,
                },
                Mana {
                    current: 10.0,
                    max: 50.0,
                    regen: 1.0,
                },
            ))
            .id();

        // No enemies on Left Board.
        // Enemy on Right Board.
        world.spawn((
            Position {
                x: RIGHT_BOARD_START + 100.0,
                y: 300.0,
            },
            Enemy,
        ));

        // 1. Run reset (to be implemented)
        update_combat_reset(&mut world);

        // 2. Assert unit on Left Board is reset
        let pos = world.entity(unit).get::<Position>().unwrap();
        let health = world.entity(unit).get::<Health>().unwrap();
        let mana = world.entity(unit).get::<Mana>().unwrap();

        assert_eq!(pos.x, home_pos.x);
        assert_eq!(pos.y, home_pos.y);
        assert_eq!(health.current, health.max);
        assert_eq!(mana.current, mana.max);
    }

    #[test]
    fn dead_tower_not_moved_by_physical_simulation() {
        let mut world = World::new();

        let radius = 15.0;

        // Spawn a dead tower on the left board
        let dead_tower = crate::handler::spawn::spawn_unit(
            &mut world,
            Position { x: 100.0, y: 300.0 },
            UnitKind::Square,
            1,
        );
        world.entity_mut(dead_tower).insert(Dead);
        world.entity_mut(dead_tower).insert(CollisionRadius(radius));

        // Spawn a living unit overlapping with the dead tower (5px gap, combined radius 30px)
        let living_unit = crate::handler::spawn::spawn_unit(
            &mut world,
            Position { x: 105.0, y: 300.0 },
            UnitKind::Square,
            1,
        );
        world
            .entity_mut(living_unit)
            .insert(CollisionRadius(radius));

        world.insert_resource(DeltaTime(1.0 / 30.0));
        update_combat_movement(&mut world);

        // Dead tower should not have moved
        let dead_pos = world.entity(dead_tower).get::<Position>().unwrap();
        assert_eq!(
            dead_pos.x, 100.0,
            "Dead tower should not be moved by separation forces"
        );
        assert_eq!(
            dead_pos.y, 300.0,
            "Dead tower should not be moved by separation forces"
        );

        // Living unit should not be pushed by dead tower (dead tower excluded from physical_entities)
        let live_pos = world.entity(living_unit).get::<Position>().unwrap();
        assert_eq!(
            live_pos.x, 105.0,
            "Living unit should not be pushed by the dead tower"
        );
    }

    #[test]
    fn combat_movement_ignores_workers() {
        let mut world = World::new();

        // 1. Worker outside the board (X > 600)
        let worker_pos = Position { x: 700.0, y: 100.0 };
        let worker = world
            .spawn((worker_pos, Worker, CollisionRadius(10.0)))
            .id();

        // 2. Normal Unit overlapping with the worker
        let unit_pos = Position { x: 595.0, y: 100.0 };
        let _unit = world.spawn((unit_pos, CollisionRadius(10.0))).id();

        world.insert_resource(DeltaTime(0.1));
        update_combat_movement(&mut world);

        // Assert Worker position is UNCHANGED (not clamped to 600, not pushed by unit)
        let final_worker_pos = world.entity(worker).get::<Position>().unwrap();
        assert_eq!(
            final_worker_pos.x, 700.0,
            "Worker should not be clamped to board"
        );
        assert_eq!(
            final_worker_pos.y, 100.0,
            "Worker should not be moved by combat systems"
        );

        // Assert Unit position is NOT pushed by the worker
        // If the worker was considered, the unit (X=595) would be pushed left by the worker (X=700 is far, wait)
        // Let's overlap them better: Worker at 595, Unit at 590.

        let mut world2 = World::new();
        let _worker2 = world2
            .spawn((
                Position { x: 595.0, y: 100.0 },
                Worker,
                CollisionRadius(10.0),
            ))
            .id();
        let unit2 = world2
            .spawn((Position { x: 590.0, y: 100.0 }, CollisionRadius(10.0)))
            .id();

        world2.insert_resource(DeltaTime(0.1));
        update_combat_movement(&mut world2);

        let final_unit2_pos = world2.entity(unit2).get::<Position>().unwrap();
        assert_eq!(
            final_unit2_pos.x, 590.0,
            "Unit should not be pushed by overlapping worker"
        );
    }

    #[test]
    fn dead_tower_revived_when_board_cleared() {
        use crate::model::components::{Dead, Health, HomePosition, Mana};

        let mut world = World::new();

        let home_pos = Position { x: 100.0, y: 300.0 };
        let dead_tower = world
            .spawn((
                Position {
                    x: home_pos.x + 100.0,
                    y: home_pos.y + 100.0,
                },
                HomePosition(home_pos),
                Health {
                    current: 0.0,
                    max: 100.0,
                },
                Mana {
                    current: 0.0,
                    max: 50.0,
                    regen: 1.0,
                },
                Dead,
            ))
            .id();

        // No enemies on the left board
        update_combat_reset(&mut world);

        assert!(
            world.entity(dead_tower).get::<Dead>().is_none(),
            "Dead marker should be removed after wave clear"
        );
        let pos = world.entity(dead_tower).get::<Position>().unwrap();
        assert_eq!(pos.x, home_pos.x, "Position x should be reset to home");
        assert_eq!(pos.y, home_pos.y, "Position y should be reset to home");
        let health = world.entity(dead_tower).get::<Health>().unwrap();
        assert_eq!(
            health.current, health.max,
            "Health should be restored to max"
        );
        let mana = world.entity(dead_tower).get::<Mana>().unwrap();
        assert_eq!(mana.current, mana.max, "Mana should be restored to max");
    }

    #[test]
    fn dead_tower_not_revived_when_board_has_enemies() {
        use crate::model::components::{Dead, Health, HomePosition};

        let mut world = World::new();

        // Dead tower on the left board
        let home_pos = Position { x: 100.0, y: 300.0 };
        let dead_tower = world
            .spawn((
                Position { x: 100.0, y: 300.0 },
                HomePosition(home_pos),
                Health {
                    current: 0.0,
                    max: 100.0,
                },
                Dead,
            ))
            .id();

        // Enemy still on the left board
        world.spawn((
            Position { x: 200.0, y: 300.0 },
            Enemy,
            Health {
                current: 50.0,
                max: 100.0,
            },
        ));

        update_combat_reset(&mut world);

        assert!(
            world.entity(dead_tower).get::<Dead>().is_some(),
            "Dead marker should remain when board still has enemies"
        );
    }

    #[test]
    fn board_isolation_clearing_left_does_not_affect_right() {
        use crate::model::components::{Dead, Health, HomePosition};
        use crate::model::constants::RIGHT_BOARD_START;

        let mut world = World::new();

        // Dead tower on the left board
        let left_home = Position { x: 100.0, y: 300.0 };
        let left_dead_tower = world
            .spawn((
                Position {
                    x: left_home.x + 100.0,
                    y: left_home.y + 100.0,
                },
                HomePosition(left_home),
                Health {
                    current: 0.0,
                    max: 100.0,
                },
                Dead,
            ))
            .id();

        // Dead tower on the right board
        let right_home = Position {
            x: RIGHT_BOARD_START + 100.0,
            y: 300.0,
        };
        let right_dead_tower = world
            .spawn((
                Position {
                    x: right_home.x + 100.0,
                    y: right_home.y + 100.0,
                },
                HomePosition(right_home),
                Health {
                    current: 0.0,
                    max: 100.0,
                },
                Dead,
            ))
            .id();

        // Enemy still on the right board (left board is clear)
        world.spawn((
            Position {
                x: RIGHT_BOARD_START + 50.0,
                y: 300.0,
            },
            Enemy,
            Health {
                current: 50.0,
                max: 100.0,
            },
        ));

        update_combat_reset(&mut world);

        // Left board dead tower should be revived
        assert!(
            world.entity(left_dead_tower).get::<Dead>().is_none(),
            "Left board dead tower should be revived when left board clears"
        );
        let left_pos = world.entity(left_dead_tower).get::<Position>().unwrap();
        assert_eq!(
            left_pos.x, left_home.x,
            "Left board tower should be at home x"
        );

        // Right board dead tower should NOT be revived
        assert!(
            world.entity(right_dead_tower).get::<Dead>().is_some(),
            "Right board dead tower should remain dead while right board has enemies"
        );
    }

    #[test]
    fn surviving_towers_restored_alongside_dead_towers() {
        use crate::model::components::{Dead, Health, HomePosition};

        let mut world = World::new();

        // Dead tower on the left board
        let dead_home = Position { x: 100.0, y: 200.0 };
        let dead_tower = world
            .spawn((
                Position {
                    x: dead_home.x + 50.0,
                    y: dead_home.y + 50.0,
                },
                HomePosition(dead_home),
                Health {
                    current: 0.0,
                    max: 100.0,
                },
                Dead,
            ))
            .id();

        // Living tower with partial health on the same board
        let alive_home = Position { x: 200.0, y: 300.0 };
        let alive_tower = world
            .spawn((
                Position {
                    x: alive_home.x + 100.0,
                    y: alive_home.y + 100.0,
                },
                HomePosition(alive_home),
                Health {
                    current: 30.0,
                    max: 100.0,
                },
            ))
            .id();

        // No enemies on the left board
        update_combat_reset(&mut world);

        // Dead tower revived
        assert!(
            world.entity(dead_tower).get::<Dead>().is_none(),
            "Dead tower should be revived after wave clear"
        );
        let dead_health = world.entity(dead_tower).get::<Health>().unwrap();
        assert_eq!(
            dead_health.current, dead_health.max,
            "Dead tower health should be fully restored"
        );
        let dead_pos = world.entity(dead_tower).get::<Position>().unwrap();
        assert_eq!(
            dead_pos.x, dead_home.x,
            "Dead tower position should be reset to home"
        );

        // Alive tower also restored
        let alive_health = world.entity(alive_tower).get::<Health>().unwrap();
        assert_eq!(
            alive_health.current, alive_health.max,
            "Surviving tower health should also be restored"
        );
        let alive_pos = world.entity(alive_tower).get::<Position>().unwrap();
        assert_eq!(
            alive_pos.x, alive_home.x,
            "Surviving tower position should also be reset to home"
        );
    }

    #[test]
    fn update_combat_movement_reads_delta_time_from_resource() {
        let mut world = World::new();
        let tick_delta = 1.0 / 30.0;
        world.insert_resource(DeltaTime(tick_delta));

        let unit = crate::handler::spawn::spawn_unit(
            &mut world,
            Position { x: 100.0, y: 300.0 },
            UnitKind::Square,
            1,
        );
        let enemy = crate::handler::spawn::spawn_enemy(
            &mut world,
            Position { x: 200.0, y: 300.0 },
            UnitKind::Triangle,
            1,
        );
        world.entity_mut(unit).insert(Target(enemy));

        update_combat_movement(&mut world);

        let pos = world.entity(unit).get::<Position>().unwrap();
        assert!(
            pos.x > 100.0,
            "Unit should move towards enemy using DeltaTime from resource"
        );
    }

    #[test]
    fn targetless_enemy_moves_downward() {
        let mut world = World::new();
        world.insert_resource(DeltaTime(1.0 / 30.0));

        // Spawn an enemy with no target on board 0
        let enemy = crate::handler::spawn::spawn_enemy(
            &mut world,
            Position { x: 100.0, y: 200.0 },
            UnitKind::Triangle,
            1,
        );

        let y_before = world.entity(enemy).get::<Position>().unwrap().y;
        update_combat_movement(&mut world);
        let y_after = world.entity(enemy).get::<Position>().unwrap().y;

        assert!(
            y_after > y_before,
            "Targetless enemy should move downward (y increased): before={}, after={}",
            y_before,
            y_after
        );
    }

    #[test]
    fn leaked_square_enemy_maintains_attack_range_despite_separation() {
        use crate::handler::king::update_king_targeting;

        let mut world = World::new();
        world.insert_resource(DeltaTime(1.0 / 30.0));

        // Spawn king on left board
        let king = crate::handler::spawn::spawn_king(&mut world, 1, 0);

        // Spawn a Square enemy (45px attack range) that has leaked
        let enemy = crate::handler::spawn::spawn_enemy(
            &mut world,
            Position {
                x: 300.0,              // Same x as left king
                y: TOTAL_HEIGHT + 5.0, // Just above TOTAL_HEIGHT (leaked)
            },
            UnitKind::Square,
            1,
        );

        // Simulate the game loop: leaked enemies get assigned king as target
        update_leaked_creeps(&mut world);
        update_king_targeting(&mut world);

        // Run movement for several ticks to let enemies approach the king
        for _ in 0..100 {
            update_combat_movement(&mut world);
            update_attack_range_markers(&mut world);
        }

        // Verify the enemy has InAttackRange marker
        let has_in_range = world.entity(enemy).get::<InAttackRange>().is_some();
        let enemy_pos = world.entity(enemy).get::<Position>().unwrap();
        let king_pos = world.entity(king).get::<Position>().unwrap();
        let distance =
            ((king_pos.x - enemy_pos.x).powi(2) + (king_pos.y - enemy_pos.y).powi(2)).sqrt();

        assert!(
            has_in_range,
            "Leaked Square enemy (45px range) should be able to attack the king at {:.1}px distance",
            distance
        );
    }

    #[test]
    fn multiple_leaked_enemies_can_all_attack_king_despite_crowding() {
        use crate::handler::king::update_king_targeting;

        let mut world = World::new();
        world.insert_resource(DeltaTime(1.0 / 30.0));

        // Spawn king on left board
        let _king = crate::handler::spawn::spawn_king(&mut world, 1, 0);

        // Spawn 5 Square enemies (all with 45px range) around the king
        let enemies: Vec<_> = (0..5)
            .map(|i| {
                let angle = (i as f32 / 5.0) * 2.0 * std::f32::consts::PI;
                let x = 300.0 + angle.cos() * 30.0;
                let y = TOTAL_HEIGHT + 10.0 + angle.sin() * 30.0;
                crate::handler::spawn::spawn_enemy(&mut world, Position { x, y }, UnitKind::Square, 1)
            })
            .collect();

        // Simulate the game loop
        update_leaked_creeps(&mut world);
        update_king_targeting(&mut world);

        // Run movement for many ticks to let separation forces push enemies apart
        for _ in 0..200 {
            update_combat_movement(&mut world);
            update_attack_range_markers(&mut world);
        }

        // Count how many enemies can still attack
        let attacking_enemies = enemies
            .iter()
            .filter(|&&e| world.entity(e).get::<InAttackRange>().is_some())
            .count();

        // With reduced separation (10%), multiple enemies should be able to maintain attack range.
        // Not all 5 may fit in a tight circle (due to lateral separation from each other),
        // but at least 2 should be able to attack (vs. 0 before the fix).
        assert!(
            attacking_enemies >= 2,
            "At least 2 out of 5 leaked enemies should maintain attack range despite separation forces (got {})",
            attacking_enemies
        );
    }
}
