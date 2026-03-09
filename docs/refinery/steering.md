# Refinery Agent Steering

Context for background agents operating on this repository.

## Project

Nick's Tower Defense (`nicktd`) — multiplayer web-based tower defense. Rust server (Bevy ECS game logic, Tokio async runtime, SQLite via SQLx); TypeScript/Vite frontend.

## Repository Layout

```
server/src/
  model/
    unit_config.rs  ← balance constants and unit profile factory
    components.rs   - ECS component structs (AttackProfile, CombatProfile, Mana, etc.)
    shape.rs        - Shape enum: Circle | Square | Triangle
  handler/
    spawn.rs        - unit and enemy spawning (consumes unit_config constants)
view/               - TypeScript frontend (Vite)
docs/refinery/      - agent documentation and skills
```

## Unit System

Three tower types, each identified by a `Shape` variant:

| Shape    | Role       | Primary Damage Type | Attack Range      | Gold Cost |
|----------|------------|---------------------|-------------------|-----------|
| Square   | Melee      | PhysicalBasic       | Short (45)        | 25        |
| Triangle | Ranged DPS | PhysicalPierce      | Long (150)        | 40        |
| Circle   | Mage       | FireMagical         | Long (150) + mana | 75        |

Circle (mage) also has a melee secondary attack (`PhysicalBasic`, `MAGE_MELEE_DAMAGE`) used when mana is below the fireball cost.

Enemy health is based on `DEFAULT_HEALTH`, scaled by `get_scaling_multiplier(wave)` in `spawn.rs`. Wave 6 is a boss wave with `BOSS_HEALTH_MULTIPLIER` and `BOSS_DAMAGE_MULTIPLIER` applied on top.

## Balance Constants

All tunable numbers are `pub const` at the top of `server/src/model/unit_config.rs`:

```rust
DEFAULT_COLLISION_RADIUS: f32 = 20.0   // base entity collision radius
DEFAULT_ATTACK_RANGE: f32    = 45.0    // melee attack range (Square)
RANGED_ATTACK_RANGE: f32     = 150.0   // ranged attack range (Triangle and Circle primary)
DEFAULT_HEALTH: f32          = 100.0   // base enemy health (before wave scaling)
DEFAULT_DAMAGE: f32          = 10.0    // primary damage shared across all three shapes
DEFAULT_ATTACK_RATE: f32     = 0.8     // seconds between attacks (shared across shapes)
FIREBALL_MANA_COST: f32      = 20.0    // mana cost per Circle fireball cast
MAGE_MELEE_DAMAGE: f32       = 2.0     // Circle secondary melee damage
MAGE_MANA_MAX: f32           = 100.0   // Circle max mana pool
MAGE_MANA_REGEN: f32         = 5.0     // Circle mana regeneration per tick
BOSS_HEALTH_MULTIPLIER: f32  = 10.0    // wave 6 boss HP multiplier
BOSS_DAMAGE_MULTIPLIER: f32  = 3.0     // wave 6 boss damage multiplier
```

Gold costs are inline in the `match shape` block (not constants):
- Square: 25, Triangle: 40, Circle: 75

## Making Changes

`get_unit_profile(shape: Shape) -> UnitProfile` assembles each tower's profile from the constants above. Because several constants are **shared across all shapes** (e.g., `DEFAULT_DAMAGE`), modifying a shared const changes all shapes at once.

**To change only one shape's stat**, add a new shape-specific constant derived from the shared one:

```rust
// Add at the top of unit_config.rs:
pub const CIRCLE_PRIMARY_DAMAGE: f32 = DEFAULT_DAMAGE * 1.1;

// Then in get_unit_profile, update the Shape::Circle match arm:
damage: CIRCLE_PRIMARY_DAMAGE,
```

Never hardcode magic numbers directly in the `match` arms — always express them as named `pub const` values.

## Running Tests

```bash
cd server && cargo test
```

Tests live in `#[cfg(test)]` modules within each source file. Key tests:
- `unit_config.rs` — `unit_profiles_have_gold_costs`: asserts gold costs per shape
- `components.rs` — asserts component construction and field values

After any change, all tests must pass. If a test assertion references a value you intentionally changed (e.g., a gold cost), update the assertion to the new expected value.

Also run the formatter after editing Rust files:

```bash
cd server && cargo fmt
```

## Code Conventions

- New tunable values: `pub const NAME: f32 = value;` in `SCREAMING_SNAKE_CASE`
- Derive per-shape constants using const arithmetic (`DEFAULT_DAMAGE * 1.1`) rather than raw literals
- Keep the `pub const` block at the top of `unit_config.rs`; do not scatter constants into the match arms
