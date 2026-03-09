---
name: unit-refiner
description: Update unit balance stats in the tower defense game. Use when a command requests buffing, nerfing, or adjusting any tower or enemy stat — damage, attack rate, range, gold cost, health, mana, boss multipliers — by a percentage or flat amount. Examples: "make Circle towers deal 10% more damage", "increase Square gold cost to 30", "reduce Triangle attack range by 20", "buff all tower damage by 5%".
---

# Unit Refiner

Modify balance values in `server/src/model/unit_config.rs` and verify with tests.

## Workflow

### 1. Load context

Read `docs/refinery/steering.md` for project layout, constant descriptions, and the per-shape isolation pattern.
Read `server/src/model/unit_config.rs` to see current values before making any change.

### 2. Identify the target constant

| Request targets             | Where to change                                          |
|-----------------------------|----------------------------------------------------------|
| All towers' primary damage  | `DEFAULT_DAMAGE`                                         |
| One shape's primary damage  | Add new `SHAPE_PRIMARY_DAMAGE` const (see steering)      |
| Circle secondary (melee)    | `MAGE_MELEE_DAMAGE`                                      |
| Melee range (Square)        | `DEFAULT_ATTACK_RANGE`                                   |
| Ranged range (Triangle/Circle) | `RANGED_ATTACK_RANGE`                                 |
| Attack speed (all shapes)   | `DEFAULT_ATTACK_RATE` (seconds between attacks)          |
| Gold cost (one shape)       | Inline `match shape` block in `get_unit_profile`         |
| Enemy base health           | `DEFAULT_HEALTH`                                         |
| Circle mana pool            | `MAGE_MANA_MAX`                                          |
| Circle mana regen           | `MAGE_MANA_REGEN`                                        |
| Fireball mana cost          | `FIREBALL_MANA_COST`                                     |
| Boss stats                  | `BOSS_HEALTH_MULTIPLIER`, `BOSS_DAMAGE_MULTIPLIER`       |

If only one shape should be affected but the constant is currently shared, add a new shape-specific constant — see the example in `docs/refinery/steering.md`.

### 3. Compute the new value

- Percentage increase: `new = current * (1.0 + pct / 100.0)`
- Percentage decrease: `new = current * (1.0 - pct / 100.0)`
- Flat change: `new = current + amount`
- Prefer const arithmetic (`DEFAULT_DAMAGE * 1.1`) over raw literals.

### 4. Apply the change

Edit `server/src/model/unit_config.rs`:

1. Update the existing `pub const` value, or add a new one at the top of the const block.
2. If adding a per-shape const, update the corresponding `Shape::X` arm in `get_unit_profile` to reference it.
3. For gold cost changes, update the `match shape { Shape::X => N, ... }` block.

### 5. Validate

```bash
cd server && cargo fmt && cargo test
```

All tests must pass. If a test asserts an old value that you intentionally changed (e.g., `unit_profiles_have_gold_costs`), update the assertion to the new expected value.

### 6. Report

State: what constant changed, old value → new value, and confirm tests passed.
