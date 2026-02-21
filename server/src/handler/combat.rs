use crate::model::components::{
    AttackRange, AttackStats, AttackTimer, CollisionRadius, CombatProfile, Enemy, Health,
    HomePosition, InAttackRange, Mana, Position, Target, Worker,
};
use crate::model::constants::{LEFT_BOARD_END, RIGHT_BOARD_END, RIGHT_BOARD_START, TOTAL_HEIGHT};
use crate::model::messages::CombatEvent;
use bevy_ecs::prelude::{Entity, With, Without, World};

pub const SPEED: f32 = 80.0; // pixels per second

fn get_board(x: f32) -> Option<u8> {
    if x < LEFT_BOARD_END {
        Some(0)
    } else if x >= RIGHT_BOARD_START && x < RIGHT_BOARD_END {
        Some(1)
    } else {
        None
    }
}

pub fn update_targeting(world: &mut World) {
    // --- 1. VALIDATE AND REMOVE INVALID TARGETS IMMEDIATELY ---
    let mut to_remove = Vec::new();
    {
        let mut query = world.query::<(Entity, &Target, &Position)>();
        for (entity, target, pos) in query.iter(world) {
            if !world.entities().contains(target.0) {
                to_remove.push(entity);
                continue;
            }

            // Also check if target is on the same board
            if let Some(target_pos) = world.get::<Position>(target.0) {
                if get_board(pos.x) != get_board(target_pos.x) {
                    to_remove.push(entity);
                }
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
            let unit_board = get_board(unit_pos.x);
            if unit_board.is_none() {
                continue;
            }

            let mut closest_enemy: Option<(Entity, f32)> = None;
            for (enemy_entity, enemy_pos) in &enemy_positions {
                if get_board(enemy_pos.x) != unit_board {
                    continue;
                }

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
            let enemy_board = get_board(enemy_pos.x);
            if enemy_board.is_none() {
                continue;
            }

            let mut closest_unit: Option<(Entity, f32)> = None;
            for (unit_entity, unit_pos) in &unit_positions {
                if get_board(unit_pos.x) != enemy_board {
                    continue;
                }

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
        .query_filtered::<(Entity, &Position, &CollisionRadius), Without<Worker>>()
        .iter(world)
        .map(|(e, p, r)| (e, *p, r.0))
        .collect();

    let mut movements = Vec::new();
    let mut combat_markers = Vec::new(); // (Entity, bool) where true = add, false = remove

    // First pass: Calculate all movement vectors
    let mut query = world.query_filtered::<(
        Entity,
        &Position,
        Option<&Target>,
        Option<&AttackRange>,
        &CollisionRadius,
    ), Without<Worker>>();
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
        let radius = world
            .get::<CollisionRadius>(entity)
            .map(|r| r.0)
            .unwrap_or(0.0);
        let home_x = world
            .get::<HomePosition>(entity)
            .map(|h| h.0.x)
            .unwrap_or(0.0);

        if let Some(mut pos) = world.get_mut::<Position>(entity) {
            pos.x += dx;
            pos.y = (pos.y + dy).clamp(radius, TOTAL_HEIGHT - radius);

            if home_x < LEFT_BOARD_END {
                pos.x = pos.x.clamp(radius, LEFT_BOARD_END - radius);
            } else {
                pos.x = pos
                    .x
                    .clamp(RIGHT_BOARD_START + radius, RIGHT_BOARD_END - radius);
            }
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

    let mut query = world.query::<(
        Entity,
        &mut Position,
        &HomePosition,
        Option<&mut Health>,
        Option<&mut Mana>,
    )>();
    for (_entity, mut pos, home, health_opt, mana_opt) in query.iter_mut(world) {
        if let Some(board_idx) = get_board(home.0.x) {
            if boards_to_reset.contains(&board_idx) {
                // Reset position
                *pos = home.0;

                // Restore Health
                if let Some(mut health) = health_opt {
                    health.current = health.max;
                }

                // Restore Mana
                if let Some(mut mana) = mana_opt {
                    mana.current = mana.max;
                }
            }
        }
    }
}

pub fn update_active_combat_stats(world: &mut World) {
    let mut updates = Vec::new(); // (Entity, damage, rate, range, type)

    let mut query = world.query::<(
        Entity,
        &CombatProfile,
        Option<&Mana>,
        &AttackStats,
        &AttackRange,
    )>();
    for (entity, profile, mana_opt, current_stats, current_range) in query.iter(world) {
        let use_primary = if profile.mana_cost > 0.0 {
            if let Some(mana) = mana_opt {
                mana.current >= profile.mana_cost
            } else {
                false
            }
        } else {
            true
        };

        let selected = if use_primary {
            &profile.primary
        } else {
            profile.secondary.as_ref().unwrap_or(&profile.primary)
        };

        if selected.damage != current_stats.damage
            || selected.rate != current_stats.rate
            || selected.damage_type != current_stats.damage_type
            || selected.range != current_range.0
        {
            updates.push((
                entity,
                selected.damage,
                selected.rate,
                selected.range,
                selected.damage_type,
            ));
        }
    }

    for (entity, damage, rate, range, damage_type) in updates {
        if let Some(mut stats) = world.get_mut::<AttackStats>(entity) {
            stats.damage = damage;
            stats.rate = rate;
            stats.damage_type = damage_type;
        }
        if let Some(mut r) = world.get_mut::<AttackRange>(entity) {
            r.0 = range;
        }
    }
}

pub fn process_combat(world: &mut World, tick_delta: f32) -> Vec<CombatEvent> {
    let mut attacks = Vec::new(); // (AttackerEntity, TargetEntity, Damage, DamageType)
    let mut timer_updates = Vec::new(); // (AttackerEntity, NewTimerValue)
    let mut mana_updates = Vec::new(); // (AttackerEntity, NewManaValue)

    let mut query = world.query::<(
        Entity,
        &AttackStats,
        &AttackTimer,
        Option<&Target>,
        Option<&InAttackRange>,
        Option<&CombatProfile>,
        Option<&Mana>,
    )>();
    for (attacker_entity, stats, timer, target_opt, in_range_opt, profile_opt, mana_opt) in
        query.iter(world)
    {
        // Update timer
        let mut new_timer = (timer.0 - tick_delta).max(0.0);

        // Try to attack if in range and timer is 0
        if in_range_opt.is_some() && new_timer <= 0.0 {
            if let Some(target) = target_opt {
                // Deduct mana if it was a primary attack with cost
                if let (Some(profile), Some(mana)) = (profile_opt, mana_opt) {
                    if profile.mana_cost > 0.0 && mana.current >= profile.mana_cost {
                        mana_updates.push((attacker_entity, mana.current - profile.mana_cost));
                    }
                }

                // Record attack
                attacks.push((attacker_entity, target.0, stats.damage, stats.damage_type));
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

    // Apply mana updates
    for (entity, new_val) in mana_updates {
        if let Some(mut mana) = world.get_mut::<Mana>(entity) {
            mana.current = new_val;
        }
    }

    let mut combat_events = Vec::new();

    // Apply damage and generate events
    for (attacker_entity, target_entity, damage, damage_type) in attacks {
        // Capture positions for event
        let start_pos = world
            .get::<Position>(attacker_entity)
            .copied()
            .unwrap_or(Position { x: 0.0, y: 0.0 });
        let end_pos = world
            .get::<Position>(target_entity)
            .copied()
            .unwrap_or(Position { x: 0.0, y: 0.0 });

        combat_events.push(CombatEvent {
            attacker_id: attacker_entity.index(),
            target_id: target_entity.index(),
            attack_type: damage_type,
            start_pos,
            end_pos,
        });

        if let Some(mut health) = world.get_mut::<Health>(target_entity) {
            health.current -= damage;
        }
    }

    combat_events
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

pub fn update_mana(world: &mut World, tick_delta: f32) {
    let mut query = world.query::<(Entity, &mut Mana)>();
    for (_entity, mut mana) in query.iter_mut(world) {
        mana.current = (mana.current + mana.regen * tick_delta).min(mana.max);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handler::worker::{CART_POSITIONS, VEIN_POSITIONS};
    use crate::model::components::{AttackProfile, DamageType, TargetPositions};
    use crate::model::shape::Shape;
    use crate::model::unit_config::{DEFAULT_ATTACK_RANGE, FIREBALL_MANA_COST, MAGE_MELEE_DAMAGE};

    #[test]
    fn targeting_ignores_workers() {
        let mut world = World::new();
        // Spawn Enemy
        let _enemy = crate::handler::spawn::spawn_enemy(
            &mut world,
            Position { x: 100.0, y: 100.0 },
            Shape::Triangle,
            1,
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
            1,
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
            1,
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
            1,
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

        let radius = 10.0;
        // Spawn unit at the very edge
        let unit = crate::handler::spawn::spawn_unit(
            &mut world,
            Position { x: 5.0, y: 5.0 },
            Shape::Square,
            1,
        );
        world.entity_mut(unit).insert(CollisionRadius(radius));

        // Spawn many units around it to push it out
        for _ in 0..10 {
            let _ = crate::handler::spawn::spawn_unit(
                &mut world,
                Position { x: 15.0, y: 15.0 },
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
                    damage_type: DamageType::PhysicalBasic,
                }, // 1 attack per second
                CombatProfile {
                    primary: AttackProfile {
                        damage: 10.0,
                        rate: 1.0,
                        range: DEFAULT_ATTACK_RANGE,
                        damage_type: DamageType::PhysicalBasic,
                    },
                    secondary: None,
                    mana_cost: 0.0,
                },
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
    fn mana_regeneration_works() {
        use crate::model::components::Mana;
        let mut world = World::new();

        let unit = world
            .spawn(Mana {
                current: 10.0,
                max: 100.0,
                regen: 5.0, // 5 mana per second
            })
            .id();

        let tick_delta = 0.5; // Half a second
        update_mana(&mut world, tick_delta);

        let mana = world.entity(unit).get::<Mana>().unwrap();
        assert_eq!(mana.current, 12.5, "Should regenerate 2.5 mana in 0.5s");

        // Test capping at max
        let unit_max = world
            .spawn(Mana {
                current: 99.0,
                max: 100.0,
                regen: 5.0,
            })
            .id();

        update_mana(&mut world, tick_delta);
        let mana_max = world.entity(unit_max).get::<Mana>().unwrap();
        assert_eq!(mana_max.current, 100.0, "Should cap at max mana");
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
            1,
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
    fn test_targeting_isolation() {
        use crate::model::constants::RIGHT_BOARD_START;
        let mut world = World::new();

        // Unit on Left Board (x=100)
        let unit = world
            .spawn((
                Position {
                    x: 0.0 + 100.0,
                    y: 300.0,
                },
                CollisionRadius(10.0),
            ))
            .id();

        // Enemy on Right Board (x=900)
        let enemy = world
            .spawn((
                Position {
                    x: RIGHT_BOARD_START + 100.0,
                    y: 300.0,
                },
                Enemy,
                CollisionRadius(10.0),
            ))
            .id();

        // 1. Update targeting
        update_targeting(&mut world);

        // 2. Assert NO targeting happens between them
        assert!(
            world.entity(unit).get::<Target>().is_none(),
            "Unit on Left Board should NOT target Enemy on Right Board"
        );
        assert!(
            world.entity(enemy).get::<Target>().is_none(),
            "Enemy on Right Board should NOT target Unit on Left Board"
        );
    }

    #[test]
    fn physical_simulation_cycle() {
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
            1,
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
    fn mage_switches_to_melee_when_out_of_mana() {
        use crate::model::components::{
            AttackProfile, AttackRange, AttackStats, CombatProfile, DamageType, Health, Mana,
        };
        use crate::model::unit_config::{MAGE_MANA_MAX, RANGED_ATTACK_RANGE};

        let mut world = World::new();

        let fireball_cost = FIREBALL_MANA_COST;
        let melee_damage = MAGE_MELEE_DAMAGE;
        let fireball_damage = 10.0;
        let ranged_range = RANGED_ATTACK_RANGE;
        let melee_range = DEFAULT_ATTACK_RANGE;

        // Mage (Attacker)
        let mage = world
            .spawn((
                InAttackRange,
                AttackStats {
                    damage: fireball_damage,
                    rate: 1.0,
                    damage_type: DamageType::FireMagical,
                },
                AttackRange(ranged_range),
                CombatProfile {
                    primary: AttackProfile {
                        damage: fireball_damage,
                        rate: 1.0,
                        range: ranged_range,
                        damage_type: DamageType::FireMagical,
                    },
                    secondary: Some(AttackProfile {
                        damage: melee_damage,
                        rate: 1.0,
                        range: melee_range,
                        damage_type: DamageType::PhysicalBasic,
                    }),
                    mana_cost: fireball_cost,
                },
                AttackTimer(0.0),
                Mana {
                    current: fireball_cost, // Enough for 1 fireball
                    max: MAGE_MANA_MAX,
                    regen: 0.0,
                },
            ))
            .id();

        // Target
        let target = world
            .spawn((Health {
                current: 100.0,
                max: 100.0,
            },))
            .id();

        world.entity_mut(mage).insert(Target(target));

        // 1. First attack: should be Fireball
        update_active_combat_stats(&mut world);
        process_combat(&mut world, 0.1);

        let target_health = world.entity(target).get::<Health>().unwrap().current;
        assert_eq!(
            target_health,
            100.0 - fireball_damage,
            "Should deal fireball damage"
        );

        let mana = world.entity(mage).get::<Mana>().unwrap().current;
        assert_eq!(mana, 0.0, "Should consume mana for fireball");

        // 2. Second attack: out of mana, should be weak melee
        // Reset timer manually for test
        world.entity_mut(mage).insert(AttackTimer(0.0));
        update_active_combat_stats(&mut world);
        process_combat(&mut world, 0.1);

        let target_health = world.entity(target).get::<Health>().unwrap().current;
        assert_eq!(
            target_health,
            100.0 - fireball_damage - melee_damage,
            "Should deal weak melee damage when out of mana"
        );

        let stats = world.entity(mage).get::<AttackStats>().unwrap();
        assert_eq!(
            stats.damage_type,
            DamageType::PhysicalBasic,
            "Should switch to PhysicalBasic when out of mana"
        );
        assert_eq!(
            world.entity(mage).get::<AttackRange>().unwrap().0,
            melee_range,
            "Should switch to melee range"
        );

        // 3. Third attack: mana regenerated enough for fireball
        world.entity_mut(mage).insert(Mana {
            current: FIREBALL_MANA_COST,
            max: MAGE_MANA_MAX,
            regen: 0.0,
        });
        world.entity_mut(mage).insert(AttackTimer(0.0));
        update_active_combat_stats(&mut world);
        process_combat(&mut world, 0.1);

        let target_health = world.entity(target).get::<Health>().unwrap().current;
        assert_eq!(
            target_health,
            100.0 - fireball_damage - melee_damage - fireball_damage,
            "Should deal fireball damage again after mana regen"
        );

        let stats = world.entity(mage).get::<AttackStats>().unwrap();
        assert_eq!(
            stats.damage_type,
            DamageType::FireMagical,
            "Should switch back to FireMagical when mana is sufficient"
        );
        assert_eq!(
            world.entity(mage).get::<AttackRange>().unwrap().0,
            ranged_range,
            "Should switch back to ranged range"
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
            1,
        );
        let enemy2 = crate::handler::spawn::spawn_enemy(
            &mut world,
            Position { x: 100.0, y: 0.0 },
            Shape::Circle,
            1,
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

    #[test]
    fn process_combat_returns_events() {
        use crate::model::components::{AttackStats, AttackTimer, DamageType, Health, Position};
        let mut world = World::new();

        // Attacker
        let attacker = world
            .spawn((
                Position { x: 0.0, y: 0.0 },
                InAttackRange,
                AttackStats {
                    damage: 10.0,
                    rate: 1.0,
                    damage_type: DamageType::PhysicalPierce,
                },
                AttackTimer(0.0),
            ))
            .id();

        // Target
        let target = world
            .spawn((
                Position { x: 10.0, y: 0.0 },
                Health {
                    current: 100.0,
                    max: 100.0,
                },
            ))
            .id();

        world.entity_mut(attacker).insert(Target(target));

        let events = process_combat(&mut world, 0.1);

        assert_eq!(events.len(), 1, "Should return 1 combat event");
        let event = &events[0];
        assert_eq!(event.attacker_id, attacker.index());
        assert_eq!(event.target_id, target.index());
        assert_eq!(event.attack_type, DamageType::PhysicalPierce);
        assert_eq!(event.start_pos, Position { x: 0.0, y: 0.0 });
        assert_eq!(event.end_pos, Position { x: 10.0, y: 0.0 });
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

        update_combat_movement(&mut world, 0.1);

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

        update_combat_movement(&mut world2, 0.1);

        let final_unit2_pos = world2.entity(unit2).get::<Position>().unwrap();
        assert_eq!(
            final_unit2_pos.x, 590.0,
            "Unit should not be pushed by overlapping worker"
        );
    }
}
