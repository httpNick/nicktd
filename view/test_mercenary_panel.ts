import assert from 'node:assert';
import test from 'node:test';
import { MercenaryPanel, SENT_UNIT_PROFILES } from './mercenary_panel.js';

// ---- Mock helpers ----

type ButtonState = { unaffordable: boolean; _handler: (() => void) | null };

function makePanel() {
    // Persistent button state: survives multiple querySelectorAll calls so
    // handlers registered by _renderUnits are still present when updateGold
    // calls querySelectorAll to update affordability attributes.
    const buttonState = new Map<string, ButtonState>();
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
                for (const m of this.innerHTML.matchAll(/data-shape="(\w+)"([^>]*?)>/g)) {
                    const shape = m[1];
                    const unaffordable = m[0].includes('data-unaffordable="true"');
                    buttonState.set(shape, { unaffordable, _handler: null });
                }
            }
            return [...buttonState.entries()].map(([shape, state]) => ({
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
            }));
        },
        simulateSendClick(shape: string) {
            buttonState.get(shape)?._handler?.();
        },
        isUnaffordable(shape: string) {
            return buttonState.get(shape)?.unaffordable ?? false;
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

test('SENT_UNIT_PROFILES', async (t) => {
    await t.test('contains three unit types', () => {
        assert.strictEqual(SENT_UNIT_PROFILES.length, 3);
    });

    await t.test('contains Square shape with correct stats', () => {
        const sq = SENT_UNIT_PROFILES.find(p => p.shape === 'Square');
        assert.ok(sq, 'Square profile missing');
        assert.strictEqual(sq!.cost, 5);
        assert.strictEqual(sq!.income, 1);
        assert.strictEqual(sq!.bounty, 2);
        assert.ok(sq!.name.length > 0);
    });

    await t.test('contains Triangle shape with correct stats', () => {
        const tr = SENT_UNIT_PROFILES.find(p => p.shape === 'Triangle');
        assert.ok(tr, 'Triangle profile missing');
        assert.strictEqual(tr!.cost, 20);
        assert.strictEqual(tr!.income, 3);
        assert.strictEqual(tr!.bounty, 8);
    });

    await t.test('contains Circle shape with correct stats', () => {
        const ci = SENT_UNIT_PROFILES.find(p => p.shape === 'Circle');
        assert.ok(ci, 'Circle profile missing');
        assert.strictEqual(ci!.cost, 50);
        assert.strictEqual(ci!.income, 7);
        assert.strictEqual(ci!.bounty, 20);
    });

    await t.test('Square has highest income-per-gold ratio', () => {
        const [sq, tr, ci] = ['Square', 'Triangle', 'Circle'].map(
            s => SENT_UNIT_PROFILES.find(p => p.shape === s)!
        );
        const sqRatio = sq.income / sq.cost;
        assert.ok(sqRatio >= tr.income / tr.cost);
        assert.ok(sqRatio >= ci.income / ci.cost);
    });

    await t.test('all unit names are unique and non-empty', () => {
        const names = SENT_UNIT_PROFILES.map(p => p.name);
        assert.strictEqual(new Set(names).size, names.length);
        names.forEach(n => assert.ok(n.length > 0));
    });
});

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

    // --- Unit list rendering ---

    await t.test('unit list HTML contains all three unit names', () => {
        const { unitListEl } = makePanel();
        const names = SENT_UNIT_PROFILES.map(p => p.name);
        names.forEach(name => {
            assert.ok(unitListEl.innerHTML.includes(name), `Missing unit name: ${name}`);
        });
    });

    await t.test('unit list HTML contains all three costs', () => {
        const { unitListEl } = makePanel();
        SENT_UNIT_PROFILES.forEach(p => {
            assert.ok(unitListEl.innerHTML.includes(`${p.cost}`), `Missing cost for ${p.name}`);
        });
    });

    await t.test('unit list HTML contains income values for all units', () => {
        const { unitListEl } = makePanel();
        SENT_UNIT_PROFILES.forEach(p => {
            assert.ok(unitListEl.innerHTML.includes(`${p.income}`), `Missing income for ${p.name}`);
        });
    });

    await t.test('unit list HTML contains data-shape attributes for all shapes', () => {
        const { unitListEl } = makePanel();
        ['Square', 'Triangle', 'Circle'].forEach(shape => {
            assert.ok(
                unitListEl.innerHTML.includes(`data-shape="${shape}"`),
                `Missing data-shape="${shape}"`
            );
        });
    });

    // --- onSend callback ---

    await t.test('onSend fires with Square when Square send button clicked', () => {
        const { panel, onSendCalls, unitListEl } = makePanel();
        panel.updateGold(100);
        unitListEl.simulateSendClick('Square');
        assert.strictEqual(onSendCalls.length, 1);
        assert.strictEqual(onSendCalls[0], 'Square');
    });

    await t.test('onSend fires with Triangle when Triangle send button clicked', () => {
        const { panel, onSendCalls, unitListEl } = makePanel();
        panel.updateGold(100);
        unitListEl.simulateSendClick('Triangle');
        assert.strictEqual(onSendCalls[0], 'Triangle');
    });

    await t.test('onSend fires with Circle when Circle send button clicked', () => {
        const { panel, onSendCalls, unitListEl } = makePanel();
        panel.updateGold(100);
        unitListEl.simulateSendClick('Circle');
        assert.strictEqual(onSendCalls[0], 'Circle');
    });

    await t.test('onSend does not fire when unaffordable button is clicked', () => {
        const { panel, onSendCalls, unitListEl } = makePanel();
        panel.updateGold(0); // all buttons unaffordable
        unitListEl.simulateSendClick('Square');
        assert.strictEqual(onSendCalls.length, 0);
    });

    // --- updateGold: affordability ---

    await t.test('updateGold with 0 gold marks all buttons unaffordable', () => {
        const { panel, unitListEl } = makePanel();
        panel.updateGold(0);
        ['Square', 'Triangle', 'Circle'].forEach(shape => {
            assert.strictEqual(unitListEl.isUnaffordable(shape), true, `${shape} should be unaffordable with 0g`);
        });
    });

    await t.test('updateGold with 100 gold enables all buttons', () => {
        const { panel, unitListEl } = makePanel();
        panel.updateGold(100);
        assert.strictEqual(unitListEl.isUnaffordable('Circle'), false, 'Circle should be affordable with 100g');
        assert.strictEqual(unitListEl.isUnaffordable('Square'), false, 'Square should be affordable with 100g');
    });

    await t.test('updateGold with 20 gold marks Circle unaffordable but not Square or Triangle', () => {
        const { panel, unitListEl } = makePanel();
        panel.updateGold(20);
        assert.strictEqual(unitListEl.isUnaffordable('Circle'), true, 'Circle (50g) should be unaffordable with 20g');
        assert.strictEqual(unitListEl.isUnaffordable('Square'), false, 'Square (5g) should be affordable with 20g');
        assert.strictEqual(unitListEl.isUnaffordable('Triangle'), false, 'Triangle (20g) should be affordable with 20g');
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
        const { unitListEl } = makePanel();
        const expectedNames = SENT_UNIT_PROFILES.map(p => p.name);
        // Verify Scout, Raider, Siege Mage are all present
        expectedNames.forEach(name => {
            assert.ok(unitListEl.innerHTML.includes(name), `Panel must display unit name: ${name}`);
        });
    });

    await t.test('panel displays correct gold costs for all unit types', () => {
        const { panel, unitListEl } = makePanel();
        panel.updateGold(999);
        SENT_UNIT_PROFILES.forEach(p => {
            assert.ok(
                unitListEl.innerHTML.includes(`${p.cost}g`),
                `Panel must display cost "${p.cost}g" for ${p.name}`
            );
        });
    });

    await t.test('panel displays permanent income values for all unit types', () => {
        const { unitListEl } = makePanel();
        SENT_UNIT_PROFILES.forEach(p => {
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
        panel.updateGold(100);
        ['Square', 'Triangle', 'Circle'].forEach(shape => {
            unitListEl.simulateSendClick(shape);
        });
        assert.deepStrictEqual(onSendCalls, ['Square', 'Triangle', 'Circle'],
            'All three shapes dispatched in order');
    });
});
