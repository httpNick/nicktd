use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Resource)]
pub enum GamePhase {
    Build,
    Combat,
    Victory,
}
