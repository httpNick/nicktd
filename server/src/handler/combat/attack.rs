use super::apply_damage;
use super::get_board;
use crate::model::components::{
    AttackRange, AttackStats, AttackTimer, Bounty, CombatProfile, Dead, DefenseStats, Enemy,
    Health, InAttackRange, Mana, Position, Target,
};
use crate::model::game_state::DeltaTime;
use crate::model::messages::CombatEvent;
use crate::model::player::Players;
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::{Entity, Query, Res, Without, World};

pub fn update_active_combat_stats(world: &mut World) {
    let mut updates = Vec::new(); // (Entity, damage, rate, range, type)

    let mut query = world.query_filtered::<(
        Entity,
        &CombatProfile,
        Option<&Mana>,
        &AttackStats,
        &AttackRange,
    ), Without<Dead>>();
    for (entity, profile, mana_opt, current_stats, current_range) in query.iter(world) {
        let use_primary = if profile.mana_cost > 0.0 {
            if let Some(mana) = mana_opt {
                mana.current >= profile.mana_cost
            } else {
                false
            }
        } else {
            true
        };

        let selected = if use_primary {
            &profile.primary
        } else {
            profile.secondary.as_ref().unwrap_or(&profile.primary)
        };

        if selected.damage != current_stats.damage
            || selected.rate != current_stats.rate
            || selected.damage_type != current_stats.damage_type
            || selected.range != current_range.0
        {
            updates.push((
                entity,
                selected.damage,
                selected.rate,
                selected.range,
                selected.damage_type,
            ));
        }
    }

    for (entity, damage, rate, range, damage_type) in updates {
        if let Some(mut stats) = world.get_mut::<AttackStats>(entity) {
            stats.damage = damage;
            stats.rate = rate;
            stats.damage_type = damage_type;
        }
        if let Some(mut r) = world.get_mut::<AttackRange>(entity) {
            r.0 = range;
        }
    }
}

fn execute_combat_round(world: &mut World, tick_delta: f32) -> Vec<CombatEvent> {
    let mut attacks = Vec::new(); // (AttackerEntity, TargetEntity, Damage, DamageType)
    let mut timer_updates = Vec::new(); // (AttackerEntity, NewTimerValue)
    let mut mana_updates = Vec::new(); // (AttackerEntity, NewManaValue)

    let mut query = world.query_filtered::<(
        Entity,
        &AttackStats,
        &AttackTimer,
        Option<&Target>,
        Option<&InAttackRange>,
        Option<&CombatProfile>,
        Option<&Mana>,
    ), Without<Dead>>();
    for (attacker_entity, stats, timer, target_opt, in_range_opt, profile_opt, mana_opt) in
        query.iter(world)
    {
        // Update timer
        let mut new_timer = (timer.0 - tick_delta).max(0.0);

        // Try to attack if in range and timer is 0
        if in_range_opt.is_some() && new_timer <= 0.0 {
            if let Some(target) = target_opt {
                // Deduct mana if it was a primary attack with cost
                if let (Some(profile), Some(mana)) = (profile_opt, mana_opt) {
                    if profile.mana_cost > 0.0 && mana.current >= profile.mana_cost {
                        mana_updates.push((attacker_entity, mana.current - profile.mana_cost));
                    }
                }

                // Record attack
                attacks.push((attacker_entity, target.0, stats.damage, stats.damage_type));
                // Reset timer: 1.0 / rate
                new_timer = 1.0 / stats.rate;
            }
        }

        timer_updates.push((attacker_entity, new_timer));
    }

    // Apply timer updates
    for (entity, new_val) in timer_updates {
        if let Some(mut timer) = world.get_mut::<AttackTimer>(entity) {
            timer.0 = new_val;
        }
    }

    // Apply mana updates
    for (entity, new_val) in mana_updates {
        if let Some(mut mana) = world.get_mut::<Mana>(entity) {
            mana.current = new_val;
        }
    }

    let mut combat_events = Vec::new();

    // Apply damage and generate events
    for (attacker_entity, target_entity, damage, damage_type) in attacks {
        // Capture positions for event
        let start_pos = world
            .get::<Position>(attacker_entity)
            .copied()
            .unwrap_or(Position { x: 0.0, y: 0.0 });
        let end_pos = world
            .get::<Position>(target_entity)
            .copied()
            .unwrap_or(Position { x: 0.0, y: 0.0 });

        combat_events.push(CombatEvent {
            attacker_id: attacker_entity.to_bits(),
            target_id: target_entity.to_bits(),
            attack_type: damage_type,
            start_pos,
            end_pos,
        });

        let defense = world
            .get::<DefenseStats>(target_entity)
            .copied()
            .unwrap_or_default();
        let mitigated = apply_damage(damage, damage_type, &defense);

        if let Some(mut health) = world.get_mut::<Health>(target_entity) {
            health.current -= mitigated;
        }
    }

    combat_events
}

