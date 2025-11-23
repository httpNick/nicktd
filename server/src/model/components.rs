use bevy_ecs::prelude::{Component, Entity};
use uuid::Uuid;

use super::shape::Shape;

#[derive(Component)]
pub struct Position {
    pub x: f32,
    pub y: f32,
}

#[derive(Component)]
pub struct Target(pub Entity);

#[derive(Component)]
pub struct ShapeComponent(pub Shape);

#[derive(Component)]
pub struct PlayerIdComponent(pub Uuid);

#[derive(Component)]
pub struct Enemy;
