// Pure functions deciding which visual effects a state sync triggers,
// plus interpolation math. No pixi.js, no DOM.
import type { Unit, Position } from './types';

export interface VisualSnapshot {
    hp: number;
    x: number;
    y: number;
}

export interface VisualDiff {
    spawned: number[];
    died: { id: number; x: number; y: number }[];
    hit: number[];
}

export function snapshotOf(u: Unit): VisualSnapshot {
    return { hp: u.current_hp, x: u.x, y: u.y };
}

export function diffVisualState(
    prev: Map<number, VisualSnapshot>,
    next: Map<number, Unit>,
): VisualDiff {
    const spawned: number[] = [];
    const hit: number[] = [];
    const died: { id: number; x: number; y: number }[] = [];

    for (const [id, u] of next) {
        const p = prev.get(id);
        if (!p) {
            spawned.push(id);
        } else if (u.current_hp < p.hp) {
            hit.push(id);
        }
    }
    for (const [id, p] of prev) {
        if (!next.has(id)) died.push({ id, x: p.x, y: p.y });
    }
    return { spawned, died, hit };
}

// Moves cur toward target by at most maxStep. Snaps when close (arrival)
// or when the gap exceeds snapDist (server teleport / resync — do not glide).
export function stepToward(
    cur: Position,
    target: Position,
    maxStep: number,
    snapDist: number,
): Position {
    const dx = target.x - cur.x;
    const dy = target.y - cur.y;
    const dist = Math.hypot(dx, dy);
    if (dist <= maxStep || dist > snapDist) return { x: target.x, y: target.y };
    const s = maxStep / dist;
    return { x: cur.x + dx * s, y: cur.y + dy * s };
}
