import assert from 'node:assert';
import test from 'node:test';
import { MercenaryPanel } from './mercenary_panel.js';
import { SendUnitCatalogEntry } from './types.js';

// ---- Fixtures ----

/** Matches the real server catalog values (see server unit_config.rs). */
const CATALOG: SendUnitCatalogEntry[] = [
    { shape: 'Square', name: 'Scout', base_cost: 8, income: 1, bounty: 6 },
    { shape: 'Triangle', name: 'Raider', base_cost: 20, income: 2, bounty: 12 },
    { shape: 'Circle', name: 'Siege Mage', base_cost: 50, income: 4, bounty: 30 },
];

// ---- Mock helpers ----

type ButtonState = { unaffordable: boolean; _handler: (() => void) | null; costText: string };

function makePanel() {
    // Persistent button state: survives multiple querySelectorAll calls so
    // handlers registered by _renderUnits are still present when updateGold
    // calls querySelectorAll to update affordability attributes.
    const buttonState = new Map<string, ButtonState>();
    const shapeOrder: string[] = [];
    let lastParsedHtml = '';

    const unitListEl = {
        innerHTML: '',
        addEventListener(_evt: string, _fn: unknown) { /* no-op: delegation removed */ },
        querySelectorAll(sel: string) {
            if (sel !== '[data-shape]') return [];
            // Re-parse only when innerHTML changed (i.e. _renderUnits was called)
            if (this.innerHTML !== lastParsedHtml) {
                lastParsedHtml = this.innerHTML;
                buttonState.clear();
                shapeOrder.length = 0;
                // Each unit row: cost span appears before the data-shape button.
                for (const m of this.innerHTML.matchAll(
                    /<span class="merc-unit-cost">([^<]*)<\/span>[\s\S]*?data-shape="(\w+)"([^>]*?)>/g
                )) {
                    const costText = m[1];
                    const shape = m[2];
                    const unaffordable = m[0].includes('data-unaffordable="true"');
                    buttonState.set(shape, { unaffordable, _handler: null, costText });
                    shapeOrder.push(shape);
                }
            }
            return shapeOrder.map((shape) => {
                const state = buttonState.get(shape)!;
                return {
                    getAttribute: (a: string) => {
                        if (a === 'data-shape') return shape;
                        if (a === 'data-unaffordable') return state.unaffordable ? 'true' : null;
                        return null;
                    },
                    setAttribute: (a: string, v: string) => {
                        if (a === 'data-unaffordable') state.unaffordable = v === 'true';
                    },
                    removeAttribute: (a: string) => {
                        if (a === 'data-unaffordable') state.unaffordable = false;
                    },
                    addEventListener: (evt: string, fn: () => void) => {
                        if (evt === 'click') state._handler = fn;
                    },
                    parentElement: {
                        querySelector: (s: string) => {
                            if (s !== '.merc-unit-cost') return null;
                            return {
                                get textContent() { return state.costText; },
                                set textContent(v: string) { state.costText = v; },
                            };
                        },
                    },
                };
            });
        },
        simulateSendClick(shape: string) {
            buttonState.get(shape)?._handler?.();
        },
        isUnaffordable(shape: string) {
            return buttonState.get(shape)?.unaffordable ?? false;
        },
        costText(shape: string) {
            return buttonState.get(shape)?.costText ?? null;
        },
    };

    const closeBtn: {
        _handlers: Record<string, () => void>;
        addEventListener: (evt: string, fn: () => void) => void;
    } = {
        _handlers: {},
        addEventListener(evt, fn) { this._handlers[evt] = fn; },
    };

    const elemMap: Record<string, unknown> = {
        '[data-merc="unit-list"]': unitListEl,
        '[data-merc="close-btn"]': closeBtn,
    };

    const container = {
        style: { display: 'none' },
        querySelector: (sel: string) => elemMap[sel] ?? null,
    };

    const onSendCalls: string[] = [];
    const panel = new MercenaryPanel(container as unknown as HTMLElement, {
        onSend: (shape) => onSendCalls.push(shape),
    });

    return { panel, container, unitListEl, closeBtn, onSendCalls };
}

