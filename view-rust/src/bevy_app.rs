/// Leptos ↔ Bevy bridge via thread-local buffers.
///
/// # Why thread-local?
///
/// Both Leptos (JS callbacks) and Bevy (requestAnimationFrame) execute on the
/// same single WASM thread.  A `thread_local!` avoids `Arc<Mutex<>>` overhead
/// and keeps bridge types `!Send`-friendly while remaining safe.
///
/// # Rendering approach: 2D (Mesh2d + ColorMaterial)
///
/// `StandardMaterial` (3-D PBR) fails silently in Bevy 0.17 + WebGL2 and
/// falls back to the hot-pink error material.  `ColorMaterial` (2-D) is far
/// simpler, compiles correctly in WebGL2, and is appropriate for this
/// top-down game which has no meaningful 3-D perspective.
use common::messages::{CombatEvent, SerializableGameState};
use common::Shape;

// ── Board / world coordinate constants ───────────────────────────────────────

/// Pixel width of each board (left or right).
pub const BOARD_SIZE: f32 = 600.0;
/// Pixel width of the gap between the two boards.
pub const GAP_SIZE: f32 = 200.0;
/// Total pixel width of the combined play field.
pub const TOTAL_WIDTH: f32 = 1400.0;
/// Total pixel height of the play field.
pub const BOARD_HEIGHT: f32 = 600.0;

/// Server-x at which the left board ends.
pub const LEFT_BOARD_END: f32 = BOARD_SIZE; // 600
/// Server-x at which the right board begins.
pub const RIGHT_BOARD_START: f32 = BOARD_SIZE + GAP_SIZE; // 800
/// Server-x at which the right board ends (= total width).
pub const RIGHT_BOARD_END: f32 = TOTAL_WIDTH; // 1400

/// Convert server pixel coordinates `(sx, sy)` to 2-D world-space `(x, y)`.
///
/// Camera2d (orthographic, 1 world unit = 1 screen pixel) is centred at the
/// origin.  The board occupies world X ∈ [–700, 700] and Y ∈ [–300, 300].
///
/// Mapping:
/// - `(0,    0  )` → `(–700,  300)` — top-left of board
/// - `(700,  300)` → `(   0,    0)` — board centre
/// - `(1400, 600)` → `( 700, –300)` — bottom-right of board
pub fn server_to_world(sx: f32, sy: f32) -> (f32, f32) {
    let x = sx - TOTAL_WIDTH * 0.5;    // server_x − 700
    let y = BOARD_HEIGHT * 0.5 - sy;   // 300 − server_y  (Y is flipped)
    (x, y)
}

// ── Client-action command type ────────────────────────────────────────────────

/// Commands issued by the Leptos UI that Bevy systems consume each frame.
/// Each action is also transmitted to the server via WebSocket.
#[derive(Clone, Debug, PartialEq)]
pub enum ClientAction {
    SendUnit(Shape),
    HireWorker,
    SkipToCombat,
    SellUnit(u32),
}

// ── Thread-local buffers ──────────────────────────────────────────────────────

thread_local! {
    /// Latest game-state from the WebSocket (overwritten, not queued).
    static GAME_STATE_BUFFER: std::cell::RefCell<Option<SerializableGameState>> =
        std::cell::RefCell::new(None);

    /// Queued player actions from the Leptos UI layer.
    static CLIENT_ACTION_BUFFER: std::cell::RefCell<Vec<ClientAction>> =
        std::cell::RefCell::new(Vec::new());

    /// Queued combat events from the WebSocket for Bevy to consume.
    static COMBAT_EVENT_BUFFER: std::cell::RefCell<Vec<CombatEvent>> =
        std::cell::RefCell::new(Vec::new());
}

// ── Leptos → bridge API ───────────────────────────────────────────────────────

/// Push the latest server game-state snapshot into the bridge.
pub fn push_game_state(state: SerializableGameState) {
    GAME_STATE_BUFFER.with(|c| *c.borrow_mut() = Some(state));
}

/// Pop the pending game-state snapshot (returns `None` if nothing new).
pub fn pop_game_state() -> Option<SerializableGameState> {
    GAME_STATE_BUFFER.with(|c| c.borrow_mut().take())
}

