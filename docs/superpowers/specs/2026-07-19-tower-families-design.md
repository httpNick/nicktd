# Tower Families Design
**Date:** 2026-07-19
**Status:** Approved (brainstorm complete)
**Supersedes:** unit-content portions of `docs/analysis/missing-unit-systems-analysis.md`

## Summary

Replace the three basic shape towers with three thematic tower families — Ice, Poison Beasts, and Elves — chosen per player before the match starts. Each family has three towers, a distinct combat identity, and explicit strengths/weaknesses expressed through a unified attack/defense matrix. This is the "builder race" concept from the missing-unit-systems analysis, scoped to three families of three units.

Delivery is chunked into six independently shippable phases; the game remains playable after every merge.

## Decisions Locked During Brainstorm

1. **Mercenaries stay shared.** All players send the same Scout/Raider/Siege Mage regardless of family. Mercs stay basic; towers are the focus. (Sent-unit catalog unchanged.)
2. **Unified damage/defense matrix.** `DefenseSpecialty` is deleted. Every unit (tower, enemy, merc, king) carries one `DefenseStats` describing armor, magic resist, and per-element resists. Old `Resistances { fire, ice, lightning }` is replaced.
3. **Two-axis damage type.** Damage = school (PhysicalBasic / PhysicalPierce / Magical) × element (None / Fire / Ice / Poison). E.g. elf ranger = PhysicalPierce+None; poison spitter = PhysicalPierce+Poison; frostbolt = Magical+Ice.
4. **Family pick in pre-game phase.** After match found, before wave 1. Locked for the match. Both players may pick the same family. Opponent's pick is visible.
5. **Three towers per family** at cost slots ~25/40/75 (matching today's menu size). Rosters grow later (tier system deferred).
6. **Basic shapes leave the build menu** once all three families ship (end of Chunk 5). Shapes live on as mercenaries only.
7. **Future: family reroll** (Legion TD style). Not built now, but architecture keeps it cheap: `family` is a mutable per-player field, and a placed tower references its own `UnitKind`, never the player's family. Reroll later = swap the shop catalog; existing towers stay.
8. **After towers: unique wave enemies** with their own strengths/weaknesses (Chunk 6), so family choice creates good/bad wave matchups.

Deferred (from analysis doc): tier progression, auras, upgrade paths, active abilities, per-family mercs.

## Family Rosters

Costs use today's three slots (25/40/75 gold). Exact numbers are balance-pass targets, not contracts.

### Ice — control family
Slows everything; kills bosses with combo hits. All damage Magical+Ice; fragile, low armor. Whole family suffers vs ice-resistant or magic-resistant waves.

| Tower | Cost | Attack | Role |
|---|---|---|---|
| Frost Whelp | 25g | Frostbolt, ranged, Magical+Ice | Single-target DPS; each hit applies a stacking slow (~15%/stack, max 3, ~2s, refresh on hit) |
| Blizzard Totem | 40g | AoE pulse, Magical+Ice, short-mid range | Wave clear; damages + light slow to all enemies in radius |
| Ice Lance | 75g | Very long range, slow rate, large hit, Magical+Ice | Boss killer; +50% damage vs slowed targets (combo with Whelp/Totem) |

### Poison Beasts — attrition family
DoT plus sustain; wins long fights. Boss answer: poison stacks keep ticking — long fights favor poison. Weakness: slow against fast/swarm waves; a poison-resistant wave is a hard counter.

| Tower | Cost | Attack | Role |
|---|---|---|---|
| Thorn Beast | 25g | Melee, PhysicalBasic+Poison | Tank; hits apply stacking poison DoT |
| Venom Spitter | 40g | Ranged, PhysicalPierce+Poison | Sustain DPS; lifesteal — self-heals a % of direct-hit damage (not DoT ticks) |
| Plague Cloud | 75g | Lobbed AoE, Magical+Poison | Zone control; leaves a persistent cloud, enemies inside take DoT |

### Elves — precision/support family
Focus fire and healing. Boss answer: Mark amplification multiplies ranger volleys. Weakness: fragile, pure pierce core (armored waves resist), and **no AoE at all** — swarm waves are the elves' nightmare (deliberate, starkest identity).

| Tower | Cost | Attack | Role |
|---|---|---|---|
| Ranger | 25g | Ranged, PhysicalPierce, fast rate | Core DPS; cheap and efficient |
| Healer | 40g | Heal (Lesser Heal, mana-gated) / weak Magical wand attack when OOM | Heals the lowest-HP% allied tower in range on its attack timer, spending mana per heal; out of mana, falls back to a weak Magical wand attack on enemies until mana regenerates (Circle-mage primary/secondary pattern) |
| Spellsinger | 75g | Ranged, Magical | **Mark**: target takes +25% damage from all sources; one mark at a time, newest wins |

**Balance guardrail:** every family must solo-clear generic waves 1–6. Armor-matrix waves (Chunk 6) create matchup tension on top, never below that floor.

## Architecture (server, Bevy ECS)

### Damage and defense model

```rust
// components.rs — replaces flat DamageType, DefenseSpecialty, Resistances
pub enum School { PhysicalBasic, PhysicalPierce, Magical }
pub enum Element { None, Fire, Ice, Poison }
pub struct DamageType { pub school: School, pub element: Element }

pub struct DefenseStats {
    pub armor: f32,        // % mitigation vs Physical* schools
    pub magic_resist: f32, // % mitigation vs Magical school
    pub fire: f32,
    pub ice: f32,
    pub poison: f32,       // per-element resist %, negative = weakness (amplified)
}
```

Damage formula: `dmg × (1 − school_mitigation) × (1 − element_resist)`. Implemented once as a pure function (`damage.rs::apply_damage`, no ECS access); every damage source — direct hit, DoT tick, AoE, king — funnels through it. `DamageType` is serialized to the client, so client attack-type rendering updates with the enum change.

### Unit identity

`Shape` enum is replaced by `UnitKind` (9 family towers + 3 mercs + wave-enemy kinds). `unit_config.rs` becomes a data table: `get_unit_profile(UnitKind) -> UnitProfile` carrying combat profile, defense stats, cost, and ability spec. Mercs are re-keyed with unchanged behavior.

### Status effects engine (buffs AND debuffs)

```rust
pub enum StatusKind { Slow { pct: f32 }, PoisonDot { dps: f32 }, Mark { amp: f32 } /* buffs later ride the same enum */ }
pub struct StatusEffect { kind: StatusKind, remaining: f32, stacks: u8, max_stacks: u8, source: Entity }
#[derive(Component)] pub struct ActiveStatuses(pub Vec<StatusEffect>);
```

- One tick system: decrement timers, apply DoT damage (through `apply_damage`), drop expired effects.
- Consumers read, never write each other: movement reads Slow (speed multiplier); damage calc reads Mark (amplification); DoT ticks credit kills/bounty to `source`'s owner.
- Stack rules: Slow stacks to max and refreshes duration; PoisonDot stacks to max; Mark is single, newest wins.
- Application: `AttackProfile` gains `on_hit: Option<StatusApply>` — combat code stays generic, no per-tower special cases.
- Any entity (tower, enemy, merc, king) may hold `ActiveStatuses`. Positive/negative is derivable from the kind for client display (green vs red icons). Dispel categories: deferred (YAGNI).

### AoE

`AttackProfile` gains `aoe: Option<AoeSpec { radius, falloff }>` for instant splash (Blizzard Totem pulse). Persistent zones (Plague Cloud) are their own entities: `Position + AreaEffect { radius, status, remaining }`, ticked by the status engine.

### Ally targeting (healing)

New system alongside `update_targeting`: healers query allied towers (same player, `Tower`, damaged, in range), pick lowest HP%, heal on their attack timer. Lifesteal reuses the same heal-application function with no targeting.

The Healer is mana-gated using the existing Circle-mage primary/secondary pattern (`CombatProfile { primary, secondary, mana_cost }` + `Mana`): primary = Lesser Heal (costs mana, targets ally); secondary = weak Magical wand attack (no mana, targets enemy). When out of mana it fights instead of idling, and swaps back to healing once mana regenerates. The one extension to the pattern: a primary profile can be a heal (ally-targeted) while the secondary is an attack (enemy-targeted), so target selection must follow the active profile.

### Family selection

`Player.family: Option<Family>`. Pre-game phase gains a pick message + validation; pick broadcast to opponent. Build requests are rejected if the `UnitKind` is not in the player's family catalog. Client shop renders from a server-sent per-family catalog (same pattern as the merc catalog).

### Combat module split

`combat.rs` (2,678 lines) becomes `server/src/handler/combat/`:

```
mod.rs        // re-exports, shared consts (SPEED, leak penalties), get_board
targeting.rs  // update_targeting, update_leaked_creeps, range markers
movement.rs   // update_combat_movement, update_combat_reset
attack.rs     // execute_combat_round, process_combat, update_active_combat_stats, cleanup_dead_entities
damage.rs     // NEW: pure apply_damage(dmg, DamageType, &DefenseStats) -> f32
status.rs     // NEW: ActiveStatuses tick, apply/stack rules
aoe.rs        // NEW: AoE application + persistent zone entities
healing.rs    // NEW: healer targeting + heal application
```

Rules: each file owns its systems and their tests; systems communicate via components only (game_loop schedules them); `mod.rs` re-exports keep external callers unchanged. The split itself is a pure move refactor verified by the existing test suite.

### Concurrency

All new systems run inside the existing game-loop tick under the lobby guard — no new locks, no awaits. The four invariants in CLAUDE.md are untouched.

## Rollout Chunks

Each chunk is its own superpowers spec → plan → implement cycle. Dependencies are linear 0→5; 6 is possible any time after 1 but content-sensible after 5.

**Chunk 0 — combat module split.** Pure refactor per layout above. No behavior change; green test suite = done.

**Chunk 1 — damage matrix foundation.** Two-axis `DamageType`, new `DefenseStats`, delete `DefenseSpecialty` + old `Resistances`, pure `apply_damage`, all damage funneled through it. Existing shapes/mercs/enemies get mapped stats with all-zero resists — gameplay identical, matrix armed but neutral. Client updates for the serialization change.

**Chunk 2 — UnitKind + family selection.** `Shape` → `UnitKind`; unit table; `Family` enum + `Player.family`; pre-game pick message, validation, opponent broadcast; server-sent build catalog; client pick UI + catalog-driven shop. Ships with families = {Basic} (the 3 shapes) to prove plumbing before content. Reroll-ready by construction.

**Chunk 3 — status engine + Ice family.** `ActiveStatuses` + tick system; Slow; stacking/refresh rules; AoE primitive. Ships Frost Whelp, Blizzard Totem, Ice Lance. Ice selectable; Basic remains as fallback. Client: status icons on health bars, AoE visuals.

**Chunk 4 — DoT/lifesteal + Poison family.** PoisonDot (ticks via `apply_damage`, kill credit to source), persistent zone entity, heal-application function, lifesteal. Ships Thorn Beast, Venom Spitter, Plague Cloud.

**Chunk 5 — ally targeting + Elf family.** Healer targeting system, Mark status, damage-calc amplification. Ships Ranger, Healer, Spellsinger. **End of chunk: Basic shapes removed from the build menu**; three real families live.

**Chunk 6 — armor matrix activation (waves).** Wave enemies get real per-wave `DefenseStats` (ice-resistant wave, armored wave, …). Balance pass against the solo-clear guardrail. Gets its own brainstorm/spec when reached — wave design is its own domain.