/// Bevy-compatible exclusive system: reads `DeltaTime` from the world, runs the combat
/// round, and writes any resulting [`CombatEvent`]s into the `Messages<CombatEvent>` resource.
pub fn process_combat(world: &mut World) {
    let tick_delta = world.resource::<DeltaTime>().0;
    let events = execute_combat_round(world, tick_delta);
    if !events.is_empty() {
        let mut messages = world.resource_mut::<Messages<CombatEvent>>();
        for event in events {
            messages.write(event);
        }
    }
}

pub fn cleanup_dead_entities(world: &mut World) {
    // Collect dead entities: enemies get despawned (capturing any bounty info),
    // non-enemies (towers/units) get tagged Dead.
    let mut enemies_to_despawn: Vec<(Entity, Option<Bounty>, Option<u8>)> = Vec::new();
    let mut towers_to_tag = Vec::new();

    {
        let mut query = world.query::<(Entity, &Health)>();
        for (entity, health) in query.iter(world) {
            if health.current <= 0.0 {
                if world.get::<Enemy>(entity).is_some() {
                    let bounty = world.get::<Bounty>(entity).copied();
                    let board = world.get::<Position>(entity).and_then(|p| get_board(p.x));
                    enemies_to_despawn.push((entity, bounty, board));
                } else {
                    towers_to_tag.push(entity);
                }
            }
        }
    }

    // Award bounty gold to the defending player before despawning.
    if let Some(mut players) = world.get_resource_mut::<Players>() {
        for (_, bounty, board) in &enemies_to_despawn {
            if let (Some(b), Some(board_idx)) = (bounty, board) {
                if let Some(player) = players.0.get_mut(*board_idx as usize) {
                    player.gold += b.0;
                }
            }
        }
    }

    for (entity, _, _) in enemies_to_despawn {
        world.despawn(entity);
    }

    for entity in towers_to_tag {
        world.entity_mut(entity).insert(Dead);
    }
}

