use crate::model::components::{
    AttackRange, AttackStats, AttackTimer, CollisionRadius, Enemy, Health, InAttackRange, Position,
    Target, Worker,
};
use bevy_ecs::prelude::{Entity, With, Without, World};

pub const SPEED: f32 = 100.0; // pixels per second
pub const DEFAULT_COLLISION_RADIUS: f32 = 20.0;
pub const DEFAULT_ATTACK_RANGE: f32 = 45.0; // Melee range: slightly more than 2x radius to ensure they can hit

pub fn update_targeting(world: &mut World) {
    // --- 1. VALIDATE AND REMOVE INVALID TARGETS IMMEDIATELY ---
    let mut to_remove = Vec::new();
    {
        let mut query = world.query::<(Entity, &Target)>();
        for (entity, target) in query.iter(world) {
            if !world.entities().contains(target.0) {
                to_remove.push(entity);
            }
        }
    }
    for entity in to_remove {
        world.entity_mut(entity).remove::<Target>();
    }

    let mut commands = Vec::new(); // (Entity, Target)

    // --- 2. UNIT TARGETING (Units target closest Enemy) ---
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
                let distance_sq =
                    (unit_pos.x - enemy_pos.x).powi(2) + (unit_pos.y - enemy_pos.y).powi(2);
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
        let mut query =
            world.query_filtered::<(Entity, &Position), (With<Enemy>, Without<Target>)>();
        for (enemy_entity, enemy_pos) in query.iter(world) {
            let mut closest_unit: Option<(Entity, f32)> = None;
            for (unit_entity, unit_pos) in &unit_positions {
                let distance_sq =
                    (enemy_pos.x - unit_pos.x).powi(2) + (enemy_pos.y - unit_pos.y).powi(2);
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
    let mut query = world.query::<(
        Entity,
        &Position,
        Option<&Target>,
        Option<&AttackRange>,
        &CollisionRadius,
    )>();
    for (entity, pos, target_opt, attack_range_opt, collision_radius) in query.iter(world) {
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

pub fn process_combat(world: &mut World, tick_delta: f32) {
    let mut damage_to_apply = Vec::new(); // (TargetEntity, Damage)
    let mut timer_updates = Vec::new(); // (AttackerEntity, NewTimerValue)

    let mut query = world.query::<(
        Entity,
        &AttackStats,
        &AttackTimer,
        Option<&Target>,
        Option<&InAttackRange>,
    )>();
    for (attacker_entity, stats, timer, target_opt, in_range_opt) in query.iter(world) {
        // Update timer
        let mut new_timer = (timer.0 - tick_delta).max(0.0);

        // Try to attack if in range and timer is 0
        if in_range_opt.is_some() && new_timer <= 0.0 {
            if let Some(target) = target_opt {
                // We deal damage!
                damage_to_apply.push((target.0, stats.damage));
                // Reset timer: 1.0 / rate
                new_timer = 1.0 / stats.rate;
            }
        }

        timer_updates.push((attacker_entity, new_timer));
    }

    // Apply timer updates
    for (entity, new_val) in timer_updates {
        if let Some(mut timer) = world.get_mut::<AttackTimer>(entity) {
            timer.0 = new_val;
        }
    }

    // Apply damage
    for (target_entity, damage) in damage_to_apply {
        if let Some(mut health) = world.get_mut::<Health>(target_entity) {
            health.current -= damage;
        }
    }
}

pub fn cleanup_dead_entities(world: &mut World) {
    let mut dead_entities = Vec::new();
    let mut query = world.query::<(Entity, &Health)>();
    for (entity, health) in query.iter(world) {
        if health.current <= 0.0 {
            dead_entities.push(entity);
        }
    }

    for entity in dead_entities {
        world.despawn(entity);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handler::worker::{CART_POSITIONS, VEIN_POSITIONS};
    use crate::model::components::TargetPositions;
    use crate::model::shape::Shape;

    #[test]
    fn targeting_ignores_workers() {
        let mut world = World::new();
        // Spawn Enemy
        let _enemy = crate::handler::spawn::spawn_enemy(
            &mut world,
            Position { x: 100.0, y: 100.0 },
            Shape::Triangle,
        );

        let targets = TargetPositions {
            vein: VEIN_POSITIONS[0],
            cart: CART_POSITIONS[0],
        };
        // Spawn Worker (close to enemy)
        let worker = crate::handler::spawn::spawn_worker(&mut world, 1, targets);

        // Spawn Normal Unit (farther)
        let unit = crate::handler::spawn::spawn_unit(
            &mut world,
            Position { x: 200.0, y: 200.0 },
            Shape::Square,
            1,
        );

        update_targeting(&mut world);

        // Assert Worker does NOT have Target
        assert!(
            world.entity(worker).get::<Target>().is_none(),
            "Worker should not have target"
        );
        // Assert Unit DOES have Target
        assert!(
            world.entity(unit).get::<Target>().is_some(),
            "Unit should have target"
        );
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
        let enemy = crate::handler::spawn::spawn_enemy(
            &mut world,
            Position { x: 105.0, y: 105.0 },
            Shape::Triangle,
        );

        // Spawn Normal Unit (farther than worker)
        let unit = crate::handler::spawn::spawn_unit(
            &mut world,
            Position { x: 200.0, y: 200.0 },
            Shape::Square,
            1,
        );

        update_targeting(&mut world);

        // Enemy should NOT target the worker, but SHOULD target the unit
        let target = world.entity(enemy).get::<Target>();
        assert!(target.is_some(), "Enemy should have a target");
        assert_eq!(
            target.unwrap().0,
            unit,
            "Enemy should target unit, not worker"
        );
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
        assert!(
            world.entity(unit).get::<InAttackRange>().is_none(),
            "Should NOT have InAttackRange when far away"
        );

        // 2. Within range: has marker
        world.entity_mut(unit).insert(Position { x: 60.0, y: 0.0 });
        update_combat_movement(&mut world, tick_delta);
        assert!(
            world.entity(unit).get::<InAttackRange>().is_some(),
            "Should have InAttackRange when within range"
        );

        // 3. Move back out: marker removed
        world.entity_mut(unit).insert(Position { x: 0.0, y: 0.0 });
        update_combat_movement(&mut world, tick_delta);
        assert!(
            world.entity(unit).get::<InAttackRange>().is_none(),
            "Should remove InAttackRange when moving out of range"
        );
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
                Position {
                    x: 10.0 + (i as f32),
                    y: 10.0 + (i as f32),
                },
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
        assert!(
            pos.x >= 0.0,
            "Entity should not be pushed off the left edge (x={})",
            pos.x
        );
        assert!(
            pos.y >= 0.0,
            "Entity should not be pushed off the top edge (y={})",
            pos.y
        );
        assert!(
            pos.x <= 600.0,
            "Entity should not be pushed off the right edge (x={})",
            pos.x
        );
        assert!(
            pos.y <= 600.0,
            "Entity should not be pushed off the bottom edge (y={})",
            pos.y
        );
    }

    #[test]
    fn combat_system_applies_damage() {
        use crate::model::components::{AttackStats, AttackTimer, Health};

        let mut world = World::new();

        // Attacker
        let attacker = world
            .spawn((
                InAttackRange,
                AttackStats {
                    damage: 10.0,
                    rate: 1.0,
                }, // 1 attack per second
                AttackTimer(0.0), // Ready to attack
            ))
            .id();

        // Target
        let target = world
            .spawn((Health {
                current: 100.0,
                max: 100.0,
            },))
            .id();

        world.entity_mut(attacker).insert(Target(target));

        // 1. Process combat - should deal damage and reset timer
        process_combat(&mut world, 0.1);

        let target_health = world.entity(target).get::<Health>().unwrap();
        assert_eq!(target_health.current, 90.0, "Target should have lost 10 HP");

        let attacker_timer = world.entity(attacker).get::<AttackTimer>().unwrap();
        assert_eq!(attacker_timer.0, 1.0, "Timer should be reset to 1.0");

        // 2. Process combat again with small delta - should NOT deal damage
        process_combat(&mut world, 0.5);
        let target_health = world.entity(target).get::<Health>().unwrap();
        assert_eq!(
            target_health.current, 90.0,
            "Target should NOT have lost more HP yet"
        );

        let attacker_timer = world.entity(attacker).get::<AttackTimer>().unwrap();
        assert_eq!(attacker_timer.0, 0.5, "Timer should have decreased by 0.5");

        // 3. Process combat enough to trigger second attack
        process_combat(&mut world, 0.5); // Timer reaches 0.0
        let target_health = world.entity(target).get::<Health>().unwrap();
        assert_eq!(
            target_health.current, 80.0,
            "Target should have lost another 10 HP"
        );

        let attacker_timer = world.entity(attacker).get::<AttackTimer>().unwrap();
        assert_eq!(attacker_timer.0, 1.0, "Timer should be reset again");
    }

    #[test]
    fn cleanup_removes_dead_entities() {
        use crate::model::components::Health;

        let mut world = World::new();

        let alive = world
            .spawn(Health {
                current: 10.0,
                max: 100.0,
            })
            .id();
        let dead = world
            .spawn(Health {
                current: 0.0,
                max: 100.0,
            })
            .id();
        let overkill = world
            .spawn(Health {
                current: -5.0,
                max: 100.0,
            })
            .id();

        cleanup_dead_entities(&mut world);

        assert!(
            world.entities().contains(alive),
            "Alive entity should still exist"
        );
        assert!(
            !world.entities().contains(dead),
            "Dead entity should be removed"
        );
        assert!(
            !world.entities().contains(overkill),
            "Overkill entity should be removed"
        );
    }

    #[test]
    fn targeting_handles_removed_entities() {
        let mut world = World::new();

        let unit = crate::handler::spawn::spawn_unit(
            &mut world,
            Position { x: 0.0, y: 0.0 },
            Shape::Circle,
            1,
        );
        let enemy = crate::handler::spawn::spawn_enemy(
            &mut world,
            Position { x: 10.0, y: 10.0 },
            Shape::Circle,
        );

        // 1. Initial targeting
        update_targeting(&mut world);
        assert_eq!(world.entity(unit).get::<Target>().unwrap().0, enemy);

        // 2. Remove enemy
        world.despawn(enemy);

        // 3. Update targeting again - should not crash and should ideally clear or re-target
        update_targeting(&mut world);

        // The current implementation of update_targeting only ADDS targets to entities WITHOUT targets.
        // We might need to update it to REMOVE invalid targets.
        let target = world.entity(unit).get::<Target>();
        assert!(
            target.is_none(),
            "Unit should no longer have a target if the target is gone"
        );
    }

    #[test]
    fn full_combat_cycle_integration() {
        let mut world = World::new();
        let tick_delta = 1.0 / 30.0;

        // Spawn unit and enemy
        let unit = crate::handler::spawn::spawn_unit(
            &mut world,
            Position { x: 0.0, y: 0.0 },
            Shape::Circle,
            1,
        );
        let enemy = crate::handler::spawn::spawn_enemy(
            &mut world,
            Position { x: 100.0, y: 0.0 },
            Shape::Circle,
        );

        // 1. Targeting
        update_targeting(&mut world);
        assert_eq!(world.entity(unit).get::<Target>().unwrap().0, enemy);

        // 2. Movement - multiple ticks until in range
        for _ in 0..100 {
            update_combat_movement(&mut world, tick_delta);
        }
        assert!(world.entity(unit).get::<InAttackRange>().is_some());

        // 3. Combat - deal damage
        let initial_health = world.entity(enemy).get::<Health>().unwrap().current;
        process_combat(&mut world, tick_delta);
        let post_attack_health = world.entity(enemy).get::<Health>().unwrap().current;
        assert!(
            post_attack_health < initial_health,
            "Enemy should have taken damage"
        );

        // 4. Cleanup - reduce HP to 0 and verify removal
        world.entity_mut(enemy).get_mut::<Health>().unwrap().current = 0.0;
        cleanup_dead_entities(&mut world);
        assert!(
            !world.entities().contains(enemy),
            "Enemy should be removed from world"
        );

        // 5. Re-targeting - Unit should lose target
        update_targeting(&mut world);
        assert!(
            world.entity(unit).get::<Target>().is_none(),
            "Unit should have no target after enemy is dead"
        );
    }

    #[test]
    fn target_reacquisition_after_kill() {
        let mut world = World::new();

        // Spawn 1 unit and 2 enemies
        let unit = crate::handler::spawn::spawn_unit(
            &mut world,
            Position { x: 0.0, y: 0.0 },
            Shape::Circle,
            1,
        );
        let enemy1 = crate::handler::spawn::spawn_enemy(
            &mut world,
            Position { x: 10.0, y: 0.0 },
            Shape::Circle,
        );
        let enemy2 = crate::handler::spawn::spawn_enemy(
            &mut world,
            Position { x: 100.0, y: 0.0 },
            Shape::Circle,
        );

        // 1. Target first enemy (the closest one)
        update_targeting(&mut world);
        assert_eq!(world.entity(unit).get::<Target>().unwrap().0, enemy1);

        // 2. Kill first enemy
        world
            .entity_mut(enemy1)
            .get_mut::<Health>()
            .unwrap()
            .current = 0.0;
        cleanup_dead_entities(&mut world);
        assert!(!world.entities().contains(enemy1));

        // 3. Update targeting - should remove invalid target and find enemy2
        update_targeting(&mut world);
        let target = world.entity(unit).get::<Target>();
        assert!(target.is_some(), "Unit should have a new target");
        assert_eq!(
            target.unwrap().0,
            enemy2,
            "Unit should now target the second enemy"
        );
    }
}
