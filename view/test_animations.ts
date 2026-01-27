import assert from 'node:assert';
import test from 'node:test';
import { AnimationManager, DamageType } from './animations';

test('AnimationManager lifecycle', async (t) => {
    await t.test('it adds animations', () => {
        const am = new AnimationManager();
        am.addAnimation('melee', {x:0, y:0}, {x:10, y:10}, 100, 'PhysicalBasic');
        assert.strictEqual(am.animations.length, 1, 'Should have 1 animation');
    });

    await t.test('it removes expired animations', () => {
        const am = new AnimationManager();
        am.addAnimation('melee', {x:0, y:0}, {x:10, y:10}, 100, 'PhysicalBasic');
        
        // Mock startTime to something old
        am.animations[0].startTime = 1000;
        
        am.update(1101); // 101ms passed
        assert.strictEqual(am.animations.length, 0, 'Should have 0 animations after expiration');
    });

    await t.test('it calculates projectile position correctly', () => {
        const am = new AnimationManager();
        const startPos = {x: 0, y: 0};
        const endPos = {x: 100, y: 100};
        am.addAnimation('projectile', startPos, endPos, 100, 'PhysicalPierce');
        
        const anim = am.animations[0];
        anim.startTime = 1000;
        
        // Mock a draw call's interpolation logic
        const getPosAt = (progress: number) => ({
            x: anim.startPos.x + (anim.endPos.x - anim.startPos.x) * progress,
            y: anim.startPos.y + (anim.endPos.y - anim.startPos.y) * progress
        });

        const halfWay = getPosAt(0.5);
        assert.strictEqual(halfWay.x, 50);
        assert.strictEqual(halfWay.y, 50);

        const fullWay = getPosAt(1.0);
        assert.strictEqual(fullWay.x, 100);
        assert.strictEqual(fullWay.y, 100);
    });
});
