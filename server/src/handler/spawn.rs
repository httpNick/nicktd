use bevy_ecs::prelude::{World, Entity};
use crate::model::components::{Position, ShapeComponent, Enemy, Worker, CollisionRadius, AttackRange, PlayerIdComponent, WorkerState, TargetPositions, Health, AttackStats, AttackTimer, DamageType, DefenseStats, DefenseSpecialty, Resistances};
use crate::model::shape::Shape;
use crate::handler::combat::{DEFAULT_COLLISION_RADIUS, DEFAULT_ATTACK_RANGE};

pub const DEFAULT_HEALTH: f32 = 100.0;
pub const DEFAULT_DAMAGE: f32 = 10.0;
pub const DEFAULT_ATTACK_RATE: f32 = 1.0; // Attacks per second

pub fn spawn_enemy(world: &mut World, pos: Position, shape: Shape) -> Entity {
    let radius = match shape {
        Shape::Square => DEFAULT_COLLISION_RADIUS + 2.0,
        Shape::Circle => DEFAULT_COLLISION_RADIUS,
        Shape::Triangle => DEFAULT_COLLISION_RADIUS - 2.0,
    };

    world.spawn((
        pos,
        ShapeComponent(shape),
        Enemy,
        CollisionRadius(radius),
        AttackRange(DEFAULT_ATTACK_RANGE),
        Health { current: DEFAULT_HEALTH, max: DEFAULT_HEALTH },
        AttackStats { damage: DEFAULT_DAMAGE, rate: DEFAULT_ATTACK_RATE, damage_type: DamageType::PhysicalBasic },
        DefenseStats { 
            armor: 0.0, 
            resistances: Resistances { fire: 0.0, ice: 0.0, lightning: 0.0 },
            specialty: DefenseSpecialty::None 
        },
        AttackTimer(0.0),
    )).id()
}

pub fn spawn_unit(world: &mut World, pos: Position, shape: Shape, player_id: i64) -> Entity {
    let radius = match shape {
        Shape::Square => DEFAULT_COLLISION_RADIUS + 2.0,
        Shape::Circle => DEFAULT_COLLISION_RADIUS,
        Shape::Triangle => DEFAULT_COLLISION_RADIUS - 2.0,
    };

    world.spawn((
        pos,
        ShapeComponent(shape),
        PlayerIdComponent(player_id),
        CollisionRadius(radius),
        AttackRange(DEFAULT_ATTACK_RANGE),
        Health { current: DEFAULT_HEALTH, max: DEFAULT_HEALTH },
        AttackStats { damage: DEFAULT_DAMAGE, rate: DEFAULT_ATTACK_RATE, damage_type: DamageType::PhysicalBasic },
        DefenseStats { 
            armor: 0.0, 
            resistances: Resistances { fire: 0.0, ice: 0.0, lightning: 0.0 },
            specialty: DefenseSpecialty::None 
        },
        AttackTimer(0.0),
    )).id()
}

pub fn spawn_worker(world: &mut World, player_id: i64, targets: TargetPositions) -> Entity {
    world.spawn((
        targets.cart, // Start at cart
        ShapeComponent(Shape::Circle),
        PlayerIdComponent(player_id),
        Worker,
        WorkerState::MovingToVein,
        targets,
        CollisionRadius(DEFAULT_COLLISION_RADIUS),
        AttackRange(0.0), // Workers don't attack
        Health { current: DEFAULT_HEALTH, max: DEFAULT_HEALTH },
        DefenseStats { 
            armor: 0.0, 
            resistances: Resistances { fire: 0.0, ice: 0.0, lightning: 0.0 },
            specialty: DefenseSpecialty::None 
        },
    )).id()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handler::combat::DEFAULT_COLLISION_RADIUS;

    #[test]
    fn spawn_helpers_apply_shape_radii() {
        let mut world = World::new();
        
        let square = spawn_unit(&mut world, Position { x: 0.0, y: 0.0 }, Shape::Square, 1);
        let circle = spawn_unit(&mut world, Position { x: 0.0, y: 0.0 }, Shape::Circle, 1);
        let triangle = spawn_unit(&mut world, Position { x: 0.0, y: 0.0 }, Shape::Triangle, 1);

        assert_eq!(world.entity(square).get::<CollisionRadius>().unwrap().0, DEFAULT_COLLISION_RADIUS + 2.0);
        assert_eq!(world.entity(circle).get::<CollisionRadius>().unwrap().0, DEFAULT_COLLISION_RADIUS);
        assert_eq!(world.entity(triangle).get::<CollisionRadius>().unwrap().0, DEFAULT_COLLISION_RADIUS - 2.0);
    }

    #[test]
    fn spawn_initializes_combat_components() {
        use crate::model::components::{Health, AttackStats, AttackTimer};
        let mut world = World::new();
        
        let unit = spawn_unit(&mut world, Position { x: 0.0, y: 0.0 }, Shape::Circle, 1);
        let enemy = spawn_enemy(&mut world, Position { x: 0.0, y: 0.0 }, Shape::Circle);

        for entity in [unit, enemy] {
            let e = world.entity(entity);
            assert!(e.get::<Health>().is_some(), "Should have Health");
            assert!(e.get::<AttackStats>().is_some(), "Should have AttackStats");
            assert!(e.get::<AttackTimer>().is_some(), "Should have AttackTimer");
            
            let health = e.get::<Health>().unwrap();
            assert!(health.current > 0.0);
            assert_eq!(health.current, health.max);

            let stats = e.get::<AttackStats>().unwrap();
            assert!(stats.damage > 0.0);
            assert!(stats.rate > 0.0);
        }
    }
}
