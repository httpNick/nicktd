import { SendUnitCatalogEntry } from './types';

export type UnitKind = 'Square' | 'Circle' | 'Triangle';

export interface MercenaryPanelCallbacks {
    onSend: (shape: UnitKind) => void;
}

export class MercenaryPanel {
    private container: HTMLElement;
    private callbacks: MercenaryPanelCallbacks;
    private unitListEl: HTMLElement | null;
    private _currentGold: number = 0;
    private _currentCosts: number[] = [];
    private _catalog: SendUnitCatalogEntry[] = [];

    constructor(containerElement: HTMLElement, callbacks: MercenaryPanelCallbacks) {
        this.container = containerElement;
        this.callbacks = callbacks;
        this.container.style.display = 'none';

        const q = <T extends Element>(sel: string) =>
            containerElement.querySelector<T>(sel);

        this.unitListEl = q<HTMLElement>('[data-merc="unit-list"]');

        q<HTMLButtonElement>('[data-merc="close-btn"]')?.addEventListener('click', () => this.hide());

        this._renderPlaceholder();
    }

    show(): void { this.container.style.display = 'block'; }
    hide(): void { this.container.style.display = 'none'; }

    toggle(): void {
        if (this.isVisible) { this.hide(); } else { this.show(); }
    }

    get isVisible(): boolean { return this.container.style.display !== 'none'; }

    /** Store the server-driven catalog and (re)build rows from it. Catalog
     * position i pairs with `next_send_costs[i]` — the order contract. */
    setCatalog(entries: SendUnitCatalogEntry[]): void {
        this._catalog = entries;
        this._currentCosts = entries.map(e => e.base_cost);
        this._renderUnits();
        this._applyAffordability();
    }

    /** Update gold and the server-computed escalating prices together.
     * `nextSendCosts[i]` maps to `catalog[i]` by position. */
    updatePlayer(gold: number, nextSendCosts?: number[]): void {
        this._currentGold = gold;
        if (nextSendCosts && nextSendCosts.length === this._catalog.length) {
            this._currentCosts = nextSendCosts;
        }
        if (!this.unitListEl) return;
        this.unitListEl.querySelectorAll<HTMLButtonElement>('[data-shape]').forEach((btn, i) => {
            const cost = this._currentCosts[i];
            const costEl = btn.parentElement?.querySelector<HTMLElement>('.merc-unit-cost');
            if (costEl) costEl.textContent = `${cost}g`;
            if (gold >= cost) {
                btn.removeAttribute('data-unaffordable');
            } else {
                btn.setAttribute('data-unaffordable', 'true');
            }
        });
    }

    updateGold(gold: number): void {
        this.updatePlayer(gold);
    }

    private _applyAffordability(): void {
        this.updatePlayer(this._currentGold);
    }

    private _renderPlaceholder(): void {
        if (!this.unitListEl) return;
        this.unitListEl.innerHTML = '<div class="merc-unit-placeholder">Waiting for catalog…</div>';
    }

    private _renderUnits(): void {
        if (!this.unitListEl) return;
        if (this._catalog.length === 0) {
            this._renderPlaceholder();
            return;
        }
        this.unitListEl.innerHTML = this._catalog.map(p => {
            return [
                `<div class="merc-unit-row">`,
                `<span class="merc-unit-name">${p.name}</span>`,
                `<span class="merc-unit-cost">${p.base_cost}g</span>`,
                `<span class="merc-unit-income">+${p.income}/round</span>`,
                `<button class="merc-send-btn" data-shape="${p.shape}" data-unaffordable="true">Send</button>`,
                `</div>`,
            ].join('');
        }).join('');

        this.unitListEl.querySelectorAll<HTMLButtonElement>('[data-shape]').forEach(btn => {
            btn.addEventListener('click', () => {
                if (btn.getAttribute('data-unaffordable') === 'true') return;
                const shape = btn.getAttribute('data-shape') as UnitKind;
                if (shape) this.callbacks.onSend(shape);
            });
        });
    }
}
