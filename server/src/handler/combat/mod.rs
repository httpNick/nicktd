mod attack;
mod movement;
mod targeting;

pub use attack::{cleanup_dead_entities, process_combat, update_active_combat_stats, update_mana};
pub use movement::{update_combat_movement, update_combat_reset};
pub use targeting::{update_attack_range_markers, update_leaked_creeps, update_targeting};

use crate::model::constants::{LEFT_BOARD_END, RIGHT_BOARD_END, RIGHT_BOARD_START};

pub const SPEED: f32 = 80.0; // pixels per second

/// Gold charged to the board owner per leaked creep (spec §3).
pub const LEAK_GOLD_PENALTY: u32 = 5;
/// Maximum leak gold charged per player per wave.
pub const LEAK_PENALTY_WAVE_CAP: u32 = 50;

pub(super) fn get_board(x: f32) -> Option<u8> {
    if x < LEFT_BOARD_END {
        Some(0)
    } else if x >= RIGHT_BOARD_START && x < RIGHT_BOARD_END {
        Some(1)
    } else {
        None
    }
}
