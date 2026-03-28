use crate::components::DamageType;
use crate::components::Position;
use crate::game_phase::GamePhase;
use crate::shape::Shape;
use bevy_ecs::message::Message;
use serde::{Deserialize, Serialize};

/// A frontend-safe view of a player; contains no server-side business logic.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PlayerView {
    pub id: i64,
    pub username: String,
    pub gold: u32,
    pub income: u32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PlaceMessage {
    pub shape: Shape,
    pub row: u32,
    pub col: u32,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "action", content = "payload", rename_all = "camelCase")]
pub enum ClientMessage {
    JoinLobby(usize),
    Place(PlaceMessage),
    SellById { entity_id: u32 },
    SkipToCombat,
    LeaveLobby,
    HireWorker {},
    RequestUnitInfo { entity_id: u32 },
    SendUnit { shape: Shape },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LobbyInfo {
    pub id: usize,
    pub player_count: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Unit {
    pub id: u32,
    pub shape: Shape,
    pub x: f32,
    pub y: f32,
    pub owner_id: i64,
    pub is_enemy: bool,
    pub current_hp: f32,
    pub max_hp: f32,
    pub is_worker: bool,
    pub current_mana: Option<f32>,
    pub max_mana: Option<f32>,
    pub worker_state: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UnitInfoData {
    pub entity_id: u32,
    pub attack_damage: Option<f32>,
    pub attack_rate: Option<f32>,
    pub attack_range: Option<f32>,
    pub damage_type: Option<DamageType>,
    pub armor: Option<f32>,
    pub is_boss: bool,
    pub sell_value: Option<u32>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Message)]
pub struct CombatEvent {
    pub attacker_id: u32,
    pub target_id: u32,
    pub attack_type: DamageType,
    pub start_pos: Position,
    pub end_pos: Position,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SerializableGameState {
    pub units: Vec<Unit>,
    pub players: Vec<PlayerView>,
    pub phase: GamePhase,
    pub phase_timer: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type", content = "data")]
pub enum ServerMessage {
    LobbyStatus(Vec<LobbyInfo>),
    GameState(SerializableGameState),
    CombatEvents(Vec<CombatEvent>),
    PlayerId(i64),
    Error(String),
    UnitInfo(UnitInfoData),
}
