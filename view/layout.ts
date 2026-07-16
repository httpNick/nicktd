export const BOARD_SIZE = 10;
export const SQUARE_SIZE = 60;
export const GAP_SIZE = 200;
export const LEFT_BOARD_END = 600;
export const RIGHT_BOARD_START = 800;

export const TOTAL_HEIGHT = 600;
export const KING_ZONE_HEIGHT = 120;
export const CANVAS_WIDTH = 1400;
export const CANVAS_HEIGHT = 720; // TOTAL_HEIGHT + KING_ZONE_HEIGHT

export const KING_RADIUS = 30;

export const MERC_BUILDING_X = LEFT_BOARD_END + GAP_SIZE / 2; // 700
export const MERC_BUILDING_Y = [150, 450] as const;
export const MERC_BUILDING_HALF = 18;

// Vein / cart / label rows per player index (0 = top half, 1 = bottom half)
export const VEIN_Y = [50, 350] as const;
export const CART_Y = [250, 550] as const;
export const LABEL_Y = [20, 320] as const;

// Rows 8-9 are the king protection zone (no placement, dimmed)
export const PROTECTED_ROW_START = 8;

// Hit-test half-extent for clicking a unit (matches old SQUARE_SIZE - 20 box)
export const UNIT_HIT_HALF = (SQUARE_SIZE - 20) / 2;
