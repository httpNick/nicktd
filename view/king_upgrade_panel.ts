interface KingUpgradeTier {
    readonly tier: number;
    readonly cost: number;
    readonly hpDelta: number;
    readonly newDamage: number;
    readonly incomeDelta: number;
}

const KING_UPGRADE_TIERS: KingUpgradeTier[] = [
    { tier: 1, cost: 75,  hpDelta: 100, newDamage: 20, incomeDelta: 2 },
    { tier: 2, cost: 100, hpDelta: 150, newDamage: 25, incomeDelta: 3 },
    { tier: 3, cost: 150, hpDelta: 250, newDamage: 30, incomeDelta: 4 },
    { tier: 4, cost: 200, hpDelta: 350, newDamage: 35, incomeDelta: 5 },
];

export interface KingPanelState {
    currentTier: number;
    currentHp: number;
    maxHp: number;
}

export interface KingUpgradePanelOptions {
    container: HTMLElement;
    onUpgrade: () => void;
}

export class KingUpgradePanel {
    private container: HTMLElement;
    private onUpgrade: () => void;

    private titleEl: HTMLElement;
    private hpEl: HTMLElement;
    private upgradeInfoEl: HTMLElement;
    private upgradeBtn: HTMLButtonElement;

    constructor(options: KingUpgradePanelOptions) {
        this.container = options.container;
        this.onUpgrade = options.onUpgrade;

        this.container.style.display = 'none';

        this.container.innerHTML = `
            <div class="king-panel-header">
                <span class="king-panel-title">King - Tier 0</span>
            </div>
            <div class="king-panel-hp">HP: -- / --</div>
            <div class="king-panel-upgrade-info"></div>
            <button class="king-upgrade-btn" style="display: none;">Upgrade</button>
        `;

        this.titleEl = this.container.querySelector('.king-panel-title') as HTMLElement;
        this.hpEl = this.container.querySelector('.king-panel-hp') as HTMLElement;
        this.upgradeInfoEl = this.container.querySelector('.king-panel-upgrade-info') as HTMLElement;
        this.upgradeBtn = this.container.querySelector('.king-upgrade-btn') as HTMLButtonElement;

        this.upgradeBtn.addEventListener('click', () => {
            this.onUpgrade();
        });
    }

    show(): void {
        this.container.style.display = 'block';
    }

    hide(): void {
        this.container.style.display = 'none';
    }

    update(state: KingPanelState, playerGold: number, phase: string): void {
        this.show();

        this.titleEl.textContent = `King - Tier ${state.currentTier}`;
        this.hpEl.textContent = `HP: ${state.currentHp} / ${state.maxHp}`;

        const MAX_TIER = 4;
        if (state.currentTier >= MAX_TIER) {
            this.upgradeInfoEl.textContent = 'Max Tier Reached';
            this.upgradeBtn.style.display = 'none';
        } else {
            const nextTier = KING_UPGRADE_TIERS[state.currentTier]; // currentTier is 0-based index
            this.upgradeInfoEl.textContent =
                `Cost: ${nextTier.cost}g | +${nextTier.hpDelta} HP | Damage: ${nextTier.newDamage} | +${nextTier.incomeDelta} income`;

            this.upgradeBtn.style.display = 'inline-block';
            const canAfford = playerGold >= nextTier.cost;
            const isBuildPhase = phase === 'Build';
            this.upgradeBtn.disabled = !canAfford || !isBuildPhase;
        }
    }
}
