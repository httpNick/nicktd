use super::components::{AttackProfile, CombatProfile, DamageType, Mana};
use super::shape::Shape;

// --- Balance Constants ---
pub const DEFAULT_COLLISION_RADIUS: f32 = 20.0;
pub const DEFAULT_ATTACK_RANGE: f32 = 45.0;
pub const FIREBALL_MANA_COST: f32 = 20.0;
pub const MAGE_MELEE_DAMAGE: f32 = 2.2; // Increased by 10% from 2.0

pub const DEFAULT_HEALTH: f32 = 100.0;
pub const DEFAULT_DAMAGE: f32 = 10.0;
pub const DEFAULT_ATTACK_RATE: f32 = 0.8;
pub const RANGED_ATTACK_RANGE: f32 = 150.0;
pub const MAGE_MANA_MAX: f32 = 100.0;
pub const MAGE_MANA_REGEN: f32 = 5.0;

pub const BOSS_HEALTH_MULTIPLIER: f32 = 10.0;
pub const BOSS_DAMAGE_MULTIPLIER: f32 = 3.0;

// --- Profile Structures ---
pub struct UnitProfile {
    pub radius: f32,
    pub combat: CombatProfile,
    pub mana: Option<Mana>,
    pub gold_cost: u32,
}

pub fn get_unit_profile(shape: Shape) -> UnitProfile {
    let radius = match shape {
        Shape::Square => DEFAULT_COLLISION_RADIUS + 2.0,
        Shape::Circle => DEFAULT_COLLISION_RADIUS,
        Shape::Triangle => DEFAULT_COLLISION_RADIUS - 2.0,
    };

    let gold_cost = match shape {
        Shape::Square => 25,
        Shape::Triangle => 40,
        Shape::Circle => 75,
    };

    let combat = match shape {
        Shape::Triangle => CombatProfile {
            primary: AttackProfile {
                damage: DEFAULT_DAMAGE,
                rate: DEFAULT_ATTACK_RATE,
                range: RANGED_ATTACK_RANGE,
                damage_type: DamageType::PhysicalPierce,
            },
            secondary: None,
            mana_cost: 0.0,
        },
        Shape::Square => CombatProfile {
            primary: AttackProfile {
                damage: DEFAULT_DAMAGE,
                rate: DEFAULT_ATTACK_RATE,
                range: DEFAULT_ATTACK_RANGE,
                damage_type: DamageType::PhysicalBasic,
            },
            secondary: None,
            mana_cost: 0.0,
        },
        Shape::Circle => CombatProfile {
            primary: AttackProfile {
                damage: DEFAULT_DAMAGE,
                rate: DEFAULT_ATTACK_RATE,
                range: RANGED_ATTACK_RANGE,
                damage_type: DamageType::FireMagical,
            },
            secondary: Some(AttackProfile {
                damage: MAGE_MELEE_DAMAGE,
                rate: DEFAULT_ATTACK_RATE,
                range: DEFAULT_ATTACK_RANGE,
                damage_type: DamageType::PhysicalBasic,
            }),
            mana_cost: FIREBALL_MANA_COST,
        },
    };

    let mana = if shape == Shape::Circle {
        Some(Mana {
            current: MAGE_MANA_MAX,
            max: MAGE_MANA_MAX,
            regen: MAGE_MANA_REGEN,
        })
    } else {
        None
    };

    UnitProfile {
        radius,
        combat,
        mana,
        gold_cost,
    }
}

// --- Sent Unit Balance Constants ---

/// Gold cost to send each unit type.
pub const SENT_SQUARE_COST: u32 = 5;
pub const SENT_TRIANGLE_COST: u32 = 20;
pub const SENT_CIRCLE_COST: u32 = 50;

/// Permanent income added to the sender per round after purchase.
pub const SENT_SQUARE_INCOME: u32 = 1;
pub const SENT_TRIANGLE_INCOME: u32 = 3;
pub const SENT_CIRCLE_INCOME: u32 = 7;

/// Gold bounty awarded to the defending player on kill.
pub const SENT_SQUARE_BOUNTY: u32 = 2;
pub const SENT_TRIANGLE_BOUNTY: u32 = 8;
pub const SENT_CIRCLE_BOUNTY: u32 = 20;

/// Health multiplier applied on top of DEFAULT_HEALTH when spawned.
pub const SENT_SQUARE_HEALTH_MULT: f32 = 1.0;
pub const SENT_TRIANGLE_HEALTH_MULT: f32 = 1.2;
pub const SENT_CIRCLE_HEALTH_MULT: f32 = 1.5;

