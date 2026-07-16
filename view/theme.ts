// Single source of visual truth: palette, glow, fonts.
// Pure module — no pixi.js, no DOM globals at top level.

export const theme = {
    colors: {
        // Backgrounds
        bgDeep: '#0a0e1a',        // page + canvas background
        bgPanel: '#111827',       // HUD/panel background
        boardDark: '#151b2e',     // checkerboard dark cell
        boardLight: '#1c2440',    // checkerboard light cell
        boardProtectedDark: '#0f1322', // rows 8-9 dark cell
        boardProtectedLight: '#131a2d', // rows 8-9 light cell
        kingZone: '#0d1120',
        gridLine: '#2a3558',
        laneDivider: '#4c5b9e',

        // Text
        textPrimary: '#e5e7eb',
        textMuted: '#9ca3af',

        // Accents
        accent: '#22d3ee',        // neon cyan — primary accent
        accentGold: '#fbbf24',    // gold/economy accent

        // Unit ownership palette
        friendly: '#38bdf8',      // my units (neon blue)
        allyOther: '#f472b6',     // other player's units (pink)
        enemy: '#4ade80',         // wave creeps (toxic green)
        kingFriendly: '#fbbf24',
        kingHostile: '#ef4444',

        // Bars
        hpFill: '#22c55e',
        hpBack: '#7f1d1d',
        manaFill: '#818cf8',
        manaBack: '#1e1b4b',

        // Midlane objects
        vein: '#fbbf24',
        cart: '#b45309',
        mercBuilding: '#991b1b',
        spawnQueueIcon: '#fb923c',

        // Combat effect colors by damage type
        fxFire: '#fb923c',
        fxPierce: '#fde047',
        fxBasic: '#f8fafc',
        fxError: '#ef4444',
    },
    // Fake-glow stroke widths (outer soft stroke / inner bright stroke), px
    glow: { unitOuter: 8, unitInner: 2, kingOuter: 14, kingInner: 3 },
    font: "'Segoe UI', system-ui, sans-serif",
} as const;

export type ThemeColor = keyof typeof theme.colors;

export function hexNum(color: string): number {
    return parseInt(color.slice(1), 16);
}

// kebab-cases color keys: accentGold -> --td-accent-gold
export function cssVariables(): Record<string, string> {
    const vars: Record<string, string> = {};
    for (const [key, value] of Object.entries(theme.colors)) {
        const kebab = key.replace(/[A-Z]/g, (m) => `-${m.toLowerCase()}`);
        vars[`--td-${kebab}`] = value;
    }
    return vars;
}

export function applyThemeToDom(root: HTMLElement): void {
    for (const [name, value] of Object.entries(cssVariables())) {
        root.style.setProperty(name, value);
    }
}
