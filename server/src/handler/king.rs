use crate::model::components::{
    AttackRange, Dead, Enemy, Health, InAttackRange, King, PlayerIdComponent, Position, Target,
};
use crate::model::constants::TOTAL_HEIGHT;
use crate::model::king_config::KING_REGEN_PER_WAVE;
use bevy_ecs::prelude::{Entity, With, World};

fn get_board(x: f32) -> Option<u8> {
    use crate::model::constants::{LEFT_BOARD_END, RIGHT_BOARD_END, RIGHT_BOARD_START};
    if x < LEFT_BOARD_END {
        Some(0)
    } else if x >= RIGHT_BOARD_START && x < RIGHT_BOARD_END {
        Some(1)
    } else {
        None
    }
}

/// King targeting: kings target the nearest leaked enemy (pos.y >= TOTAL_HEIGHT)
/// on the same board. Kings do NOT target in-lane enemies.
pub fn update_king_targeting(world: &mut World) {
    // Collect leaked enemies (pos.y >= TOTAL_HEIGHT)
    let leaked_enemies: Vec<(Entity, Position)> = world
        .query_filtered::<(Entity, &Position), With<Enemy>>()
        .iter(world)
        .filter(|(_, pos)| pos.y >= TOTAL_HEIGHT)
        .map(|(e, pos)| (e, *pos))
        .collect();

    // Clear stale targets on kings (target entity was despawned or marked Dead)
    let stale_king_targets: Vec<Entity> = {
        let mut query =
            world.query_filtered::<(Entity, &Target), With<King>>();
        query
            .iter(world)
            .filter(|(_, target)| {
                !world.entities().contains(target.0)
                    || world.get::<Dead>(target.0).is_some()
            })
            .map(|(e, _)| e)
            .collect()
    };
    for king in stale_king_targets {
        world.entity_mut(king).remove::<Target>();
    }

    if leaked_enemies.is_empty() {
        return;
    }

    // Collect kings that have no target (including those just cleared above)
    let kings_without_target: Vec<(Entity, i64, Position)> = {
        let mut query =
            world.query_filtered::<(Entity, &PlayerIdComponent, &Position), With<King>>();
        query
            .iter(world)
            .filter(|(entity, _, _)| world.get::<Target>(*entity).is_none())
            .map(|(e, pid, pos)| (e, pid.0, *pos))
            .collect()
    };

    let mut commands: Vec<(Entity, Target)> = Vec::new();
    for (king_entity, _player_id, king_pos) in &kings_without_target {
        let king_board = get_board(king_pos.x);
        if king_board.is_none() {
            continue;
        }

        let mut closest: Option<(Entity, f32)> = None;
        for (enemy_entity, enemy_pos) in &leaked_enemies {
            if get_board(enemy_pos.x) != king_board {
                continue;
            }
            let dist_sq = (king_pos.x - enemy_pos.x).powi(2) + (king_pos.y - enemy_pos.y).powi(2);
            if closest.is_none() || dist_sq < closest.unwrap().1 {
                closest = Some((*enemy_entity, dist_sq));
            }
        }
        if let Some((target_entity, _)) = closest {
            commands.push((*king_entity, Target(target_entity)));
        }
    }

    for (entity, target) in commands {
        world.entity_mut(entity).insert(target);
    }
}

/// Manage InAttackRange for kings based on whether their target is within range.
/// Kings don't move, so this range check isn't done by update_combat_movement.
pub fn update_king_attack_range(world: &mut World) {
    // Collect (king_entity, king_pos, attack_range, target_entity)
    let kings: Vec<(Entity, Position, f32, Option<Entity>)> = {
        let mut q = world.query_filtered::<(Entity, &Position, &AttackRange, Option<&Target>), With<King>>();
        q.iter(world)
            .map(|(e, pos, range, target)| (e, *pos, range.0, target.map(|t| t.0)))
            .collect()
    };

    for (king_entity, king_pos, range, target_entity) in kings {
        let in_range = if let Some(te) = target_entity {
            if let Some(target_pos) = world.get::<Position>(te) {
                let dx = target_pos.x - king_pos.x;
                let dy = target_pos.y - king_pos.y;
                (dx * dx + dy * dy).sqrt() <= range
            } else {
                false
            }
        } else {
            false
        };

        if in_range {
            if world.get::<InAttackRange>(king_entity).is_none() {
                world.entity_mut(king_entity).insert(InAttackRange);
            }
        } else if world.get::<InAttackRange>(king_entity).is_some() {
            world.entity_mut(king_entity).remove::<InAttackRange>();
        }
    }
}