/// Static profile for a unit a player can send to the opponent's board.
#[derive(Debug, Clone, PartialEq)]
pub struct SentUnitProfile {
    /// Display name shown in the Mercenary Panel.
    pub name: &'static str,
    /// Gold spent by the sending player to queue this unit.
    pub send_cost: u32,
    /// Permanent income added to the sender each round after purchase.
    pub income: u32,
    /// Gold awarded to the defending player when this unit is killed.
    pub bounty: u32,
    /// Multiplier applied to `DEFAULT_HEALTH` when the unit spawns.
    pub health_multiplier: f32,
}

/// Returns the balance profile for a player-sent unit of the given shape.
pub fn get_sent_unit_profile(shape: Shape) -> SentUnitProfile {
    match shape {
        Shape::Square => SentUnitProfile {
            name: "Scout",
            send_cost: SENT_SQUARE_COST,
            income: SENT_SQUARE_INCOME,
            bounty: SENT_SQUARE_BOUNTY,
            health_multiplier: SENT_SQUARE_HEALTH_MULT,
        },
        Shape::Triangle => SentUnitProfile {
            name: "Raider",
            send_cost: SENT_TRIANGLE_COST,
            income: SENT_TRIANGLE_INCOME,
            bounty: SENT_TRIANGLE_BOUNTY,
            health_multiplier: SENT_TRIANGLE_HEALTH_MULT,
        },
        Shape::Circle => SentUnitProfile {
            name: "Siege Mage",
            send_cost: SENT_CIRCLE_COST,
            income: SENT_CIRCLE_INCOME,
            bounty: SENT_CIRCLE_BOUNTY,
            health_multiplier: SENT_CIRCLE_HEALTH_MULT,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Task 1.2 TDD: sent unit profiles ---

    #[test]
    fn sent_unit_profiles_exist_for_all_shapes() {
        let _ = get_sent_unit_profile(Shape::Square);
        let _ = get_sent_unit_profile(Shape::Triangle);
        let _ = get_sent_unit_profile(Shape::Circle);
    }

    #[test]
    fn sent_unit_profiles_have_unique_names() {
        let square = get_sent_unit_profile(Shape::Square);
        let triangle = get_sent_unit_profile(Shape::Triangle);
        let circle = get_sent_unit_profile(Shape::Circle);
        assert_ne!(square.name, triangle.name);
        assert_ne!(triangle.name, circle.name);
        assert_ne!(square.name, circle.name);
    }

    #[test]
    fn all_sent_unit_bounties_are_nonzero() {
        for shape in [Shape::Square, Shape::Triangle, Shape::Circle] {
            let p = get_sent_unit_profile(shape);
            assert!(p.bounty > 0, "{} bounty must be > 0", p.name);
        }
    }

    #[test]
    fn sent_square_has_best_income_per_gold_ratio() {
        let square = get_sent_unit_profile(Shape::Square);
        let triangle = get_sent_unit_profile(Shape::Triangle);
        let circle = get_sent_unit_profile(Shape::Circle);
        let sq_ratio = square.income as f32 / square.send_cost as f32;
        let tr_ratio = triangle.income as f32 / triangle.send_cost as f32;
        let ci_ratio = circle.income as f32 / circle.send_cost as f32;
        assert!(
            sq_ratio >= tr_ratio && sq_ratio >= ci_ratio,
            "Square should have the best income-per-gold ratio"
        );
    }

    #[test]
    fn sent_circle_has_highest_health_multiplier() {
        let square = get_sent_unit_profile(Shape::Square);
        let triangle = get_sent_unit_profile(Shape::Triangle);
        let circle = get_sent_unit_profile(Shape::Circle);
        assert!(circle.health_multiplier > triangle.health_multiplier);
        assert!(triangle.health_multiplier > square.health_multiplier);
    }

    #[test]
    fn unit_profiles_have_gold_costs() {
        let square = get_unit_profile(Shape::Square);
        let triangle = get_unit_profile(Shape::Triangle);
        let circle = get_unit_profile(Shape::Circle);

        // Expected costs: Square (25), Triangle (40), Circle (75)
        // These will fail to compile initially because gold_cost field is missing
        assert_eq!(square.gold_cost, 25);
        assert_eq!(triangle.gold_cost, 40);
        assert_eq!(circle.gold_cost, 75);
    }
}
