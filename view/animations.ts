import anime from 'animejs';

export interface Position {
    x: number;
    y: number;
}

export type DamageType = 'FireMagical' | 'PhysicalPierce' | 'PhysicalBasic';

export class AnimationInstance {
    type: 'projectile' | 'melee';
    pos: Position;
    attackType: DamageType;
    duration: number;
    startTime: number;
    progress: number = 0;

    // For melee "pop" effect
    scale: number = 1;
    opacity: number = 1;

    constructor(type: 'projectile' | 'melee', startPos: Position, endPos: Position, duration: number, attackType: DamageType) {
        this.type = type;
        this.pos = { ...startPos };
        this.attackType = attackType;
        this.duration = duration;
        this.startTime = Date.now();

        if (type === 'projectile') {
            anime({
                targets: this.pos,
                x: endPos.x,
                y: endPos.y,
                duration: duration,
                easing: 'easeOutQuad',
                update: (anim) => {
                    this.progress = anim.progress / 100;
                }
            });
        } else if (type === 'melee') {
            this.pos = { ...endPos };
            this.scale = 0;
            anime({
                targets: this,
                scale: [0, 1.5, 1],
                opacity: [1, 0],
                duration: duration,
                easing: 'easeOutCubic',
                update: (anim) => {
                    this.progress = anim.progress / 100;
                }
            });
        }
    }

    get isFinished(): boolean {
        return (Date.now() - this.startTime) >= this.duration;
    }
}

export class AnimationManager {
    animations: AnimationInstance[] = [];

    addAnimation(type: 'projectile' | 'melee', startPos: Position, endPos: Position, duration: number, attackType: DamageType) {
        this.animations.push(new AnimationInstance(type, startPos, endPos, duration, attackType));
    }

    update() {
        this.animations = this.animations.filter(anim => !anim.isFinished);
    }

    draw(ctx: CanvasRenderingContext2D) {
        this.animations.forEach(anim => {
            if (anim.type === 'projectile') {
                this.drawProjectile(ctx, anim);
            } else if (anim.type === 'melee') {
                this.drawMelee(ctx, anim);
            }
        });
    }

    private drawProjectile(ctx: CanvasRenderingContext2D, anim: AnimationInstance) {
        ctx.fillStyle = this.getAttackColor(anim.attackType);
        ctx.beginPath();
        ctx.arc(anim.pos.x, anim.pos.y, 5, 0, 2 * Math.PI);
        ctx.fill();

        // Trail effect
        ctx.fillStyle = this.getAttackColor(anim.attackType) + '44'; // Add transparency
        ctx.beginPath();
        ctx.arc(anim.pos.x, anim.pos.y, 8, 0, 2 * Math.PI);
        ctx.fill();
    }

    private drawMelee(ctx: CanvasRenderingContext2D, anim: AnimationInstance) {
        ctx.strokeStyle = `rgba(255, 255, 255, ${anim.opacity})`;
        ctx.lineWidth = 3;
        ctx.beginPath();
        ctx.arc(anim.pos.x, anim.pos.y, 20 * anim.scale, 0, 2 * Math.PI);
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
