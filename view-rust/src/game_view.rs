use crate::app_state::AppState;
use crate::bevy_app::{ClientAction, BOARD_HEIGHT, LEFT_BOARD_END, RIGHT_BOARD_END,
                      RIGHT_BOARD_START, TOTAL_WIDTH};
use common::messages::{ClientMessage, PlaceMessage};
use common::Shape;
use leptos::prelude::*;

const SQUARE_SIZE: f32 = 60.0;

/// Convert a screen click on the Bevy canvas to server-space (x, y) coordinates.
///
/// Camera2d (orthographic, 1 world unit = 1 screen pixel) is centred at the
/// origin.  The mapping is:
///
///   world_x = px − canvas_w / 2
///   world_y = canvas_h / 2 − py   (screen Y is flipped in CSS/browser)
///
///   server_x = world_x + TOTAL_WIDTH  / 2   (= world_x + 700)
///   server_y = BOARD_HEIGHT / 2 − world_y   (= 300 − world_y)
///
/// Returns `None` if the click lands outside the combined board area.
fn click_to_server(px: f32, py: f32, canvas_w: f32, canvas_h: f32) -> Option<(f32, f32)> {
    if canvas_w == 0.0 || canvas_h == 0.0 {
        return None;
    }

    // World-space position (origin = canvas centre).
    let world_x = px - canvas_w * 0.5;
    let world_y = canvas_h * 0.5 - py; // screen Y is inverted

    // Server-space position (origin = top-left of board).
    let server_x = world_x + TOTAL_WIDTH * 0.5;
    let server_y = BOARD_HEIGHT * 0.5 - world_y;

    if server_x < 0.0 || server_x > RIGHT_BOARD_END || server_y < 0.0 || server_y > BOARD_HEIGHT {
        return None;
    }

    Some((server_x, server_y))
}

/// Determine the grid cell `(row, col)` for a click at `(server_x, server_y)`.
///
/// `is_player1` determines which board the player owns (left vs right).
/// Returns `None` if the click is outside the player's own board.
fn server_to_grid(server_x: f32, server_y: f32, is_player1: bool) -> Option<(u32, u32)> {
    let local_x = if is_player1 {
        if server_x > LEFT_BOARD_END {
            return None;
        }
        server_x
    } else {
        if !(RIGHT_BOARD_START..=RIGHT_BOARD_END).contains(&server_x) {
            return None;
        }
        server_x - RIGHT_BOARD_START
    };

    let row = (server_y / SQUARE_SIZE) as u32;
    let col = (local_x / SQUARE_SIZE) as u32;

    if row >= 10 || col >= 10 {
        return None;
    }

    Some((row, col))
}

