import { FederatedPointerEvent, Rectangle } from 'pixi.js';
import type { Unit, Player, CombatEvent } from '../types';
import { diffVisualState, snapshotOf, VisualSnapshot } from '../visual_diff';
import {
    SQUARE_SIZE, LEFT_BOARD_END, RIGHT_BOARD_START, TOTAL_HEIGHT,
    CANVAS_WIDTH, CANVAS_HEIGHT, MERC_BUILDING_X, MERC_BUILDING_Y,
    MERC_BUILDING_HALF,
} from '../layout';
import { createScene } from './scene';
import { buildStaticBoard, MidlaneLayer } from './board';
import { UnitLayer } from './units';
import { EffectsLayer } from './effects';

export type ClickHit =
    | { kind: 'unit'; unitId: number }
    | { kind: 'cell'; row: number; col: number }
    | { kind: 'mercBuilding' }
    | { kind: 'empty' };

export interface RendererHandle {
    syncState(units: Map<number, Unit>, players: Player[], phase: string, myPlayerId: number | null): void;
    playCombatEvents(events: CombatEvent[]): void;
    onClick(cb: (hit: ClickHit) => void): void;
    flashError(msg: string): void;
    reset(): void;
    destroy(): void;
}

// Pure click resolution (exported for testing without Pixi if desired later).
export function resolveClick(
    x: number,
    y: number,
    unitIdAt: (x: number, y: number) => number | null,
    players: Player[],
    myPlayerId: number | null,
): ClickHit {
    const unitId = unitIdAt(x, y);
    if (unitId !== null) return { kind: 'unit', unitId };

    // Gap area: my own mercenary building?
    if (x >= LEFT_BOARD_END && x < RIGHT_BOARD_START) {
        if (myPlayerId !== null) {
            const myIndex = players.findIndex(p => p.id === myPlayerId);
            if (myIndex === 0 || myIndex === 1) {
                const dy = y - MERC_BUILDING_Y[myIndex];
                const dx = x - MERC_BUILDING_X;
                if (dx * dx + dy * dy <= MERC_BUILDING_HALF * MERC_BUILDING_HALF) {
                    return { kind: 'mercBuilding' };
                }
            }
        }
        return { kind: 'empty' };
    }

    // Board cell (only above the king zone)
    if (y < TOTAL_HEIGHT) {
        if (x < LEFT_BOARD_END) {
            return { kind: 'cell', row: Math.floor(y / SQUARE_SIZE), col: Math.floor(x / SQUARE_SIZE) };
        }
        if (x >= RIGHT_BOARD_START && x < RIGHT_BOARD_START + LEFT_BOARD_END) {
            return { kind: 'cell', row: Math.floor(y / SQUARE_SIZE), col: Math.floor((x - RIGHT_BOARD_START) / SQUARE_SIZE) };
        }
    }
    return { kind: 'empty' };
}

export async function initRenderer(container: HTMLElement): Promise<RendererHandle> {
    const scene = await createScene(container);

    buildStaticBoard(scene.layers.board);
    const midlane = new MidlaneLayer(scene.layers.midlane);
    const effects = new EffectsLayer(scene.layers.effects, scene.layers.overlay);
    const unitLayer = new UnitLayer(scene.layers.units);

    let prevSnapshot = new Map<number, VisualSnapshot>();
    let lastPhase: string | null = null;
    let players: Player[] = [];
    let myPlayerId: number | null = null;
    let clickCb: (hit: ClickHit) => void = () => {};

    scene.app.stage.eventMode = 'static';
    scene.app.stage.hitArea = new Rectangle(0, 0, CANVAS_WIDTH, CANVAS_HEIGHT);
    scene.app.stage.on('pointertap', (e: FederatedPointerEvent) => {
        const pos = e.getLocalPosition(scene.app.stage);
        clickCb(resolveClick(pos.x, pos.y, (x, y) => unitLayer.hitTest(x, y), players, myPlayerId));
    });

    scene.app.ticker.add((ticker) => {
        const dt = ticker.deltaMS;
        unitLayer.update(dt);
        effects.update(dt);
        midlane.update(dt);
    });

    return {
        syncState(units, newPlayers, phase, newMyPlayerId) {
            players = newPlayers;
            myPlayerId = newMyPlayerId;
            const diff = diffVisualState(prevSnapshot, units);
            unitLayer.sync(units, myPlayerId, diff, effects);
            midlane.sync(players);
            if (lastPhase !== null && phase !== lastPhase && (phase === 'Combat' || phase === 'Build')) {
                effects.phaseBanner(phase.toUpperCase());
            }
            lastPhase = phase;
            prevSnapshot = new Map(Array.from(units, ([id, u]) => [id, snapshotOf(u)] as const));
        },
        playCombatEvents(events) {
            for (const e of events) effects.combatEvent(e);
        },
        onClick(cb) {
            clickCb = cb;
        },
        flashError(_msg) {
            effects.errorVignette();
        },
        reset() {
            unitLayer.reset();
            prevSnapshot = new Map();
            lastPhase = null;
        },
        destroy() {
            scene.app.destroy(true, { children: true });
        },
    };
}