/// Apply per-wave HP regeneration to all King entities, clamped to max HP.
pub fn apply_king_regen(world: &mut World) {
    let kings: Vec<Entity> = world
        .query_filtered::<Entity, With<King>>()
        .iter(world)
        .collect();

    for entity in kings {
        if let Some(mut health) = world.get_mut::<Health>(entity) {
            health.current = (health.current + KING_REGEN_PER_WAVE).min(health.max);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handler::spawn::spawn_king;
    use crate::model::components::Health;
    use crate::model::king_config::{KING_BASE_HP, KING_REGEN_PER_WAVE};

    // --- Task 9.1 TDD: apply_king_regen ---

    #[test]
    fn apply_king_regen_clamps_to_max() {
        let mut world = World::new();
        let king = spawn_king(&mut world, 1, 0);

        // Set HP to nearly full (max - 1.0), so regen would exceed max
        {
            let mut health = world.get_mut::<Health>(king).unwrap();
            health.current = KING_BASE_HP - 1.0;
        }

        apply_king_regen(&mut world);

        let health = world.get::<Health>(king).unwrap();
        assert!(
            (health.current - KING_BASE_HP).abs() < f32::EPSILON,
            "Regen should clamp to max HP ({}), got {}",
            KING_BASE_HP,
            health.current
        );
    }

    #[test]
    fn apply_king_regen_increases_hp_when_below_max() {
        let mut world = World::new();
        let king = spawn_king(&mut world, 1, 0);

        // Set HP to 100 below max so there's room to regen
        {
            let mut health = world.get_mut::<Health>(king).unwrap();
            health.current = KING_BASE_HP - 100.0;
        }

        apply_king_regen(&mut world);

        let health = world.get::<Health>(king).unwrap();
        let expected = KING_BASE_HP - 100.0 + KING_REGEN_PER_WAVE;
        assert!(
            (health.current - expected).abs() < f32::EPSILON,
            "Regen should add {} HP",
            KING_REGEN_PER_WAVE
        );
    }

    // --- Task 9.3 Integration test: king targeting ---

    #[test]
    fn update_king_targeting_assigns_leaked_enemy_as_target() {
        use crate::model::components::{CollisionRadius, Enemy, ShapeComponent};
        use crate::model::shape::Shape;

        let mut world = World::new();
        let king = spawn_king(&mut world, 1, 0);

        // Spawn a leaked enemy on the left board above TOTAL_HEIGHT
        let leaked_enemy = world
            .spawn((
                Position {
                    x: 300.0,
                    y: TOTAL_HEIGHT + 10.0,
                },
                Enemy,
                CollisionRadius(10.0),
                ShapeComponent(Shape::Circle),
            ))
            .id();

        update_king_targeting(&mut world);

        let target = world.get::<Target>(king);
        assert!(target.is_some(), "King should have a target after update");
        assert_eq!(
            target.unwrap().0,
            leaked_enemy,
            "King should target the leaked enemy"
        );
    }

    #[test]
    fn update_king_targeting_ignores_in_lane_enemies() {
        use crate::model::components::{CollisionRadius, Enemy, ShapeComponent};
        use crate::model::shape::Shape;

        let mut world = World::new();
        let king = spawn_king(&mut world, 1, 0);

        // Spawn an in-lane enemy (below TOTAL_HEIGHT)
        world.spawn((
            Position {
                x: 300.0,
                y: TOTAL_HEIGHT - 10.0,
            },
            Enemy,
            CollisionRadius(10.0),
            ShapeComponent(Shape::Circle),
        ));

        update_king_targeting(&mut world);

        let target = world.get::<Target>(king);
        assert!(
            target.is_none(),
            "King must NOT target in-lane enemies (pos.y < TOTAL_HEIGHT)"
        );
    }
}
