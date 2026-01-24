use bevy_ecs::prelude::{Component, Entity};

use super::shape::Shape;

#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Position {
    pub x: f32,
    pub y: f32,
}

#[derive(Component)]
pub struct Target(pub Entity);

#[derive(Component)]
pub struct ShapeComponent(pub Shape);

#[derive(Component)]
pub struct PlayerIdComponent(pub i64);

#[derive(Component)]
pub struct Enemy;

#[derive(Component)]
pub struct Worker;

#[derive(Component, Debug, Clone, Copy)]
pub struct TargetPositions {
    pub vein: Position,
    pub cart: Position,
}

#[derive(Component, Debug, PartialEq)]
pub enum WorkerState {
    MovingToVein,
    Mining,
    MovingToCart,
}

#[derive(Component)]
pub struct MiningTimer(pub f32);

#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct CollisionRadius(pub f32);

#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct AttackRange(pub f32);

#[derive(Component)]
pub struct InAttackRange;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn physical_components_exist() {
        let _ = CollisionRadius(5.0);
        let _ = AttackRange(20.0);
    }

    #[test]
    fn worker_components_exist() {
        let _ = Worker;
        let _ = WorkerState::MovingToVein;
        let _ = MiningTimer(10.0);
    }
}
