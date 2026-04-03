use crate::model::components::{
    AttackRange, AttackStats, AttackTimer, Boss, Bounty, CollisionRadius, DefenseSpecialty,
    DefenseStats, Enemy, Health, HomePosition, King, PlayerIdComponent, Position, Resistances,
    ShapeComponent, TargetPositions, Worker, WorkerState,
};
use crate::model::king_config::{
    KING_BASE_DAMAGE, KING_BASE_HP, KING_BASE_RANGE, KING_BASE_RATE, KING_COLLISION_RADIUS,
};
use crate::model::shape::Shape;
use crate::model::unit_config::{
    BOSS_DAMAGE_MULTIPLIER, BOSS_HEALTH_MULTIPLIER, DEFAULT_COLLISION_RADIUS, DEFAULT_HEALTH,
    get_sent_unit_profile, get_unit_profile,
};
use bevy_ecs::prelude::{Entity, World};

pub fn spawn_enemy(world: &mut World, pos: Position, shape: Shape, wave: u32) -> Entity {
    let profile = get_unit_profile(shape);
    let scaling_multiplier = crate::handler::wave::get_scaling_multiplier(wave);

    let is_boss = wave == 6;
    let (hp_multiplier, damage_multiplier) = if is_boss {
        (BOSS_HEALTH_MULTIPLIER, BOSS_DAMAGE_MULTIPLIER)
    } else {
        (1.0, 1.0)
    };

    let final_health = DEFAULT_HEALTH * scaling_multiplier * hp_multiplier;
    let final_damage = profile.combat.primary.damage * scaling_multiplier * damage_multiplier;

    let mut entity = world.spawn((
        pos,
        HomePosition(pos),
        ShapeComponent(shape),
        Enemy,
        CollisionRadius(profile.radius),
        AttackRange(profile.combat.primary.range),
        Health {
            current: final_health,
            max: final_health,
        },
        AttackStats {
            damage: final_damage,
            rate: profile.combat.primary.rate,
            damage_type: profile.combat.primary.damage_type,
        },
        profile.combat,
        DefenseStats {
            armor: 0.0,
            resistances: Resistances {
                fire: 0.0,
                ice: 0.0,
                lightning: 0.0,
            },
            specialty: DefenseSpecialty::None,
        },
        AttackTimer(0.0),
    ));

    if is_boss {
        entity.insert(Boss);
    }

    if let Some(mana) = profile.mana {
        entity.insert(mana);
    }

    entity.id()
}

/// Spawns a player-sent enemy on the opponent's board.
///
/// Applies the sent unit's health multiplier on top of wave scaling, and attaches
/// a [`Bounty`] component so the defending player receives gold on kill.
pub fn spawn_sent_enemy(
    world: &mut World,
    pos: Position,
    shape: Shape,
    wave: u32,
    bounty: u32,
) -> Entity {
    let profile = get_unit_profile(shape);
    let sent_profile = get_sent_unit_profile(shape);
    let scaling_multiplier = crate::handler::wave::get_scaling_multiplier(wave);

    let final_health = DEFAULT_HEALTH * scaling_multiplier * sent_profile.health_multiplier;
    let final_damage = profile.combat.primary.damage * scaling_multiplier;

    world
        .spawn((
            pos,
            HomePosition(pos),
            ShapeComponent(shape),
            Enemy,
            CollisionRadius(profile.radius),
            AttackRange(profile.combat.primary.range),
            Health {
                current: final_health,
                max: final_health,
            },
            AttackStats {
                damage: final_damage,
                rate: profile.combat.primary.rate,
                damage_type: profile.combat.primary.damage_type,
            },
            profile.combat,
            DefenseStats {
                armor: 0.0,
                resistances: Resistances {
                    fire: 0.0,
                    ice: 0.0,
                    lightning: 0.0,
                },
                specialty: DefenseSpecialty::None,
            },
            AttackTimer(0.0),
            Bounty(bounty),
        ))
        .id()
}

