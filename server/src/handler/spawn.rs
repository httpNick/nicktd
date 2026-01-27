use crate::model::components::{
    AttackRange, AttackStats, AttackTimer, CollisionRadius, DefenseSpecialty, DefenseStats, Enemy,
    Health, HomePosition, PlayerIdComponent, Position, Resistances, ShapeComponent,
    TargetPositions, Worker, WorkerState,
};
use crate::model::shape::Shape;
use crate::model::unit_config::{DEFAULT_COLLISION_RADIUS, DEFAULT_HEALTH, get_unit_profile};
use bevy_ecs::prelude::{Entity, World};

pub fn spawn_enemy(world: &mut World, pos: Position, shape: Shape) -> Entity {
    let profile = get_unit_profile(shape);
    let mut entity = world.spawn((
        pos,
        HomePosition(pos),
        ShapeComponent(shape),
        Enemy,
        CollisionRadius(profile.radius),
        AttackRange(profile.combat.primary.range),
        Health {
            current: DEFAULT_HEALTH,
            max: DEFAULT_HEALTH,
        },
        AttackStats {
            damage: profile.combat.primary.damage,
            rate: profile.combat.primary.rate,
            damage_type: profile.combat.primary.damage_type,
        },
        profile.combat,
        DefenseStats {
            armor: 0.0,
            resistances: Resistances {
                fire: 0.0,
                ice: 0.0,
                lightning: 0.0,
            },
            specialty: DefenseSpecialty::None,
        },
        AttackTimer(0.0),
    ));

    if let Some(mana) = profile.mana {
        entity.insert(mana);
    }

    entity.id()
}

pub fn spawn_unit(world: &mut World, pos: Position, shape: Shape, player_id: i64) -> Entity {
    let profile = get_unit_profile(shape);
    let mut entity = world.spawn((
        pos,
        HomePosition(pos),
        ShapeComponent(shape),
        PlayerIdComponent(player_id),
        CollisionRadius(profile.radius),
        AttackRange(profile.combat.primary.range),
        Health {
            current: DEFAULT_HEALTH,
            max: DEFAULT_HEALTH,
        },
        AttackStats {
            damage: profile.combat.primary.damage,
            rate: profile.combat.primary.rate,
            damage_type: profile.combat.primary.damage_type,
        },
        profile.combat,
        DefenseStats {
            armor: 0.0,
            resistances: Resistances {
                fire: 0.0,
                ice: 0.0,
                lightning: 0.0,
            },
            specialty: DefenseSpecialty::None,
        },
        AttackTimer(0.0),
    ));

    if let Some(mana) = profile.mana {
        entity.insert(mana);
    }

    entity.id()
}

