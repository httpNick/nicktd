pub const KING_BASE_HP: f32 = 400.0;
pub const KING_BASE_DAMAGE: f32 = 15.0;
pub const KING_BASE_RATE: f32 = 0.5;
pub const KING_BASE_RANGE: f32 = 200.0;
pub const KING_REGEN_PER_WAVE: f32 = 20.0;
pub const KING_COLLISION_RADIUS: f32 = 30.0;

pub struct KingUpgradeTier {
    pub cost: u32,
    pub hp_delta: f32,
    pub new_damage: f32,
    pub income_delta: u32,
}

pub const KING_UPGRADE_TIERS: [KingUpgradeTier; 4] = [
    KingUpgradeTier {
        cost: 75,
        hp_delta: 100.0,
        new_damage: 20.0,
        income_delta: 2,
    },
    KingUpgradeTier {
        cost: 100,
        hp_delta: 150.0,
        new_damage: 25.0,
        income_delta: 3,
    },
    KingUpgradeTier {
        cost: 150,
        hp_delta: 250.0,
        new_damage: 30.0,
        income_delta: 4,
    },
    KingUpgradeTier {
        cost: 200,
        hp_delta: 350.0,
        new_damage: 35.0,
        income_delta: 5,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn king_base_stats_are_correct() {
        assert_eq!(KING_BASE_HP, 400.0);
        assert_eq!(KING_BASE_DAMAGE, 15.0);
        assert_eq!(KING_BASE_RATE, 0.5);
        assert_eq!(KING_BASE_RANGE, 200.0);
        assert_eq!(KING_REGEN_PER_WAVE, 20.0);
        assert_eq!(KING_COLLISION_RADIUS, 30.0);
    }

    #[test]
    fn king_upgrade_tiers_have_correct_values() {
        assert_eq!(KING_UPGRADE_TIERS[0].cost, 75);
        assert_eq!(KING_UPGRADE_TIERS[0].hp_delta, 100.0);
        assert_eq!(KING_UPGRADE_TIERS[0].new_damage, 20.0);
        assert_eq!(KING_UPGRADE_TIERS[0].income_delta, 2);

        assert_eq!(KING_UPGRADE_TIERS[1].cost, 100);
        assert_eq!(KING_UPGRADE_TIERS[1].hp_delta, 150.0);
        assert_eq!(KING_UPGRADE_TIERS[1].new_damage, 25.0);
        assert_eq!(KING_UPGRADE_TIERS[1].income_delta, 3);

        assert_eq!(KING_UPGRADE_TIERS[2].cost, 150);
        assert_eq!(KING_UPGRADE_TIERS[2].hp_delta, 250.0);
        assert_eq!(KING_UPGRADE_TIERS[2].new_damage, 30.0);
        assert_eq!(KING_UPGRADE_TIERS[2].income_delta, 4);

        assert_eq!(KING_UPGRADE_TIERS[3].cost, 200);
        assert_eq!(KING_UPGRADE_TIERS[3].hp_delta, 350.0);
        assert_eq!(KING_UPGRADE_TIERS[3].new_damage, 35.0);
        assert_eq!(KING_UPGRADE_TIERS[3].income_delta, 5);
    }

    #[test]
    fn king_upgrade_tiers_count_is_four() {
        assert_eq!(KING_UPGRADE_TIERS.len(), 4);
    }
}