pub fn spawn_unit(world: &mut World, pos: Position, shape: Shape, player_id: i64) -> Entity {
    let profile = get_unit_profile(shape);
    let mut entity = world.spawn((
        pos,
        HomePosition(pos),
        ShapeComponent(shape),
        PlayerIdComponent(player_id),
        CollisionRadius(profile.radius),
        AttackRange(profile.combat.primary.range),
        Health {
            current: DEFAULT_HEALTH,
            max: DEFAULT_HEALTH,
        },
        AttackStats {
            damage: profile.combat.primary.damage,
            rate: profile.combat.primary.rate,
            damage_type: profile.combat.primary.damage_type,
        },
        profile.combat,
        DefenseStats {
            armor: 0.0,
            resistances: Resistances {
                fire: 0.0,
                ice: 0.0,
                lightning: 0.0,
            },
            specialty: DefenseSpecialty::None,
        },
        AttackTimer(0.0),
    ));

    if let Some(mana) = profile.mana {
        entity.insert(mana);
    }

    entity.id()
}

/// Spawns the King entity for the given player on the specified board.
///
/// `board_idx 0` → left board position; `board_idx 1` → right board position.
/// The King has no `HomePosition` or `Enemy` — it persists through wave resets
/// and is not targeted by towers.
pub fn spawn_king(world: &mut World, player_id: i64, board_idx: usize) -> Entity {
    use crate::model::components::DamageType;
    use crate::model::constants::{KING_LEFT_X, KING_RIGHT_X, KING_Y};

    let pos = if board_idx == 0 {
        Position {
            x: KING_LEFT_X,
            y: KING_Y,
        }
    } else {
        Position {
            x: KING_RIGHT_X,
            y: KING_Y,
        }
    };

    world
        .spawn((
            pos,
            King,
            PlayerIdComponent(player_id),
            Health {
                current: KING_BASE_HP,
                max: KING_BASE_HP,
            },
            AttackStats {
                damage: KING_BASE_DAMAGE,
                rate: KING_BASE_RATE,
                damage_type: DamageType::PhysicalBasic,
            },
            AttackTimer(0.0),
            AttackRange(KING_BASE_RANGE),
            CollisionRadius(KING_COLLISION_RADIUS),
            ShapeComponent(Shape::Circle),
        ))
        .id()
}

