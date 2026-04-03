pub const BOARD_SIZE: f32 = 600.0;
pub const GAP_SIZE: f32 = 200.0;
pub const SQUARE_SIZE: f32 = 60.0;
pub const LEFT_BOARD_END: f32 = BOARD_SIZE;
pub const RIGHT_BOARD_START: f32 = BOARD_SIZE + GAP_SIZE;
pub const RIGHT_BOARD_END: f32 = RIGHT_BOARD_START + BOARD_SIZE;
pub const TOTAL_HEIGHT: f32 = BOARD_SIZE;

pub const KING_Y: f32 = TOTAL_HEIGHT + 60.0;
pub const KING_LEFT_X: f32 = BOARD_SIZE / 2.0;
pub const KING_RIGHT_X: f32 = RIGHT_BOARD_START + BOARD_SIZE / 2.0;
pub const KING_PLACEMENT_ROW_LIMIT: u32 = 8;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn king_zone_constants_are_correct() {
        assert_eq!(KING_Y, 660.0);
        assert_eq!(KING_LEFT_X, 300.0);
        assert_eq!(KING_RIGHT_X, 1100.0);
        assert_eq!(KING_PLACEMENT_ROW_LIMIT, 8);
    }
}
