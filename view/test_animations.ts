// Minimal browser stubs to allow animejs to run in Node environment for unit tests
(global as any).window = {};
(global as any).document = { querySelectorAll: () => [] };
(global as any).NodeList = class { };
(global as any).HTMLCollection = class { };
(global as any).SVGElement = class { };
(global as any).requestAnimationFrame = () => { };

import assert from 'node:assert';
import test from 'node:test';
import { AnimationManager, DamageType } from './animations';

test('AnimationManager lifecycle', async (t) => {
    await t.test('it adds animations', () => {
        const am = new AnimationManager();
        am.addAnimation('melee', { x: 0, y: 0 }, { x: 10, y: 10 }, 100, 'PhysicalBasic');
        assert.strictEqual(am.animations.length, 1, 'Should have 1 animation');
    });

    await t.test('it removes expired animations', () => {
        const am = new AnimationManager();
        am.addAnimation('melee', { x: 0, y: 0 }, { x: 10, y: 10 }, 100, 'PhysicalBasic');

        // Mock startTime to something old
        am.animations[0].startTime = 1000;

        am.update(); // No arguments needed now
        assert.strictEqual(am.animations.length, 0, 'Should have 0 animations after expiration');
    });

});