pub fn spawn_worker(world: &mut World, player_id: i64, targets: TargetPositions) -> Entity {
    world
        .spawn((
            targets.cart, // Start at cart
            ShapeComponent(Shape::Circle),
            PlayerIdComponent(player_id),
            Worker,
            WorkerState::MovingToVein,
            targets,
            CollisionRadius(DEFAULT_COLLISION_RADIUS),
            AttackRange(0.0), // Workers don't attack
            Health {
                current: DEFAULT_HEALTH,
                max: DEFAULT_HEALTH,
            },
            DefenseStats {
                armor: 0.0,
                resistances: Resistances {
                    fire: 0.0,
                    ice: 0.0,
                    lightning: 0.0,
                },
                specialty: DefenseSpecialty::None,
            },
        ))
        .id()
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Task 9.1 TDD tests for spawn_king ---

    #[test]
    fn spawn_king_has_no_home_position() {
        use crate::model::components::HomePosition;
        let mut world = World::new();
        let king = spawn_king(&mut world, 1, 0);
        assert!(
            world.entity(king).get::<HomePosition>().is_none(),
            "King must not have HomePosition"
        );
    }

    #[test]
    fn spawn_king_has_correct_base_stats() {
        use crate::model::components::{AttackRange, AttackStats, CollisionRadius, Health};
        use crate::model::king_config::{
            KING_BASE_DAMAGE, KING_BASE_HP, KING_BASE_RANGE, KING_COLLISION_RADIUS,
        };
        let mut world = World::new();
        let king = spawn_king(&mut world, 1, 0);
        let e = world.entity(king);

        let health = e.get::<Health>().unwrap();
        assert!(
            (health.current - KING_BASE_HP).abs() < f32::EPSILON,
            "King current HP mismatch"
        );
        assert!(
            (health.max - KING_BASE_HP).abs() < f32::EPSILON,
            "King max HP mismatch"
        );

        let stats = e.get::<AttackStats>().unwrap();
        assert!(
            (stats.damage - KING_BASE_DAMAGE).abs() < f32::EPSILON,
            "King damage mismatch"
        );

        let range = e.get::<AttackRange>().unwrap();
        assert!(
            (range.0 - KING_BASE_RANGE).abs() < f32::EPSILON,
            "King range mismatch"
        );

        let radius = e.get::<CollisionRadius>().unwrap();
        assert!(
            (radius.0 - KING_COLLISION_RADIUS).abs() < f32::EPSILON,
            "King collision radius mismatch"
        );
    }

    #[test]
    fn spawn_king_positions_are_correct() {
        use crate::model::constants::{KING_LEFT_X, KING_RIGHT_X, KING_Y};
        let mut world = World::new();
        let king_left = spawn_king(&mut world, 1, 0);
        let king_right = spawn_king(&mut world, 2, 1);

        let pos_left = world.entity(king_left).get::<Position>().unwrap();
        assert!((pos_left.x - KING_LEFT_X).abs() < f32::EPSILON);
        assert!((pos_left.y - KING_Y).abs() < f32::EPSILON);

        let pos_right = world.entity(king_right).get::<Position>().unwrap();
        assert!((pos_right.x - KING_RIGHT_X).abs() < f32::EPSILON);
        assert!((pos_right.y - KING_Y).abs() < f32::EPSILON);
    }

    #[test]
    fn spawn_king_is_not_enemy() {
        use crate::model::components::Enemy;
        let mut world = World::new();
        let king = spawn_king(&mut world, 1, 0);
        assert!(
            world.entity(king).get::<Enemy>().is_none(),
            "King must not be tagged as Enemy"
        );
    }

    #[test]
    fn spawn_helpers_apply_shape_radii() {
        let mut world = World::new();

        let square = spawn_unit(&mut world, Position { x: 0.0, y: 0.0 }, Shape::Square, 1);
        let circle = spawn_unit(&mut world, Position { x: 0.0, y: 0.0 }, Shape::Circle, 1);
        let triangle = spawn_unit(&mut world, Position { x: 0.0, y: 0.0 }, Shape::Triangle, 1);

        assert_eq!(
            world.entity(square).get::<CollisionRadius>().unwrap().0,
            DEFAULT_COLLISION_RADIUS + 2.0
        );
        assert_eq!(
            world.entity(circle).get::<CollisionRadius>().unwrap().0,
            DEFAULT_COLLISION_RADIUS
        );
        assert_eq!(
            world.entity(triangle).get::<CollisionRadius>().unwrap().0,
            DEFAULT_COLLISION_RADIUS - 2.0
        );
    }

    #[test]
    fn spawn_initializes_combat_components() {
        use crate::model::components::{AttackStats, AttackTimer, Health, HomePosition};
        let mut world = World::new();

        let unit_pos = Position { x: 10.0, y: 20.0 };
        let unit = spawn_unit(&mut world, unit_pos, Shape::Circle, 1);
        let enemy_pos = Position { x: 100.0, y: 200.0 };
        let enemy = spawn_enemy(&mut world, enemy_pos, Shape::Circle, 1);

        for (entity, pos) in [(unit, unit_pos), (enemy, enemy_pos)] {
            let e = world.entity(entity);
            assert!(e.get::<Health>().is_some(), "Should have Health");
            assert!(e.get::<AttackStats>().is_some(), "Should have AttackStats");
            assert!(e.get::<AttackTimer>().is_some(), "Should have AttackTimer");
            assert!(
                e.get::<HomePosition>().is_some(),
                "Should have HomePosition"
            );

            let health = e.get::<Health>().unwrap();
            assert!(health.current > 0.0);
            assert_eq!(health.current, health.max);

            let stats = e.get::<AttackStats>().unwrap();
            assert!(stats.damage > 0.0);
            assert!(stats.rate > 0.0);

            let home = e.get::<HomePosition>().unwrap();
            assert_eq!(home.0.x, pos.x);
            assert_eq!(home.0.y, pos.y);
        }
    }

    #[test]
    fn spawn_applies_specialized_stats() {
        use crate::model::components::{AttackRange, AttackStats, DamageType, Mana};
        use crate::model::unit_config::DEFAULT_ATTACK_RANGE;
        let mut world = World::new();

        // Triangle: Ranged Physical Pierce
        let triangle = spawn_unit(&mut world, Position { x: 0.0, y: 0.0 }, Shape::Triangle, 1);
        let t_stats = world.entity(triangle).get::<AttackStats>().unwrap();
        let t_range = world.entity(triangle).get::<AttackRange>().unwrap();
        assert_eq!(t_stats.damage_type, DamageType::PhysicalPierce);
        assert!(
            t_range.0 > DEFAULT_ATTACK_RANGE,
            "Triangle should be ranged"
        );

        // Square: Melee Physical Basic
        let square = spawn_unit(&mut world, Position { x: 0.0, y: 0.0 }, Shape::Square, 1);
        let s_stats = world.entity(square).get::<AttackStats>().unwrap();
        let s_range = world.entity(square).get::<AttackRange>().unwrap();
        assert_eq!(s_stats.damage_type, DamageType::PhysicalBasic);
        assert!(s_range.0 <= DEFAULT_ATTACK_RANGE, "Square should be melee");

        // Circle: Fire Mage (Mana + Ranged Fire Magical)
        let circle = spawn_unit(&mut world, Position { x: 0.0, y: 0.0 }, Shape::Circle, 1);
        let c_stats = world.entity(circle).get::<AttackStats>().unwrap();
        let c_range = world.entity(circle).get::<AttackRange>().unwrap();
        let c_mana = world.entity(circle).get::<Mana>();

        assert_eq!(c_stats.damage_type, DamageType::FireMagical);
        assert!(
            c_range.0 > DEFAULT_ATTACK_RANGE,
            "Circle should be ranged (Mage)"
        );
        assert!(c_mana.is_some(), "Circle (Mage) should have mana");
    }

    #[test]
    fn test_enemy_scaling_is_applied() {
        use crate::handler::wave::get_scaling_multiplier;
        let mut world = World::new();
        let wave = 3;
        let multiplier = get_scaling_multiplier(wave);

        let enemy_id = spawn_enemy(&mut world, Position { x: 0.0, y: 0.0 }, Shape::Square, wave);
        let e = world.entity(enemy_id);

        let health = e.get::<Health>().unwrap();
        let stats = e.get::<AttackStats>().unwrap();

        assert!((health.max - DEFAULT_HEALTH * multiplier).abs() < 0.1);
        assert!(
            (stats.damage - get_unit_profile(Shape::Square).combat.primary.damage * multiplier)
                .abs()
                < 0.1
        );
    }

    #[test]
    fn spawn_sent_enemy_has_bounty_component() {
        use crate::model::components::Bounty;

        let mut world = World::new();
        let entity = spawn_sent_enemy(
            &mut world,
            Position { x: 100.0, y: 30.0 },
            Shape::Square,
            1,
            5,
        );
        let bounty = world.entity(entity).get::<Bounty>();
        assert!(bounty.is_some(), "Sent enemy should have Bounty component");
        assert_eq!(bounty.unwrap().0, 5);
    }

    #[test]
    fn spawn_sent_enemy_applies_health_multiplier() {
        use crate::handler::wave::get_scaling_multiplier;
        use crate::model::components::Health;
        use crate::model::unit_config::{DEFAULT_HEALTH, SENT_CIRCLE_HEALTH_MULT};

        let mut world = World::new();
        let wave = 1;
        let entity = spawn_sent_enemy(
            &mut world,
            Position { x: 100.0, y: 30.0 },
            Shape::Circle,
            wave,
            20,
        );
        let health = world.entity(entity).get::<Health>().unwrap();
        let expected = DEFAULT_HEALTH * get_scaling_multiplier(wave) * SENT_CIRCLE_HEALTH_MULT;
        assert!(
            (health.max - expected).abs() < 0.01,
            "Sent Circle health should apply the health multiplier"
        );
    }

    #[test]
    fn spawn_sent_enemy_is_tagged_as_enemy() {
        use crate::model::components::Enemy;

        let mut world = World::new();
        let entity = spawn_sent_enemy(
            &mut world,
            Position { x: 100.0, y: 30.0 },
            Shape::Triangle,
            1,
            8,
        );
        assert!(
            world.entity(entity).get::<Enemy>().is_some(),
            "Sent enemy should have Enemy component"
        );
    }

    #[test]
    fn test_wave_6_boss_spawning() {
        use crate::handler::wave::get_scaling_multiplier;
        let mut world = World::new();
        let wave = 6;
        let multiplier = get_scaling_multiplier(wave);

        let enemy_id = spawn_enemy(&mut world, Position { x: 0.0, y: 0.0 }, Shape::Circle, wave);
        let e = world.entity(enemy_id);

        assert!(
            e.get::<Boss>().is_some(),
            "Wave 6 should have Boss component"
        );

        let health = e.get::<Health>().unwrap();
        let stats = e.get::<AttackStats>().unwrap();

        let expected_health = DEFAULT_HEALTH * multiplier * BOSS_HEALTH_MULTIPLIER;
        let expected_damage = get_unit_profile(Shape::Circle).combat.primary.damage
            * multiplier
            * BOSS_DAMAGE_MULTIPLIER;

        assert!((health.max - expected_health).abs() < 0.1);
        assert!((stats.damage - expected_damage).abs() < 0.1);
    }
}
