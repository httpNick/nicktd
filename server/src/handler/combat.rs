use crate::model::{
    components::{Enemy, Position, Target, CollisionRadius, AttackRange, InAttackRange, Worker},
};
use bevy_ecs::prelude::{Entity, With, Without, World};

pub const SPEED: f32 = 100.0; // pixels per second
pub const DEFAULT_COLLISION_RADIUS: f32 = 20.0;
pub const DEFAULT_ATTACK_RANGE: f32 = 45.0; // Melee range: slightly more than 2x radius to ensure they can hit

pub fn update_targeting(world: &mut World) {
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

pub fn update_combat_movement(world: &mut World, tick_delta: f32) {
    // --- MOVEMENT & COLLISION SYSTEM ---
    let physical_entities: Vec<(Entity, Position, f32)> = world
        .query::<(Entity, &Position, &CollisionRadius)>()
        .iter(world)
        .map(|(e, p, r)| (e, *p, r.0))
        .collect();

    let mut movements = Vec::new();
    let mut combat_markers = Vec::new(); // (Entity, bool) where true = add, false = remove

    // First pass: Calculate all movement vectors
    let mut query = world.query::<(Entity, &Position, Option<&Target>, Option<&AttackRange>, &CollisionRadius)>();
    for (entity, pos, target_opt, attack_range_opt, collision_radius) in query.iter(world) {
        let mut velocity_x = 0.0;
        let mut velocity_y = 0.0;

        // 1. Chasing Force & Range Gating
        if let Some(target) = target_opt {
            if let Some(target_pos) = physical_entities.iter().find(|(e, _, _)| *e == target.0).map(|(_, p, _)| p) {
                let dx = target_pos.x - pos.x;
                let dy = target_pos.y - pos.y;
                let distance = (dx*dx + dy*dy).sqrt();
                
                let range = attack_range_opt.map(|r| r.0).unwrap_or(0.0);

                if distance > range && distance > 0.0 {
                    velocity_x += (dx / distance) * SPEED;
                    velocity_y += (dy / distance) * SPEED;
                    combat_markers.push((entity, false)); // Out of range
                } else if distance <= range {
                    combat_markers.push((entity, true)); // In range
                }
            } else {
                combat_markers.push((entity, false)); // No target found
            }
        } else {
            combat_markers.push((entity, false)); // No target assigned
        }

        // 2. Separation Force
        for (other_entity, other_pos, other_radius) in &physical_entities {
            if entity == *other_entity { continue; }

            let dx = pos.x - other_pos.x;
            let dy = pos.y - other_pos.y;
            let distance = (dx*dx + dy*dy).sqrt();
            let min_dist = collision_radius.0 + other_radius;

            if distance < min_dist {
                // Overlap detected
                if distance > 0.0 {
                    // Repulsive force away from neighbor
                    velocity_x += (dx / distance) * SPEED;
                    velocity_y += (dy / distance) * SPEED;
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
        if let Some(mut pos) = world.get_mut::<Position>(entity) {
            pos.x = (pos.x + dx).clamp(0.0, 600.0);
            pos.y = (pos.y + dy).clamp(0.0, 600.0);
        }
    }

    // Third pass: Apply combat markers
    for (entity, in_range) in combat_markers {
        if in_range {
            if world.entity(entity).get::<InAttackRange>().is_none() {
                world.entity_mut(entity).insert(InAttackRange);
            }
        } else {
            if world.entity(entity).get::<InAttackRange>().is_some() {
                world.entity_mut(entity).remove::<InAttackRange>();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::components::TargetPositions;
    use crate::handler::worker::{VEIN_POSITIONS, CART_POSITIONS};
    use crate::model::shape::Shape;

    #[test]
    fn targeting_ignores_workers() {
        let mut world = World::new();
        // Spawn Enemy
        let _enemy = crate::handler::spawn::spawn_enemy(&mut world, Position { x: 100.0, y: 100.0 }, Shape::Triangle);
        
        let targets = TargetPositions {
            vein: VEIN_POSITIONS[0],
            cart: CART_POSITIONS[0],
        };
        // Spawn Worker (close to enemy)
        let worker = crate::handler::spawn::spawn_worker(&mut world, 1, targets);
        
        // Spawn Normal Unit (farther)
        let unit = crate::handler::spawn::spawn_unit(&mut world, Position { x: 200.0, y: 200.0 }, Shape::Square, 1);

        update_targeting(&mut world);

        // Assert Worker does NOT have Target
        assert!(world.entity(worker).get::<Target>().is_none(), "Worker should not have target");
        // Assert Unit DOES have Target
        assert!(world.entity(unit).get::<Target>().is_some(), "Unit should have target");
    }

    #[test]
    fn nothing_targets_workers() {
        let mut world = World::new();
        let targets = TargetPositions {
            vein: VEIN_POSITIONS[0],
            cart: CART_POSITIONS[0],
        };
        // Spawn Worker (very close to enemy)
        let _ = crate::handler::spawn::spawn_worker(&mut world, 1, targets);
        
        // Spawn Enemy
        let enemy = crate::handler::spawn::spawn_enemy(&mut world, Position { x: 105.0, y: 105.0 }, Shape::Triangle);
        
        // Spawn Normal Unit (farther than worker)
        let unit = crate::handler::spawn::spawn_unit(&mut world, Position { x: 200.0, y: 200.0 }, Shape::Square, 1);

        update_targeting(&mut world);

        // Enemy should NOT target the worker, but SHOULD target the unit
        let target = world.entity(enemy).get::<Target>();
        assert!(target.is_some(), "Enemy should have a target");
        assert_eq!(target.unwrap().0, unit, "Enemy should target unit, not worker");
    }

    #[test]
    fn range_aware_movement_stops_at_range() {
        let mut world = World::new();
        
        let range = 50.0;
        let unit = crate::handler::spawn::spawn_unit(
            &mut world,
            Position { x: 0.0, y: 0.0 },
            Shape::Square,
            1,
        );
        // Overwrite default range for test
        world.entity_mut(unit).insert(AttackRange(range));

        let enemy = crate::handler::spawn::spawn_enemy(
            &mut world,
            Position { x: 100.0, y: 0.0 }, // 100 pixels away
            Shape::Triangle,
        );

        world.entity_mut(unit).insert(Target(enemy));

        let tick_delta = 1.0 / 30.0;

        // 1. Far away: should move
        update_combat_movement(&mut world, tick_delta);
        let pos = world.entity(unit).get::<Position>().unwrap();
        assert!(pos.x > 0.0, "Unit should move towards enemy when far away");

        // 2. Within range: should stop
        // Teleport to just inside range (40 pixels away)
        world.entity_mut(unit).insert(Position { x: 60.0, y: 0.0 });
        update_combat_movement(&mut world, tick_delta);
        let pos = world.entity(unit).get::<Position>().unwrap();
        assert_eq!(pos.x, 60.0, "Unit should NOT move when within attack range");

        // 3. Exactly at range: should stop
        world.entity_mut(unit).insert(Position { x: 50.0, y: 0.0 });
        update_combat_movement(&mut world, tick_delta);
        let pos = world.entity(unit).get::<Position>().unwrap();
        assert_eq!(pos.x, 50.0, "Unit should NOT move when exactly at attack range");
    }

    #[test]
    fn units_separate_when_overlapping() {
        let mut world = World::new();
        
        let radius = 10.0;
        // Spawn two units very close to each other
        let unit_a = crate::handler::spawn::spawn_unit(
            &mut world,
            Position { x: 100.0, y: 100.0 },
            Shape::Square,
            1,
        );
        world.entity_mut(unit_a).insert(CollisionRadius(radius));

        let unit_b = crate::handler::spawn::spawn_unit(
            &mut world,
            Position { x: 105.0, y: 100.0 }, // Overlapping by 15 pixels (combined radius 20)
            Shape::Square,
            1,
        );
        world.entity_mut(unit_b).insert(CollisionRadius(radius));

        let tick_delta = 1.0 / 30.0;
        update_combat_movement(&mut world, tick_delta);

        let pos_a = world.entity(unit_a).get::<Position>().unwrap();
        let pos_b = world.entity(unit_b).get::<Position>().unwrap();

        assert!(pos_a.x < 100.0, "Unit A should move away from Unit B (Left)");
        assert!(pos_b.x > 105.0, "Unit B should move away from Unit A (Right)");
    }

    #[test]
    fn unit_can_attack_only_when_in_range() {
        let mut world = World::new();
        
        let range = 50.0;
        let unit = crate::handler::spawn::spawn_unit(
            &mut world,
            Position { x: 0.0, y: 0.0 },
            Shape::Square,
            1,
        );
        world.entity_mut(unit).insert(AttackRange(range));

        let enemy = crate::handler::spawn::spawn_enemy(
            &mut world,
            Position { x: 100.0, y: 0.0 },
            Shape::Triangle,
        );

        world.entity_mut(unit).insert(Target(enemy));

        let tick_delta = 1.0 / 30.0;

        // 1. Far away: no marker
        update_combat_movement(&mut world, tick_delta);
        assert!(world.entity(unit).get::<InAttackRange>().is_none(), "Should NOT have InAttackRange when far away");

        // 2. Within range: has marker
        world.entity_mut(unit).insert(Position { x: 60.0, y: 0.0 });
        update_combat_movement(&mut world, tick_delta);
        assert!(world.entity(unit).get::<InAttackRange>().is_some(), "Should have InAttackRange when within range");

        // 3. Move back out: marker removed
        world.entity_mut(unit).insert(Position { x: 0.0, y: 0.0 });
        update_combat_movement(&mut world, tick_delta);
        assert!(world.entity(unit).get::<InAttackRange>().is_none(), "Should remove InAttackRange when moving out of range");
    }

    #[test]
    fn entities_stay_within_bounds() {
        let mut world = World::new();
        
        // Spawn unit at the very edge
        let unit = crate::handler::spawn::spawn_unit(
            &mut world,
            Position { x: 5.0, y: 5.0 },
            Shape::Square,
            1,
        );

        // Spawn many units around it to push it out
        for i in 0..10 {
            let _ = crate::handler::spawn::spawn_unit(
                &mut world,
                Position { x: 10.0 + (i as f32), y: 10.0 + (i as f32) },
                Shape::Square,
                1,
            );
        }

        let tick_delta = 1.0 / 30.0;

        // Run many ticks to let separation force push it
        for _ in 0..100 {
            update_combat_movement(&mut world, tick_delta);
        }

        let pos = world.entity(unit).get::<Position>().unwrap();
        assert!(pos.x >= 0.0, "Entity should not be pushed off the left edge (x={})", pos.x);
        assert!(pos.y >= 0.0, "Entity should not be pushed off the top edge (y={})", pos.y);
        assert!(pos.x <= 600.0, "Entity should not be pushed off the right edge (x={})", pos.x);
        assert!(pos.y <= 600.0, "Entity should not be pushed off the bottom edge (y={})", pos.y);
    }
}
