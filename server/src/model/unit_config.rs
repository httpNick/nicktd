use super::components::{AttackProfile, CombatProfile, DamageType, Mana};
use super::shape::Shape;

// --- Balance Constants ---
pub const DEFAULT_COLLISION_RADIUS: f32 = 20.0;
pub const DEFAULT_ATTACK_RANGE: f32 = 45.0;
pub const FIREBALL_MANA_COST: f32 = 20.0;
pub const MAGE_MELEE_DAMAGE: f32 = 2.0;

pub const DEFAULT_HEALTH: f32 = 100.0;
pub const DEFAULT_DAMAGE: f32 = 10.0;
pub const DEFAULT_ATTACK_RATE: f32 = 1.0;
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

#[cfg(test)]
mod tests {
    use super::*;

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
