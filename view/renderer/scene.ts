import { Application, Container } from 'pixi.js';
import { theme, hexNum } from '../theme';
import { CANVAS_WIDTH, CANVAS_HEIGHT } from '../layout';

export interface SceneLayers {
    board: Container;    // static board graphics, built once
    midlane: Container;  // gap-area dynamic content (vein, cart, merc, labels)
    units: Container;    // one child Container per living unit
    effects: Container;  // transient world-space effects
    overlay: Container;  // screen-space: phase banner, error vignette
}

export interface Scene {
    app: Application;
    layers: SceneLayers;
}

export async function createScene(container: HTMLElement): Promise<Scene> {
    const app = new Application();
    await app.init({
        width: CANVAS_WIDTH,
        height: CANVAS_HEIGHT,
        background: hexNum(theme.colors.bgDeep),
        antialias: true,
    });
    container.appendChild(app.canvas);

    const layers: SceneLayers = {
        board: new Container(),
        midlane: new Container(),
        units: new Container(),
        effects: new Container(),
        overlay: new Container(),
    };
    app.stage.addChild(layers.board, layers.midlane, layers.units, layers.effects, layers.overlay);
    return { app, layers };
}
