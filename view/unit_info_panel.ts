// Exported types (consumed by app.ts in task 7)

export interface Unit {
    id: number;
    shape: 'Square' | 'Circle' | 'Triangle';
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
}

export interface UnitStaticInfo {
    entity_id: number;
    attack_damage: number | null;
    attack_rate: number | null;
    attack_range: number | null;
    damage_type: 'PhysicalBasic' | 'PhysicalPierce' | 'FireMagical' | null;
    armor: number | null;
    is_boss: boolean;
    sell_value: number | null;
}

export interface UnitInfoPanelCallbacks {
    onSell: (entityId: number) => void;
}

// Visual unit size within the 80×80 portrait canvas (matches drawUnits geometry)
const PORTRAIT_UNIT_SIZE = 60;

export class UnitInfoPanel {
    private _selectedEntityId: number | null = null;
    readonly staticInfoCache: Map<number, UnitStaticInfo> = new Map();

    private container: HTMLElement;
    private callbacks: UnitInfoPanelCallbacks;

    private portraitCtx: CanvasRenderingContext2D | null;
    private nameEl: HTMLElement | null;
    private hpFill: HTMLElement | null;
    private hpLabel: HTMLElement | null;
    private manaRow: HTMLElement | null;
    private manaFill: HTMLElement | null;
    private manaLabel: HTMLElement | null;
    private workerStateRow: HTMLElement | null;
    private workerStateLabel: HTMLElement | null;
    private statsSection: HTMLElement | null;
    private sellBtn: HTMLButtonElement | null;
    private sellValueEl: HTMLElement | null;

    constructor(containerElement: HTMLElement, callbacks: UnitInfoPanelCallbacks) {
        this.container = containerElement;
        this.callbacks = callbacks;
        this.container.style.display = 'none';

        const q = <T extends Element>(sel: string) =>
            containerElement.querySelector<T>(sel);

        const canvas = q<HTMLCanvasElement>('[data-panel="portrait"]');
        this.portraitCtx = canvas?.getContext('2d') ?? null;
        this.nameEl          = q<HTMLElement>('[data-panel="name"]');
        this.hpFill          = q<HTMLElement>('[data-panel="hp-fill"]');
        this.hpLabel         = q<HTMLElement>('[data-panel="hp-label"]');
        this.manaRow         = q<HTMLElement>('[data-panel="mana-row"]');
        this.manaFill        = q<HTMLElement>('[data-panel="mana-fill"]');
        this.manaLabel       = q<HTMLElement>('[data-panel="mana-label"]');
        this.workerStateRow  = q<HTMLElement>('[data-panel="worker-state-row"]');
        this.workerStateLabel = q<HTMLElement>('[data-panel="worker-state-label"]');
        this.statsSection    = q<HTMLElement>('[data-panel="stats"]');
        this.sellBtn         = q<HTMLButtonElement>('[data-panel="sell-btn"]');
        this.sellValueEl     = q<HTMLElement>('[data-panel="sell-value"]');

        this.sellBtn?.addEventListener('click', () => {
            if (this._selectedEntityId !== null) {
                this.callbacks.onSell(this._selectedEntityId);
            }
        });
    }

    /**
     * Called when the player clicks an entity.
     * Shows the panel, draws the portrait, seeds the bars from live broadcast data,
     * and applies any cached static info immediately.
     */
    selectUnit(unit: Unit, myPlayerId: number, gamePhase: string): void {
        this._selectedEntityId = unit.id;
        this.container.style.display = 'flex';

        // Name / type label
        const entityType = unit.is_enemy ? 'Enemy' : unit.is_worker ? 'Worker' : 'Tower';
        if (this.nameEl) this.nameEl.textContent = `${unit.shape} ${entityType}`;

        // Portrait
        this._drawPortrait(unit, myPlayerId);

        // HP bar (live data)
        this._updateHpBar(unit.current_hp, unit.max_hp);

        // Mana bar
        if (unit.max_mana != null) {
            if (this.manaRow) this.manaRow.style.display = 'flex';
            this._updateManaBar(unit.current_mana ?? 0, unit.max_mana);
        } else {
            if (this.manaRow) this.manaRow.style.display = 'none';
        }

        // Worker state label
        if (unit.is_worker && unit.worker_state) {
            if (this.workerStateRow) this.workerStateRow.style.display = 'block';
            if (this.workerStateLabel) this.workerStateLabel.textContent = unit.worker_state;
        } else {
            if (this.workerStateRow) this.workerStateRow.style.display = 'none';
        }

        // Clear stats section — will be populated by applyStaticInfo / cache hit below
        if (this.statsSection) this.statsSection.innerHTML = '';

        // Ability grid: Sell button visible only for own non-worker towers
        const isOwnTower = !unit.is_enemy && !unit.is_worker && unit.owner_id === myPlayerId;
        if (this.sellBtn) {
            this.sellBtn.style.display = isOwnTower ? 'block' : 'none';
            this.sellBtn.disabled = gamePhase !== 'Build';
        }

        // Apply cached static info if already available (avoids redundant RequestUnitInfo)
        const cached = this.staticInfoCache.get(unit.id);
        if (cached) {
            this._renderStaticInfo(cached);
        }
    }

