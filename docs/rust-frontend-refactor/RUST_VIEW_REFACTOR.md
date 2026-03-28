# Rust Frontend Refactor — Knowledge Transfer

**Branch:** `feature/rust-frontend-refactor`
**Status:** Paused — functionally working but plays worse than the HTML/JS frontend
**Date parked:** 2026-03-27

---

## What This Branch Set Out To Do

Replace the existing HTML/JS frontend with a fully Rust/WASM frontend. The goals were:

- **Type safety end-to-end** — share types between server and client via a `common` crate, eliminating hand-written TS interfaces
- **Bevy for game rendering** — use the ECS game engine instead of hand-drawing on a canvas in JS
- **Leptos for the UI shell** — reactive Rust UI framework (think SolidJS but in Rust) for auth, lobby, and HUD

All three phases from the original plan were completed. The result works but the feel is worse than the JS version.

---

## Architecture

```
┌─────────────────────────────────────────────────────┐
│                    Browser (WASM)                    │
│                                                      │
│  ┌───────────────────┐   ┌──────────────────────┐   │
│  │   Leptos (UI)     │   │   Bevy (game canvas) │   │
│  │                   │   │                      │   │
│  │  - Auth views     │   │  - 2D unit rendering │   │
│  │  - Lobby view     │   │  - Unit interpolation│   │
│  │  - Game HUD       │◄──►  - Projectiles       │   │
│  │  - Routing        │   │  - Scene setup       │   │
│  │  - WebSocket      │   │                      │   │
│  └───────────────────┘   └──────────────────────┘   │
│            ▲                        ▲                │
│            └──── thread_local! ─────┘                │
│                  bridge buffers                      │
└─────────────────────────────────────────────────────┘
            │
            │  JSON over WebSocket
            ▼
    ┌───────────────┐
    │  Axum server  │
    └───────────────┘
```

The central architectural tension: **two framework event loops sharing one WASM thread**. Leptos handles reactive DOM updates; Bevy runs its ECS `Update` schedule on every `requestAnimationFrame`. They communicate via `thread_local!` polling buffers rather than direct shared state.

---

## Workspace Structure

A Cargo workspace was added at the repo root:

| Crate | Purpose |
|---|---|
| `server/` | Existing Axum backend — modified to depend on `common` |
| `common/` | New shared types used by both server and view |
| `view-rust/` | New Leptos + Bevy WASM frontend |

---

## `common/` Crate

Types extracted from the server so the frontend gets compile-time access to all message and game types:

- `shape.rs` — `Shape` enum (`Circle`, `Square`, `Triangle`)
- `game_phase.rs` — `GamePhase` enum (`Build`, `Combat`, `Victory`)
- `components.rs` — `Position { x, y }`, `DamageType` enum
- `messages.rs` — All WS wire types:
  - `ClientMessage` — tagged `{ action, payload }` JSON format (`camelCase`)
  - `ServerMessage` — tagged `{ type, data }` JSON format
  - `SerializableGameState`, `Unit`, `PlayerView`, `LobbyInfo`, `CombatEvent`, `UnitInfoData`, `PlaceMessage`

All types derive `serde::Serialize`/`Deserialize` and match the existing wire format exactly.

---

## `view-rust/` Crate — File By File