/// Enqueue a player action into the bridge for Bevy to consume.
pub fn push_client_action(action: ClientAction) {
    CLIENT_ACTION_BUFFER.with(|c| c.borrow_mut().push(action));
}

/// Drain all queued player actions (clears the buffer).
pub fn drain_client_actions() -> Vec<ClientAction> {
    CLIENT_ACTION_BUFFER.with(|c| c.borrow_mut().drain(..).collect())
}

/// Push a batch of combat events into the bridge for Bevy to consume.
pub fn push_combat_events(events: Vec<CombatEvent>) {
    COMBAT_EVENT_BUFFER.with(|c| c.borrow_mut().extend(events));
}

/// Drain all queued combat events (clears the buffer).
pub fn drain_combat_events() -> Vec<CombatEvent> {
    COMBAT_EVENT_BUFFER.with(|c| c.borrow_mut().drain(..).collect())
}

// ── Bevy app scaffold ─────────────────────────────────────────────────────────

#[cfg(target_arch = "wasm32")]
thread_local! {
    static BEVY_STARTED: std::cell::Cell<bool> = std::cell::Cell::new(false);
}

/// Initialise the Bevy app on the given canvas element ID.
/// Safe to call multiple times — the app is started only once.
#[cfg(target_arch = "wasm32")]
pub fn start_bevy_app(canvas_id: &'static str) {
    BEVY_STARTED.with(|started| {
        if !started.get() {
            started.set(true);
            leptos::task::spawn_local(async move {
                build_app(canvas_id).run();
            });
        }
    });
}

#[cfg(target_arch = "wasm32")]
fn build_app(canvas_id: &str) -> bevy::app::App {
    use bevy::prelude::*;

    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                canvas: Some(format!("#{canvas_id}")),
                fit_canvas_to_parent: true,
                ..default()
            }),
            ..default()
        }),
    )
    .init_resource::<GameStateBuffer>()
    .init_resource::<ClientActionBuffer>()
    .init_resource::<CombatEventBufferRes>()
    .init_resource::<UnitEntityMap>()
    .add_systems(Startup, setup_scene)
    .add_systems(
        Update,
        (
            (sync_game_state, sync_client_actions, sync_combat_events),
            (reconcile_units, process_combat_events),
            (interpolate_units, update_projectiles),
        )
            .chain(),
    );
    app
}

// ── Bevy resources ────────────────────────────────────────────────────────────

#[cfg(target_arch = "wasm32")]
use bevy::prelude::*;

/// Holds the latest game-state snapshot consumed by Bevy systems.
#[cfg(target_arch = "wasm32")]
#[derive(Resource, Default)]
pub struct GameStateBuffer {
    pub latest: Option<SerializableGameState>,
}

/// Holds player actions queued from the Leptos UI layer.
#[cfg(target_arch = "wasm32")]
#[derive(Resource, Default)]
pub struct ClientActionBuffer {
    pub actions: Vec<ClientAction>,
}

/// Holds combat events drained from the thread-local buffer each frame.
#[cfg(target_arch = "wasm32")]
#[derive(Resource, Default)]
pub struct CombatEventBufferRes {
    pub events: Vec<CombatEvent>,
}

/// Maps server-assigned unit IDs to their Bevy `Entity` handles.
#[cfg(target_arch = "wasm32")]
#[derive(Resource, Default)]
pub struct UnitEntityMap(pub std::collections::HashMap<u32, Entity>);

/// Pre-built mesh and material handles shared across all unit entities.
/// Uses `ColorMaterial` (2-D) which works correctly in WebGL2.
#[cfg(target_arch = "wasm32")]
#[derive(Resource)]
pub struct UnitAssets {
    pub circle_mesh: Handle<Mesh>,
    pub square_mesh: Handle<Mesh>,
    pub triangle_mesh: Handle<Mesh>,
    pub projectile_mesh: Handle<Mesh>,

    pub circle_ally_mat: Handle<ColorMaterial>,
    pub square_ally_mat: Handle<ColorMaterial>,
    pub triangle_ally_mat: Handle<ColorMaterial>,
    pub circle_enemy_mat: Handle<ColorMaterial>,
    pub square_enemy_mat: Handle<ColorMaterial>,
    pub triangle_enemy_mat: Handle<ColorMaterial>,
    pub projectile_mat: Handle<ColorMaterial>,
}

