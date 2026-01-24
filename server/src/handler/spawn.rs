use bevy_ecs::prelude::{World, Entity};
use crate::model::components::{Position, ShapeComponent, Enemy, Worker, CollisionRadius, AttackRange, PlayerIdComponent, WorkerState, TargetPositions};
use crate::model::shape::Shape;
use crate::handler::combat::{DEFAULT_COLLISION_RADIUS, DEFAULT_ATTACK_RANGE};

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
}
