use super::{LEAK_GOLD_PENALTY, LEAK_PENALTY_WAVE_CAP, get_board};
use crate::model::components::{
    AttackRange, Dead, Enemy, InAttackRange, King, Position, Target, Tower,
};
use crate::model::constants::TOTAL_HEIGHT;
use crate::model::player::Players;
use bevy_ecs::prelude::{Entity, With, Without, World};

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

            // Remove target if the target entity is dead (tagged Dead, not despawned).
            // Enemies that were targeting a Dead tower would otherwise keep the stale
            // target indefinitely, lose InAttackRange, and stop attacking.
            if world.get::<Dead>(target.0).is_some() {
                to_remove.push(entity);
                continue;
            }

            // Also check if target is on the same board (but only for alive targets)
            if let Some(target_pos) = world.get::<Position>(target.0) {
                if get_board(pos.x) != get_board(target_pos.x) {
                    to_remove.push(entity);
                    continue;
                }
            }

            // Remove target if the attacker is a leaked enemy (y >= TOTAL_HEIGHT)
            // but the target is an in-lane entity (y < TOTAL_HEIGHT). Leaked enemies
            // should only target entities in the king zone.
            if world.get::<Enemy>(entity).is_some() && pos.y >= TOTAL_HEIGHT {
                if let Some(target_pos) = world.get::<Position>(target.0) {
                    if target_pos.y < TOTAL_HEIGHT {
                        to_remove.push(entity);
                        continue;
                    }
                }
            }
        }
    }
    for entity in to_remove {
        world.entity_mut(entity).remove::<Target>();
    }

    let mut commands = Vec::new(); // (Entity, Target)

    // --- 2. UNIT TARGETING (Units target closest Enemy, in-lane only) ---
    // Leaked enemies (pos.y >= TOTAL_HEIGHT) are excluded from tower targeting.
    let enemy_positions: Vec<(Entity, Position)> = world
        .query_filtered::<(Entity, &Position), With<Enemy>>()
        .iter(world)
        .filter(|(_, pos)| pos.y < TOTAL_HEIGHT)
        .map(|(entity, pos)| (entity, Position { x: pos.x, y: pos.y }))
        .collect();

    if !enemy_positions.is_empty() {
        let mut query = world
            .query_filtered::<(Entity, &Position), (With<Tower>, Without<Target>, Without<Dead>)>();
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

    // --- ENEMY TARGETING (Enemies target closest non-Worker, non-King Unit) ---
    // Kings are excluded: in-lane enemies drift downward by default and are routed to the
    // king zone by update_leaked_creeps once they cross TOTAL_HEIGHT.
    let unit_positions: Vec<(Entity, Position)> = world
        .query_filtered::<(Entity, &Position), (With<Tower>, Without<Dead>)>()
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

/// Validates and updates InAttackRange markers for all entities with targets.
/// This runs independently of movement to ensure range status is always current,
/// even when entities are stationary or after other entities die/reposition.
pub fn update_attack_range_markers(world: &mut World) {
    // First, remove InAttackRange from any Dead entities
    let dead_entities_with_range: Vec<Entity> = world
        .query_filtered::<Entity, (With<Dead>, With<InAttackRange>)>()
        .iter(world)
        .collect();

    for entity in dead_entities_with_range {
        world.entity_mut(entity).remove::<InAttackRange>();
    }

    // Collect all entity positions for distance calculations (excluding Dead)
    let entity_positions: Vec<(Entity, Position)> = world
        .query_filtered::<(Entity, &Position), Without<Dead>>()
        .iter(world)
        .map(|(e, p)| (e, *p))
        .collect();

    let mut markers_to_add = Vec::new();
    let mut markers_to_remove = Vec::new();

    // Check all entities that have both a target and attack range (excluding Dead)
    let mut query = world.query_filtered::<(
        Entity,
        &Target,
        &AttackRange,
        &Position,
        Option<&InAttackRange>,
    ), Without<Dead>>();

    for (entity, target, attack_range, pos, in_range_opt) in query.iter(world) {
        // Find target's position
        if let Some((_, target_pos)) = entity_positions.iter().find(|(e, _)| *e == target.0) {
            let dx = target_pos.x - pos.x;
            let dy = target_pos.y - pos.y;
            let distance = (dx * dx + dy * dy).sqrt();

            let should_be_in_range = distance <= attack_range.0;
            let is_in_range = in_range_opt.is_some();

            if should_be_in_range && !is_in_range {
                markers_to_add.push(entity);
            } else if !should_be_in_range && is_in_range {
                markers_to_remove.push(entity);
            }
        } else if in_range_opt.is_some() {
            // Target doesn't exist (despawned) - remove marker
            markers_to_remove.push(entity);
        }
    }

    // Apply marker changes
    for entity in markers_to_add {
        world.entity_mut(entity).insert(InAttackRange);
    }
    for entity in markers_to_remove {
        world.entity_mut(entity).remove::<InAttackRange>();
    }
}

/// Detect leaked enemies (pos.y >= TOTAL_HEIGHT) and assign them the King entity
/// on their same board as a target. Does NOT despawn enemies or decrement lives.
pub fn update_leaked_creeps(world: &mut World) {
    // Collect leaked enemies without a target
    let leaked_enemies: Vec<(Entity, Position)> = world
        .query_filtered::<(Entity, &Position), With<Enemy>>()
        .iter(world)
        .filter(|(entity, pos)| pos.y >= TOTAL_HEIGHT && world.get::<Target>(*entity).is_none())
        .map(|(e, pos)| (e, *pos))
        .collect();

    if leaked_enemies.is_empty() {
        return;
    }

    // Collect king positions
    let kings: Vec<(Entity, Position)> = world
        .query_filtered::<(Entity, &Position), With<King>>()
        .iter(world)
        .map(|(e, pos)| (e, *pos))
        .collect();

    let mut commands: Vec<(Entity, Target)> = Vec::new();
    for (enemy_entity, enemy_pos) in &leaked_enemies {
        let enemy_board = get_board(enemy_pos.x);
        if enemy_board.is_none() {
            continue;
        }
        // Find the king on the same board
        if let Some((king_entity, _)) = kings
            .iter()
            .find(|(_, king_pos)| get_board(king_pos.x) == enemy_board)
        {
            commands.push((*enemy_entity, Target(*king_entity)));
        }
    }

    for (entity, target) in commands {
        // Assign the king as the leaked enemy's target.
        // Do NOT modify attack stats - enemies keep their original combat stats when leaked.
        world.entity_mut(entity).insert(target);
    }

    // Charge the board owner for each newly leaked creep (once per creep — the
    // Target-absence guard above makes this loop see a creep only once).
    let mut penalties: Vec<u8> = Vec::new(); // board index per leaked creep
    for (_, enemy_pos) in &leaked_enemies {
        if let Some(board) = get_board(enemy_pos.x) {
            penalties.push(board);
        }
    }
    if !penalties.is_empty() {
        if let Some(mut players) = world.get_resource_mut::<Players>() {
            for board in penalties {
                if let Some(player) = players.0.get_mut(board as usize) {
                    let charged_so_far = player.leaks_this_wave * LEAK_GOLD_PENALTY;
                    if charged_so_far < LEAK_PENALTY_WAVE_CAP {
                        player.gold = player.gold.saturating_sub(LEAK_GOLD_PENALTY);
                    }
                    player.leaks_this_wave += 1;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handler::combat::cleanup_dead_entities;
    use crate::handler::combat::update_combat_movement;
    use crate::handler::worker::{CART_POSITIONS, VEIN_POSITIONS};
    use crate::model::components::TargetPositions;
    use crate::model::components::{AttackStats, AttackTimer, CollisionRadius, DamageType, Health};
    use crate::model::game_state::DeltaTime;
    use crate::model::unit_kind::UnitKind;

    #[test]
    fn targeting_ignores_workers() {
        let mut world = World::new();
        // Spawn Enemy
        let _enemy = crate::handler::spawn::spawn_enemy(
            &mut world,
            Position { x: 100.0, y: 100.0 },
            UnitKind::Triangle,
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
            UnitKind::Square,
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
            UnitKind::Triangle,
            1,
        );

        // Spawn Normal Unit (farther than worker)
        let unit = crate::handler::spawn::spawn_unit(
            &mut world,
            Position { x: 200.0, y: 200.0 },
            UnitKind::Square,
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
    fn unit_can_attack_only_when_in_range() {
        let mut world = World::new();

        let range = 50.0;
        let unit = crate::handler::spawn::spawn_unit(
            &mut world,
            Position { x: 0.0, y: 0.0 },
            UnitKind::Square,
            1,
        );
        world.entity_mut(unit).insert(AttackRange(range));

        let enemy = crate::handler::spawn::spawn_enemy(
            &mut world,
            Position { x: 100.0, y: 0.0 },
            UnitKind::Triangle,
            1,
        );

        world.entity_mut(unit).insert(Target(enemy));

        let tick_delta = 1.0 / 30.0;
        world.insert_resource(DeltaTime(tick_delta));

        // 1. Far away: no marker
        update_combat_movement(&mut world);
        update_attack_range_markers(&mut world);
        assert!(
            world.entity(unit).get::<InAttackRange>().is_none(),
            "Should NOT have InAttackRange when far away"
        );

        // 2. Within range: has marker
        world.entity_mut(unit).insert(Position { x: 60.0, y: 0.0 });
        update_combat_movement(&mut world);
        update_attack_range_markers(&mut world);
        assert!(
            world.entity(unit).get::<InAttackRange>().is_some(),
            "Should have InAttackRange when within range"
        );

        // 3. Move back out: marker removed
        world.entity_mut(unit).insert(Position { x: 0.0, y: 0.0 });
        update_combat_movement(&mut world);
        update_attack_range_markers(&mut world);
        assert!(
            world.entity(unit).get::<InAttackRange>().is_none(),
            "Should remove InAttackRange when moving out of range"
        );
    }

    #[test]
    fn targeting_handles_removed_entities() {
        let mut world = World::new();

        let unit = crate::handler::spawn::spawn_unit(
            &mut world,
            Position { x: 0.0, y: 0.0 },
            UnitKind::Circle,
            1,
        );
        let enemy = crate::handler::spawn::spawn_enemy(
            &mut world,
            Position { x: 10.0, y: 10.0 },
            UnitKind::Circle,
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
    fn target_reacquisition_after_kill() {
        let mut world = World::new();

        // Spawn 1 unit and 2 enemies
        let unit = crate::handler::spawn::spawn_unit(
            &mut world,
            Position { x: 0.0, y: 0.0 },
            UnitKind::Circle,
            1,
        );
        let enemy1 = crate::handler::spawn::spawn_enemy(
            &mut world,
            Position { x: 10.0, y: 0.0 },
            UnitKind::Circle,
            1,
        );
        let enemy2 = crate::handler::spawn::spawn_enemy(
            &mut world,
            Position { x: 100.0, y: 0.0 },
            UnitKind::Circle,
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
    fn dead_tower_does_not_acquire_target() {
        let mut world = World::new();

        // Spawn a dead tower on the left board
        let dead_tower = crate::handler::spawn::spawn_unit(
            &mut world,
            Position { x: 100.0, y: 300.0 },
            UnitKind::Square,
            1,
        );
        world.entity_mut(dead_tower).insert(Dead);

        // Spawn an enemy on the same board
        let _enemy = crate::handler::spawn::spawn_enemy(
            &mut world,
            Position { x: 200.0, y: 300.0 },
            UnitKind::Triangle,
            1,
        );

        update_targeting(&mut world);

        assert!(
            world.entity(dead_tower).get::<Target>().is_none(),
            "Dead tower should not acquire a target"
        );
    }

    #[test]
    fn enemy_does_not_target_dead_tower() {
        let mut world = World::new();

        // Spawn a dead tower close to the enemy position
        let dead_tower = crate::handler::spawn::spawn_unit(
            &mut world,
            Position { x: 110.0, y: 300.0 },
            UnitKind::Square,
            1,
        );
        world.entity_mut(dead_tower).insert(Dead);

        // Spawn a living tower farther from the enemy
        let living_tower = crate::handler::spawn::spawn_unit(
            &mut world,
            Position { x: 300.0, y: 300.0 },
            UnitKind::Square,
            1,
        );

        // Enemy closer to the dead tower than the living tower
        let enemy = crate::handler::spawn::spawn_enemy(
            &mut world,
            Position { x: 100.0, y: 300.0 },
            UnitKind::Triangle,
            1,
        );

        update_targeting(&mut world);

        let target = world.entity(enemy).get::<Target>();
        assert!(target.is_some(), "Enemy should have a target");
        assert_eq!(
            target.unwrap().0,
            living_tower,
            "Enemy should target the living tower, not the dead one"
        );
    }

    #[test]
    fn tower_targeting_excludes_leaked_enemies() {
        use crate::handler::spawn::spawn_king;

        let mut world = World::new();
        let _king = spawn_king(&mut world, 1, 0);

        // Spawn a tower on left board
        let tower = crate::handler::spawn::spawn_unit(
            &mut world,
            Position { x: 100.0, y: 300.0 },
            UnitKind::Square,
            1,
        );

        // Spawn a leaked enemy (y >= TOTAL_HEIGHT) on left board — towers should ignore it
        world.spawn((
            Position {
                x: 150.0,
                y: TOTAL_HEIGHT + 5.0,
            },
            Enemy,
            CollisionRadius(10.0),
        ));

        update_targeting(&mut world);

        // Tower should have no target because the only enemy is leaked
        let target = world.entity(tower).get::<Target>();
        assert!(
            target.is_none(),
            "Tower must NOT target leaked enemies (pos.y >= TOTAL_HEIGHT)"
        );
    }

    #[test]
    fn attack_range_markers_updated_when_entities_reposition() {
        use crate::model::components::{King, Target};

        let mut world = World::new();
        world.insert_resource(DeltaTime(0.016));

        // Create a king at position (100, 900)
        let king = world
            .spawn((
                Position { x: 100.0, y: 900.0 },
                CollisionRadius(30.0),
                Health {
                    current: 500.0,
                    max: 500.0,
                },
                King,
                AttackStats {
                    damage: 50.0,
                    rate: 1.0,
                    damage_type: DamageType::PHYSICAL_BASIC,
                },
                AttackRange(150.0),
                AttackTimer(0.0),
            ))
            .id();

        // Create two squares: one in range (80 pixels away), one out of range (200 pixels away)
        let square_in_range = crate::handler::spawn::spawn_enemy(
            &mut world,
            Position { x: 100.0, y: 820.0 }, // 80 pixels away
            UnitKind::Square,
            1,
        );

        let square_out_of_range = crate::handler::spawn::spawn_enemy(
            &mut world,
            Position { x: 100.0, y: 700.0 }, // 200 pixels away
            UnitKind::Square,
            1,
        );

        // Assign both squares to target the king
        world.entity_mut(square_in_range).insert(Target(king));
        world.entity_mut(square_out_of_range).insert(Target(king));

        // Both squares have 45px attack range (default for squares)
        world.entity_mut(square_in_range).insert(AttackRange(45.0));
        world
            .entity_mut(square_out_of_range)
            .insert(AttackRange(45.0));

        // Run the range marker system
        update_attack_range_markers(&mut world);

        // The close square should NOT have InAttackRange (80px > 45px range)
        assert!(
            world
                .entity(square_in_range)
                .get::<InAttackRange>()
                .is_none(),
            "Square 80px away should not have InAttackRange with 45px range"
        );

        // The far square should NOT have InAttackRange either
        assert!(
            world
                .entity(square_out_of_range)
                .get::<InAttackRange>()
                .is_none(),
            "Square 200px away should not have InAttackRange"
        );

        // Now move the first square much closer (within attack range)
        world
            .entity_mut(square_in_range)
            .get_mut::<Position>()
            .unwrap()
            .y = 880.0; // 20 pixels away

        // Re-run the range marker system
        update_attack_range_markers(&mut world);

        // Now the close square SHOULD have InAttackRange (20px < 45px range)
        assert!(
            world
                .entity(square_in_range)
                .get::<InAttackRange>()
                .is_some(),
            "Square 20px away should have InAttackRange with 45px range"
        );

        // The far square still should not
        assert!(
            world
                .entity(square_out_of_range)
                .get::<InAttackRange>()
                .is_none(),
            "Square 200px away should still not have InAttackRange"
        );

        // Simulate a scenario: kill the first square (add Dead marker)
        world.entity_mut(square_in_range).insert(Dead);

        // Move the second square closer (simulating repositioning after first dies)
        world
            .entity_mut(square_out_of_range)
            .get_mut::<Position>()
            .unwrap()
            .y = 870.0; // 30 pixels away

        // Re-run the range marker system
        update_attack_range_markers(&mut world);

        // Dead entity should lose InAttackRange (because query excludes Dead entities)
        assert!(
            world
                .entity(square_in_range)
                .get::<InAttackRange>()
                .is_none(),
            "Dead square should not have InAttackRange"
        );

        // The repositioned square should now have InAttackRange (30px < 45px)
        assert!(
            world
                .entity(square_out_of_range)
                .get::<InAttackRange>()
                .is_some(),
            "Square repositioned to 30px away should have InAttackRange with 45px range"
        );
    }

    #[test]
    fn update_leaked_creeps_assigns_king_target() {
        use crate::handler::spawn::spawn_king;

        let mut world = World::new();
        let king = spawn_king(&mut world, 1, 0);

        // Spawn a leaked enemy (y >= TOTAL_HEIGHT) on left board
        let leaked_enemy = world
            .spawn((
                Position {
                    x: 300.0,
                    y: TOTAL_HEIGHT + 1.0,
                },
                Enemy,
                CollisionRadius(10.0),
            ))
            .id();

        update_leaked_creeps(&mut world);

        let target = world.get::<Target>(leaked_enemy);
        assert!(
            target.is_some(),
            "Leaked enemy should have a Target assigned"
        );
        assert_eq!(
            target.unwrap().0,
            king,
            "Leaked enemy should target the king"
        );
    }

    #[test]
    fn update_leaked_creeps_does_not_despawn() {
        use crate::handler::spawn::spawn_king;

        let mut world = World::new();
        let _king = spawn_king(&mut world, 1, 0);

        // Spawn a leaked enemy
        let leaked_enemy = world
            .spawn((
                Position {
                    x: 300.0,
                    y: TOTAL_HEIGHT + 1.0,
                },
                Enemy,
                CollisionRadius(10.0),
            ))
            .id();

        update_leaked_creeps(&mut world);

        assert!(
            world.entities().contains(leaked_enemy),
            "Leaked enemy must NOT be despawned by update_leaked_creeps"
        );
    }

    #[test]
    fn leak_charges_board_owner_five_gold() {
        use crate::handler::spawn::{spawn_enemy, spawn_king};
        use crate::model::player::{Player, Players};
        let mut world = World::new();
        world.insert_resource(Players(vec![
            Player::new(1, "left".into(), 100),
            Player::new(2, "right".into(), 100),
        ]));
        spawn_king(&mut world, 1, 0);
        // Leak on the LEFT board (x=100) → players[0] pays.
        spawn_enemy(
            &mut world,
            Position {
                x: 100.0,
                y: TOTAL_HEIGHT + 10.0,
            },
            UnitKind::Triangle,
            1,
        );
        update_leaked_creeps(&mut world);
        let players = world.resource::<Players>();
        assert_eq!(players.0[0].gold, 95);
        assert_eq!(players.0[0].leaks_this_wave, 1);
        assert_eq!(players.0[1].gold, 100, "opponent unaffected");
    }

    #[test]
    fn leak_penalty_caps_at_fifty_per_wave_and_floors_at_zero() {
        use crate::handler::spawn::{spawn_enemy, spawn_king};
        use crate::model::player::{Player, Players};
        let mut world = World::new();
        let mut poor = Player::new(1, "poor".into(), 12);
        poor.leaks_this_wave = 9; // 45g already charged this wave
        world.insert_resource(Players(vec![poor]));
        spawn_king(&mut world, 1, 0);
        // Two more leaks: 10th charges min(5, cap-45)=5 → gold 12→7;
        // 11th exceeds the 50g cap → no charge.
        for x in [100.0, 140.0] {
            spawn_enemy(
                &mut world,
                Position {
                    x,
                    y: TOTAL_HEIGHT + 10.0,
                },
                UnitKind::Square,
                1,
            );
        }
        update_leaked_creeps(&mut world);
        let players = world.resource::<Players>();
        assert_eq!(players.0[0].gold, 7);
        assert_eq!(players.0[0].leaks_this_wave, 11);
    }

    #[test]
    fn leak_penalty_never_underflows_gold() {
        use crate::handler::spawn::{spawn_enemy, spawn_king};
        use crate::model::player::{Player, Players};
        let mut world = World::new();
        world.insert_resource(Players(vec![Player::new(1, "broke".into(), 3)]));
        spawn_king(&mut world, 1, 0);
        spawn_enemy(
            &mut world,
            Position {
                x: 100.0,
                y: TOTAL_HEIGHT + 10.0,
            },
            UnitKind::Square,
            1,
        );
        update_leaked_creeps(&mut world);
        assert_eq!(world.resource::<Players>().0[0].gold, 0);
    }
}
