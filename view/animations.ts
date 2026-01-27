export interface Position {
    x: number;
    y: number;
}

export type DamageType = 'FireMagical' | 'PhysicalPierce' | 'PhysicalBasic';

export interface Animation {
    type: 'projectile' | 'melee';
    startPos: Position;
    endPos: Position;
    duration: number;
    attackType: DamageType;
    startTime: number;
}

export class AnimationManager {
    animations: Animation[] = [];

    addAnimation(type: 'projectile' | 'melee', startPos: Position, endPos: Position, duration: number, attackType: DamageType) {
        this.animations.push({
            type,
            startPos,
            endPos,
            duration,
            attackType,
            startTime: Date.now()
        });
    }

    update(now: number) {
        this.animations = this.animations.filter(anim => (now - anim.startTime) < anim.duration);
    }

    draw(ctx: CanvasRenderingContext2D, now: number) {
        this.animations.forEach(anim => {
            const elapsed = now - anim.startTime;
            const progress = Math.min(1, elapsed / anim.duration);

            if (anim.type === 'projectile') {
                this.drawProjectile(ctx, anim, progress);
            } else if (anim.type === 'melee') {
                this.drawMelee(ctx, anim, progress);
            }
        });
    }

    private drawProjectile(ctx: CanvasRenderingContext2D, anim: Animation, progress: number) {
        const x = anim.startPos.x + (anim.endPos.x - anim.startPos.x) * progress;
        const y = anim.startPos.y + (anim.endPos.y - anim.startPos.y) * progress;

        ctx.fillStyle = this.getAttackColor(anim.attackType);
        ctx.beginPath();
        ctx.arc(x, y, 5, 0, 2 * Math.PI);
        ctx.fill();
    }

    private drawMelee(ctx: CanvasRenderingContext2D, anim: Animation, progress: number) {
        const alpha = 1 - progress;
        ctx.strokeStyle = `rgba(255, 255, 255, ${alpha})`;
        ctx.lineWidth = 3;
        ctx.beginPath();
        ctx.arc(anim.endPos.x, anim.endPos.y, 20 * progress, 0, 2 * Math.PI);
        ctx.stroke();
    }

    private getAttackColor(attackType: DamageType): string {
        switch (attackType) {
            case 'FireMagical': return '#FF4500';
            case 'PhysicalPierce': return '#FFFF00';
            case 'PhysicalBasic': return '#FFFFFF';
            default: return '#FFFFFF';
        }
    }
}
