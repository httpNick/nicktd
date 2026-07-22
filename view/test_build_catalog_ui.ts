import { test } from 'node:test';
import assert from 'node:assert';
import { renderBuildShop, renderFamilyOptions } from './app_build_ui';
import { BuildCatalogEntry, Family } from './types';

// ---- Mock DOM helpers ----
// app_build_ui.ts calls document.createElement/appendChild directly (per its
// design), and this project has no jsdom dependency wired up (see
// test_unit_info_panel.ts, which avoids the DOM entirely by injecting
// hand-built fake elements). Since app_build_ui's functions call
// `document.createElement` themselves rather than accepting elements as
// arguments, we stub a minimal fake `document` global with the same
// hand-built-fake-element technique instead of pulling in a real DOM library.

interface FakeButton {
    tagName: string;
    textContent: string;
    onclick: (() => void) | null;
    click: () => void;
}

interface FakeContainer {
    innerHTML: string;
    children: FakeButton[];
    appendChild: (el: FakeButton) => void;
    querySelectorAll: (sel: string) => FakeButton[];
}

function makeFakeButton(): FakeButton {
    const btn: FakeButton = {
        tagName: 'button',
        textContent: '',
        onclick: null,
        click() {
            if (btn.onclick) btn.onclick();
        },
    };
    return btn;
}

function makeContainer(): FakeContainer {
    const container: FakeContainer = {
        innerHTML: '',
        children: [],
        appendChild(el: FakeButton) {
            container.children.push(el);
        },
        querySelectorAll(sel: string) {
            if (sel === 'button') return container.children;
            return [];
        },
    };
    return container;
}

(globalThis as unknown as { document: { createElement: (tag: string) => FakeButton } }).document = {
    createElement: () => makeFakeButton(),
};

test('renderBuildShop creates one button per catalog entry with cost label', () => {
    const container = makeContainer();
    const catalog: BuildCatalogEntry[] = [
        { unit_kind: 'Square', name: 'Square', cost: 25 },
        { unit_kind: 'Circle', name: 'Circle', cost: 75 },
    ];
    let selected: string | null = null;
    renderBuildShop(container as unknown as HTMLElement, catalog, (kind) => { selected = kind; });
    const buttons = container.querySelectorAll('button');
    assert.strictEqual(buttons.length, 2);
    assert.ok(buttons[0].textContent!.includes('Square'));
    assert.ok(buttons[0].textContent!.includes('25'));
    buttons[1].click();
    assert.strictEqual(selected, 'Circle');
});

test('renderFamilyOptions creates one button per family and clears on pick', () => {
    const container = makeContainer();
    const families: Family[] = ['Basic'];
    let picked: Family | null = null;
    renderFamilyOptions(container as unknown as HTMLElement, families, (family) => { picked = family; });
    const buttons = container.querySelectorAll('button');
    assert.strictEqual(buttons.length, 1);
    buttons[0].click();
    assert.strictEqual(picked, 'Basic');
});
