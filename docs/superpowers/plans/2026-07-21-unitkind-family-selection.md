# UnitKind + Family Selection Implementation Plan (Tower Families Chunk 2)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rename `Shape` to `UnitKind` across the server and client (pure rename, no new variants yet), then add the `Family` selection plumbing: a `Family` enum (currently only `Family::Basic`, wrapping today's three shapes), `Player.family`, a `PickFamily` client message with server-side validation and lock, a server-sent build catalog keyed by family, and a client pick UI + catalog-driven shop that replaces the three hardcoded build buttons. Ships with exactly one family (`Basic` = Square/Triangle/Circle) so the pick→catalog→shop pipeline is proven before Chunk 3 adds Ice as a second real choice.

**Architecture:** `UnitKind` is the renamed `Shape` enum (still 3 variants: `Square`, `Triangle`, `Circle` — real family content lands in Chunks 3-5). `Family` is a new enum (`Basic` only, for now) with `family_catalog(Family) -> Vec<UnitKind>` in `unit_config.rs` mapping family → its buildable units — this is the function that makes reroll cheap later per the design doc (`Player.family` is mutable, placed towers reference their own `UnitKind`, never their owner's family). `Player` gains `family: Option<Family>`, serialized like every other player field, so the opponent's pick is visible for free via the existing `Players` broadcast — no new broadcast message needed for visibility. Picking is one-shot: `ClientMessage::PickFamily` is rejected once `player.family` is `Some`. On a successful pick the server replies with `ServerMessage::BuildCatalog` (catalog for that player's family only) and the client rebuilds its build-menu buttons from it, replacing the hardcoded `#selectSquare/#selectCircle/#selectTriangle` buttons in `index.html`. `ClientMessage::Place` now rejects any `UnitKind` not in the picker's family catalog (and rejects placement entirely if no family is picked yet).

**Tech Stack:** Rust (Bevy ECS, server), TypeScript (Pixi.js client, `view/`)

## Global Constraints

- No lumber/secondary resource — not applicable to this chunk.
- Concurrency: all new logic runs inside `handle_client_message` (synchronous, already under the lobby guard) or in the existing pre-game reply path (`pre_game.rs`, already network-`.await` outside any lobby guard) — no new locks, no `.await` under a lobby guard (CLAUDE.md invariants).
- Existing test suite (`cargo test` in `server/`, client test suite in `view/`) must stay green throughout.
- `UnitKind` keeps exactly the same 3 variants (`Square`, `Triangle`, `Circle`) and the same wire values — this chunk is rename + plumbing only, no balance or content change.
- Family pick is locked for the whole match once set (design doc decision 4/7) — no un-pick, no re-pick in this chunk (reroll is future work, decision 7).

---

## File Map

**Task 1 — mechanical rename `Shape` → `UnitKind`:**
- Rename `server/src/model/shape.rs` → `server/src/model/unit_kind.rs`; rename the enum `Shape` → `UnitKind` inside it.
- Modify `server/src/model/mod.rs` (or wherever `mod shape;` is declared) → `mod unit_kind;`.
- Modify every file importing `crate::model::shape::Shape` / `super::shape::Shape` to `crate::model::unit_kind::UnitKind` / `super::unit_kind::UnitKind`, and every use of the type name `Shape` to `UnitKind`, in: `server/src/model/components.rs`, `server/src/model/player.rs`, `server/src/model/messages.rs`, `server/src/model/lobby.rs`, `server/src/model/unit_config.rs`, `server/src/handler/spawn.rs`, `server/src/handler/wave.rs`, `server/src/handler/king.rs`, `server/src/handler/game_loop.rs`, `server/src/handler/in_game.rs`, `server/src/handler/sim_tests.rs`, `server/src/handler/combat/attack.rs`, `server/src/handler/combat/movement.rs`, `server/src/handler/combat/targeting.rs`.
- Field/component names are unchanged: `ShapeComponent(pub UnitKind)` keeps its name `ShapeComponent`, `Player.spawning_queue: Vec<UnitKind>` keeps its field name `spawning_queue`, `PlaceMessage.shape: UnitKind` keeps its field name `shape`, `Unit.shape` keeps its field name `shape`. Only the **type name** changes — JSON wire shape for existing fields is untouched (variant strings `"Square"/"Triangle"/"Circle"` unchanged).
- Modify `view/mercenary_panel.ts:3` — `export type Shape = 'Square' | 'Circle' | 'Triangle';` → rename the type alias to `UnitKind` (used by `MercenaryPanelCallbacks.onSend`).
- Modify `view/app.ts` — `selectedShape` variable stays (still refers to a `UnitKind` string value) but its type annotation and any local type references to `Shape` become `UnitKind`.
- Modify `view/types.ts` — no shape-of-JSON change needed (see below in Task 5 for the catalog-driven rename of the `Unit.shape` / `Player.spawning_queue` type annotations from the inline union to a shared `UnitKind` type alias).

**Task 2 — `Family` enum + catalog function:**
- Modify `server/src/model/unit_kind.rs` — add `Family` enum in the same file (or a new `server/src/model/family.rs` — pick the latter to keep `unit_kind.rs` focused; see Task 2 below for exact choice).
- Modify `server/src/model/unit_config.rs` — add `family_catalog(family: Family) -> Vec<UnitKind>` and `unit_kind_name(kind: UnitKind) -> &'static str`.

**Task 3 — `Player.family` + `PickFamily` message + handler:**
- Modify `server/src/model/player.rs` — add `pub family: Option<Family>` field, initialize to `None` in `Player::new`.
- Modify `server/src/model/messages.rs` — add `ClientMessage::PickFamily { family: Family }`, `ServerMessage::FamilyOptions(Vec<Family>)`, `ServerMessage::BuildCatalog(Vec<BuildCatalogEntry>)`, and the `BuildCatalogEntry` struct.
- Modify `server/src/handler/in_game.rs` — new `ClientMessage::PickFamily` match arm in `handle_client_message`; `ClientMessage::Place` arm gains a family-catalog validation check.
- Modify `server/src/handler/pre_game.rs` — send `ServerMessage::FamilyOptions(family_catalog_options())` alongside the existing `SendUnitCatalog` send, at both `Matched` and `Waiting`-resolved sites.

**Task 4 — client family pick UI + catalog-driven shop:**
- Modify `view/index.html` — remove the three hardcoded `#selectSquare/#selectCircle/#selectTriangle` buttons; add an empty `<div id="family-pick"></div>` (shown once, before the first build catalog arrives) and an empty `<div id="build-shop"></div>` (populated from `BuildCatalog`) in their place inside `#controls`.
- Modify `view/types.ts` — add `export type UnitKind = 'Square' | 'Circle' | 'Triangle';`, reuse it for `Unit.shape` and `Player.spawning_queue`; add `Family`, `BuildCatalogEntry`, and the two new `ServerMessage` payload shapes.
- Modify `view/app.ts` — handle `familyOptions` / `buildCatalog` server messages, render the pick buttons and the shop buttons, send `pickFamily`, and drive `selectedShape` (renamed `selectedUnitKind`) from the catalog instead of hardcoded IDs.

---

### Task 1: Mechanical rename `Shape` → `UnitKind`

**Files:** see File Map above (14 server files + 2 client files).

**Interfaces:**
- Produces: `crate::model::unit_kind::UnitKind` (server), replacing `crate::model::shape::Shape` everywhere; `UnitKind` type alias (client), replacing `Shape` in `mercenary_panel.ts`.
- No behavior or wire-format change — this is the checkpoint: after this task, `cargo test` and the client test suite must be identical in pass/fail count to before the rename.

- [ ] **Step 1: Rename the file and the enum**

```bash
git mv server/src/model/shape.rs server/src/model/unit_kind.rs
```

Edit `server/src/model/unit_kind.rs`, change:

```rust
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    Circle,
    Square,
    Triangle,
}
```

to:

```rust
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnitKind {
    Circle,
    Square,
    Triangle,
}
```

- [ ] **Step 2: Update the module declaration**

Find the `mod shape;` declaration (in `server/src/model/mod.rs` — run `grep -rn "mod shape" server/src/model/mod.rs` to confirm the exact line) and change it to `mod unit_kind;`. If `pub use` re-exports exist for `shape::Shape`, rename them to `unit_kind::UnitKind`.

- [ ] **Step 3: Mechanical find/replace across the server crate**

Run, from the repo root:

```bash
cd server/src
grep -rl 'model::shape::Shape\|super::shape::Shape' . | xargs sed -i \
  -e 's/model::shape::Shape/model::unit_kind::UnitKind/g' \
  -e 's/super::shape::Shape/super::unit_kind::UnitKind/g'
grep -rl '\bShape\b' . | xargs sed -i 's/\bShape\b/UnitKind/g'
```

The second `sed` is a bare word-boundary replace of `Shape` → `UnitKind`; run it only after the import-path replace above so `use ...::Shape` lines are already correctly pathed, and manually re-check with `grep -rn 'UnitKind' server/src | grep -i "shapecomponent\|shape:"` that `ShapeComponent` (component name) and `.shape` / `shape:` field names were **not** touched — the bare-word regex only matches the exact token `Shape`, so `ShapeComponent` and `shape` are already safe, but verify visually on the diff before committing.

- [ ] **Step 4: Fix compile errors from the mechanical pass**

Run `cd server && cargo build --lib 2>&1 | head -100` and fix any remaining `Shape` references the sed pass missed (e.g. doc comments, `Shape::` in string-free contexts the regex didn't catch due to line wrapping). Do not change any logic — only type-name references.

- [ ] **Step 5: Client rename**

In `view/mercenary_panel.ts`, change:

```typescript
export type Shape = 'Square' | 'Circle' | 'Triangle';
```

to:

```typescript
export type UnitKind = 'Square' | 'Circle' | 'Triangle';
```

and update its one use site, `MercenaryPanelCallbacks.onSend: (shape: Shape) => void;` → `onSend: (shape: UnitKind) => void;`. Update the importer in `view/app.ts` (`import { ... } from './mercenary_panel'` or wherever `Shape` is imported from) to import `UnitKind` instead, and change `let selectedShape: 'Square' | 'Circle' | 'Triangle' = 'Square';` to `let selectedShape: UnitKind = 'Square';` (kept as a plain type-only rename here; the variable itself is renamed to `selectedUnitKind` in Task 4 alongside the shop rebuild, to keep this step a pure type rename).

- [ ] **Step 6: Run the full test suites**

Run: `cd server && cargo test` — expect PASS, same test count as before this task.
Run: `cd view && npx tsc --noEmit && npx tsx --test *.ts` — expect PASS.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "refactor: rename Shape to UnitKind across server and client"
```

---

### Task 2: `Family` enum and catalog lookup

**Files:**
- Create: `server/src/model/family.rs`
- Modify: `server/src/model/mod.rs` (register `mod family;`)
- Modify: `server/src/model/unit_config.rs` (add `family_catalog`, `unit_kind_name`, `family_catalog_options`)

**Interfaces:**
- Consumes: `UnitKind` (Task 1).
- Produces: `Family { Basic }` (Serialize/Deserialize/Clone/Copy/Debug/PartialEq/Eq), `family_catalog(family: Family) -> Vec<UnitKind>`, `unit_kind_name(kind: UnitKind) -> &'static str`, `family_catalog_options() -> Vec<Family>` — used by Task 3.

- [ ] **Step 1: Write the failing tests**

Create `server/src/model/family.rs`:

```rust
use serde::{Deserialize, Serialize};

/// A player's chosen tower family for the match. Only `Basic` (today's three
/// shapes) exists in this chunk; Ice/Poison Beasts/Elves land in Chunks 3-5.
/// Deliberately just an enum, not a struct carrying data: the catalog lookup
/// (`unit_config::family_catalog`) is the single source of truth for which
/// `UnitKind`s a family unlocks, so adding a family later is one match arm,
/// not a schema change.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    Basic,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn family_serializes_as_plain_string() {
        assert_eq!(serde_json::to_string(&Family::Basic).unwrap(), "\"Basic\"");
    }
}
```

Add to `server/src/model/unit_config.rs` (bottom, inside the existing `#[cfg(test)] mod tests`):

```rust
    #[test]
    fn family_catalog_basic_has_all_three_shapes() {
        use crate::model::family::Family;
        let catalog = family_catalog(Family::Basic);
        assert_eq!(catalog.len(), 3);
        assert!(catalog.contains(&UnitKind::Square));
        assert!(catalog.contains(&UnitKind::Triangle));
        assert!(catalog.contains(&UnitKind::Circle));
    }

    #[test]
    fn unit_kind_name_is_nonempty_for_all_kinds() {
        for kind in [UnitKind::Square, UnitKind::Triangle, UnitKind::Circle] {
            assert!(!unit_kind_name(kind).is_empty());
        }
    }

    #[test]
    fn family_catalog_options_includes_basic() {
        use crate::model::family::Family;
        assert!(family_catalog_options().contains(&Family::Basic));
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd server && cargo test --lib model::family model::unit_config`
Expected: FAIL to compile — `family` module not registered, `family_catalog`/`unit_kind_name`/`family_catalog_options` don't exist.

- [ ] **Step 3: Register the module**

In `server/src/model/mod.rs`, add `pub mod family;` alongside the other `pub mod` declarations (match the visibility used for `unit_kind`/`player`/etc. — check with `grep -n "^pub mod\|^mod" server/src/model/mod.rs` first and match the existing pattern).

- [ ] **Step 4: Implement the catalog functions**

In `server/src/model/unit_config.rs`, add near the top (after the existing imports):

```rust
use super::family::Family;
```

And add these functions (near `shape_index`, which this parallels):

```rust
/// Display name for a unit kind, used in the client build-catalog message.
pub fn unit_kind_name(kind: UnitKind) -> &'static str {
    match kind {
        UnitKind::Square => "Square",
        UnitKind::Triangle => "Triangle",
        UnitKind::Circle => "Circle",
    }
}

/// The buildable `UnitKind`s for a given family. Single source of truth for
/// family→roster: `Place` validation and the server-sent build catalog both
/// read this, so adding a family later only touches this one match arm.
pub fn family_catalog(family: Family) -> Vec<UnitKind> {
    match family {
        Family::Basic => vec![UnitKind::Square, UnitKind::Triangle, UnitKind::Circle],
    }
}

/// All families a player may currently pick from (sent to the client right
/// after `MatchFound` as `ServerMessage::FamilyOptions`).
pub fn family_catalog_options() -> Vec<Family> {
    vec![Family::Basic]
}
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cd server && cargo test --lib model::family model::unit_config`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add server/src/model/family.rs server/src/model/mod.rs server/src/model/unit_config.rs
git commit -m "feat: add Family enum and family-to-UnitKind catalog lookup"
```

---

### Task 3: `Player.family`, `PickFamily` message, and server-side validation

**Files:**
- Modify: `server/src/model/player.rs` (add `family` field)
- Modify: `server/src/model/messages.rs` (new message types)
- Modify: `server/src/handler/in_game.rs` (`PickFamily` handler, `Place` validation)
- Modify: `server/src/handler/pre_game.rs` (send `FamilyOptions` on match found)

**Interfaces:**
- Consumes: `Family`, `family_catalog`, `family_catalog_options`, `unit_kind_name` (Task 2).
- Produces: `Player.family: Option<Family>`; `ClientMessage::PickFamily { family: Family }`; `ServerMessage::FamilyOptions(Vec<Family>)`; `ServerMessage::BuildCatalog(Vec<BuildCatalogEntry>)` where `BuildCatalogEntry { unit_kind: UnitKind, name: &'static str, cost: u32 }` — consumed by Task 4 (client).

- [ ] **Step 1: Write the failing tests**

Add to `server/src/model/player.rs` tests module:

```rust
    #[test]
    fn new_player_has_no_family_picked() {
        let player = Player::new(1, "test".to_string(), 100);
        assert_eq!(player.family, None);
    }
```

Add to `server/src/model/messages.rs` tests module:

```rust
    #[test]
    fn deserialize_pick_family() {
        use crate::model::family::Family;
        let json = r#"{"action":"pickFamily","payload":{"family":"Basic"}}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();
        match msg {
            ClientMessage::PickFamily { family } => assert_eq!(family, Family::Basic),
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn serialize_family_options() {
        use crate::model::family::Family;
        let msg = ServerMessage::FamilyOptions(vec![Family::Basic]);
        let json = serde_json::to_string(&msg).unwrap();
        assert_eq!(json, r#"{"type":"FamilyOptions","data":["Basic"]}"#);
    }

    #[test]
    fn serialize_build_catalog() {
        let msg = ServerMessage::BuildCatalog(vec![BuildCatalogEntry {
            unit_kind: UnitKind::Square,
            name: "Square",
            cost: 25,
        }]);
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.starts_with(r#"{"type":"BuildCatalog","data":["#));
        assert!(json.contains(r#""unit_kind":"Square""#));
        assert!(json.contains(r#""name":"Square""#));
        assert!(json.contains(r#""cost":25"#));
    }
```

Add to `server/src/handler/in_game.rs` tests module (near the other `Place` tests):

```rust
    #[test]
    fn place_rejected_without_family_picked() {
        use crate::model::messages::PlaceMessage;
        let mut lobby = test_lobby_two_players();
        let msg = ClientMessage::Place(PlaceMessage {
            shape: UnitKind::Square,
            row: 0,
            col: 0,
        });
        let outcome = handle_client_message(&mut lobby, lobby.players[0].id, msg);
        match outcome {
            MessageOutcome::Reply(ServerMessage::Error(e)) => {
                assert!(e.contains("family"), "expected family error, got: {e}");
            }
            other => panic!("expected family-required error, got {other:?}"),
        }
    }

    #[test]
    fn pick_family_locks_and_rejects_second_pick() {
        use crate::model::family::Family;
        let mut lobby = test_lobby_two_players();
        let player_id = lobby.players[0].id;

        let first = handle_client_message(
            &mut lobby,
            player_id,
            ClientMessage::PickFamily { family: Family::Basic },
        );
        assert!(matches!(
            first,
            MessageOutcome::Reply(ServerMessage::BuildCatalog(_))
        ));
        assert_eq!(lobby.players[0].family, Some(Family::Basic));

        let second = handle_client_message(
            &mut lobby,
            player_id,
            ClientMessage::PickFamily { family: Family::Basic },
        );
        match second {
            MessageOutcome::Reply(ServerMessage::Error(e)) => {
                assert!(e.contains("locked") || e.contains("already"));
            }
            other => panic!("expected already-locked error, got {other:?}"),
        }
    }

    #[test]
    fn place_succeeds_after_family_picked() {
        use crate::model::family::Family;
        use crate::model::messages::PlaceMessage;
        let mut lobby = test_lobby_two_players();
        let player_id = lobby.players[0].id;
        handle_client_message(
            &mut lobby,
            player_id,
            ClientMessage::PickFamily { family: Family::Basic },
        );
        let msg = ClientMessage::Place(PlaceMessage {
            shape: UnitKind::Square,
            row: 0,
            col: 0,
        });
        let outcome = handle_client_message(&mut lobby, player_id, msg);
        assert!(matches!(outcome, MessageOutcome::Handled));
    }
```

(If `test_lobby_two_players()` doesn't already exist as a shared test helper in `in_game.rs`, use whatever the existing `Place` tests use to construct a two-player lobby — check lines ~440-470 of the current file for the established pattern and match it exactly; do not invent a new helper name.)

- [ ] **Step 2: Run test to verify it fails**

Run: `cd server && cargo test --lib model::player model::messages handler::in_game`
Expected: FAIL to compile — `Player.family`, `ClientMessage::PickFamily`, `ServerMessage::FamilyOptions`/`BuildCatalog`, `BuildCatalogEntry` don't exist yet.

- [ ] **Step 3: Add `Player.family`**

In `server/src/model/player.rs`, add the import and field:

```rust
use crate::model::family::Family;
```

Add `pub family: Option<Family>,` to the `Player` struct (after `leaks_this_wave`), and `family: None,` to the `Self { ... }` literal in `Player::new`.

- [ ] **Step 4: Add the message types**

In `server/src/model/messages.rs`, add the import:

```rust
use super::family::Family;
use super::unit_kind::UnitKind;
```

(Adjust based on Task 1's actual import path for `UnitKind` — `messages.rs` already imports `Shape`/`UnitKind` per Task 1's rename; just add the `Family` import alongside it.)

Add to `ClientMessage`:

```rust
    PickFamily {
        family: Family,
    },
```

Add after `SendUnitCatalogEntry`:

```rust
/// One entry in the server-sent build catalog for the picking player's
/// family. Sent as `ServerMessage::BuildCatalog` right after a successful
/// `PickFamily`. The client builds its shop buttons purely from this list —
/// adding a tower to a family requires no client change.
#[derive(Serialize, Clone, Debug)]
pub struct BuildCatalogEntry {
    pub unit_kind: UnitKind,
    pub name: &'static str,
    pub cost: u32,
}
```

Add to `ServerMessage`:

```rust
    /// Families the player may pick from, sent once right after `MatchFound`.
    FamilyOptions(Vec<Family>),
    /// Server-driven build catalog for the picking player's chosen family,
    /// sent once in reply to a successful `PickFamily`.
    BuildCatalog(Vec<BuildCatalogEntry>),
```

- [ ] **Step 5: Implement the `PickFamily` handler and `Place` validation**

In `server/src/handler/in_game.rs`, add a new match arm to `handle_client_message` (place it right before the `ClientMessage::Place` arm):

```rust
        ClientMessage::PickFamily { family } => {
            let player_idx = lobby.players.iter().position(|p| p.id == player_id);
            let Some(idx) = player_idx else {
                return MessageOutcome::Ignored;
            };
            if lobby.players[idx].family.is_some() {
                return MessageOutcome::Reply(ServerMessage::Error(
                    "Family already locked for this match.".into(),
                ));
            }
            lobby.players[idx].family = Some(family);
            lobby.broadcast_changes();
            let catalog = crate::model::unit_config::family_catalog(family)
                .into_iter()
                .map(|unit_kind| crate::model::messages::BuildCatalogEntry {
                    unit_kind,
                    name: crate::model::unit_config::unit_kind_name(unit_kind),
                    cost: crate::model::unit_config::get_unit_profile(unit_kind).gold_cost,
                })
                .collect();
            MessageOutcome::Reply(ServerMessage::BuildCatalog(catalog))
        }
```

Then, inside the existing `ClientMessage::Place(p) => { ... }` arm, immediately after the `let Some(idx) = player_idx else { ... };` line and before the coordinate-bounds check, insert:

```rust
            match lobby.players[idx].family {
                None => {
                    return MessageOutcome::Reply(ServerMessage::Error(
                        "Pick a family before building.".into(),
                    ));
                }
                Some(family) => {
                    if !crate::model::unit_config::family_catalog(family).contains(&p.shape) {
                        return MessageOutcome::Reply(ServerMessage::Error(
                            "That unit isn't in your family.".into(),
                        ));
                    }
                }
            }
```

Every other existing `Place` test in this file that doesn't already pick a family will now fail — Step 6 covers fixing them.

- [ ] **Step 6: Fix existing `Place` tests to pick a family first**

Run `cd server && cargo test --lib handler::in_game 2>&1 | grep FAILED`. For each pre-existing test that constructs a `ClientMessage::Place` and expects `MessageOutcome::Handled` (or otherwise expects placement to succeed), add a `handle_client_message(&mut lobby, player_id, ClientMessage::PickFamily { family: Family::Basic })` call for the relevant player(s) before the `Place` call. Do this for every failing test — there is no shortcut here; each one needs the pick call inserted for whichever player's `Place` it exercises (both players, in tests that place for both boards).

- [ ] **Step 7: Send `FamilyOptions` alongside `SendUnitCatalog`**

In `server/src/handler/pre_game.rs`, at both sites that currently do:

```rust
                                    let _ = send_message(ws_sender, ServerMessage::MatchFound).await;
                                    let _ = send_message(
                                        ws_sender,
                                        ServerMessage::SendUnitCatalog(unit_config::send_unit_catalog()),
                                    )
                                    .await;
```

add a third send immediately after:

```rust
                                    let _ = send_message(
                                        ws_sender,
                                        ServerMessage::FamilyOptions(unit_config::family_catalog_options()),
                                    )
                                    .await;
```

(There are two occurrences — the `Matched` branch and the `match_rx` resolution inside the waiting loop — update both, matching the existing pattern exactly.)

- [ ] **Step 8: Run test to verify it passes**

Run: `cd server && cargo test`
Expected: PASS, zero failures.

- [ ] **Step 9: Commit**

```bash
git add server/src/model/player.rs server/src/model/messages.rs server/src/handler/in_game.rs server/src/handler/pre_game.rs
git commit -m "feat: add family pick message, server validation, and build catalog"
```

---

### Task 4: Client family pick UI + catalog-driven shop

**Files:**
- Modify: `view/types.ts`
- Modify: `view/index.html`
- Modify: `view/app.ts`

**Interfaces:**
- Consumes: `ServerMessage` variants `FamilyOptions`/`BuildCatalog` (Task 3), JSON shapes `{ type: "FamilyOptions", data: Family[] }` and `{ type: "BuildCatalog", data: BuildCatalogEntry[] }` where `Family = "Basic"` and `BuildCatalogEntry = { unit_kind: UnitKind, name: string, cost: number }`.
- Produces: `pickFamily` client action `{ action: "pickFamily", payload: { family: Family } }`.

- [ ] **Step 1: Write the failing test**

Create `view/test_build_catalog_ui.ts` (new file, mirrors the structure of `view/test_unit_info_panel.ts` — check that file's imports/harness first and match them exactly):

```typescript
import { test } from 'node:test';
import assert from 'node:assert';
import { renderBuildShop, renderFamilyOptions } from './app_build_ui';
import { BuildCatalogEntry, Family } from './types';

test('renderBuildShop creates one button per catalog entry with cost label', () => {
    const container = document.createElement('div');
    const catalog: BuildCatalogEntry[] = [
        { unit_kind: 'Square', name: 'Square', cost: 25 },
        { unit_kind: 'Circle', name: 'Circle', cost: 75 },
    ];
    let selected: string | null = null;
    renderBuildShop(container, catalog, (kind) => { selected = kind; });
    const buttons = container.querySelectorAll('button');
    assert.strictEqual(buttons.length, 2);
    assert.ok(buttons[0].textContent!.includes('Square'));
    assert.ok(buttons[0].textContent!.includes('25'));
    (buttons[1] as HTMLButtonElement).click();
    assert.strictEqual(selected, 'Circle');
});

test('renderFamilyOptions creates one button per family and clears on pick', () => {
    const container = document.createElement('div');
    const families: Family[] = ['Basic'];
    let picked: Family | null = null;
    renderFamilyOptions(container, families, (family) => { picked = family; });
    const buttons = container.querySelectorAll('button');
    assert.strictEqual(buttons.length, 1);
    (buttons[0] as HTMLButtonElement).click();
    assert.strictEqual(picked, 'Basic');
});
```

(This test requires a DOM — check how `test_unit_info_panel.ts` provides `document`; if the project uses `jsdom` via a test setup file, follow the same setup here. If no DOM shim is wired up yet, this reveals a gap Task 4 must also close by matching whatever `test_unit_info_panel.ts` already relies on — do not introduce a second DOM-testing approach.)

- [ ] **Step 2: Run test to verify it fails**

Run: `cd view && npx tsx --test test_build_catalog_ui.ts`
Expected: FAIL — `./app_build_ui` module doesn't exist.

- [ ] **Step 3: Add the `UnitKind`/`Family`/`BuildCatalogEntry` types**

In `view/types.ts`, add:

```typescript
export type UnitKind = 'Square' | 'Circle' | 'Triangle';
export type Family = 'Basic';

export interface BuildCatalogEntry {
    unit_kind: UnitKind;
    name: string;
    cost: number;
}
```

Replace the inline unions in `Unit` and `Player`:

```typescript
export interface Unit {
    id: number;
    shape: UnitKind;
    ...
```

```typescript
export interface Player {
    id: number;
    username: string;
    gold: number;
    income: number;
    king_tier: number;
    family: Family | null;
    spawning_queue: UnitKind[];
    next_send_costs: number[];
}
```

- [ ] **Step 4: Create `view/app_build_ui.ts`**

```typescript
import { BuildCatalogEntry, Family, UnitKind } from './types';

export function renderFamilyOptions(
    container: HTMLElement,
    families: Family[],
    onPick: (family: Family) => void,
): void {
    container.innerHTML = '';
    for (const family of families) {
        const btn = document.createElement('button');
        btn.textContent = `Play ${family}`;
        btn.onclick = () => onPick(family);
        container.appendChild(btn);
    }
}

export function renderBuildShop(
    container: HTMLElement,
    catalog: BuildCatalogEntry[],
    onSelect: (unitKind: UnitKind) => void,
): void {
    container.innerHTML = '';
    for (const entry of catalog) {
        const btn = document.createElement('button');
        btn.textContent = `${entry.name} (${entry.cost}g)`;
        btn.onclick = () => onSelect(entry.unit_kind);
        container.appendChild(btn);
    }
}
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cd view && npx tsx --test test_build_catalog_ui.ts`
Expected: PASS.

- [ ] **Step 6: Wire into `index.html`**

Replace:

```html
            <button id="selectSquare">Square (25g)</button>
            <button id="selectCircle">Circle (75g)</button>
            <button id="selectTriangle">Triangle (40g)</button>
```

with:

```html
            <div id="family-pick"></div>
            <div id="build-shop"></div>
```

- [ ] **Step 7: Wire into `app.ts`**

Add the import:

```typescript
import { renderBuildShop, renderFamilyOptions } from './app_build_ui';
import { BuildCatalogEntry, Family, UnitKind } from './types';
```

Replace `let selectedShape: 'Square' | 'Circle' | 'Triangle' = 'Square';` with `let selectedUnitKind: UnitKind | null = null;` and update every existing reference to `selectedShape` in this file to `selectedUnitKind` (the `Place` click handler and the two `gamePhase === 'GameOver' | 'Victory'` reset blocks that currently do `selectedShape = 'Square';` — reset those to `selectedUnitKind = null;` instead, since there's no default until a catalog arrives).

Remove the three lines:

```typescript
document.getElementById('selectSquare')!.onclick = () => { selectedShape = 'Square'; };
document.getElementById('selectCircle')!.onclick = () => { selectedShape = 'Circle'; };
document.getElementById('selectTriangle')!.onclick = () => { selectedShape = 'Triangle'; };
```

Add, near the other one-time DOM element lookups at the top of the file (alongside `hireWorkerBtn` etc.):

```typescript
const familyPickEl = document.getElementById('family-pick') as HTMLDivElement;
const buildShopEl = document.getElementById('build-shop') as HTMLDivElement;
```

In the WebSocket message-handling switch (find the existing handling for `case 'sendUnitCatalog':`-equivalent — check how the client currently consumes `ServerMessage::SendUnitCatalog` in the `onmessage` handler and match that exact pattern; likely a `switch (msg.type)` on the tagged union), add two new cases:

```typescript
        case 'FamilyOptions': {
            renderFamilyOptions(familyPickEl, msg.data as Family[], (family) => {
                if (socket && socket.readyState === WebSocket.OPEN) {
                    socket.send(JSON.stringify({ action: 'pickFamily', payload: { family } }));
                }
            });
            break;
        }
        case 'BuildCatalog': {
            familyPickEl.innerHTML = '';
            renderBuildShop(buildShopEl, msg.data as BuildCatalogEntry[], (unitKind) => {
                selectedUnitKind = unitKind;
            });
            break;
        }
```

(Match the exact case-label casing and `msg.data` access pattern already used for `SendUnitCatalog` in this switch — do not guess a different shape than what's already established for the other server-driven-catalog message.)

Update the `place` click handler to guard on `selectedUnitKind` being non-null:

```typescript
        case 'cell': {
            panel.clearSelection();
            if (hit.row >= 8) return; // king protection zone — no placement
            if (!selectedUnitKind) return; // no family/tower picked yet
            const placeMessage = { action: 'place', payload: { shape: selectedUnitKind, row: hit.row, col: hit.col } };
            if (socket && socket.readyState === WebSocket.OPEN) socket.send(JSON.stringify(placeMessage));
            return;
        }
```

- [ ] **Step 8: Run the full client test suite and typecheck**

Run: `cd view && npx tsc --noEmit && npx tsx --test *.ts`
Expected: PASS.

- [ ] **Step 9: Commit**

```bash
git add view/types.ts view/index.html view/app.ts view/app_build_ui.ts view/test_build_catalog_ui.ts
git commit -m "feat: add family pick UI and catalog-driven build shop to client"
```

---

### Task 5: Full-suite verification

**Files:** none (verification only)

- [ ] **Step 1: Run the full server test suite**

Run: `cd server && cargo test`
Expected: PASS, zero failures.

- [ ] **Step 2: Run `cargo clippy`**

Run: `cd server && cargo clippy --all-targets -- -D warnings`
Expected: PASS. If `Family` triggers a single-variant-enum lint, that's expected and fine — more variants land in Chunks 3-5.

- [ ] **Step 3: Run the full client suite**

Run: `cd view && npx tsc --noEmit && npx tsx --test *.ts`
Expected: PASS.

- [ ] **Step 4: Manual smoke check**

Use the project's `run` skill to start server + client. Queue two browser tabs into a match. Confirm: (1) neither tab can place a tower until it picks a family, (2) picking "Play Basic" replaces the pick UI with three shop buttons (Square/Triangle/Circle with correct costs), (3) placing towers still works exactly as before, (4) the opponent's family pick becomes visible in that tab's player state (check via a `RequestUnitInfo`-adjacent debug path or by confirming `currentPlayers` includes `family: "Basic"` for the opponent after they pick — a `console.log(currentPlayers)` breakpoint is sufficient for this manual check).

---

## Chunk 2 Definition of Done

- `Shape` no longer exists anywhere in the codebase; `UnitKind` is the type name everywhere, same 3 variants, same wire values.
- `Family` enum exists with one variant (`Basic`) wrapping the existing 3-unit roster; `family_catalog(Family) -> Vec<UnitKind>` is the single source of truth for family→roster, ready for Chunk 3 to add `Family::Ice` as a second arm.
- `Player.family: Option<Family>` is set via `PickFamily`, locked after first pick, and visible to the opponent for free via the existing player-list broadcast.
- `ClientMessage::Place` rejects any `UnitKind` not in the picker's family catalog, and rejects placement entirely with no family picked.
- Client replaces the three hardcoded build buttons with a catalog-driven shop rendered from `ServerMessage::BuildCatalog`, gated behind a family-pick UI rendered from `ServerMessage::FamilyOptions`.
- `cargo test`, `cargo clippy`, and the client test suite are green; manual two-tab smoke test confirms the pick→catalog→shop→place pipeline end to end.