// ── Bevy components ───────────────────────────────────────────────────────────

/// Stores the server-assigned unit ID on the corresponding Bevy entity.
#[cfg(target_arch = "wasm32")]
#[derive(Component)]
pub struct ServerId(pub u32);

/// Smooth-movement target position set each frame from the game-state snapshot.
#[cfg(target_arch = "wasm32")]
#[derive(Component)]
pub struct TargetPosition(pub Vec3);

/// Marks a projectile entity; drives it toward `end` at `speed` world units/s.
#[cfg(target_arch = "wasm32")]
#[derive(Component)]
pub struct Projectile {
    pub end: Vec3,
    pub speed: f32,
}

// ── Bridge systems ────────────────────────────────────────────────────────────

/// ECS system: drain the thread-local game-state buffer into the Bevy resource.
#[cfg(target_arch = "wasm32")]
fn sync_game_state(mut buffer: ResMut<GameStateBuffer>) {
    if let Some(state) = pop_game_state() {
        buffer.latest = Some(state);
    }
}

/// ECS system: drain the thread-local action buffer into the Bevy resource.
#[cfg(target_arch = "wasm32")]
fn sync_client_actions(mut buffer: ResMut<ClientActionBuffer>) {
    buffer.actions = drain_client_actions();
}

/// ECS system: drain the thread-local combat-event buffer into the Bevy resource.
#[cfg(target_arch = "wasm32")]
fn sync_combat_events(mut buf: ResMut<CombatEventBufferRes>) {
    buf.events.extend(drain_combat_events());
}

// ── Scene setup ───────────────────────────────────────────────────────────────

/// Startup system: spawn camera, board backgrounds, and build the `UnitAssets` resource.
///
/// Uses 2-D rendering (Camera2d + Mesh2d + ColorMaterial) because
/// StandardMaterial falls back to hot-pink in Bevy 0.17 + WebGL2.
#[cfg(target_arch = "wasm32")]
fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    // Dark background so units are always visible.
    commands.insert_resource(ClearColor(Color::srgb(0.04, 0.04, 0.06)));

    // 2-D orthographic camera — 1 world unit = 1 screen pixel.
    // Board (1400 × 600 world units) is centred at the origin.
    commands.spawn(Camera2d);

    // ── Board background panels ──────────────────────────────────────────────
    // Left board: server_x 0–600 → world_x –700 to –100, centre (–400, 0)
    commands.spawn((
        Sprite {
            color: Color::srgb(0.08, 0.10, 0.15),
            custom_size: Some(Vec2::new(BOARD_SIZE, BOARD_HEIGHT)),
            ..default()
        },
        Transform::from_xyz(-(GAP_SIZE * 0.5 + BOARD_SIZE * 0.5), 0.0, -1.0),
    ));
    // Gap: server_x 600–800 → world_x –100 to 100, centre (0, 0)
    commands.spawn((
        Sprite {
            color: Color::srgb(0.04, 0.04, 0.05),
            custom_size: Some(Vec2::new(GAP_SIZE, BOARD_HEIGHT)),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, -1.0),
    ));
    // Right board: server_x 800–1400 → world_x 100 to 700, centre (400, 0)
    commands.spawn((
        Sprite {
            color: Color::srgb(0.15, 0.10, 0.08),
            custom_size: Some(Vec2::new(BOARD_SIZE, BOARD_HEIGHT)),
            ..default()
        },
        Transform::from_xyz(GAP_SIZE * 0.5 + BOARD_SIZE * 0.5, 0.0, -1.0),
    ));

    // ── Unit meshes (2-D) ────────────────────────────────────────────────────
    // Sizes are in world units (= screen pixels at 1:1 scale).
    let unit_r = 22.0_f32; // circle radius / triangle circumradius
    let unit_s = 44.0_f32; // square side length

    let circle_mesh = meshes.add(Circle::new(unit_r));
    let square_mesh = meshes.add(Rectangle::new(unit_s, unit_s));
    let triangle_mesh = meshes.add(Triangle2d::new(
        Vec2::new(0.0, unit_r * 1.2),
        Vec2::new(-unit_r, -unit_r * 0.7),
        Vec2::new(unit_r, -unit_r * 0.7),
    ));
    let projectile_mesh = meshes.add(Circle::new(5.0));

    // ── Unit materials (ColorMaterial — reliable in WebGL2) ──────────────────
    let mut mat = |color: Color| -> Handle<ColorMaterial> {
        materials.add(ColorMaterial::from(color))
    };

    // Ally colours (blue / green / gold family).
    let circle_ally_mat   = mat(Color::srgb(0.20, 0.50, 1.00));
    let square_ally_mat   = mat(Color::srgb(0.20, 0.90, 0.40));
    let triangle_ally_mat = mat(Color::srgb(1.00, 0.85, 0.20));

    // Enemy colours (red / orange / purple family).
    let circle_enemy_mat   = mat(Color::srgb(1.00, 0.20, 0.20));
    let square_enemy_mat   = mat(Color::srgb(1.00, 0.55, 0.10));
    let triangle_enemy_mat = mat(Color::srgb(0.70, 0.20, 0.90));

    // Projectile (bright white-yellow).
    let projectile_mat = mat(Color::srgb(1.00, 1.00, 0.80));

    commands.insert_resource(UnitAssets {
        circle_mesh,
        square_mesh,
        triangle_mesh,
        projectile_mesh,
        circle_ally_mat,
        square_ally_mat,
        triangle_ally_mat,
        circle_enemy_mat,
        square_enemy_mat,
        triangle_enemy_mat,
        projectile_mat,
    });
}