    /**
     * Hides the panel and resets selection state.
     */
    clearSelection(): void {
        this._selectedEntityId = null;
        this.container.style.display = 'none';
        if (this.statsSection) this.statsSection.innerHTML = '';
        if (this.sellBtn) this.sellBtn.style.display = 'none';
    }

    /**
     * Called from app.ts on every GameState broadcast.
     * Updates live fields (HP, mana, worker state, sell-button phase gate).
     * Calls clearSelection when the selected entity is no longer in the broadcast.
     */
    syncDynamicState(units: Unit[], gamePhase: string): void {
        if (this._selectedEntityId === null) return;

        const unit = units.find(u => u.id === this._selectedEntityId);
        if (!unit) {
            // Entity was despawned — evict cache and clear panel
            this.staticInfoCache.delete(this._selectedEntityId);
            this.clearSelection();
            return;
        }

        this._updateHpBar(unit.current_hp, unit.max_hp);

        if (unit.max_mana != null) {
            if (this.manaRow) this.manaRow.style.display = 'flex';
            this._updateManaBar(unit.current_mana ?? 0, unit.max_mana);
        } else {
            if (this.manaRow) this.manaRow.style.display = 'none';
        }

        if (unit.is_worker && unit.worker_state) {
            if (this.workerStateRow) this.workerStateRow.style.display = 'block';
            if (this.workerStateLabel) this.workerStateLabel.textContent = unit.worker_state;
        }

        // Phase gate: only update disabled state when sell button is visible
        if (this.sellBtn && this.sellBtn.style.display !== 'none') {
            this.sellBtn.disabled = gamePhase !== 'Build';
        }
    }

    /**
     * Called when a UnitInfo response arrives from the server.
     * Always writes to cache; only renders to DOM when the entity matches
     * the current selection (stale-response guard).
     */
    applyStaticInfo(info: UnitStaticInfo): void {
        this.staticInfoCache.set(info.entity_id, info);
        if (info.entity_id !== this._selectedEntityId) return;
        this._renderStaticInfo(info);
    }

    get selectedEntityId(): number | null {
        return this._selectedEntityId;
    }

    // ---- Private helpers ----

    private _drawPortrait(unit: Unit, myPlayerId: number): void {
        if (!this.portraitCtx) return;
        const ctx = this.portraitCtx;
        const cx = 40, cy = 40;
        const s = PORTRAIT_UNIT_SIZE;

        ctx.clearRect(0, 0, 80, 80);
        ctx.fillStyle = unit.is_enemy
            ? '#2E8B57'
            : unit.owner_id === myPlayerId ? '#88F' : '#F88';

        if (unit.shape === 'Square') {
            ctx.fillRect(cx - (s / 2 - 10), cy - (s / 2 - 10), s - 20, s - 20);
        } else if (unit.shape === 'Circle') {
            ctx.beginPath();
            ctx.arc(cx, cy, s / 2 - 10, 0, 2 * Math.PI);
            ctx.fill();
        } else if (unit.shape === 'Triangle') {
            ctx.beginPath();
            ctx.moveTo(cx, cy - (s / 2 - 10));
            ctx.lineTo(cx - (s / 2 - 10), cy + (s / 2 - 10));
            ctx.lineTo(cx + (s / 2 - 10), cy + (s / 2 - 10));
            ctx.closePath();
            ctx.fill();
        }
    }

    private _updateHpBar(current: number, max: number): void {
        const pct = max > 0 ? Math.max(0, Math.min(1, current / max)) * 100 : 0;
        if (this.hpFill) this.hpFill.style.width = `${pct}%`;
        if (this.hpLabel) this.hpLabel.textContent = `${Math.round(current)}/${Math.round(max)}`;
    }

    private _updateManaBar(current: number, max: number): void {
        const pct = max > 0 ? Math.max(0, Math.min(1, current / max)) * 100 : 0;
        if (this.manaFill) this.manaFill.style.width = `${pct}%`;
        if (this.manaLabel) this.manaLabel.textContent = `${Math.round(current)}/${Math.round(max)}`;
    }

    private _renderStaticInfo(info: UnitStaticInfo): void {
        if (!this.statsSection) return;

        const rows: string[] = [];
        if (info.attack_damage !== null)
            rows.push(`<div class="stat-row"><span>Damage</span><span>${info.attack_damage}</span></div>`);
        if (info.attack_rate !== null)
            rows.push(`<div class="stat-row"><span>Rate</span><span>${info.attack_rate.toFixed(2)}/s</span></div>`);
        if (info.attack_range !== null)
            rows.push(`<div class="stat-row"><span>Range</span><span>${info.attack_range}</span></div>`);
        if (info.damage_type !== null)
            rows.push(`<div class="stat-row"><span>Type</span><span>${info.damage_type}</span></div>`);
        if (info.armor !== null)
            rows.push(`<div class="stat-row"><span>Armor</span><span>${info.armor}</span></div>`);
        if (info.is_boss)
            rows.push(`<div class="stat-row boss-row"><span>BOSS</span></div>`);

        this.statsSection.innerHTML = rows.join('');

        if (info.sell_value !== null && this.sellValueEl) {
            this.sellValueEl.textContent = `${info.sell_value}g`;
        }
    }
}