// ---- Tests ----

test('MercenaryPanel', async (t) => {

    // --- Visibility management ---

    await t.test('panel hidden initially', () => {
        const { container } = makePanel();
        assert.strictEqual(container.style.display, 'none');
    });

    await t.test('show() makes panel visible', () => {
        const { panel, container } = makePanel();
        panel.show();
        assert.notStrictEqual(container.style.display, 'none');
    });

    await t.test('hide() makes panel invisible', () => {
        const { panel, container } = makePanel();
        panel.show();
        panel.hide();
        assert.strictEqual(container.style.display, 'none');
    });

    await t.test('toggle() shows when hidden', () => {
        const { panel } = makePanel();
        assert.strictEqual(panel.isVisible, false);
        panel.toggle();
        assert.strictEqual(panel.isVisible, true);
    });

    await t.test('toggle() hides when visible', () => {
        const { panel } = makePanel();
        panel.show();
        panel.toggle();
        assert.strictEqual(panel.isVisible, false);
    });

    await t.test('isVisible returns false when hidden', () => {
        const { panel } = makePanel();
        assert.strictEqual(panel.isVisible, false);
    });

    await t.test('isVisible returns true when shown', () => {
        const { panel } = makePanel();
        panel.show();
        assert.strictEqual(panel.isVisible, true);
    });

    await t.test('close button click hides panel', () => {
        const { panel, closeBtn, container } = makePanel();
        panel.show();
        closeBtn._handlers['click']?.();
        assert.strictEqual(container.style.display, 'none');
    });

    // --- Empty state before catalog arrives ---

    await t.test('panel renders no send buttons before setCatalog is called', () => {
        const { unitListEl } = makePanel();
        assert.strictEqual(unitListEl.innerHTML.includes('data-shape='), false);
    });

    // --- setCatalog: unit list rendering ---

    await t.test('setCatalog renders all unit names', () => {
        const { panel, unitListEl } = makePanel();
        panel.setCatalog(CATALOG);
        CATALOG.forEach(p => {
            assert.ok(unitListEl.innerHTML.includes(p.name), `Missing unit name: ${p.name}`);
        });
    });

    await t.test('setCatalog renders base costs', () => {
        const { panel, unitListEl } = makePanel();
        panel.setCatalog(CATALOG);
        CATALOG.forEach(p => {
            assert.ok(unitListEl.innerHTML.includes(`${p.base_cost}`), `Missing cost for ${p.name}`);
        });
    });

    await t.test('setCatalog renders income values', () => {
        const { panel, unitListEl } = makePanel();
        panel.setCatalog(CATALOG);
        CATALOG.forEach(p => {
            assert.ok(unitListEl.innerHTML.includes(`${p.income}`), `Missing income for ${p.name}`);
        });
    });

    await t.test('setCatalog renders data-shape attributes for all shapes', () => {
        const { panel, unitListEl } = makePanel();
        panel.setCatalog(CATALOG);
        CATALOG.forEach(p => {
            assert.ok(
                unitListEl.innerHTML.includes(`data-shape="${p.shape}"`),
                `Missing data-shape="${p.shape}"`
            );
        });
    });

    // --- onSend callback ---

    await t.test('onSend fires with Square when Square send button clicked', () => {
        const { panel, onSendCalls, unitListEl } = makePanel();
        panel.setCatalog(CATALOG);
        panel.updateGold(100);
        unitListEl.simulateSendClick('Square');
        assert.strictEqual(onSendCalls.length, 1);
        assert.strictEqual(onSendCalls[0], 'Square');
    });

    await t.test('onSend fires with Triangle when Triangle send button clicked', () => {
        const { panel, onSendCalls, unitListEl } = makePanel();
        panel.setCatalog(CATALOG);
        panel.updateGold(100);
        unitListEl.simulateSendClick('Triangle');
        assert.strictEqual(onSendCalls[0], 'Triangle');
    });

    await t.test('onSend fires with Circle when Circle send button clicked', () => {
        const { panel, onSendCalls, unitListEl } = makePanel();
        panel.setCatalog(CATALOG);
        panel.updateGold(100);
        unitListEl.simulateSendClick('Circle');
        assert.strictEqual(onSendCalls[0], 'Circle');
    });

    await t.test('onSend does not fire when unaffordable button is clicked', () => {
        const { panel, onSendCalls, unitListEl } = makePanel();
        panel.setCatalog(CATALOG);
        panel.updateGold(0); // all buttons unaffordable
        unitListEl.simulateSendClick('Square');
        assert.strictEqual(onSendCalls.length, 0);
    });

    // --- updateGold: affordability ---

    await t.test('updateGold with 0 gold marks all buttons unaffordable', () => {
        const { panel, unitListEl } = makePanel();
        panel.setCatalog(CATALOG);
        panel.updateGold(0);
        ['Square', 'Triangle', 'Circle'].forEach(shape => {
            assert.strictEqual(unitListEl.isUnaffordable(shape), true, `${shape} should be unaffordable with 0g`);
        });
    });

    await t.test('updateGold with 100 gold enables all buttons', () => {
        const { panel, unitListEl } = makePanel();
        panel.setCatalog(CATALOG);
        panel.updateGold(100);
        assert.strictEqual(unitListEl.isUnaffordable('Circle'), false, 'Circle should be affordable with 100g');
        assert.strictEqual(unitListEl.isUnaffordable('Square'), false, 'Square should be affordable with 100g');
    });

    await t.test('updateGold with 20 gold marks Circle unaffordable but not Square or Triangle', () => {
        const { panel, unitListEl } = makePanel();
        panel.setCatalog(CATALOG);
        panel.updateGold(20);
        assert.strictEqual(unitListEl.isUnaffordable('Circle'), true, 'Circle (50g) should be unaffordable with 20g');
        assert.strictEqual(unitListEl.isUnaffordable('Square'), false, 'Square (8g) should be affordable with 20g');
        assert.strictEqual(unitListEl.isUnaffordable('Triangle'), false, 'Triangle (20g) should be affordable with 20g');
    });

    // --- updatePlayer: server-driven dynamic prices ---

    await t.test('updatePlayer rewrites cost labels and affordability from server prices', () => {
        const { panel, unitListEl } = makePanel();
        panel.setCatalog(CATALOG);
        panel.updatePlayer(10, [7, 29, 63]);
        assert.strictEqual(unitListEl.costText('Square'), '7g');
        assert.strictEqual(unitListEl.isUnaffordable('Square'), false, '10 >= 7 should be affordable');
        assert.strictEqual(unitListEl.costText('Triangle'), '29g');
        assert.strictEqual(unitListEl.isUnaffordable('Triangle'), true, '10 < 29 should be unaffordable');
        assert.strictEqual(unitListEl.costText('Circle'), '63g');
        assert.strictEqual(unitListEl.isUnaffordable('Circle'), true, '10 < 63 should be unaffordable');
    });

    await t.test('updatePlayer without nextSendCosts keeps previously known costs', () => {
        const { panel, unitListEl } = makePanel();
        panel.setCatalog(CATALOG);
        panel.updatePlayer(100, [7, 29, 63]);
        panel.updatePlayer(5); // gold-only update, e.g. via updateGold pathway
        assert.strictEqual(unitListEl.costText('Square'), '7g', 'cost should persist from last server update');
        assert.strictEqual(unitListEl.isUnaffordable('Square'), true, '5 < 7 should now be unaffordable');
    });

    // --- Scalability contract: catalog shape/order/length is not hardcoded ---

    await t.test('a catalog with different length and order still renders rows and maps costs by position', () => {
        const { panel, unitListEl } = makePanel();
        const reversedTwo: SendUnitCatalogEntry[] = [
            { shape: 'Circle', name: 'Siege Mage', base_cost: 50, income: 4, bounty: 30 },
            { shape: 'Square', name: 'Scout', base_cost: 8, income: 1, bounty: 6 },
        ];
        panel.setCatalog(reversedTwo);

        // Renders exactly the two rows, in catalog order.
        assert.ok(unitListEl.innerHTML.includes('data-shape="Circle"'));
        assert.ok(unitListEl.innerHTML.includes('data-shape="Square"'));
        assert.ok(!unitListEl.innerHTML.includes('data-shape="Triangle"'));

        // Costs map by catalog position, not by shape identity: position 0
        // (Circle) gets 15, position 1 (Square) gets 3 — inverted from the
        // shapes' usual base costs, proving positional (not shape-keyed) mapping.
        panel.updatePlayer(20, [15, 3]);
        assert.strictEqual(unitListEl.costText('Circle'), '15g');
        assert.strictEqual(unitListEl.costText('Square'), '3g');
        assert.strictEqual(unitListEl.isUnaffordable('Circle'), false, '20 >= 15 should be affordable');
        assert.strictEqual(unitListEl.isUnaffordable('Square'), false, '20 >= 3 should be affordable');
    });
});