// ── Unit reconciliation ───────────────────────────────────────────────────────

/// ECS system: reconcile Bevy entities with the latest server game-state snapshot.
#[cfg(target_arch = "wasm32")]
fn reconcile_units(
    mut commands: Commands,
    game_state_buf: Res<GameStateBuffer>,
    unit_assets: Option<Res<UnitAssets>>,
    mut unit_map: ResMut<UnitEntityMap>,
) {
    let (Some(state), Some(assets)) = (&game_state_buf.latest, unit_assets) else {
        return;
    };

    use std::collections::HashSet;

    let snapshot_ids: HashSet<u32> = state.units.iter().map(|u| u.id).collect();

    unit_map.0.retain(|id, entity| {
        if snapshot_ids.contains(id) {
            true
        } else {
            commands.entity(*entity).despawn();
            false
        }
    });

    for unit in &state.units {
        let (wx, wy) = server_to_world(unit.x, unit.y);
        let target = Vec3::new(wx, wy, 0.0);

        if let Some(&entity) = unit_map.0.get(&unit.id) {
            commands.entity(entity).insert(TargetPosition(target));
        } else {
            let (mesh, mat) = unit_mesh_and_material(unit.shape, unit.is_enemy, &assets);
            let entity = commands
                .spawn((
                    Mesh2d(mesh),
                    MeshMaterial2d(mat),
                    Transform::from_translation(target),
                    ServerId(unit.id),
                    TargetPosition(target),
                ))
                .id();
            unit_map.0.insert(unit.id, entity);
        }
    }
}

/// Return the mesh and material handles for a unit given its shape and faction.
#[cfg(target_arch = "wasm32")]
fn unit_mesh_and_material(
    shape: common::Shape,
    is_enemy: bool,
    assets: &UnitAssets,
) -> (Handle<Mesh>, Handle<ColorMaterial>) {
    match (shape, is_enemy) {
        (Shape::Circle,   false) => (assets.circle_mesh.clone(),   assets.circle_ally_mat.clone()),
        (Shape::Square,   false) => (assets.square_mesh.clone(),   assets.square_ally_mat.clone()),
        (Shape::Triangle, false) => (assets.triangle_mesh.clone(), assets.triangle_ally_mat.clone()),
        (Shape::Circle,   true)  => (assets.circle_mesh.clone(),   assets.circle_enemy_mat.clone()),
        (Shape::Square,   true)  => (assets.square_mesh.clone(),   assets.square_enemy_mat.clone()),
        (Shape::Triangle, true)  => (assets.triangle_mesh.clone(), assets.triangle_enemy_mat.clone()),
    }
}