pub fn update_mana(mut query: Query<&mut Mana>, time: Res<DeltaTime>) {
    for mut mana in query.iter_mut() {
        mana.current = (mana.current + mana.regen * time.0).min(mana.max);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::components::{AttackProfile, DamageType};
    use crate::model::shape::Shape;
    use crate::model::unit_config::{DEFAULT_ATTACK_RANGE, FIREBALL_MANA_COST, MAGE_MELEE_DAMAGE};

    #[test]
    fn combat_system_applies_damage() {
        use crate::model::components::{AttackStats, AttackTimer, Health};

        let mut world = World::new();

        // Attacker
        let attacker = world
            .spawn((
                InAttackRange,
                AttackStats {
                    damage: 10.0,
                    rate: 1.0,
                    damage_type: DamageType::PHYSICAL_BASIC,
                }, // 1 attack per second
                CombatProfile {
                    primary: AttackProfile {
                        damage: 10.0,
                        rate: 1.0,
                        range: DEFAULT_ATTACK_RANGE,
                        damage_type: DamageType::PHYSICAL_BASIC,
                    },
                    secondary: None,
                    mana_cost: 0.0,
                },
                AttackTimer(0.0), // Ready to attack
            ))
            .id();

        // Target
        let target = world
            .spawn((Health {
                current: 100.0,
                max: 100.0,
            },))
            .id();

        world.entity_mut(attacker).insert(Target(target));

        // 1. Process combat - should deal damage and reset timer
        execute_combat_round(&mut world, 0.1);

        let target_health = world.entity(target).get::<Health>().unwrap();
        assert_eq!(target_health.current, 90.0, "Target should have lost 10 HP");

        let attacker_timer = world.entity(attacker).get::<AttackTimer>().unwrap();
        assert_eq!(attacker_timer.0, 1.0, "Timer should be reset to 1.0");

        // 2. Process combat again with small delta - should NOT deal damage
        execute_combat_round(&mut world, 0.5);
        let target_health = world.entity(target).get::<Health>().unwrap();
        assert_eq!(
            target_health.current, 90.0,
            "Target should NOT have lost more HP yet"
        );

        let attacker_timer = world.entity(attacker).get::<AttackTimer>().unwrap();
        assert_eq!(attacker_timer.0, 0.5, "Timer should have decreased by 0.5");

        // 3. Process combat enough to trigger second attack
        execute_combat_round(&mut world, 0.5); // Timer reaches 0.0
        let target_health = world.entity(target).get::<Health>().unwrap();
        assert_eq!(
            target_health.current, 80.0,
            "Target should have lost another 10 HP"
        );

        let attacker_timer = world.entity(attacker).get::<AttackTimer>().unwrap();
        assert_eq!(attacker_timer.0, 1.0, "Timer should be reset again");
    }

    #[test]
    fn combat_system_mitigates_damage_through_target_defense() {
        use crate::model::components::{AttackStats, AttackTimer, DefenseStats, Health};

        let mut world = World::new();

        let attacker = world
            .spawn((
                InAttackRange,
                AttackStats {
                    damage: 10.0,
                    rate: 1.0,
                    damage_type: DamageType::PHYSICAL_BASIC,
                },
                CombatProfile {
                    primary: AttackProfile {
                        damage: 10.0,
                        rate: 1.0,
                        range: DEFAULT_ATTACK_RANGE,
                        damage_type: DamageType::PHYSICAL_BASIC,
                    },
                    secondary: None,
                    mana_cost: 0.0,
                },
                AttackTimer(0.0),
            ))
            .id();

        // Armored target: 50% physical mitigation.
        let target = world
            .spawn((
                Health {
                    current: 100.0,
                    max: 100.0,
                },
                DefenseStats {
                    armor: 0.5,
                    ..Default::default()
                },
            ))
            .id();

        world.entity_mut(attacker).insert(Target(target));

        execute_combat_round(&mut world, 0.1);

        let target_health = world.entity(target).get::<Health>().unwrap();
        assert_eq!(
            target_health.current, 95.0,
            "50% armor should halve the 10 damage hit to 5"
        );
    }

    #[test]
    fn cleanup_removes_dead_entities() {
        use crate::model::components::{Dead, Health};

        let mut world = World::new();

        let alive = world
            .spawn(Health {
                current: 10.0,
                max: 100.0,
            })
            .id();
        let dead = world
            .spawn(Health {
                current: 0.0,
                max: 100.0,
            })
            .id();
        let overkill = world
            .spawn(Health {
                current: -5.0,
                max: 100.0,
            })
            .id();

        cleanup_dead_entities(&mut world);

        assert!(
            world.entities().contains(alive),
            "Alive entity should still exist"
        );
        assert!(
            world.entity(alive).get::<Dead>().is_none(),
            "Alive entity should not have Dead marker"
        );
        // Non-enemy entities at 0 HP are tagged Dead, not despawned
        assert!(
            world.entities().contains(dead),
            "Non-enemy dead entity should remain in world"
        );
        assert!(
            world.entity(dead).get::<Dead>().is_some(),
            "Non-enemy dead entity should have Dead marker"
        );
        assert!(
            world.entities().contains(overkill),
            "Non-enemy overkill entity should remain in world"
        );
        assert!(
            world.entity(overkill).get::<Dead>().is_some(),
            "Non-enemy overkill entity should have Dead marker"
        );
    }

    #[test]
    fn cleanup_enemy_despawned_tower_tagged_dead() {
        use crate::model::components::{Dead, Health};

        let mut world = World::new();

        // Player tower at 0 HP (no Enemy marker)
        let tower = world
            .spawn(Health {
                current: 0.0,
                max: 100.0,
            })
            .id();

        // Enemy at 0 HP
        let enemy = world
            .spawn((
                Health {
                    current: 0.0,
                    max: 50.0,
                },
                Enemy,
            ))
            .id();

        // Tower with positive HP should be unaffected
        let healthy_tower = world
            .spawn(Health {
                current: 50.0,
                max: 100.0,
            })
            .id();

        cleanup_dead_entities(&mut world);

        assert!(
            world.entities().contains(tower),
            "Tower entity should remain in world"
        );
        assert!(
            world.entity(tower).get::<Dead>().is_some(),
            "Tower should have Dead marker"
        );
        assert!(
            !world.entities().contains(enemy),
            "Enemy should be despawned"
        );
        assert!(
            world.entities().contains(healthy_tower),
            "Healthy tower should remain in world"
        );
        assert!(
            world.entity(healthy_tower).get::<Dead>().is_none(),
            "Healthy tower should not have Dead marker"
        );
    }

    #[test]
    fn mana_regeneration_works() {
        use crate::model::components::Mana;
        use bevy_ecs::system::RunSystemOnce;
        let mut world = World::new();

        let unit = world
            .spawn(Mana {
                current: 10.0,
                max: 100.0,
                regen: 5.0, // 5 mana per second
            })
            .id();

        let tick_delta = 0.5; // Half a second
        world.insert_resource(DeltaTime(tick_delta));
        world.run_system_once(update_mana).unwrap();

        let mana = world.entity(unit).get::<Mana>().unwrap();
        assert_eq!(mana.current, 12.5, "Should regenerate 2.5 mana in 0.5s");

        // Test capping at max
        let unit_max = world
            .spawn(Mana {
                current: 99.0,
                max: 100.0,
                regen: 5.0,
            })
            .id();

        world.run_system_once(update_mana).unwrap();
        let mana_max = world.entity(unit_max).get::<Mana>().unwrap();
        assert_eq!(mana_max.current, 100.0, "Should cap at max mana");
    }

    #[test]
    fn physical_simulation_cycle() {
        let mut world = World::new();
        let tick_delta = 1.0 / 30.0;
        world.insert_resource(DeltaTime(tick_delta));

        // Spawn unit and enemy
        let unit = crate::handler::spawn::spawn_unit(
            &mut world,
            Position { x: 0.0, y: 0.0 },
            Shape::Circle,
            1,
        );
        let enemy = crate::handler::spawn::spawn_enemy(
            &mut world,
            Position { x: 100.0, y: 0.0 },
            Shape::Circle,
            1,
        );

        // 1. Targeting
        crate::handler::combat::update_targeting(&mut world);
        assert_eq!(world.entity(unit).get::<Target>().unwrap().0, enemy);

        // 2. Movement - multiple ticks until in range
        for _ in 0..100 {
            crate::handler::combat::update_combat_movement(&mut world);
            crate::handler::combat::update_attack_range_markers(&mut world);
        }
        assert!(world.entity(unit).get::<InAttackRange>().is_some());

        // 3. Combat - deal damage
        let initial_health = world.entity(enemy).get::<Health>().unwrap().current;
        execute_combat_round(&mut world, tick_delta);
        let post_attack_health = world.entity(enemy).get::<Health>().unwrap().current;
        assert!(
            post_attack_health < initial_health,
            "Enemy should have taken damage"
        );

        // 4. Cleanup - reduce HP to 0 and verify removal
        world.entity_mut(enemy).get_mut::<Health>().unwrap().current = 0.0;
        cleanup_dead_entities(&mut world);
        assert!(
            !world.entities().contains(enemy),
            "Enemy should be removed from world"
        );

        // 5. Re-targeting - Unit should lose target
        crate::handler::combat::update_targeting(&mut world);
        assert!(
            world.entity(unit).get::<Target>().is_none(),
            "Unit should have no target after enemy is dead"
        );
    }

    #[test]
    fn mage_switches_to_melee_when_out_of_mana() {
        use crate::model::components::{
            AttackProfile, AttackRange, AttackStats, CombatProfile, DamageType, Health, Mana,
        };
        use crate::model::unit_config::{MAGE_MANA_MAX, RANGED_ATTACK_RANGE};

        let mut world = World::new();

        let fireball_cost = FIREBALL_MANA_COST;
        let melee_damage = MAGE_MELEE_DAMAGE;
        let fireball_damage = 10.0;
        let ranged_range = RANGED_ATTACK_RANGE;
        let melee_range = DEFAULT_ATTACK_RANGE;

        // Mage (Attacker)
        let mage = world
            .spawn((
                InAttackRange,
                AttackStats {
                    damage: fireball_damage,
                    rate: 1.0,
                    damage_type: DamageType::FIRE_MAGICAL,
                },
                AttackRange(ranged_range),
                CombatProfile {
                    primary: AttackProfile {
                        damage: fireball_damage,
                        rate: 1.0,
                        range: ranged_range,
                        damage_type: DamageType::FIRE_MAGICAL,
                    },
                    secondary: Some(AttackProfile {
                        damage: melee_damage,
                        rate: 1.0,
                        range: melee_range,
                        damage_type: DamageType::PHYSICAL_BASIC,
                    }),
                    mana_cost: fireball_cost,
                },
                AttackTimer(0.0),
                Mana {
                    current: fireball_cost, // Enough for 1 fireball
                    max: MAGE_MANA_MAX,
                    regen: 0.0,
                },
            ))
            .id();

        // Target
        let target = world
            .spawn((Health {
                current: 100.0,
                max: 100.0,
            },))
            .id();

        world.entity_mut(mage).insert(Target(target));

        // 1. First attack: should be Fireball
        update_active_combat_stats(&mut world);
        execute_combat_round(&mut world, 0.1);

        let target_health = world.entity(target).get::<Health>().unwrap().current;
        assert_eq!(
            target_health,
            100.0 - fireball_damage,
            "Should deal fireball damage"
        );

        let mana = world.entity(mage).get::<Mana>().unwrap().current;
        assert_eq!(mana, 0.0, "Should consume mana for fireball");

        // 2. Second attack: out of mana, should be weak melee
        // Reset timer manually for test
        world.entity_mut(mage).insert(AttackTimer(0.0));
        update_active_combat_stats(&mut world);
        execute_combat_round(&mut world, 0.1);

        let target_health = world.entity(target).get::<Health>().unwrap().current;
        assert_eq!(
            target_health,
            100.0 - fireball_damage - melee_damage,
            "Should deal weak melee damage when out of mana"
        );

        let stats = world.entity(mage).get::<AttackStats>().unwrap();
        assert_eq!(
            stats.damage_type,
            DamageType::PHYSICAL_BASIC,
            "Should switch to PhysicalBasic when out of mana"
        );
        assert_eq!(
            world.entity(mage).get::<AttackRange>().unwrap().0,
            melee_range,
            "Should switch to melee range"
        );

        // 3. Third attack: mana regenerated enough for fireball
        world.entity_mut(mage).insert(Mana {
            current: FIREBALL_MANA_COST,
            max: MAGE_MANA_MAX,
            regen: 0.0,
        });
        world.entity_mut(mage).insert(AttackTimer(0.0));
        update_active_combat_stats(&mut world);
        execute_combat_round(&mut world, 0.1);

        let target_health = world.entity(target).get::<Health>().unwrap().current;
        assert_eq!(
            target_health,
            100.0 - fireball_damage - melee_damage - fireball_damage,
            "Should deal fireball damage again after mana regen"
        );

        let stats = world.entity(mage).get::<AttackStats>().unwrap();
        assert_eq!(
            stats.damage_type,
            DamageType::FIRE_MAGICAL,
            "Should switch back to FireMagical when mana is sufficient"
        );
        assert_eq!(
            world.entity(mage).get::<AttackRange>().unwrap().0,
            ranged_range,
            "Should switch back to ranged range"
        );
    }

    #[test]
    fn process_combat_returns_events() {
        use crate::model::components::{AttackStats, AttackTimer, DamageType, Health, Position};
        let mut world = World::new();

        // Attacker
        let attacker = world
            .spawn((
                Position { x: 0.0, y: 0.0 },
                InAttackRange,
                AttackStats {
                    damage: 10.0,
                    rate: 1.0,
                    damage_type: DamageType::PHYSICAL_PIERCE,
                },
                AttackTimer(0.0),
            ))
            .id();

        // Target
        let target = world
            .spawn((
                Position { x: 10.0, y: 0.0 },
                Health {
                    current: 100.0,
                    max: 100.0,
                },
            ))
            .id();

        world.entity_mut(attacker).insert(Target(target));

        let events = execute_combat_round(&mut world, 0.1);

        assert_eq!(events.len(), 1, "Should return 1 combat event");
        let event = &events[0];
        assert_eq!(event.attacker_id, attacker.to_bits());
        assert_eq!(event.target_id, target.to_bits());
        assert_eq!(event.attack_type, DamageType::PHYSICAL_PIERCE);
        assert_eq!(event.start_pos, Position { x: 0.0, y: 0.0 });
        assert_eq!(event.end_pos, Position { x: 10.0, y: 0.0 });
    }

    // --- Event System Tests (Task 2) ---

    #[test]
    fn process_combat_writes_messages_to_world() {
        use crate::model::components::{
            AttackProfile, AttackStats, AttackTimer, DamageType, Health,
        };
        use crate::model::unit_config::DEFAULT_ATTACK_RANGE;
        use bevy_ecs::message::Messages;
        use bevy_ecs::system::RunSystemOnce;

        let mut world = World::new();
        world.init_resource::<Messages<CombatEvent>>();
        world.insert_resource(DeltaTime(0.1));

        let attacker = world
            .spawn((
                Position { x: 0.0, y: 0.0 },
                InAttackRange,
                AttackStats {
                    damage: 10.0,
                    rate: 1.0,
                    damage_type: DamageType::PHYSICAL_BASIC,
                },
                CombatProfile {
                    primary: AttackProfile {
                        damage: 10.0,
                        rate: 1.0,
                        range: DEFAULT_ATTACK_RANGE,
                        damage_type: DamageType::PHYSICAL_BASIC,
                    },
                    secondary: None,
                    mana_cost: 0.0,
                },
                AttackTimer(0.0),
            ))
            .id();

        let target = world
            .spawn((
                Position { x: 10.0, y: 0.0 },
                Health {
                    current: 100.0,
                    max: 100.0,
                },
            ))
            .id();

        world.entity_mut(attacker).insert(Target(target));

        world.run_system_once(process_combat).unwrap();

        let messages = world.resource::<Messages<CombatEvent>>();
        let mut cursor = messages.get_cursor();
        let events: Vec<&CombatEvent> = cursor.read(messages).collect();

        assert_eq!(
            events.len(),
            1,
            "process_combat should write 1 CombatEvent message"
        );
        assert_eq!(events[0].attacker_id, attacker.to_bits());
        assert_eq!(events[0].target_id, target.to_bits());
        assert_eq!(events[0].attack_type, DamageType::PHYSICAL_BASIC);
    }

    // --- Task 3: Refactored System Signature Tests ---

    #[test]
    fn update_mana_reads_delta_time_from_resource() {
        use crate::model::components::Mana;
        use bevy_ecs::system::RunSystemOnce;

        let mut world = World::new();
        world.insert_resource(DeltaTime(1.0)); // 1 second delta

        let unit = world
            .spawn(Mana {
                current: 0.0,
                max: 100.0,
                regen: 10.0,
            })
            .id();

        world.run_system_once(update_mana).unwrap();

        let mana = world.entity(unit).get::<Mana>().unwrap();
        assert_eq!(
            mana.current, 10.0,
            "Should regenerate 10 mana/s using DeltaTime resource"
        );
    }

    // --- Task 2.2: Bounty reward mechanics ---

    #[test]
    fn bounty_awarded_to_left_board_defender_on_kill() {
        use crate::model::components::{Bounty, Enemy, Health};
        use crate::model::player::{Player, Players};
        use bevy_ecs::prelude::World;

        let mut world = World::new();

        let player1 = Player::new(1, "p1".into(), 100);
        let player2 = Player::new(2, "p2".into(), 100);
        world.insert_resource(Players(vec![player1, player2]));

        // Enemy at 0 HP with a bounty on the left board (board 0 → players[0] defends)
        world.spawn((
            Health {
                current: 0.0,
                max: 50.0,
            },
            Enemy,
            Position { x: 100.0, y: 30.0 },
            Bounty(20),
        ));

        cleanup_dead_entities(&mut world);

        let players = world.resource::<Players>();
        assert_eq!(
            players.0[0].gold, 120,
            "Left board defender should receive 20 bounty gold"
        );
        assert_eq!(
            players.0[1].gold, 100,
            "Right board defender should not receive bounty gold"
        );
    }

    #[test]
    fn bounty_awarded_to_right_board_defender_on_kill() {
        use crate::model::components::{Bounty, Enemy, Health};
        use crate::model::constants::RIGHT_BOARD_START;
        use crate::model::player::{Player, Players};
        use bevy_ecs::prelude::World;

        let mut world = World::new();

        let player1 = Player::new(1, "p1".into(), 100);
        let player2 = Player::new(2, "p2".into(), 100);
        world.insert_resource(Players(vec![player1, player2]));

        // Enemy at 0 HP with a bounty on the right board (board 1 → players[1] defends)
        world.spawn((
            Health {
                current: 0.0,
                max: 50.0,
            },
            Enemy,
            Position {
                x: RIGHT_BOARD_START + 100.0,
                y: 30.0,
            },
            Bounty(10),
        ));

        cleanup_dead_entities(&mut world);

        let players = world.resource::<Players>();
        assert_eq!(
            players.0[0].gold, 100,
            "Left board defender should not receive bounty gold"
        );
        assert_eq!(
            players.0[1].gold, 110,
            "Right board defender should receive 10 bounty gold"
        );
    }

    #[test]
    fn non_bounty_enemies_award_no_gold() {
        use crate::model::components::{Enemy, Health};
        use crate::model::player::{Player, Players};
        use bevy_ecs::prelude::World;

        let mut world = World::new();

        let player1 = Player::new(1, "p1".into(), 100);
        world.insert_resource(Players(vec![player1]));

        // Regular enemy at 0 HP without a Bounty component
        world.spawn((
            Health {
                current: 0.0,
                max: 50.0,
            },
            Enemy,
            Position { x: 100.0, y: 30.0 },
        ));

        cleanup_dead_entities(&mut world);

        let players = world.resource::<Players>();
        assert_eq!(
            players.0[0].gold, 100,
            "Regular enemy kill should award no bounty gold"
        );
    }
}
