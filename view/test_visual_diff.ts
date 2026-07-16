import { test } from 'node:test';
import assert from 'node:assert';
import { diffVisualState, snapshotOf, stepToward, VisualSnapshot } from './visual_diff';
import { Unit } from './types';

function unit(id: number, over: Partial<Unit> = {}): Unit {
    return {
        id, shape: 'Square', x: 100, y: 100, owner_id: 1, is_enemy: false,
        current_hp: 50, max_hp: 50, is_worker: false, is_king: false, ...over,
    };
}

test('new unit id is spawned', () => {
    const prev = new Map<number, VisualSnapshot>();
    const next = new Map([[7, unit(7)]]);
    const d = diffVisualState(prev, next);
    assert.deepStrictEqual(d.spawned, [7]);
    assert.deepStrictEqual(d.died, []);
    assert.deepStrictEqual(d.hit, []);
});

test('missing unit id is died, with last known position', () => {
    const prev = new Map([[7, { hp: 50, x: 120, y: 340 }]]);
    const next = new Map<number, Unit>();
    const d = diffVisualState(prev, next);
    assert.deepStrictEqual(d.died, [{ id: 7, x: 120, y: 340 }]);
});

test('hp drop is a hit; hp same or higher is not', () => {
    const prev = new Map([
        [1, { hp: 50, x: 0, y: 0 }],
        [2, { hp: 50, x: 0, y: 0 }],
        [3, { hp: 40, x: 0, y: 0 }],
    ]);
    const next = new Map([
        [1, unit(1, { current_hp: 30 })], // dropped -> hit
        [2, unit(2, { current_hp: 50 })], // same -> no
        [3, unit(3, { current_hp: 45 })], // healed -> no
    ]);
    const d = diffVisualState(prev, next);
    assert.deepStrictEqual(d.hit, [1]);
});

test('snapshotOf captures hp and position', () => {
    assert.deepStrictEqual(snapshotOf(unit(1, { x: 5, y: 6, current_hp: 9 })), { hp: 9, x: 5, y: 6 });
});

test('stepToward moves by maxStep toward target', () => {
    const p = stepToward({ x: 0, y: 0 }, { x: 10, y: 0 }, 4, 150);
    assert.ok(Math.abs(p.x - 4) < 1e-9);
    assert.strictEqual(p.y, 0);
});

test('stepToward snaps when within maxStep', () => {
    assert.deepStrictEqual(stepToward({ x: 9, y: 0 }, { x: 10, y: 0 }, 4, 150), { x: 10, y: 0 });
});

test('stepToward snaps when target is farther than snapDist (teleport)', () => {
    assert.deepStrictEqual(stepToward({ x: 0, y: 0 }, { x: 500, y: 0 }, 4, 150), { x: 500, y: 0 });
});
