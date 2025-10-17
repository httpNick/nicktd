use serde::{Deserialize, Serialize};
use super::placed_shape::PlacedShape;

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct GameState {
    pub shapes: Vec<PlacedShape>,
}
