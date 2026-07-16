import { Container, Graphics, Text, TextStyle } from 'pixi.js';
import type { Unit, Position } from '../types';
import { stepToward, VisualDiff } from '../visual_diff';
import { theme, hexNum } from '../theme';
import { SQUARE_SIZE, KING_RADIUS, UNIT_HIT_HALF } from '../layout';
import type { EffectsLayer } from './effects';

const C = theme.colors;

const COMBAT_SPEED = 80;  // px/s, server SPEED
const WORKER_SPEED = 50;  // px/s, server WORKER_SPEED
const CATCHUP = 1.25;     // run slightly faster than server so we never fall behind
const SNAP_DIST = 150;    // px — larger jumps snap (teleport/resync)
const FLASH_MS = 80;
const SPAWN_MS = 150;

function bodyColor(u: Unit, myPlayerId: number | null): number {
    if (u.is_king) return hexNum(u.owner_id === myPlayerId ? C.kingFriendly : C.kingHostile);
    if (u.is_enemy) return hexNum(C.enemy);
    return hexNum(u.owner_id === myPlayerId ? C.friendly : C.allyOther);
}

// Fake glow: wide translucent stroke + tight bright stroke around the body path.
function drawBody(g: Graphics, u: Unit, color: number): void {
    g.clear();
    const half = SQUARE_SIZE / 2 - 10; // 20 — same footprint as the old renderer

    const tracePath = () => {
        if (u.is_king) {
            g.circle(0, 0, KING_RADIUS);
        } else if (u.shape === 'Square') {
            g.rect(-half, -half, half * 2, half * 2);
        } else if (u.shape === 'Circle') {
            g.circle(0, 0, half);
        } else {
            g.moveTo(0, -half).lineTo(-half, half).lineTo(half, half).closePath();
        }
    };

    const glow = u.is_king ? theme.glow.kingOuter : theme.glow.unitOuter;
    const rim = u.is_king ? theme.glow.kingInner : theme.glow.unitInner;
    tracePath();
    g.stroke({ width: glow, color, alpha: 0.22 });
    tracePath();
    g.fill({ color, alpha: 0.92 });
    tracePath();
    g.stroke({ width: rim, color: 0xffffff, alpha: 0.35 });
}

class VisualUnit {
    container = new Container();
    body = new Graphics();
    flashOverlay = new Graphics();
    bars = new Graphics();
    kingHpText: Text | null = null;
    unit: Unit;
    renderPos: Position;
    flashMs = 0;
    spawnMs: number;
    private color: number;

    constructor(u: Unit, myPlayerId: number | null, animateSpawn: boolean) {
        this.unit = u;
        this.renderPos = { x: u.x, y: u.y };
        this.spawnMs = animateSpawn ? SPAWN_MS : 0;
        this.color = bodyColor(u, myPlayerId);
        drawBody(this.body, u, this.color);
        drawBody(this.flashOverlay, u, 0xffffff);
        this.flashOverlay.alpha = 0;
        this.container.addChild(this.body, this.flashOverlay, this.bars);

        if (u.is_king) {
            const labelFill = u.owner_id === myPlayerId ? hexNum(C.bgDeep) : hexNum(C.textPrimary);
            const label = new Text({
                text: 'KING',
                style: new TextStyle({ fontFamily: theme.font, fontSize: 10, fontWeight: 'bold', fill: labelFill }),
            });
            label.anchor.set(0.5);
            this.container.addChild(label);

            this.kingHpText = new Text({
                text: '',
                style: new TextStyle({ fontFamily: theme.font, fontSize: 9, fill: hexNum(C.textPrimary) }),
            });
            this.kingHpText.anchor.set(0.5, 1);
            this.kingHpText.position.set(0, -KING_RADIUS - 16);
            this.container.addChild(this.kingHpText);
        }
        this.container.position.set(u.x, u.y);
    }

    refreshColor(myPlayerId: number | null): void {
        const c = bodyColor(this.unit, myPlayerId);
        if (c !== this.color) {
            this.color = c;
            drawBody(this.body, this.unit, c);
        }
    }

