use bevy_ecs::prelude::{Component, Entity};
use serde::{Deserialize, Serialize};

use super::shape::Shape;

#[derive(Component, Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Position {
    pub x: f32,
    pub y: f32,
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct HomePosition(pub Position);

#[derive(Component)]
pub struct Target(pub Entity);

#[derive(Component)]
pub struct ShapeComponent(pub Shape);

#[derive(Component)]
pub struct PlayerIdComponent(pub i64);

#[derive(Component)]
pub struct Enemy;

#[derive(Component)]
pub struct King;

#[derive(Component)]
pub struct Boss;

#[derive(Component)]
pub struct Worker;

/// Marks a player-placed defensive unit (tower). Used to query player
/// structures directly instead of excluding every other entity kind.
#[derive(Component)]
pub struct Tower;

#[derive(Component)]
pub struct Dead;

#[derive(Component, Debug, Clone, Copy)]
pub struct TargetPositions {
    pub vein: Position,
    pub cart: Position,
}

#[derive(Component, Debug, PartialEq)]
pub enum WorkerState {
    MovingToVein,
    Mining,
    MovingToCart,
}

#[derive(Component)]
pub struct MiningTimer(pub f32);

#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct CollisionRadius(pub f32);

#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct AttackRange(pub f32);

#[derive(Component)]
pub struct InAttackRange;

#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Health {
    pub current: f32,
    pub max: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum School {
    PhysicalBasic,
    PhysicalPierce,
    Magical,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Element {
    None,
    Fire,
    Ice,
    Poison,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DamageType {
    pub school: School,
    pub element: Element,
}

impl DamageType {
    pub const PHYSICAL_BASIC: DamageType = DamageType {
        school: School::PhysicalBasic,
        element: Element::None,
    };
    pub const PHYSICAL_PIERCE: DamageType = DamageType {
        school: School::PhysicalPierce,
        element: Element::None,
    };
    pub const FIRE_MAGICAL: DamageType = DamageType {
        school: School::Magical,
        element: Element::Fire,
    };
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AttackProfile {
    pub damage: f32,
    pub rate: f32,
    pub range: f32,
    pub damage_type: DamageType,
}

#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct CombatProfile {
    pub primary: AttackProfile,
    pub secondary: Option<AttackProfile>,
    pub mana_cost: f32,
}

#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct AttackStats {
    pub damage: f32,
    pub rate: f32,
    pub damage_type: DamageType,
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Default)]
pub struct DefenseStats {
    pub armor: f32,
    pub magic_resist: f32,
    pub fire: f32,
    pub ice: f32,
    pub poison: f32,
}

#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Mana {
    pub current: f32,
    pub max: f32,
    pub regen: f32,
}

/// Tags a player-sent enemy with the gold reward the defending player receives on kill.
#[derive(Component, Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bounty(pub u32);

#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct AttackTimer(pub f32);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounty_component_works() {
        let bounty = Bounty(50);
        assert_eq!(bounty.0, 50);
    }

    #[test]
    fn physical_components_exist() {
        let _ = CollisionRadius(5.0);
        let _ = AttackRange(20.0);
    }

    #[test]
    fn dead_marker_component_exists() {
        let _ = Dead;
    }

    #[test]
    fn worker_components_exist() {
        let _ = Worker;
        let _ = WorkerState::MovingToVein;
        let _ = MiningTimer(10.0);
    }

    #[test]
    fn health_component_works() {
        let health = Health {
            current: 50.0,
            max: 100.0,
        };
        assert_eq!(health.current, 50.0);
        assert_eq!(health.max, 100.0);
    }

    #[test]
    fn attack_stats_component_works() {
        let stats = AttackStats {
            damage: 10.0,
            rate: 1.5,
            damage_type: DamageType::PHYSICAL_BASIC,
        };
        assert_eq!(stats.damage, 10.0);
        assert_eq!(stats.rate, 1.5);
    }

    #[test]
    fn attack_timer_component_works() {
        let timer = AttackTimer(0.5);
        assert_eq!(timer.0, 0.5);
    }

    #[test]
    fn damage_and_defense_types_exist() {
        let _ = DamageType::PHYSICAL_PIERCE;
        let _ = DamageType::PHYSICAL_BASIC;
        let _ = DamageType::FIRE_MAGICAL;
        assert_eq!(DamageType::FIRE_MAGICAL.school, School::Magical);
        assert_eq!(DamageType::FIRE_MAGICAL.element, Element::Fire);
        assert_eq!(DamageType::PHYSICAL_BASIC.element, Element::None);
    }

    #[test]
    fn attack_stats_includes_damage_type() {
        let stats = AttackStats {
            damage: 10.0,
            rate: 1.5,
            damage_type: DamageType::PHYSICAL_BASIC,
        };
        assert_eq!(stats.damage_type, DamageType::PHYSICAL_BASIC);
    }

    #[test]
    fn defense_stats_component_works() {
        let stats = DefenseStats {
            armor: 5.0,
            magic_resist: 0.0,
            fire: 10.0,
            ice: 0.0,
            poison: 0.0,
        };
        assert_eq!(stats.armor, 5.0);
        assert_eq!(stats.fire, 10.0);
    }

    #[test]
    fn defense_stats_default_is_all_zero() {
        let stats = DefenseStats::default();
        assert_eq!(stats.armor, 0.0);
        assert_eq!(stats.magic_resist, 0.0);
        assert_eq!(stats.fire, 0.0);
        assert_eq!(stats.ice, 0.0);
        assert_eq!(stats.poison, 0.0);
    }

    #[test]
    fn mana_component_works() {
        let mana = Mana {
            current: 50.0,
            max: 100.0,
            regen: 1.0,
        };
        assert_eq!(mana.current, 50.0);
        assert_eq!(mana.max, 100.0);
        assert_eq!(mana.regen, 1.0);
    }
}
