use crate::model::shape::Shape;

pub struct WaveConfig {
    pub enemies: Vec<Shape>,
    #[allow(dead_code)]
    pub is_boss_wave: bool,
}

pub fn get_scaling_multiplier(wave: u32) -> f32 {
    1.2f32.powi(wave as i32 - 1)
}

pub fn get_wave_config(wave: u32) -> WaveConfig {
    match wave {
        1 => WaveConfig {
            enemies: vec![Shape::Square, Shape::Square],
            is_boss_wave: false,
        },
        2 => WaveConfig {
            enemies: vec![Shape::Square, Shape::Square, Shape::Triangle],
            is_boss_wave: false,
        },
        3 => WaveConfig {
            enemies: vec![Shape::Square, Shape::Triangle, Shape::Triangle],
            is_boss_wave: false,
        },
        4 => WaveConfig {
            enemies: vec![Shape::Square, Shape::Triangle, Shape::Circle],
            is_boss_wave: false,
        },
        5 => WaveConfig {
            enemies: vec![
                Shape::Triangle,
                Shape::Triangle,
                Shape::Circle,
                Shape::Circle,
            ],
            is_boss_wave: false,
        },
        6 => WaveConfig {
            enemies: vec![Shape::Circle], // This will be the Boss
            is_boss_wave: true,
        },
        _ => WaveConfig {
            enemies: vec![],
            is_boss_wave: false,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scaling_multiplier() {
        // Multiplier = 1.2^(wave - 1)
        assert!((get_scaling_multiplier(1) - 1.0).abs() < f32::EPSILON);
        assert!((get_scaling_multiplier(2) - 1.2).abs() < f32::EPSILON);
        assert!((get_scaling_multiplier(3) - 1.44).abs() < 0.0001);
        assert!((get_scaling_multiplier(6) - 2.48832).abs() < 0.0001);
    }

    #[test]
    fn test_wave_configs() {
        let wave1 = get_wave_config(1);
        assert_eq!(wave1.enemies.len(), 2);
        assert!(!wave1.is_boss_wave);

        let wave6 = get_wave_config(6);
        assert_eq!(wave6.enemies.len(), 1);
        assert!(wave6.is_boss_wave);
    }
}