    redrawBars(): void {
        const u = this.unit;
        const g = this.bars;
        g.clear();
        if (u.is_worker) return;

        if (u.is_king) {
            const w = KING_RADIUS * 2 + 10, h = 8;
            const x = -w / 2, y = -KING_RADIUS - 14;
            const pct = Math.max(0, Math.min(1, u.current_hp / u.max_hp));
            g.rect(x, y, w, h).fill(hexNum(C.hpBack));
            g.rect(x, y, w * pct, h).fill(hexNum(C.hpFill));
            if (this.kingHpText) this.kingHpText.text = `${u.current_hp}/${u.max_hp}`;
            return;
        }

        const w = SQUARE_SIZE - 20, h = 5;
        const x = -w / 2;
        let y = -(SQUARE_SIZE / 2);
        const pct = Math.max(0, Math.min(1, u.current_hp / u.max_hp));
        g.rect(x, y, w, h).fill(hexNum(C.hpBack));
        g.rect(x, y, w * pct, h).fill(hexNum(C.hpFill));

        if (u.max_mana !== undefined && u.max_mana > 0 && u.current_mana !== undefined) {
            y += h + 1;
            const mpct = Math.max(0, Math.min(1, u.current_mana / u.max_mana));
            g.rect(x, y, w, 3).fill(hexNum(C.manaBack));
            g.rect(x, y, w * mpct, 3).fill(hexNum(C.manaFill));
        }
    }
}

export class UnitLayer {
    private visuals = new Map<number, VisualUnit>();
    private firstSync = true;

    constructor(private layer: Container) {}

    sync(units: Map<number, Unit>, myPlayerId: number | null, diff: VisualDiff, effects: EffectsLayer): void {
        for (const d of diff.died) {
            const v = this.visuals.get(d.id);
            if (v) {
                effects.deathBurst(v.renderPos.x, v.renderPos.y, hexNum(C.fxBasic));
                v.container.destroy({ children: true });
                this.visuals.delete(d.id);
            }
        }

        for (const [id, u] of units) {
            let v = this.visuals.get(id);
            if (!v) {
                v = new VisualUnit(u, myPlayerId, !this.firstSync);
                this.visuals.set(id, v);
                this.layer.addChild(v.container);
            } else {
                v.unit = u;
                v.refreshColor(myPlayerId);
            }
            v.redrawBars();
        }

        for (const id of diff.hit) {
            const v = this.visuals.get(id);
            if (v) v.flashMs = FLASH_MS;
        }
        this.firstSync = false;
    }

    update(dtMs: number): void {
        for (const v of this.visuals.values()) {
            const speed = v.unit.is_worker ? WORKER_SPEED : COMBAT_SPEED;
            const maxStep = speed * CATCHUP * (dtMs / 1000);
            v.renderPos = stepToward(v.renderPos, { x: v.unit.x, y: v.unit.y }, maxStep, SNAP_DIST);
            v.container.position.set(v.renderPos.x, v.renderPos.y);

            if (v.flashMs > 0) {
                v.flashMs = Math.max(0, v.flashMs - dtMs);
                v.flashOverlay.alpha = 0.7 * (v.flashMs / FLASH_MS);
            } else if (v.flashOverlay.alpha !== 0) {
                v.flashOverlay.alpha = 0;
            }

            if (v.spawnMs > 0) {
                v.spawnMs = Math.max(0, v.spawnMs - dtMs);
                const t = 1 - v.spawnMs / SPAWN_MS; // 0 -> 1
                v.container.scale.set(t * t * (3 - 2 * t)); // smoothstep
            } else if (v.container.scale.x !== 1) {
                v.container.scale.set(1);
            }
        }
    }

    hitTest(x: number, y: number): number | null {
        for (const [id, v] of this.visuals) {
            const half = v.unit.is_king ? KING_RADIUS : UNIT_HIT_HALF;
            if (Math.abs(x - v.renderPos.x) <= half && Math.abs(y - v.renderPos.y) <= half) {
                return id;
            }
        }
        return null;
    }

    reset(): void {
        for (const v of this.visuals.values()) {
            v.container.destroy({ children: true });
        }
        this.visuals.clear();
        this.firstSync = true;
    }
}
