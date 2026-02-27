import assert from 'node:assert';
import test from 'node:test';
import { UnitInfoPanel, Unit, UnitStaticInfo } from './unit_info_panel.js';

// ---- Mock helpers ----

function makeMockCtx() {
    return {
        clearRect: (_x: number, _y: number, _w: number, _h: number) => {},
        fillRect: (_x: number, _y: number, _w: number, _h: number) => {},
        beginPath: () => {},
        arc: (_cx: number, _cy: number, _r: number, _s: number, _e: number) => {},
        fill: () => {},
        moveTo: (_x: number, _y: number) => {},
        lineTo: (_x: number, _y: number) => {},
        closePath: () => {},
        fillStyle: '' as string,
    };
}

type MockCtx = ReturnType<typeof makeMockCtx>;

function makePanel() {
    const ctx = makeMockCtx();

    const canvas = {
        style: { display: '' },
        width: 80,
        height: 80,
        getContext: (_type: string) => ctx,
    };

    const nameEl = { style: {} as Record<string, string>, textContent: '' };
    const hpFill  = { style: { width: '100%' } };
    const hpLabel  = { style: {} as Record<string, string>, textContent: '' };
    const manaRow  = { style: { display: 'none' } };
    const manaFill  = { style: { width: '100%' } };
    const manaLabel  = { style: {} as Record<string, string>, textContent: '' };
    const workerStateRow  = { style: { display: 'none' } };
    const workerStateLabel  = { style: {} as Record<string, string>, textContent: '' };
    const statsSection  = { style: {} as Record<string, string>, innerHTML: '' };
    const sellValueEl  = { textContent: '' };
    const sellBtn: {
        style: { display: string };
        disabled: boolean;
        _handlers: Record<string, () => void>;
        addEventListener: (evt: string, fn: () => void) => void;
    } = {
        style: { display: 'none' },
        disabled: false,
        _handlers: {},
        addEventListener(evt, fn) { this._handlers[evt] = fn; },
    };

    const elemMap: Record<string, unknown> = {
        '[data-panel="portrait"]':           canvas,
        '[data-panel="name"]':               nameEl,
        '[data-panel="hp-fill"]':            hpFill,
        '[data-panel="hp-label"]':           hpLabel,
        '[data-panel="mana-row"]':           manaRow,
        '[data-panel="mana-fill"]':          manaFill,
        '[data-panel="mana-label"]':         manaLabel,
        '[data-panel="worker-state-row"]':   workerStateRow,
        '[data-panel="worker-state-label"]': workerStateLabel,
        '[data-panel="stats"]':              statsSection,
        '[data-panel="sell-btn"]':           sellBtn,
        '[data-panel="sell-value"]':         sellValueEl,
    };

    const container = {
        style: { display: 'none' },
        querySelector: (sel: string) => elemMap[sel] ?? null,
    };

    const onSellCalls: number[] = [];
    const panel = new UnitInfoPanel(container as unknown as HTMLElement, {
        onSell: (id) => onSellCalls.push(id),
    });

    return {
        panel,
        onSellCalls,
        container, canvas, ctx,
        nameEl, hpFill, hpLabel,
        manaRow, manaFill, manaLabel,
        workerStateRow, workerStateLabel,
        statsSection, sellBtn, sellValueEl,
    };
}

function makeTower(overrides: Partial<Unit> = {}): Unit {
    return {
        id: 42, shape: 'Circle', x: 90, y: 90,
        owner_id: 1, is_enemy: false,
        current_hp: 100, max_hp: 100,
        is_worker: false,
        ...overrides,
    };
}

function makeEnemy(overrides: Partial<Unit> = {}): Unit {
    return {
        id: 99, shape: 'Square', x: 900, y: 270,
        owner_id: -1, is_enemy: true,
        current_hp: 80, max_hp: 100,
        is_worker: false,
        ...overrides,
    };
}

function makeStaticInfo(overrides: Partial<UnitStaticInfo> = {}): UnitStaticInfo {
    return {
        entity_id: 42,
        attack_damage: 10,
        attack_rate: 0.8,
        attack_range: 150,
        damage_type: 'FireMagical',
        armor: null,
        is_boss: false,
        sell_value: 56,
        ...overrides,
    };
}

// ---- Tests ----