Built with [Trunk](https://trunkrs.dev/) as a CSR (client-side rendered) Leptos SPA.

| File | What it does |
|---|---|
| `main.rs` | Entry point — `leptos::mount_to_body(App)` |
| `app.rs` | Root `App` component; creates `AppState` context, auto-reconnects on page refresh if a token is in sessionStorage, sets up client-side routing (`/`, `/login`, `/register`, `/lobby`, `/game`) |
| `app_state.rs` | `AppState` — a bundle of `RwSignal<T>` fields provided as Leptos context: `player_id`, `lobby_status`, `game_state`, `combat_events`, `ws_error`, `disconnected` |
| `auth.rs` | `LoginView` and `RegisterView`. Uses `reqwest` (browser fetch backend) to POST to `/api/auth/login` and `/api/auth/register`. Auto-logs in after successful registration. |
| `lobby.rs` | `LobbyView` — shows lobby list, handles join, logout, disconnect/reconnect overlay. Navigates to `/game` reactively when `game_state` becomes `Some`. |
| `game_view.rs` | `GameView` + `GameHud` — embeds the Bevy `<canvas>`, lays a transparent `<div>` overlay on top to capture placement clicks, converts screen coords to server grid coords, sends `Place` messages over WS. HUD shows phase/gold/income/timer, shape selector, and send/hire/skip buttons. |
| `bevy_app.rs` | All Bevy ECS code — see section below. |
| `ws.rs` | `WsClient` built on `gloo-net`. Splits the socket into read/write halves, spawns both as `spawn_local` async tasks, uses an `mpsc` channel internally for writes. The send function is registered in a `thread_local` so any code can call `send_ws_message()` without capturing a `!Send` type. Dispatches incoming `ServerMessage`s to `AppState` signals and the Bevy bridge buffers. |
| `storage.rs` | `sessionStorage` wrapper for the JWT. Per-tab so each tab is an independent session. |
| `assets.rs` | Stub helper that builds URLs relative to the Trunk `dist/assets/` directory. |

---

## Bevy Integration (`bevy_app.rs`)

Bevy is launched once via `start_bevy_app("game-canvas")`, called from a Leptos `Effect` when the canvas element enters the DOM.

### Why 2D rendering?

`StandardMaterial` (3D PBR) silently falls back to the hot-pink error material in **Bevy 0.17 + WebGL2**. The refactor switched entirely to 2D:

- `Camera2d` (orthographic, 1 world unit = 1 screen pixel)
- `Mesh2d` + `ColorMaterial`
- Board is 1400×600 world units centred at the origin

### Coordinate mapping

Server uses top-left origin, Bevy uses centre origin, Y-up:

```
server_to_world(sx, sy):
  x = sx − 700
  y = 300 − sy
```

The two 600×600 boards sit at world X −700 to −100 (left) and 100 to 700 (right), with a 200px gap in the middle.

### ECS Systems (chained in the `Update` schedule)

| System | What it does |
|---|---|
| `sync_game_state` | Drains the thread-local game-state buffer into `GameStateBuffer` resource |
| `sync_client_actions` | Drains the thread-local action buffer into `ClientActionBuffer` resource |
| `sync_combat_events` | Drains the thread-local combat-event buffer into `CombatEventBufferRes` resource |
| `reconcile_units` | Diffs `GameStateBuffer` against `UnitEntityMap` (server ID → Entity); spawns/despawns/updates `TargetPosition` |
| `process_combat_events` | Spawns `Projectile` entities from `CombatEvent` data |
| `interpolate_units` | Lerps each unit's `Transform` toward its `TargetPosition` at 200 px/s |
| `update_projectiles` | Moves projectiles toward their `end` at 400 px/s; despawns on arrival |

### Unit visuals

Each unit shape/faction combination gets a pre-built mesh + `ColorMaterial` handle stored in the `UnitAssets` resource:

| | Circle | Square | Triangle |
|---|---|---|---|
| Ally | Blue | Green | Gold |
| Enemy | Red | Orange | Purple |

---

## The Leptos↔Bevy Bridge

Since Leptos and Bevy both need to react to WebSocket messages but run in separate execution contexts, three `thread_local! { RefCell<_> }` buffers act as the handoff:

| Buffer | Written by | Read by | Behaviour |
|---|---|---|---|
| `GAME_STATE_BUFFER` | `ws.rs` dispatch | `sync_game_state` ECS system | Overwrite (only latest matters) |
| `CLIENT_ACTION_BUFFER` | Leptos HUD click handlers | `sync_client_actions` ECS system | Queue (drain each frame) |
| `COMBAT_EVENT_BUFFER` | `ws.rs` dispatch | `sync_combat_events` ECS system | Queue (accumulates between frames) |

---

## Why It Felt Clunky

1. **Two runtimes, one thread.** Bevy's `run()` is blocking, so it's `spawn_local`'d into the WASM async executor. Leptos reactive effects and Bevy systems interleave in an uncoordinated way. There's no clean tick boundary.

2. **Input is split between two systems.** Placement clicks go through a transparent Leptos `<div>` overlay, not Bevy's input system. Bevy never knows about user input directly; actions flow in via the `CLIENT_ACTION_BUFFER`.

3. **Coordinate conversion overhead.** Every click does: browser pixel → world → server → grid, and every render does: server → world. Correct, but adds cognitive overhead and several conversion functions to keep in sync.

4. **Heavy WASM bundle.** Even with `default-features = false`, Bevy pulls in a large amount of code. Build times and bundle size are significantly worse than the JS frontend.

5. **WebGL2 limitations.** Hit the `StandardMaterial` → hot-pink fallback in Bevy 0.17. The Phase 4 vision of PBR/bloom/particles is not viable without a different rendering path.

6. **Reactivity impedance mismatch.** Leptos is fine-grained push-based reactivity; Bevy is polling-based ECS. Bridging them via polling buffers works but loses the push model that makes Leptos efficient.

---

## What Was Completed

- [x] `common` crate with all shared message and game types
- [x] Server updated to import from `common` instead of defining types locally
- [x] Cargo workspace wiring all three crates together
- [x] Leptos SPA with client-side routing
- [x] JWT auth flow (login, register, session storage, auto-reconnect on page refresh)
- [x] Lobby view (join, logout, disconnect/reconnect overlay)
- [x] Bevy canvas embedded in game view, starts once on mount
- [x] 2D unit rendering (three shapes, ally/enemy colour sets)
- [x] Unit position interpolation (smooth movement toward server snapshots)
- [x] Projectile system (spawn on `CombatEvent`, move, despawn on arrival)
- [x] Game HUD overlay (phase, gold, income, timer, shape selector, send/hire/skip)
- [x] Click-to-place on player's own board with server-space grid conversion
- [x] Extensive unit tests (coordinate math, buffer behaviour, serde round-trips, WS URL format)

## What Was Left Incomplete

- [ ] Sell unit UI — `SellUnit` action and `SellById` message exist but nothing in the HUD lets you click a specific unit to sell it
- [ ] Unit info panel — `RequestUnitInfo`/`UnitInfo` messages wired but not displayed in the HUD
- [ ] HP bars above units
- [ ] CSS/styling — class names are assigned everywhere but no stylesheet was written
- [ ] Worker state display
- [ ] `assets/` directory is empty — no sprites, sounds, or models loaded

---

## How To Resume

```sh
git checkout feature/rust-frontend-refactor

# Prerequisites
rustup target add wasm32-unknown-unknown
cargo install trunk

# Run
cd server && cargo run          # in one terminal
cd view-rust && trunk serve     # in another
```

Trunk config: `view-rust/Trunk.toml`
WASM target: `view-rust/.cargo/config.toml`

---

## Paths Forward

**Option A — Drop Bevy, use raw Canvas 2D.** The game is top-down 2D with primitive shapes. Bevy's overhead is not buying much here. Driving a `<canvas>` 2D context from Leptos effects (or a minimal `requestAnimationFrame` loop in WASM) would be far lighter and avoid the bridge complexity entirely.

**Option B — Keep `common`, abandon the Rust frontend.** The `common` crate is unconditionally good — the server and frontend now share one source of truth for all types. Even if the Rust frontend is scrapped, `common` should be kept and the JSON wire format kept in sync with it.

**Option C — Hybrid: JS frontend + Rust/WASM message parsing.** Compile `common` to WASM and generate a thin JS wrapper so the existing JS frontend can deserialize server messages through generated Rust types instead of hand-written TS interfaces. Best of both worlds in terms of type safety vs rendering feel.
