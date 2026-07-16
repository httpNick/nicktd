import { test } from 'node:test';
import assert from 'node:assert';
import { theme, hexNum, cssVariables } from './theme';

test('hexNum converts #rrggbb to a number', () => {
    assert.strictEqual(hexNum('#ffffff'), 0xffffff);
    assert.strictEqual(hexNum('#0a0e1a'), 0x0a0e1a);
});

test('cssVariables emits one --td-* var per theme color', () => {
    const vars = cssVariables();
    const colorCount = Object.keys(theme.colors).length;
    const tdVars = Object.keys(vars).filter(k => k.startsWith('--td-'));
    assert.strictEqual(tdVars.length, colorCount);
    for (const v of Object.values(vars)) {
        assert.match(v, /^#[0-9a-fA-F]{6}$/);
    }
});

test('every theme color is a 6-digit hex string', () => {
    for (const c of Object.values(theme.colors)) {
        assert.match(c, /^#[0-9a-fA-F]{6}$/);
    }
});
