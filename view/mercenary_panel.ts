export type Shape = 'Square' | 'Circle' | 'Triangle';

export interface SentUnitProfile {
    shape: Shape;
    name: string;
    cost: number;
    income: number;
    bounty: number;
}

export const SENT_UNIT_PROFILES: SentUnitProfile[] = [
    { shape: 'Square', name: 'Scout', cost: 8, income: 1, bounty: 6 },
    { shape: 'Triangle', name: 'Raider', cost: 20, income: 2, bounty: 12 },
    { shape: 'Circle', name: 'Siege Mage', cost: 50, income: 4, bounty: 30 },
];

/** Server order for next_send_costs: Square, Triangle, Circle. */
const SHAPE_INDEX: Record<Shape, number> = { Square: 0, Triangle: 1, Circle: 2 };

export interface MercenaryPanelCallbacks {
    onSend: (shape: Shape) => void;
}

export class MercenaryPanel {
    private container: HTMLElement;
    private callbacks: MercenaryPanelCallbacks;
    private unitListEl: HTMLElement | null;
    private _currentGold: number = 0;
    private _currentCosts: number[] = SENT_UNIT_PROFILES.map(p => p.cost);

    constructor(containerElement: HTMLElement, callbacks: MercenaryPanelCallbacks) {
        this.container = containerElement;
        this.callbacks = callbacks;
        this.container.style.display = 'none';

        const q = <T extends Element>(sel: string) =>
            containerElement.querySelector<T>(sel);

        this.unitListEl = q<HTMLElement>('[data-merc="unit-list"]');

        q<HTMLButtonElement>('[data-merc="close-btn"]')?.addEventListener('click', () => this.hide());

        this._renderUnits();
    }

    show(): void { this.container.style.display = 'block'; }
    hide(): void { this.container.style.display = 'none'; }

    toggle(): void {
        if (this.isVisible) { this.hide(); } else { this.show(); }
    }

    get isVisible(): boolean { return this.container.style.display !== 'none'; }

    /** Update gold and the server-computed escalating prices together. */
    updatePlayer(gold: number, nextSendCosts?: number[]): void {
        this._currentGold = gold;
        if (nextSendCosts && nextSendCosts.length === 3) {
            this._currentCosts = nextSendCosts;
        }
        if (!this.unitListEl) return;
        this.unitListEl.querySelectorAll<HTMLButtonElement>('[data-shape]').forEach(btn => {
            const shape = btn.getAttribute('data-shape') as Shape;
            const cost = this._currentCosts[SHAPE_INDEX[shape]];
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

    private _renderUnits(): void {
        if (!this.unitListEl) return;
        this.unitListEl.innerHTML = SENT_UNIT_PROFILES.map(p => {
            return [
                `<div class="merc-unit-row">`,
                `<span class="merc-unit-name">${p.name}</span>`,
                `<span class="merc-unit-cost">${p.cost}g</span>`,
                `<span class="merc-unit-income">+${p.income}/round</span>`,
                `<button class="merc-send-btn" data-shape="${p.shape}" data-unaffordable="true">Send</button>`,
                `</div>`,
            ].join('');
        }).join('');

        this.unitListEl.querySelectorAll<HTMLButtonElement>('[data-shape]').forEach(btn => {
            btn.addEventListener('click', () => {
                if (btn.getAttribute('data-unaffordable') === 'true') return;
                const shape = btn.getAttribute('data-shape') as Shape;
                if (shape) this.callbacks.onSend(shape);
            });
        });
    }
}
