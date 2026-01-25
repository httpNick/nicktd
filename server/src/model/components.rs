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

#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Health {
    pub current: f32,
    pub max: f32,
}

#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct AttackStats {
    pub damage: f32,
    pub rate: f32,
}

#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct AttackTimer(pub f32);

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

    #[test]
    fn health_component_works() {
        let health = Health {
            current: 50.0,
            max: 100.0,
        };
        assert_eq!(health.current, 50.0);
        assert_eq!(health.max, 100.0);
    }

    #[test]
    fn attack_stats_component_works() {
        let stats = AttackStats {
            damage: 10.0,
            rate: 1.5,
        };
        assert_eq!(stats.damage, 10.0);
        assert_eq!(stats.rate, 1.5);
    }

    #[test]
    fn attack_timer_component_works() {
        let timer = AttackTimer(0.5);
        assert_eq!(timer.0, 0.5);
    }
}
