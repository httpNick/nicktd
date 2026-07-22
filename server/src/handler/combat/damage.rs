use crate::model::components::{DamageType, DefenseStats, Element, School};

/// Mitigates raw damage through defense: school mitigation (armor for
/// Physical*, magic_resist for Magical) then element resist. Both factors
/// are `1 − value`, so a resist of 1.0 fully blocks that channel and a
/// negative resist (a weakness) amplifies damage instead.
pub fn apply_damage(damage: f32, damage_type: DamageType, defense: &DefenseStats) -> f32 {
    let school_mitigation = match damage_type.school {
        School::PhysicalBasic | School::PhysicalPierce => defense.armor,
        School::Magical => defense.magic_resist,
    };
    let element_resist = match damage_type.element {
        Element::None => 0.0,
        Element::Fire => defense.fire,
        Element::Ice => defense.ice,
        Element::Poison => defense.poison,
    };
    damage * (1.0 - school_mitigation) * (1.0 - element_resist)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neutral_defense_leaves_damage_unchanged() {
        let neutral = DefenseStats::default();
        assert_eq!(apply_damage(10.0, DamageType::PHYSICAL_BASIC, &neutral), 10.0);
        assert_eq!(apply_damage(10.0, DamageType::PHYSICAL_PIERCE, &neutral), 10.0);
        assert_eq!(apply_damage(10.0, DamageType::FIRE_MAGICAL, &neutral), 10.0);
    }

    #[test]
    fn armor_mitigates_physical_but_not_magical() {
        let defense = DefenseStats {
            armor: 0.5,
            ..Default::default()
        };
        assert_eq!(apply_damage(10.0, DamageType::PHYSICAL_BASIC, &defense), 5.0);
        assert_eq!(apply_damage(10.0, DamageType::PHYSICAL_PIERCE, &defense), 5.0);
        assert_eq!(apply_damage(10.0, DamageType::FIRE_MAGICAL, &defense), 10.0);
    }

    #[test]
    fn magic_resist_mitigates_magical_but_not_physical() {
        let defense = DefenseStats {
            magic_resist: 0.5,
            ..Default::default()
        };
        assert_eq!(apply_damage(10.0, DamageType::FIRE_MAGICAL, &defense), 5.0);
        assert_eq!(apply_damage(10.0, DamageType::PHYSICAL_BASIC, &defense), 10.0);
    }

    #[test]
    fn element_resist_stacks_with_school_mitigation() {
        let defense = DefenseStats {
            magic_resist: 0.5,
            fire: 0.5,
            ..Default::default()
        };
        // 10 * (1 - 0.5) * (1 - 0.5) = 2.5
        assert_eq!(apply_damage(10.0, DamageType::FIRE_MAGICAL, &defense), 2.5);
    }

    #[test]
    fn negative_element_resist_is_a_weakness_that_amplifies() {
        let defense = DefenseStats {
            fire: -0.5,
            ..Default::default()
        };
        // 10 * (1 - 0) * (1 - (-0.5)) = 15
        assert_eq!(apply_damage(10.0, DamageType::FIRE_MAGICAL, &defense), 15.0);
    }
}
