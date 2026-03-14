export type Shape = 'Square' | 'Circle' | 'Triangle';

export interface SentUnitProfile {
    shape: Shape;
    name: string;
    cost: number;
    income: number;
    bounty: number;
}

export const SENT_UNIT_PROFILES: SentUnitProfile[] = [
    { shape: 'Square', name: 'Scout', cost: 5, income: 1, bounty: 2 },
    { shape: 'Triangle', name: 'Raider', cost: 20, income: 3, bounty: 8 },
    { shape: 'Circle', name: 'Siege Mage', cost: 50, income: 7, bounty: 20 },
];

export interface MercenaryPanelCallbacks {
    onSend: (shape: Shape) => void;
}

export class MercenaryPanel {
    private container: HTMLElement;
    private callbacks: MercenaryPanelCallbacks;
    private unitListEl: HTMLElement | null;
    private _currentGold: number = 0;

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

    updateGold(gold: number): void {
        this._currentGold = gold;
        if (!this.unitListEl) return;
        this.unitListEl.querySelectorAll<HTMLButtonElement>('[data-shape]').forEach(btn => {
            const shape = btn.getAttribute('data-shape') as Shape;
            const profile = SENT_UNIT_PROFILES.find(p => p.shape === shape);
            if (!profile) return;
            if (gold >= profile.cost) {
                btn.removeAttribute('data-unaffordable');
            } else {
                btn.setAttribute('data-unaffordable', 'true');
            }
        });
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