pub fn spawn_worker(world: &mut World, player_id: i64, targets: TargetPositions) -> Entity {
    world
        .spawn((
            targets.cart, // Start at cart
            ShapeComponent(Shape::Circle),
            PlayerIdComponent(player_id),
            Worker,
            WorkerState::MovingToVein,
            targets,
            CollisionRadius(DEFAULT_COLLISION_RADIUS),
            AttackRange(0.0), // Workers don't attack
            Health {
                current: DEFAULT_HEALTH,
                max: DEFAULT_HEALTH,
            },
            DefenseStats {
                armor: 0.0,
                resistances: Resistances {
                    fire: 0.0,
                    ice: 0.0,
                    lightning: 0.0,
                },
                specialty: DefenseSpecialty::None,
            },
        ))
        .id()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_helpers_apply_shape_radii() {
        let mut world = World::new();

        let square = spawn_unit(&mut world, Position { x: 0.0, y: 0.0 }, Shape::Square, 1);
        let circle = spawn_unit(&mut world, Position { x: 0.0, y: 0.0 }, Shape::Circle, 1);
        let triangle = spawn_unit(&mut world, Position { x: 0.0, y: 0.0 }, Shape::Triangle, 1);

        assert_eq!(
            world.entity(square).get::<CollisionRadius>().unwrap().0,
            DEFAULT_COLLISION_RADIUS + 2.0
        );
        assert_eq!(
            world.entity(circle).get::<CollisionRadius>().unwrap().0,
            DEFAULT_COLLISION_RADIUS
        );
        assert_eq!(
            world.entity(triangle).get::<CollisionRadius>().unwrap().0,
            DEFAULT_COLLISION_RADIUS - 2.0
        );
    }

    #[test]
    fn spawn_initializes_combat_components() {
        use crate::model::components::{AttackStats, AttackTimer, Health, HomePosition};
        let mut world = World::new();

        let unit_pos = Position { x: 10.0, y: 20.0 };
        let unit = spawn_unit(&mut world, unit_pos, Shape::Circle, 1);
        let enemy_pos = Position { x: 100.0, y: 200.0 };
        let enemy = spawn_enemy(&mut world, enemy_pos, Shape::Circle);

        for (entity, pos) in [(unit, unit_pos), (enemy, enemy_pos)] {
            let e = world.entity(entity);
            assert!(e.get::<Health>().is_some(), "Should have Health");
            assert!(e.get::<AttackStats>().is_some(), "Should have AttackStats");
            assert!(e.get::<AttackTimer>().is_some(), "Should have AttackTimer");
            assert!(
                e.get::<HomePosition>().is_some(),
                "Should have HomePosition"
            );

            let health = e.get::<Health>().unwrap();
            assert!(health.current > 0.0);
            assert_eq!(health.current, health.max);

            let stats = e.get::<AttackStats>().unwrap();
            assert!(stats.damage > 0.0);
            assert!(stats.rate > 0.0);

            let home = e.get::<HomePosition>().unwrap();
            assert_eq!(home.0.x, pos.x);
            assert_eq!(home.0.y, pos.y);
        }
    }

    #[test]
    fn spawn_applies_specialized_stats() {
        use crate::model::components::{AttackRange, AttackStats, DamageType, Mana};
        use crate::model::unit_config::DEFAULT_ATTACK_RANGE;
        let mut world = World::new();

        // Triangle: Ranged Physical Pierce
        let triangle = spawn_unit(&mut world, Position { x: 0.0, y: 0.0 }, Shape::Triangle, 1);
        let t_stats = world.entity(triangle).get::<AttackStats>().unwrap();
        let t_range = world.entity(triangle).get::<AttackRange>().unwrap();
        assert_eq!(t_stats.damage_type, DamageType::PhysicalPierce);
        assert!(
            t_range.0 > DEFAULT_ATTACK_RANGE,
            "Triangle should be ranged"
        );

        // Square: Melee Physical Basic
        let square = spawn_unit(&mut world, Position { x: 0.0, y: 0.0 }, Shape::Square, 1);
        let s_stats = world.entity(square).get::<AttackStats>().unwrap();
        let s_range = world.entity(square).get::<AttackRange>().unwrap();
        assert_eq!(s_stats.damage_type, DamageType::PhysicalBasic);
        assert!(s_range.0 <= DEFAULT_ATTACK_RANGE, "Square should be melee");

        // Circle: Fire Mage (Mana + Ranged Fire Magical)
        let circle = spawn_unit(&mut world, Position { x: 0.0, y: 0.0 }, Shape::Circle, 1);
        let c_stats = world.entity(circle).get::<AttackStats>().unwrap();
        let c_range = world.entity(circle).get::<AttackRange>().unwrap();
        let c_mana = world.entity(circle).get::<Mana>();

        assert_eq!(c_stats.damage_type, DamageType::FireMagical);
        assert!(
            c_range.0 > DEFAULT_ATTACK_RANGE,
            "Circle should be ranged (Mage)"
        );
        assert!(c_mana.is_some(), "Circle (Mage) should have mana");
    }
}