test('UnitInfoPanel', async (t) => {

    // --- 5.1: Scaffold ---

    await t.test('5.1 panel hidden initially', () => {
        const { container } = makePanel();
        assert.strictEqual(container.style.display, 'none');
    });

    await t.test('5.1 selectUnit shows container and sets selectedEntityId', () => {
        const { panel, container } = makePanel();
        panel.selectUnit(makeTower(), 1, 'Build');
        assert.strictEqual(container.style.display, 'flex');
        assert.strictEqual(panel.selectedEntityId, 42);
    });

    await t.test('5.1 selectUnit sets name element text', () => {
        const { panel, nameEl } = makePanel();
        panel.selectUnit(makeTower({ shape: 'Circle' }), 1, 'Build');
        assert.ok((nameEl.textContent ?? '').includes('Circle'));
    });

    await t.test('5.1 clearSelection hides panel and nulls selectedEntityId', () => {
        const { panel, container } = makePanel();
        panel.selectUnit(makeTower(), 1, 'Build');
        panel.clearSelection();
        assert.strictEqual(container.style.display, 'none');
        assert.strictEqual(panel.selectedEntityId, null);
    });

    await t.test('5.1 portrait fillStyle set for own tower (Circle)', () => {
        const { panel, ctx } = makePanel();
        panel.selectUnit(makeTower({ shape: 'Circle', owner_id: 1 }), 1, 'Build');
        assert.strictEqual((ctx as MockCtx).fillStyle, '#88F');
    });

    await t.test('5.1 portrait fillStyle set for enemy', () => {
        const { panel, ctx } = makePanel();
        panel.selectUnit(makeEnemy({ shape: 'Square' }), 1, 'Build');
        assert.strictEqual((ctx as MockCtx).fillStyle, '#2E8B57');
    });

    await t.test('5.1 portrait fillStyle set for opponent tower', () => {
        const { panel, ctx } = makePanel();
        panel.selectUnit(makeTower({ owner_id: 2 }), 1, 'Build');
        assert.strictEqual((ctx as MockCtx).fillStyle, '#F88');
    });

    // --- 5.2: Live state synchronisation ---

    await t.test('5.2 syncDynamicState clears selection when entity absent (despawn)', () => {
        const { panel, container } = makePanel();
        panel.selectUnit(makeTower(), 1, 'Build');
        panel.syncDynamicState([], 'Build');
        assert.strictEqual(panel.selectedEntityId, null);
        assert.strictEqual(container.style.display, 'none');
    });

    await t.test('5.2 syncDynamicState evicts cache entry on despawn', () => {
        const { panel } = makePanel();
        panel.selectUnit(makeTower({ id: 42 }), 1, 'Build');
        panel.applyStaticInfo(makeStaticInfo({ entity_id: 42 }));
        assert.ok(panel.staticInfoCache.has(42));
        panel.syncDynamicState([], 'Build');
        assert.ok(!panel.staticInfoCache.has(42));
    });

    await t.test('5.2 syncDynamicState updates HP bar width', () => {
        const { panel, hpFill } = makePanel();
        const tower = makeTower({ current_hp: 100, max_hp: 100 });
        panel.selectUnit(tower, 1, 'Build');
        panel.syncDynamicState([{ ...tower, current_hp: 50 }], 'Build');
        assert.strictEqual(hpFill.style.width, '50%');
    });

    await t.test('5.2 syncDynamicState updates mana bar and makes it visible', () => {
        const { panel, manaRow, manaFill } = makePanel();
        const tower = makeTower({ current_mana: 100, max_mana: 100 });
        panel.selectUnit(tower, 1, 'Build');
        panel.syncDynamicState([{ ...tower, current_mana: 40, max_mana: 100 }], 'Build');
        assert.strictEqual(manaFill.style.width, '40%');
        assert.strictEqual(manaRow.style.display, 'flex');
    });

    await t.test('5.2 syncDynamicState updates worker state label', () => {
        const { panel, workerStateRow, workerStateLabel } = makePanel();
        const worker = makeTower({ is_worker: true, worker_state: 'Mining' });
        panel.selectUnit(worker, 1, 'Build');
        panel.syncDynamicState(
            [{ ...worker, worker_state: 'MovingToCart' }],
            'Build',
        );
        assert.strictEqual(workerStateRow.style.display, 'block');
        assert.strictEqual(workerStateLabel.textContent, 'MovingToCart');
    });

    await t.test('5.2 sell button disabled in Combat phase (selectUnit)', () => {
        const { panel, sellBtn } = makePanel();
        panel.selectUnit(makeTower(), 1, 'Combat');
        assert.strictEqual(sellBtn.disabled, true);
    });

    await t.test('5.2 sell button enabled in Build phase (selectUnit)', () => {
        const { panel, sellBtn } = makePanel();
        panel.selectUnit(makeTower(), 1, 'Build');
        assert.strictEqual(sellBtn.disabled, false);
    });

    await t.test('5.2 syncDynamicState toggles sell button disabled state with phase', () => {
        const { panel, sellBtn } = makePanel();
        const tower = makeTower();
        panel.selectUnit(tower, 1, 'Build');
        assert.strictEqual(sellBtn.disabled, false);

        panel.syncDynamicState([tower], 'Combat');
        assert.strictEqual(sellBtn.disabled, true);

        panel.syncDynamicState([tower], 'Build');
        assert.strictEqual(sellBtn.disabled, false);
    });

    // --- 5.3: applyStaticInfo ---

    await t.test('5.3 applyStaticInfo writes to cache regardless of selection', () => {
        const { panel } = makePanel();
        panel.applyStaticInfo(makeStaticInfo({ entity_id: 42 }));
        assert.ok(panel.staticInfoCache.has(42));
        assert.strictEqual(panel.staticInfoCache.get(42)?.attack_damage, 10);
    });

    await t.test('5.3 applyStaticInfo stale-response guard: no DOM update for wrong entity', () => {
        const { panel, statsSection } = makePanel();
        panel.selectUnit(makeTower({ id: 99 }), 1, 'Build');
        // statsSection already cleared by selectUnit; applying info for entity 42 is stale
        panel.applyStaticInfo(makeStaticInfo({ entity_id: 42 }));
        assert.ok(panel.staticInfoCache.has(42), 'cache still written');
        assert.strictEqual(statsSection.innerHTML, '', 'DOM not updated for stale entity');
    });

    await t.test('5.3 applyStaticInfo renders stat rows when entity matches selection', () => {
        const { panel, statsSection } = makePanel();
        panel.selectUnit(makeTower({ id: 42 }), 1, 'Build');
        panel.applyStaticInfo(makeStaticInfo({
            entity_id: 42,
            attack_damage: 10,
            damage_type: 'FireMagical',
        }));
        assert.ok(statsSection.innerHTML.includes('10'));
        assert.ok(statsSection.innerHTML.includes('FireMagical'));
    });

    await t.test('5.3 applyStaticInfo renders boss indicator when is_boss is true', () => {
        const { panel, statsSection } = makePanel();
        panel.selectUnit(makeEnemy({ id: 99 }), 1, 'Build');
        panel.applyStaticInfo(makeStaticInfo({ entity_id: 99, is_boss: true, sell_value: null }));
        assert.ok(statsSection.innerHTML.toLowerCase().includes('boss'));
    });

    await t.test('5.3 applyStaticInfo skips null stats (worker pattern)', () => {
        const { panel, statsSection } = makePanel();
        panel.selectUnit(makeTower({ id: 42, is_worker: true }), 1, 'Build');
        panel.applyStaticInfo(makeStaticInfo({
            entity_id: 42,
            attack_damage: null, attack_rate: null, attack_range: null,
            damage_type: null, armor: null, is_boss: false, sell_value: null,
        }));
        assert.strictEqual(statsSection.innerHTML, '');
    });

    await t.test('5.3 applyStaticInfo updates sell value label', () => {
        const { panel, sellValueEl } = makePanel();
        panel.selectUnit(makeTower({ id: 42 }), 1, 'Build');
        panel.applyStaticInfo(makeStaticInfo({ entity_id: 42, sell_value: 56 }));
        assert.ok((sellValueEl.textContent ?? '').includes('56'));
    });

    await t.test('5.3 selectUnit uses cached static info immediately on cache hit', () => {
        const { panel, statsSection } = makePanel();
        // Pre-populate cache
        panel.applyStaticInfo(makeStaticInfo({ entity_id: 42, attack_damage: 99 }));
        // selectUnit should apply cached info immediately
        panel.selectUnit(makeTower({ id: 42 }), 1, 'Build');
        assert.ok(statsSection.innerHTML.includes('99'));
    });

    // --- 5.4: Ability grid ---

    await t.test('5.4 sell button shown for own tower (not enemy, not worker)', () => {
        const { panel, sellBtn } = makePanel();
        panel.selectUnit(makeTower({ owner_id: 1 }), 1, 'Build');
        assert.strictEqual(sellBtn.style.display, 'block');
    });

    await t.test('5.4 sell button hidden for enemy', () => {
        const { panel, sellBtn } = makePanel();
        panel.selectUnit(makeEnemy(), 1, 'Build');
        assert.strictEqual(sellBtn.style.display, 'none');
    });

    await t.test('5.4 sell button hidden for worker', () => {
        const { panel, sellBtn } = makePanel();
        panel.selectUnit(makeTower({ is_worker: true, owner_id: 1 }), 1, 'Build');
        assert.strictEqual(sellBtn.style.display, 'none');
    });

    await t.test('5.4 sell button hidden for opponent tower', () => {
        const { panel, sellBtn } = makePanel();
        panel.selectUnit(makeTower({ owner_id: 2 }), 1, 'Build');
        assert.strictEqual(sellBtn.style.display, 'none');
    });

    await t.test('5.4 onSell callback fires with correct entity id', () => {
        const { panel, onSellCalls, sellBtn } = makePanel();
        panel.selectUnit(makeTower({ id: 42 }), 1, 'Build');
        sellBtn._handlers['click']();
        assert.strictEqual(onSellCalls.length, 1);
        assert.strictEqual(onSellCalls[0], 42);
    });

    await t.test('5.4 sell value label pre-populated when cache hit at selectUnit time', () => {
        const { panel, sellValueEl } = makePanel();
        panel.applyStaticInfo(makeStaticInfo({ entity_id: 42, sell_value: 75 }));
        panel.selectUnit(makeTower({ id: 42 }), 1, 'Build');
        assert.ok((sellValueEl.textContent ?? '').includes('75'));
    });

});