/// The in-game view: an embedded Bevy canvas for 2D rendering plus a Leptos
/// HUD overlay for gold, wave, phase info, and unit placement.
#[component]
pub fn GameView() -> impl IntoView {
    let state = expect_context::<AppState>();
    let token_ctx = expect_context::<RwSignal<Option<String>>>();
    let canvas_ref = NodeRef::<leptos::html::Canvas>::new();
    let overlay_ref = NodeRef::<leptos::html::Div>::new();
    let selected_shape: RwSignal<Shape> = RwSignal::new(Shape::Circle);

    // Start the Bevy app once the canvas is in the DOM.
    Effect::new(move |_| {
        if canvas_ref.get().is_some() {
            #[cfg(target_arch = "wasm32")]
            crate::bevy_app::start_bevy_app("game-canvas");
        }
    });

    // Navigate back to lobby if the game state is cleared.
    let navigate_lobby = leptos_router::hooks::use_navigate();
    let state_nav = state.clone();
    Effect::new(move |_| {
        if state_nav.game_state.get().is_none() && token_ctx.get_untracked().is_some() {
            navigate_lobby("/lobby", Default::default());
        }
    });

    // Overlay click → Place a defender on the player's own board.
    let state_click = state.clone();
    let on_overlay_click = move |ev: leptos::ev::MouseEvent| {
        let shape = selected_shape.get_untracked();

        let Some(overlay) = overlay_ref.get_untracked() else {
            return;
        };
        let overlay_w = overlay.client_width() as f32;
        let overlay_h = overlay.client_height() as f32;

        // client_x/y is always relative to the viewport top-left, which is
        // consistent with Camera2d's coordinate origin (centered on viewport).
        let px = ev.client_x() as f32;
        let py = ev.client_y() as f32;

        let Some((server_x, server_y)) = click_to_server(px, py, overlay_w, overlay_h) else {
            return;
        };

        let my_id = state_click.player_id.get_untracked().unwrap_or(-1);
        let is_player1 = state_click
            .game_state
            .get_untracked()
            .and_then(|gs| gs.players.iter().position(|p| p.id == my_id).map(|i| i == 0))
            .unwrap_or(true);

        let Some((row, col)) = server_to_grid(server_x, server_y, is_player1) else {
            return;
        };

        crate::ws::send_ws_message(ClientMessage::Place(PlaceMessage { shape, row, col }));
    };

    view! {
        <div class="game-view">
            <canvas
                node_ref=canvas_ref
                id="game-canvas"
            />
            // Transparent overlay above the canvas to capture placement clicks.
            // z-index 5 keeps it below the HUD (z-index 10).
            <div
                node_ref=overlay_ref
                class="game-click-overlay"
                on:click=on_overlay_click
            />
            <GameHud state=state selected_shape=selected_shape />
        </div>
    }
}

// ── Game HUD ──────────────────────────────────────────────────────────────────

