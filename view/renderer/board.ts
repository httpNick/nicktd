import { Container, Graphics, Text, TextStyle } from 'pixi.js';
import { theme, hexNum } from '../theme';
import {
    BOARD_SIZE, SQUARE_SIZE, GAP_SIZE, LEFT_BOARD_END, RIGHT_BOARD_START,
    TOTAL_HEIGHT, KING_ZONE_HEIGHT, PROTECTED_ROW_START,
    MERC_BUILDING_X, MERC_BUILDING_Y, MERC_BUILDING_HALF,
    VEIN_Y, CART_Y, LABEL_Y,
} from '../layout';
import type { Player } from '../types';

const C = theme.colors;

function drawOneBoard(g: Graphics, offsetX: number): void {
    for (let row = 0; row < BOARD_SIZE; row++) {
        for (let col = 0; col < BOARD_SIZE; col++) {
            const protectedRow = row >= PROTECTED_ROW_START;
            const light = (row + col) % 2 === 0;
            const color = protectedRow
                ? (light ? C.boardProtectedLight : C.boardProtectedDark)
                : (light ? C.boardLight : C.boardDark);
            g.rect(offsetX + col * SQUARE_SIZE, row * SQUARE_SIZE, SQUARE_SIZE, SQUARE_SIZE)
                .fill(hexNum(color));
        }
    }
    // Subtle grid lines
    for (let i = 0; i <= BOARD_SIZE; i++) {
        g.moveTo(offsetX + i * SQUARE_SIZE, 0).lineTo(offsetX + i * SQUARE_SIZE, TOTAL_HEIGHT);
        g.moveTo(offsetX, i * SQUARE_SIZE).lineTo(offsetX + BOARD_SIZE * SQUARE_SIZE, i * SQUARE_SIZE);
    }
    g.stroke({ width: 1, color: hexNum(C.gridLine), alpha: 0.5 });

    // Glowing divider above the protected rows
    const dividerY = PROTECTED_ROW_START * SQUARE_SIZE;
    g.moveTo(offsetX, dividerY).lineTo(offsetX + BOARD_SIZE * SQUARE_SIZE, dividerY)
        .stroke({ width: 6, color: hexNum(C.accent), alpha: 0.15 });
    g.moveTo(offsetX, dividerY).lineTo(offsetX + BOARD_SIZE * SQUARE_SIZE, dividerY)
        .stroke({ width: 2, color: hexNum(C.accent), alpha: 0.6 });
}

export function buildStaticBoard(layer: Container): void {
    const g = new Graphics();

    // King zones (below each board)
    g.rect(0, TOTAL_HEIGHT, LEFT_BOARD_END, KING_ZONE_HEIGHT).fill(hexNum(C.kingZone));
    g.rect(RIGHT_BOARD_START, TOTAL_HEIGHT, LEFT_BOARD_END, KING_ZONE_HEIGHT).fill(hexNum(C.kingZone));
    // King zone divider lines (fake glow: wide faint + narrow bright)
    for (const x0 of [0, RIGHT_BOARD_START]) {
        g.moveTo(x0, TOTAL_HEIGHT).lineTo(x0 + LEFT_BOARD_END, TOTAL_HEIGHT)
            .stroke({ width: 6, color: hexNum(C.accentGold), alpha: 0.15 });
        g.moveTo(x0, TOTAL_HEIGHT).lineTo(x0 + LEFT_BOARD_END, TOTAL_HEIGHT)
            .stroke({ width: 2, color: hexNum(C.accentGold), alpha: 0.6 });
    }

    drawOneBoard(g, 0);
    drawOneBoard(g, RIGHT_BOARD_START);

    // Lane borders and midlane separator
    g.moveTo(LEFT_BOARD_END, 0).lineTo(LEFT_BOARD_END, TOTAL_HEIGHT)
        .stroke({ width: 2, color: hexNum(C.laneDivider), alpha: 0.8 });
    g.moveTo(RIGHT_BOARD_START, 0).lineTo(RIGHT_BOARD_START, TOTAL_HEIGHT)
        .stroke({ width: 2, color: hexNum(C.laneDivider), alpha: 0.8 });
    g.moveTo(LEFT_BOARD_END, 300).lineTo(RIGHT_BOARD_START, 300)
        .stroke({ width: 2, color: hexNum(C.laneDivider), alpha: 0.8 });

    layer.addChild(g);
}

