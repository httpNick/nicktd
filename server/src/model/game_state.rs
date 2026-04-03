use bevy_ecs::prelude::{Resource, World};
use serde::Serialize;
use tokio::sync::broadcast;

#[derive(Debug, Serialize, Clone, Copy, PartialEq, Eq, Hash, Resource)]
pub enum GamePhase {
    Build,
    Combat,
    Victory,
    GameOver,
}


/// Per-tick delta time inserted into the World at the start of each game tick.
#[derive(Debug, Resource)]
pub struct DeltaTime(pub f32);

/// ECS Resource holding the tokio broadcast sender so systems can push network messages.
#[derive(Resource)]
pub struct NetworkChannel(pub broadcast::Sender<String>);

#[derive(Debug)]
pub struct GameState {
    pub world: World,
    pub phase: GamePhase,
    pub phase_timer: f32,
    pub wave_number: u32,
}

impl GameState {
    pub fn new() -> Self {
        let mut world = World::new();
        world.insert_resource(GamePhase::Build);
        Self {
            world,
            phase: GamePhase::Build,
            phase_timer: 30.0,
            wave_number: 1,
        }
    }

    /// Resets the game state for a new game without replacing the `World`.
    /// Clears all entities while preserving resources (keeps the same `WorldId`
    /// so any existing `Schedule` remains valid).
    pub fn reset(&mut self) {
        self.world.clear_entities();
        self.phase = GamePhase::Build;
        self.phase_timer = 30.0;
        self.wave_number = 1;
        self.world.insert_resource(GamePhase::Build);
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

    #[test]
    fn test_game_phase_game_over_variant() {
        let phase = GamePhase::GameOver;
        match phase {
            GamePhase::GameOver => (),
            _ => panic!("Should have GameOver variant"),
        }
    }

    #[test]
    fn test_game_phase_resource_inserted_into_world() {
        let state = GameState::new();
        let phase = state.world.get_resource::<GamePhase>();
        assert!(
            phase.is_some(),
            "GamePhase resource must be present in world"
        );
        assert_eq!(*phase.unwrap(), GamePhase::Build);
    }


    #[test]
    fn test_delta_time_resource_can_be_inserted_and_read() {
        let mut world = World::new();
        world.insert_resource(DeltaTime(1.0 / 30.0));
        let dt = world.get_resource::<DeltaTime>();
        assert!(
            dt.is_some(),
            "DeltaTime resource must be readable after insertion"
        );
        assert!(
            (dt.unwrap().0 - 1.0 / 30.0).abs() < f32::EPSILON,
            "DeltaTime value must match the inserted value"
        );
    }

    #[test]
    fn test_network_channel_resource_can_be_inserted_and_read() {
        let mut world = World::new();
        let (tx, _rx) = broadcast::channel::<String>(16);
        world.insert_resource(NetworkChannel(tx));
        let nc = world.get_resource::<NetworkChannel>();
        assert!(
            nc.is_some(),
            "NetworkChannel resource must be readable after insertion"
        );
    }
}