#[component]
fn GameHud(state: AppState, selected_shape: RwSignal<Shape>) -> impl IntoView {
    let dispatch = move |action: ClientAction| {
        let msg = match &action {
            ClientAction::SendUnit(shape) => ClientMessage::SendUnit { shape: shape.clone() },
            ClientAction::HireWorker => ClientMessage::HireWorker {},
            ClientAction::SkipToCombat => ClientMessage::SkipToCombat,
            ClientAction::SellUnit(id) => ClientMessage::SellById { entity_id: *id },
        };
        crate::ws::send_ws_message(msg);
        crate::bevy_app::push_client_action(action);
    };

    view! {
        <div class="game-hud">
            // ── Player info ────────────────────────────────────────────────
            {move || state.game_state.get().map(|gs| {
                let my_id = state.player_id.get_untracked().unwrap_or(-1);
                let player = gs.players.iter().find(|p| p.id == my_id).cloned();
                view! {
                    <div class="hud-status">
                        <span class="hud-phase">{format!("{:?}", gs.phase)}</span>
                        {player.map(|p| view! {
                            <span class="hud-gold">"Gold: " {p.gold}</span>
                            <span class="hud-income">"+" {p.income} "/wave"</span>
                        })}
                        <span class="hud-timer">{format!("{:.0}s", gs.phase_timer)}</span>
                    </div>
                }
            })}

            // ── Place defenders: shape selector + board click ──────────────
            <div class="hud-section">
                <span class="hud-label">"Place:"</span>
                <button
                    class=move || if selected_shape.get() == Shape::Circle { "shape-btn active" } else { "shape-btn" }
                    on:click=move |_| selected_shape.set(Shape::Circle)
                >"Circle"</button>
                <button
                    class=move || if selected_shape.get() == Shape::Square { "shape-btn active" } else { "shape-btn" }
                    on:click=move |_| selected_shape.set(Shape::Square)
                >"Square"</button>
                <button
                    class=move || if selected_shape.get() == Shape::Triangle { "shape-btn active" } else { "shape-btn" }
                    on:click=move |_| selected_shape.set(Shape::Triangle)
                >"Triangle"</button>
            </div>

            // ── Send attackers to opponent ─────────────────────────────────
            <div class="hud-section">
                <span class="hud-label">"Send:"</span>
                <button on:click={
                    let d = dispatch.clone();
                    move |_| d(ClientAction::SendUnit(Shape::Circle))
                }>"Circle"</button>
                <button on:click={
                    let d = dispatch.clone();
                    move |_| d(ClientAction::SendUnit(Shape::Square))
                }>"Square"</button>
                <button on:click={
                    let d = dispatch.clone();
                    move |_| d(ClientAction::SendUnit(Shape::Triangle))
                }>"Triangle"</button>
            </div>

            // ── Utility ────────────────────────────────────────────────────
            <div class="hud-section">
                <button on:click={
                    let d = dispatch.clone();
                    move |_| d(ClientAction::HireWorker)
                }>"Hire Worker"</button>
                <button on:click={
                    let d = dispatch.clone();
                    move |_| d(ClientAction::SkipToCombat)
                }>"Skip to Combat"</button>
            </div>
        </div>
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // Centre of canvas maps to board centre (700, 300).
    #[test]
    fn canvas_centre_maps_to_board_centre() {
        let w = 1400.0;
        let h = 600.0;
        let (sx, sy) = click_to_server(w / 2.0, h / 2.0, w, h).unwrap();
        assert!((sx - 700.0).abs() < 1.0, "sx={sx}");
        assert!((sy - 300.0).abs() < 1.0, "sy={sy}");
    }

    // Left quarter of canvas maps to player 1's board.
    #[test]
    fn left_quarter_maps_to_player1_board() {
        let w = 1400.0;
        let h = 600.0;
        // px=350 → world_x=350-700=-350 → server_x=-350+700=350 (left board)
        let (sx, _) = click_to_server(w * 0.25, h * 0.5, w, h).unwrap();
        assert!((sx - 350.0).abs() < 1.0, "sx={sx}");
        assert!(sx < LEFT_BOARD_END, "should be in left board, got sx={sx}");
    }

    // Right quarter of canvas maps to player 2's board.
    #[test]
    fn right_quarter_maps_to_player2_board() {
        let w = 1400.0;
        let h = 600.0;
        // px=1050 → world_x=1050-700=350 → server_x=350+700=1050 (right board)
        let (sx, _) = click_to_server(w * 0.75, h * 0.5, w, h).unwrap();
        assert!((sx - 1050.0).abs() < 1.0, "sx={sx}");
        assert!(sx >= RIGHT_BOARD_START, "should be in right board, got sx={sx}");
    }

    // Zero dimensions return None.
    #[test]
    fn canvas_zero_dimensions_returns_none() {
        assert!(click_to_server(100.0, 100.0, 0.0, 600.0).is_none());
        assert!(click_to_server(100.0, 100.0, 800.0, 0.0).is_none());
    }

    // server_to_grid: left board, centre of first cell → (0, 0).
    #[test]
    fn left_board_first_cell() {
        let (row, col) = server_to_grid(30.0, 30.0, true).unwrap();
        assert_eq!(row, 0);
        assert_eq!(col, 0);
    }

    // server_to_grid: right board click on player-2's board.
    #[test]
    fn right_board_first_cell() {
        let sx = RIGHT_BOARD_START + 30.0;
        let (row, col) = server_to_grid(sx, 30.0, false).unwrap();
        assert_eq!(row, 0);
        assert_eq!(col, 0);
    }

    // Clicking the opponent's board returns None.
    #[test]
    fn click_on_opponents_board_rejected() {
        assert!(server_to_grid(RIGHT_BOARD_START + 30.0, 30.0, true).is_none());
        assert!(server_to_grid(30.0, 30.0, false).is_none());
    }

    // Out-of-bounds row returns None.
    #[test]
    fn out_of_bounds_row_returns_none() {
        assert!(server_to_grid(30.0, 600.0, true).is_none());
    }
}
