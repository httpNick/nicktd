use bevy_ecs::prelude::World;
use serde::Serialize;

#[derive(Debug, Serialize, Clone, Copy, PartialEq)]
pub enum GamePhase {
    Build,
    Combat,
    Victory,
}

#[derive(Debug)]
pub struct GameState {
    pub world: World,
    pub phase: GamePhase,
    pub phase_timer: f32,
    pub wave_number: u32,
}

impl GameState {
    pub fn new() -> Self {
        Self {
            world: World::new(),
            phase: GamePhase::Build,
            phase_timer: 30.0,
            wave_number: 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_game_state_initialization() {
        let state = GameState::new();
        assert_eq!(state.wave_number, 1);
        match state.phase {
            GamePhase::Build => (),
            _ => panic!("Initial phase should be Build"),
        }
    }

    #[test]
    fn test_game_phase_victory_variant() {
        let phase = GamePhase::Victory;
        match phase {
            GamePhase::Victory => (),
            _ => panic!("Should have Victory variant"),
        }
    }
}