// --- Task 5.3: UI panel smoke test ---

test('MercenaryPanel smoke test (Task 5.3)', async (t) => {
    await t.test('panel opens when building is clicked (toggle) and is hidden by default', () => {
        const { panel } = makePanel();
        // Starts hidden
        assert.strictEqual(panel.isVisible, false, 'Panel must be hidden before building click');
        // Simulate building click → toggle
        panel.toggle();
        assert.strictEqual(panel.isVisible, true, 'Panel must open after building click');
    });

    await t.test('panel displays correct names for all unit types', () => {
        const { panel, unitListEl } = makePanel();
        panel.setCatalog(CATALOG);
        // Verify Scout, Raider, Siege Mage are all present
        CATALOG.forEach(p => {
            assert.ok(unitListEl.innerHTML.includes(p.name), `Panel must display unit name: ${p.name}`);
        });
    });

    await t.test('panel displays correct gold costs for all unit types', () => {
        const { panel, unitListEl } = makePanel();
        panel.setCatalog(CATALOG);
        panel.updateGold(999);
        CATALOG.forEach(p => {
            assert.ok(
                unitListEl.innerHTML.includes(`${p.base_cost}g`),
                `Panel must display cost "${p.base_cost}g" for ${p.name}`
            );
        });
    });

    await t.test('panel displays permanent income values for all unit types', () => {
        const { panel, unitListEl } = makePanel();
        panel.setCatalog(CATALOG);
        CATALOG.forEach(p => {
            assert.ok(
                unitListEl.innerHTML.includes(`+${p.income}`),
                `Panel must display income "+${p.income}" for ${p.name}`
            );
        });
    });

    await t.test('panel can be closed and re-opened', () => {
        const { panel } = makePanel();
        panel.toggle();
        assert.strictEqual(panel.isVisible, true, 'Should open');
        panel.toggle();
        assert.strictEqual(panel.isVisible, false, 'Should close');
        panel.toggle();
        assert.strictEqual(panel.isVisible, true, 'Should re-open');
    });

    await t.test('sending a unit dispatches correct shape to callback', () => {
        const { panel, onSendCalls, unitListEl } = makePanel();
        panel.setCatalog(CATALOG);
        panel.updateGold(100);
        ['Square', 'Triangle', 'Circle'].forEach(shape => {
            unitListEl.simulateSendClick(shape);
        });
        assert.deepStrictEqual(onSendCalls, ['Square', 'Triangle', 'Circle'],
            'All three shapes dispatched in order');
    });
});