// ── Unit interpolation ────────────────────────────────────────────────────────

/// ECS system: smoothly move each unit's `Transform` toward its `TargetPosition`.
///
/// Speed is in world units/s.  At 1:1 pixel scale a unit can cross ~60 px
/// (one grid cell) in 0.3 s at the default 200 px/s.
#[cfg(target_arch = "wasm32")]
fn interpolate_units(
    time: Res<Time>,
    mut query: Query<(&TargetPosition, &mut Transform), With<ServerId>>,
) {
    const SPEED: f32 = 200.0;
    let dt = time.delta_secs();

    for (target, mut transform) in &mut query {
        let delta = target.0 - transform.translation;
        let dist = delta.length();
        if dist > 0.5 {
            let step = (SPEED * dt).min(dist);
            transform.translation += delta / dist * step;
        }
    }
}

// ── Combat events → projectiles ───────────────────────────────────────────────

/// ECS system: spawn projectile entities for each pending `CombatEvent`.
#[cfg(target_arch = "wasm32")]
fn process_combat_events(
    mut commands: Commands,
    mut combat_buf: ResMut<CombatEventBufferRes>,
    unit_assets: Option<Res<UnitAssets>>,
) {
    let Some(assets) = unit_assets else {
        return;
    };
    if combat_buf.events.is_empty() {
        return;
    }

    let events = std::mem::take(&mut combat_buf.events);
    for event in events {
        let (sx, sy) = server_to_world(event.start_pos.x, event.start_pos.y);
        let (ex, ey) = server_to_world(event.end_pos.x, event.end_pos.y);
        commands.spawn((
            Mesh2d(assets.projectile_mesh.clone()),
            MeshMaterial2d(assets.projectile_mat.clone()),
            Transform::from_translation(Vec3::new(sx, sy, 1.0)), // z=1 renders above units
            Projectile {
                end: Vec3::new(ex, ey, 1.0),
                speed: 400.0, // px/s
            },
        ));
    }
}

