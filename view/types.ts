export type UnitKind = 'Square' | 'Circle' | 'Triangle';
export type Family = 'Basic';

export interface BuildCatalogEntry {
    unit_kind: UnitKind;
    name: string;
    cost: number;
}

export interface Unit {
    id: number;
    shape: UnitKind;
    x: number;
    y: number;
    owner_id: number;
    is_enemy: boolean;
    current_hp: number;
    max_hp: number;
    is_worker: boolean;
    current_mana?: number;
    max_mana?: number;
    worker_state?: 'MovingToVein' | 'Mining' | 'MovingToCart';
    is_king: boolean;
}

export interface Player {
    id: number;
    username: string;
    gold: number;
    income: number;
    king_tier: number;
    family: Family | null;
    spawning_queue: UnitKind[];
    next_send_costs: number[];
}

export interface Position {
    x: number;
    y: number;
}

export type School = 'PhysicalBasic' | 'PhysicalPierce' | 'Magical';
export type Element = 'None' | 'Fire' | 'Ice' | 'Poison';
export interface DamageType {
    school: School;
    element: Element;
}

export interface CombatEvent {
    attacker_id: number;
    target_id: number;
    attack_type: DamageType;
    start_pos: Position;
    end_pos: Position;
}

export interface SendUnitCatalogEntry {
    shape: string;
    name: string;
    base_cost: number;
    income: number;
    bounty: number;
}
