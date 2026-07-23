use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnitKind {
    Circle,
    Square,
    Triangle,
}
