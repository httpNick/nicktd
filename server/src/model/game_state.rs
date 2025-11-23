use bevy_ecs::prelude::World;
use serde::Serialize;

#[derive(Debug, Serialize, Clone, Copy)]
pub enum GamePhase {
    Build,
    Combat,
}

#[derive(Debug)]
pub struct GameState {
    pub world: World,
    pub phase: GamePhase,
    pub phase_timer: f32,
}

impl GameState {
    pub fn new() -> Self {
        Self {
            world: World::new(),
            phase: GamePhase::Build,
            phase_timer: 30.0,
        }
    }
}
