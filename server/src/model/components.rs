use bevy_ecs::prelude::{Component, Entity};
use serde::{Deserialize, Serialize};

use super::shape::Shape;

#[derive(Component, Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Position {
    pub x: f32,
    pub y: f32,
}

#[derive(Component)]
pub struct Target(pub Entity);

#[derive(Component)]
pub struct ShapeComponent(pub Shape);

#[derive(Component)]
pub struct PlayerIdComponent(pub i64);

#[derive(Component)]
pub struct Enemy;

#[derive(Component)]
pub struct Worker;

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
pub enum DamageType {
    PhysicalPierce,
    PhysicalBasic,
    FireMagical,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Resistances {
    pub fire: f32,
    pub ice: f32,
    pub lightning: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(dead_code)]
pub enum DefenseSpecialty {
    None,
    Armored,
    MagicResistant,
    RangeResistant,
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

#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct DefenseStats {
    pub armor: f32,
    pub resistances: Resistances,
    pub specialty: DefenseSpecialty,
}

#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Mana {
    pub current: f32,
    pub max: f32,
    pub regen: f32,
}

#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct AttackTimer(pub f32);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn physical_components_exist() {
        let _ = CollisionRadius(5.0);
        let _ = AttackRange(20.0);
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
            damage_type: DamageType::PhysicalBasic,
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
        let _ = DamageType::PhysicalPierce;
        let _ = DamageType::PhysicalBasic;
        let _ = DamageType::FireMagical;

        let _ = DefenseSpecialty::None;
        let _ = DefenseSpecialty::Armored;
        let _ = Resistances { fire: 0.0, ice: 0.0, lightning: 0.0 };
    }

    #[test]
    fn attack_stats_includes_damage_type() {
        let stats = AttackStats {
            damage: 10.0,
            rate: 1.5,
            damage_type: DamageType::PhysicalBasic,
        };
        assert_eq!(stats.damage_type, DamageType::PhysicalBasic);
    }

    #[test]
    fn defense_stats_component_works() {
        let stats = DefenseStats {
            armor: 5.0,
            resistances: Resistances { fire: 10.0, ice: 0.0, lightning: 0.0 },
            specialty: DefenseSpecialty::Armored,
        };
        assert_eq!(stats.armor, 5.0);
        assert_eq!(stats.resistances.fire, 10.0);
        assert_eq!(stats.specialty, DefenseSpecialty::Armored);
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