const labelStyle = () => new TextStyle({
    fontFamily: theme.font, fontSize: 14, fill: hexNum(C.textPrimary),
});
const smallStyle = (fill: number) => new TextStyle({
    fontFamily: theme.font, fontSize: 10, fill, fontWeight: 'bold',
});

export class MidlaneLayer {
    private lastPlayers: Player[] | null = null;
    private veins: Graphics[] = [];
    private elapsed = 0;

    constructor(private layer: Container) {}

    sync(players: Player[]): void {
        if (players === this.lastPlayers) return;
        this.lastPlayers = players;
        this.layer.removeChildren().forEach(c => c.destroy({ children: true }));
        this.veins = [];

        players.forEach((player, index) => {
            if (index > 1) return;
            const textX = LEFT_BOARD_END + 10;

            const name = new Text({
                text: player.username || `Player ${index + 1}`,
                style: labelStyle(),
            });
            name.position.set(textX, LABEL_Y[index] - 14);
            this.layer.addChild(name);

            const gold = new Text({
                text: `Gold: ${player.gold}`,
                style: new TextStyle({ fontFamily: theme.font, fontSize: 14, fill: hexNum(C.accentGold) }),
            });
            gold.position.set(textX + 110, LABEL_Y[index] - 14);
            this.layer.addChild(gold);

            // Vein (pulsing gold circle, fake glow ring)
            const vein = new Graphics();
            vein.circle(0, 0, 24).fill({ color: hexNum(C.vein), alpha: 0.18 });
            vein.circle(0, 0, 20).fill(hexNum(C.vein));
            vein.position.set(MERC_BUILDING_X, VEIN_Y[index]);
            this.layer.addChild(vein);
            this.veins.push(vein);
            const veinLabel = new Text({ text: 'VEIN', style: smallStyle(hexNum(C.bgDeep)) });
            veinLabel.anchor.set(0.5);
            veinLabel.position.set(MERC_BUILDING_X, VEIN_Y[index]);
            this.layer.addChild(veinLabel);

            // Cart
            const cart = new Graphics();
            cart.rect(-20, -20, 40, 40).fill(hexNum(C.cart))
                .stroke({ width: 1, color: hexNum(C.accentGold), alpha: 0.5 });
            cart.position.set(MERC_BUILDING_X, CART_Y[index]);
            this.layer.addChild(cart);
            const cartLabel = new Text({ text: 'CART', style: smallStyle(hexNum(C.textPrimary)) });
            cartLabel.anchor.set(0.5);
            cartLabel.position.set(MERC_BUILDING_X, CART_Y[index]);
            this.layer.addChild(cartLabel);

            // Mercenary building
            const merc = new Graphics();
            merc.rect(-MERC_BUILDING_HALF, -MERC_BUILDING_HALF, MERC_BUILDING_HALF * 2, MERC_BUILDING_HALF * 2)
                .fill(hexNum(C.mercBuilding))
                .stroke({ width: 2, color: hexNum(C.accentGold) });
            merc.position.set(MERC_BUILDING_X, MERC_BUILDING_Y[index]);
            this.layer.addChild(merc);
            const mercLabel = new Text({ text: '⚔ MERC', style: smallStyle(hexNum(C.accentGold)) });
            mercLabel.anchor.set(0.5);
            mercLabel.position.set(MERC_BUILDING_X, MERC_BUILDING_Y[index]);
            this.layer.addChild(mercLabel);

            // Spawning queue icons below the merc building
            if (player.spawning_queue && player.spawning_queue.length > 0) {
                const groundY = MERC_BUILDING_Y[index] + MERC_BUILDING_HALF + 14;
                const startX = MERC_BUILDING_X - (player.spawning_queue.length * 12) / 2;
                const q = new Graphics();
                player.spawning_queue.forEach((shape, i) => {
                    const radius = shape === 'Circle' ? 8 : shape === 'Triangle' ? 6 : 4;
                    q.circle(startX + i * 14, groundY, radius).fill(hexNum(C.spawnQueueIcon));
                });
                this.layer.addChild(q);
            }
        });
    }

    update(dtMs: number): void {
        this.elapsed += dtMs;
        const pulse = 0.85 + 0.15 * Math.sin(this.elapsed / 400);
        for (const v of this.veins) v.alpha = pulse;
    }
}