/// ECS system: advance each projectile toward its destination and despawn on arrival.
#[cfg(target_arch = "wasm32")]
fn update_projectiles(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &Projectile, &mut Transform)>,
) {
    let dt = time.delta_secs();

    for (entity, projectile, mut transform) in &mut query {
        let delta = projectile.end - transform.translation;
        let dist = delta.length();
        if dist < 2.0 {
            commands.entity(entity).despawn();
        } else {
            let step = (projectile.speed * dt).min(dist);
            transform.translation += delta / dist * step;
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use common::messages::SerializableGameState;
    use common::GamePhase;

    fn make_game_state() -> SerializableGameState {
        SerializableGameState {
            units: vec![],
            players: vec![],
            phase: GamePhase::Build,
            phase_timer: 30.0,
        }
    }

    #[test]
    fn game_state_buffer_initially_empty() {
        let _ = pop_game_state();
        assert!(pop_game_state().is_none());
    }

    #[test]
    fn game_state_buffer_push_then_pop() {
        let _ = pop_game_state();
        push_game_state(make_game_state());
        assert!(pop_game_state().is_some());
        assert!(pop_game_state().is_none());
    }

    #[test]
    fn game_state_buffer_overwrites_previous() {
        let _ = pop_game_state();
        push_game_state(make_game_state());
        let mut second = make_game_state();
        second.phase_timer = 15.0;
        push_game_state(second);
        assert_eq!(pop_game_state().unwrap().phase_timer, 15.0);
    }

    #[test]
    fn client_action_buffer_initially_empty() {
        let _ = drain_client_actions();
        assert!(drain_client_actions().is_empty());
    }

    #[test]
    fn client_action_buffer_push_drain() {
        let _ = drain_client_actions();
        push_client_action(ClientAction::HireWorker);
        push_client_action(ClientAction::SendUnit(Shape::Circle));
        let drained = drain_client_actions();
        assert_eq!(drained.len(), 2);
        assert_eq!(drained[0], ClientAction::HireWorker);
        assert_eq!(drained[1], ClientAction::SendUnit(Shape::Circle));
        assert!(drain_client_actions().is_empty());
    }

    #[test]
    fn skip_to_combat_action_preserved() {
        let _ = drain_client_actions();
        push_client_action(ClientAction::SkipToCombat);
        assert!(drain_client_actions().contains(&ClientAction::SkipToCombat));
    }

    // ── server_to_world (2-D) ─────────────────────────────────────────────────

    // Board top-left maps to world upper-left.
    #[test]
    fn server_to_world_origin() {
        let (x, y) = server_to_world(0.0, 0.0);
        assert!((x - (-700.0)).abs() < 1e-4, "x={x}");
        assert!((y - 300.0).abs() < 1e-4, "y={y}");
    }

    // Board centre maps to world origin.
    #[test]
    fn server_to_world_board_centre() {
        let (x, y) = server_to_world(700.0, 300.0);
        assert!(x.abs() < 1e-4, "x={x}");
        assert!(y.abs() < 1e-4, "y={y}");
    }

    // Centre of left board.
    #[test]
    fn server_to_world_left_board_centre() {
        let (x, y) = server_to_world(300.0, 300.0);
        assert!((x - (-400.0)).abs() < 1e-4, "x={x}");
        assert!(y.abs() < 1e-4, "y={y}");
    }

    // Right board starts at server_x = 800.
    #[test]
    fn server_to_world_right_board_start() {
        let (x, _) = server_to_world(800.0, 0.0);
        assert!((x - 100.0).abs() < 1e-4, "x={x}");
    }

    // Right board ends at server_x = 1400.
    #[test]
    fn server_to_world_right_board_end() {
        let (x, _) = server_to_world(1400.0, 0.0);
        assert!((x - 700.0).abs() < 1e-4, "x={x}");
    }

    // Y range: server top → world +300, server bottom → world –300.
    #[test]
    fn server_to_world_height_range() {
        let (_, y_top) = server_to_world(0.0, 0.0);
        let (_, y_bot) = server_to_world(0.0, 600.0);
        assert!((y_top - 300.0).abs() < 1e-4);
        assert!((y_bot - (-300.0)).abs() < 1e-4);
    }

    // ── combat event buffer ───────────────────────────────────────────────────

    #[test]
    fn combat_event_buffer_initially_empty() {
        let _ = drain_combat_events();
        assert!(drain_combat_events().is_empty());
    }

    #[test]
    fn combat_event_buffer_push_drain() {
        use common::components::{DamageType, Position};
        let _ = drain_combat_events();
        push_combat_events(vec![CombatEvent {
            attacker_id: 1,
            target_id: 2,
            attack_type: DamageType::PhysicalBasic,
            start_pos: Position { x: 100.0, y: 200.0 },
            end_pos: Position { x: 900.0, y: 400.0 },
        }]);
        let d = drain_combat_events();
        assert_eq!(d.len(), 1);
        assert!(drain_combat_events().is_empty());
    }

    #[test]
    fn combat_event_buffer_accumulates() {
        use common::components::{DamageType, Position};
        let _ = drain_combat_events();
        let e = || CombatEvent {
            attacker_id: 0,
            target_id: 1,
            attack_type: DamageType::FireMagical,
            start_pos: Position { x: 0.0, y: 0.0 },
            end_pos: Position { x: 10.0, y: 10.0 },
        };
        push_combat_events(vec![e(), e()]);
        push_combat_events(vec![e()]);
        assert_eq!(drain_combat_events().len(), 3);
    }

    // ── interpolation math (pure, no Bevy) ───────────────────────────────────

    #[test]
    fn interpolation_does_not_overshoot() {
        let (cx, cy) = (0.0f32, 0.0f32);
        let (tx, ty) = (3.0f32, 0.0f32); // close target
        let speed = 200.0f32;
        let dt = 1.0f32 / 60.0;
        let dx = tx - cx;
        let dy = ty - cy;
        let dist = (dx * dx + dy * dy).sqrt();
        let step = (speed * dt).min(dist);
        let new_x = cx + (dx / dist) * step;
        let new_y = cy + (dy / dist) * step;
        assert!(new_x <= tx + 1e-4, "must not overshoot x: {new_x} > {tx}");
        assert_eq!(new_y, ty);
    }
}
