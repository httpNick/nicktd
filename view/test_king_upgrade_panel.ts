import assert from 'node:assert';
import test from 'node:test';
import { KingUpgradePanel } from './king_upgrade_panel.js';

// ---- Mock helpers ----

function makeMockElement(): any {
    const el: any = {
        style: {} as Record<string, string>,
        innerHTML: '',
        textContent: '',
        children: [] as any[],
        _queried: {} as Record<string, any>,
        addEventListener: (_type: string, _cb: () => void) => {},
        querySelector(selector: string) {
            const cls = selector.replace('.', '');
            if (!el._queried[cls]) {
                el._queried[cls] = makeMockElement();
            }
            return el._queried[cls];
        },
    };
    return el;
}

function makePanel() {
    const container = makeMockElement();
    let upgradeCalls = 0;
    const panel = new KingUpgradePanel({
        container,
        onUpgrade: () => {
            upgradeCalls++;
        },
    });
    return { panel, container, getUpgradeCalls: () => upgradeCalls };
}

test('king HP display rounds fractional current and max HP to whole numbers', () => {
    const { panel, container } = makePanel();

    // Reproduces the reported bug: server HP drifts to a fractional f32
    // value (e.g. repeated 2.2-damage mage melee hits) — the panel must
    // never show that fraction to the player.
    panel.update({ currentTier: 2, currentHp: 531.19995, maxHp: 600 }, 100, 'combat');

    const hpEl = container.querySelector('.king-panel-hp');
    assert.strictEqual(hpEl.textContent, 'HP: 531 / 600');
});

test('king HP display rounds a fractional max HP too', () => {
    const { panel, container } = makePanel();

    panel.update({ currentTier: 1, currentHp: 250, maxHp: 349.6 }, 100, 'combat');

    const hpEl = container.querySelector('.king-panel-hp');
    assert.strictEqual(hpEl.textContent, 'HP: 250 / 350');
});
