import { Container, Graphics, Text, TextStyle } from 'pixi.js';
import type { CombatEvent, DamageType, Position } from '../types';
import { theme, hexNum } from '../theme';
import { CANVAS_WIDTH, CANVAS_HEIGHT } from '../layout';

const C = theme.colors;

function attackColor(t: DamageType): number {
    switch (t) {
        case 'FireMagical': return hexNum(C.fxFire);
        case 'PhysicalPierce': return hexNum(C.fxPierce);
        case 'PhysicalBasic': return hexNum(C.fxBasic);
    }
}

const easeOutQuad = (t: number) => 1 - (1 - t) * (1 - t);
const easeOutCubic = (t: number) => 1 - Math.pow(1 - t, 3);

interface ActiveEffect {
    node: Container;
    elapsed: number;
    duration: number;
    tick(t: number, node: Container): void; // t in [0,1]
}

export class EffectsLayer {
    private active: ActiveEffect[] = [];

    constructor(private layer: Container, private overlay: Container) {}

    private add(node: Container, duration: number, tick: ActiveEffect['tick'], parent?: Container): void {
        (parent ?? this.layer).addChild(node);
        this.active.push({ node, elapsed: 0, duration, tick });
    }

    deathBurst(x: number, y: number, color: number): void {
        const g = new Graphics();
        g.position.set(x, y);
        this.add(g, 350, (t, node) => {
            const gg = node as Graphics;
            gg.clear();
            const r = 6 + 26 * easeOutCubic(t);
            gg.circle(0, 0, r).stroke({ width: 3, color, alpha: 1 - t });
            gg.circle(0, 0, r * 0.6).fill({ color, alpha: 0.35 * (1 - t) });
        });
    }

    combatEvent(e: CombatEvent): void {
        const color = attackColor(e.attack_type);
        const ranged = e.attack_type === 'FireMagical' || e.attack_type === 'PhysicalPierce';
        if (ranged) {
            this.projectile(e.start_pos, e.end_pos, color, 300);
        } else {
            this.meleeRing(e.end_pos, 200);
        }
    }

    private projectile(start: Position, end: Position, color: number, duration: number): void {
        const g = new Graphics();
        g.circle(0, 0, 8).fill({ color, alpha: 0.25 }); // glow halo
        g.circle(0, 0, 4).fill(color);
        g.position.set(start.x, start.y);
        this.add(g, duration, (t, node) => {
            const k = easeOutQuad(t);
            node.position.set(start.x + (end.x - start.x) * k, start.y + (end.y - start.y) * k);
            node.alpha = t > 0.85 ? (1 - t) / 0.15 : 1;
        });
    }

    private meleeRing(at: Position, duration: number): void {
        const g = new Graphics();
        g.position.set(at.x, at.y);
        this.add(g, duration, (t, node) => {
            const gg = node as Graphics;
            gg.clear();
            gg.circle(0, 0, 20 * easeOutCubic(t) + 2)
                .stroke({ width: 3, color: 0xffffff, alpha: 1 - t });
        });
    }

    phaseBanner(text: string): void {
        const label = new Text({
            text,
            style: new TextStyle({
                fontFamily: theme.font,
                fontSize: 64,
                fontWeight: 'bold',
                fill: hexNum(C.accent),
                stroke: { color: hexNum(C.bgDeep), width: 6 },
                letterSpacing: 10,
            }),
        });
        label.anchor.set(0.5);
        this.add(label, 1200, (t, node) => {
            node.position.set(CANVAS_WIDTH / 2, CANVAS_HEIGHT / 2 - 60 - 30 * t);
            node.alpha = t < 0.15 ? t / 0.15 : t > 0.7 ? (1 - t) / 0.3 : 1;
        }, this.overlay);
    }

    errorVignette(): void {
        const g = new Graphics();
        g.rect(0, 0, CANVAS_WIDTH, CANVAS_HEIGHT)
            .stroke({ width: 16, color: hexNum(C.fxError), alpha: 1 });
        this.add(g, 400, (t, node) => {
            node.alpha = 0.6 * (1 - t);
        }, this.overlay);
    }

    update(dtMs: number): void {
        this.active = this.active.filter((fx) => {
            fx.elapsed += dtMs;
            const t = Math.min(1, fx.elapsed / fx.duration);
            fx.tick(t, fx.node);
            if (fx.elapsed >= fx.duration) {
                fx.node.destroy({ children: true });
                return false;
            }
            return true;
        });
    }
}
