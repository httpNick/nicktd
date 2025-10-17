use serde::{Deserialize, Serialize};
use uuid::Uuid;
use super::shape::Shape;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PlacedShape {
    pub shape: Shape,
    pub row: usize,
    pub col: usize,
    pub owner_id: Uuid,
}
