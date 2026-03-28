pub mod app;
pub mod app_state;
pub mod assets;
pub mod auth;
pub mod bevy_app;
pub mod game_view;
pub mod lobby;
pub mod storage;
pub mod ws;

use app::App;

fn main() {
    leptos::mount::mount_to_body(App);
}

#[cfg(test)]
mod tests {
    // RED → GREEN: shared types from common are accessible from this crate
    #[test]
    fn common_types_are_accessible_from_view() {
        use common::{DamageType, GamePhase, Shape};
        assert_eq!(GamePhase::Build, GamePhase::Build);
        assert_eq!(Shape::Circle, Shape::Circle);
        assert_eq!(DamageType::FireMagical, DamageType::FireMagical);
    }
}
