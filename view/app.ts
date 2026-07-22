import { initRenderer, RendererHandle, ClickHit } from './renderer';
import { Unit, Player, CombatEvent, SendUnitCatalogEntry, DamageType, BuildCatalogEntry, Family, UnitKind } from './types';
import { applyThemeToDom } from './theme';
import { UnitInfoPanel } from './unit_info_panel';
import { MercenaryPanel } from './mercenary_panel';
import { KingUpgradePanel } from './king_upgrade_panel';
import { renderBuildShop, renderFamilyOptions } from './app_build_ui';

// --- TYPES & INTERFACES ---
interface UnitStaticInfo {
    entity_id: number;
    attack_damage: number | null;
    attack_rate: number | null;
    attack_range: number | null;
    damage_type: DamageType | null;
    armor: number | null;
    is_boss: boolean;
    sell_value: number | null;
}

type ClientMessagePayload =
    | { action: 'joinQueue' }
    | { action: 'leaveQueue' }
    | { action: 'place'; payload: { shape: string; row: number; col: number } }
    | { action: 'sellById'; payload: { entity_id: number } }
    | { action: 'skipToCombat' }
    | { action: 'leaveLobby' }
    | { action: 'hireWorker'; payload: Record<string, never> }
    | { action: 'requestUnitInfo'; payload: { entity_id: number } }
    | { action: 'sendUnit'; payload: { shape: string } }
    | { action: 'upgradeKing'; payload: Record<string, never> }
    | { action: 'requestFullState' };

interface GameState {
    units: Unit[];
    players: Player[];
    phase: string;
    phase_timer: number;
    winner_id: number | null;
    seq: number;
}

interface PhaseInfo {
    phase: string;
    phase_timer: number;
    winner_id: number | null;
}

interface GameStateDelta {
    seq: number;
    added: Unit[];
    updated: Unit[];
    removed: number[];
    players: Player[] | null;
    phase_info: PhaseInfo | null;
}

type ServerMessage =
    | { type: 'Queued' }
    | { type: 'MatchFound' }
    | { type: 'SendUnitCatalog'; data: SendUnitCatalogEntry[] }
    | { type: 'FamilyOptions'; data: Family[] }
    | { type: 'BuildCatalog'; data: BuildCatalogEntry[] }
    | { type: 'GameState'; data: GameState }
    | { type: 'GameStateDelta'; data: GameStateDelta }
    | { type: 'CombatEvents'; data: CombatEvent[] }
    | { type: 'PlayerId'; data: number }
    | { type: 'Error'; data: string }
    | { type: 'UnitInfo'; data: UnitStaticInfo };

// Game Over overlay
const gameOverOverlay = document.getElementById('game-over-overlay') as HTMLDivElement;
const gameResultTitle = document.getElementById('game-result-title') as HTMLHeadingElement;
const gameResultSubtitle = document.getElementById('game-result-subtitle') as HTMLParagraphElement;
const overlayLeaveLobbyButton = document.getElementById('overlay-leave-lobby') as HTMLButtonElement;

// Views
const authView = document.getElementById('auth-view') as HTMLDivElement;
const lobbySelectionView = document.getElementById('lobby-selection') as HTMLDivElement;
const gameView = document.getElementById('game-view') as HTMLDivElement;

// Auth elements
const registerForm = document.getElementById('register-form') as HTMLFormElement;
const loginForm = document.getElementById('login-form') as HTMLFormElement;
const authStatus = document.getElementById('auth-status') as HTMLDivElement;

// Lobby elements
const queueBtn = document.getElementById('queue-btn') as HTMLButtonElement;
const cancelQueueBtn = document.getElementById('cancel-queue-btn') as HTMLButtonElement;
const queueStatus = document.getElementById('queue-status') as HTMLParagraphElement;

// Game elements
const leaveLobbyButton = document.getElementById('leave-lobby') as HTMLButtonElement;
const gamePhaseEl = document.getElementById('game-phase') as HTMLSpanElement;
const gameTimerEl = document.getElementById('game-timer') as HTMLSpanElement;
const goldDisplay = document.getElementById('gold-display') as HTMLSpanElement;
const livesDisplay = document.getElementById('lives-display') as HTMLSpanElement;
const hireWorkerBtn = document.getElementById('hire-worker-btn') as HTMLButtonElement;
const familyPickEl = document.getElementById('family-pick') as HTMLDivElement;
const buildShopEl = document.getElementById('build-shop') as HTMLDivElement;

const WORKER_CAP = 7;

let selectedUnitKind: UnitKind | null = null;
let unitMap = new Map<number, Unit>();
let lastSeq = -1;
let currentPlayers: Player[] = [];
let myPlayerId: number | null = null;
let socket: WebSocket | null = null;
let isInGame = false;
let gamePhase = '';
let gameTimer = 0;

const panel = new UnitInfoPanel(
    document.getElementById('ui-panel') as HTMLElement,
    {
        onSell: (entityId: number) => {
            if (socket && socket.readyState === WebSocket.OPEN) {
                socket.send(JSON.stringify({ action: 'sellById', payload: { entity_id: entityId } }));
            }
        },
    }
);

const mercPanel = new MercenaryPanel(
    document.getElementById('mercenary-panel') as HTMLElement,
    {
        onSend: (shape) => {
            if (socket && socket.readyState === WebSocket.OPEN) {
                socket.send(JSON.stringify({ action: 'sendUnit', payload: { shape } }));
            }
        },
    }
);

const kingUpgradePanel = new KingUpgradePanel({
    container: document.getElementById('king-upgrade-panel') as HTMLElement,
    onUpgrade: () => {
        if (socket && socket.readyState === WebSocket.OPEN) {
            socket.send(JSON.stringify({ action: 'upgradeKing', payload: {} }));
        }
    },
});

applyThemeToDom(document.documentElement);
const rendererHost = document.getElementById('game-canvas-host') as HTMLDivElement;
const renderer: RendererHandle = await initRenderer(rendererHost);

const API_BASE_URL = 'http://127.0.0.1:9001';

// --- VIEW MANAGEMENT ---
function showAuthView() {
    authView.style.display = 'block';
    lobbySelectionView.style.display = 'none';
    gameView.style.display = 'none';
}

function showLobbyView() {
    authView.style.display = 'none';
    lobbySelectionView.style.display = 'block';
    gameView.style.display = 'none';
}

function showGameView() {
    authView.style.display = 'none';
    lobbySelectionView.style.display = 'none';
    gameView.style.display = 'flex';
}

// --- AUTHENTICATION LOGIC ---
registerForm.addEventListener('submit', async (e) => {
    e.preventDefault();
    const username = (document.getElementById('register-username') as HTMLInputElement).value;
    const password = (document.getElementById('register-password') as HTMLInputElement).value;

    try {
        const response = await fetch(`${API_BASE_URL}/api/auth/register`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ username, password }),
        });

        if (response.ok) {
            const data = await response.json();
            authStatus.innerHTML = `<p class="green-text">Registration successful for ${data.username}! Please log in.</p>`;
            registerForm.reset();
        } else {
            const error = await response.json();
            authStatus.innerHTML = `<p class="red-text">Registration failed: ${error.error}</p>`;
        }
    } catch (error) {
        authStatus.innerHTML = `<p class="red-text">Error: Could not connect to server.</p>`;
    }
});

loginForm.addEventListener('submit', async (e) => {
    e.preventDefault();
    const username = (document.getElementById('login-username') as HTMLInputElement).value;
    const password = (document.getElementById('login-password') as HTMLInputElement).value;

    try {
        const response = await fetch(`${API_BASE_URL}/api/auth/login`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ username, password }),
        });

        if (response.ok) {
            const data = await response.json();
            localStorage.setItem('jwt', data.token);
            authStatus.innerHTML = '';
            connectAndShowLobby();
        } else {
            const error = await response.json();
            authStatus.innerHTML = `<p class="red-text">Login failed: ${error.error}</p>`;
        }
    } catch (error) {
        authStatus.innerHTML = `<p class="red-text">Error: Could not connect to server.</p>`;
    }
});


// --- WEBSOCKET AND GAME LOGIC ---
function connectAndShowLobby() {
    const token = localStorage.getItem('jwt');
    if (!token) {
        showAuthView();
        authStatus.innerHTML = `<p class="red-text">You are not logged in.</p>`;
        return;
    }

    socket = new WebSocket(`ws://127.0.0.1:9001/ws?token=${token}`);

    socket.onopen = function () {
        showLobbyView();
    };

    socket.onmessage = function (event) {
        const serverMsg: ServerMessage = JSON.parse(event.data);
        switch (serverMsg.type) {
            case 'Queued':
                queueBtn.style.display = 'none';
                cancelQueueBtn.style.display = 'inline-block';
                queueStatus.textContent = 'Searching for opponent…';
                break;
            case 'MatchFound':
                isInGame = true;
                resetQueueUi();
                showGameView();
                break;
            case 'SendUnitCatalog':
                mercPanel.setCatalog(serverMsg.data);
                break;
            case 'FamilyOptions':
                renderFamilyOptions(familyPickEl, serverMsg.data, (family) => {
                    if (socket && socket.readyState === WebSocket.OPEN) {
                        socket.send(JSON.stringify({ action: 'pickFamily', payload: { family } }));
                    }
                });
                break;
            case 'BuildCatalog':
                familyPickEl.innerHTML = '';
                renderBuildShop(buildShopEl, serverMsg.data, (unitKind) => {
                    selectedUnitKind = unitKind;
                });
                break;
            case 'GameState':
                if (!isInGame) return;
                if (gameView.style.display === 'none') showGameView();
                applyFullState(serverMsg.data);
                break;
            case 'GameStateDelta':
                if (!isInGame) return;
                applyDelta(serverMsg.data);
                break;
            case 'CombatEvents':
                if (!isInGame) return;
                handleCombatEvents(serverMsg.data);
                break;
            case 'PlayerId':
                myPlayerId = serverMsg.data;
                break;
            case 'Error':
                // @ts-ignore
                M.toast({ html: serverMsg.data });
                renderer.flashError(serverMsg.data);
                break;
            case 'UnitInfo':
                panel.applyStaticInfo(serverMsg.data);
                break;
        }
    };

    socket.onclose = function () {
        isInGame = false;
        // @ts-ignore
        M.toast({ html: 'Disconnected from server.' });
        localStorage.removeItem('jwt');
        showAuthView();
    };

    socket.onerror = function () {
        isInGame = false;
        // @ts-ignore
        M.toast({ html: 'WebSocket error.' });
        localStorage.removeItem('jwt');
        showAuthView();
    };
}


function resetQueueUi() {
    queueBtn.style.display = 'inline-block';
    cancelQueueBtn.style.display = 'none';
    queueStatus.textContent = '';
}

queueBtn.onclick = () => {
    socket?.send(JSON.stringify({ action: 'joinQueue' }));
};

cancelQueueBtn.onclick = () => {
    socket?.send(JSON.stringify({ action: 'leaveQueue' }));
    resetQueueUi();
};

function applyPanelBoardSide(): void {
    if (myPlayerId === null) return;
    const idx = currentPlayers.findIndex(p => p.id === myPlayerId);
    if (idx === -1) return;
    const uiPanel = document.getElementById('ui-panel') as HTMLElement;
    uiPanel.style.marginLeft = idx === 0 ? '0px' : '700px';
}

function currentUnits(): Unit[] {
    return Array.from(unitMap.values());
}

// Refreshes displays derived from currentPlayers + unitMap (gold, king HP,
// king upgrade panel, panel board side). Cheap enough to call unconditionally
// after any full state or delta application.
function refreshDerivedDisplays() {
    const me = currentPlayers.find(p => p.id === myPlayerId);
    if (me) {
        goldDisplay.textContent = me.income > 0
            ? `${me.gold} (+${me.income}/round)`
            : me.gold.toString();
        mercPanel.updatePlayer(me.gold, me.next_send_costs);
    }

    // Update king HP display
    const myKing = currentUnits().find(u => u.is_king && u.owner_id === myPlayerId);
    if (myKing) {
        livesDisplay.textContent = `${myKing.current_hp}/${myKing.max_hp}`;
    } else {
        livesDisplay.textContent = '--';
    }

    // Worker cap: disable hire button once this player has hired the max (server enforces the cap; this is UX polish).
    if (myPlayerId !== null) {
        const myWorkers = currentUnits().filter(u => u.is_worker && u.owner_id === myPlayerId).length;
        hireWorkerBtn.disabled = myWorkers >= WORKER_CAP;
    }

    // Update king upgrade panel
    if (me && myKing) {
        kingUpgradePanel.update(
            { currentTier: me.king_tier, currentHp: myKing.current_hp, maxHp: myKing.max_hp },
            me.gold,
            gamePhase
        );
    }

    applyPanelBoardSide();
}

function setPhaseText(phase: string, timer: number) {
    gamePhase = phase;
    gameTimer = Math.max(0, timer);

    gamePhaseEl.textContent = gamePhase;
    gameTimerEl.textContent = gameTimer.toFixed(1);
}

function updateGameOverOverlay(winnerId: number | null) {
    if (gamePhase === 'GameOver') {
        const isLoser = winnerId !== null && winnerId !== myPlayerId;
        const isDraw = winnerId === null;

        if (isDraw) {
            gameResultTitle.textContent = 'Draw!';
            gameResultTitle.className = 'defeat';
            gameResultSubtitle.textContent = 'Both kings fell simultaneously.';
        } else if (isLoser) {
            gameResultTitle.textContent = 'Defeat';
            gameResultTitle.className = 'defeat';
            gameResultSubtitle.textContent = 'Your base was overrun.';
        } else {
            gameResultTitle.textContent = 'Victory!';
            gameResultTitle.className = 'victory';
            gameResultSubtitle.textContent = 'Your opponent\'s base fell!';
        }
        gameOverOverlay.style.display = 'flex';
        mercPanel.hide();
        kingUpgradePanel.hide();
        selectedUnitKind = null;
    } else if (gamePhase === 'Victory') {
        gameResultTitle.textContent = 'Victory!';
        gameResultTitle.className = 'victory';
        gameResultSubtitle.textContent = 'All waves defeated!';
        gameOverOverlay.style.display = 'flex';
        mercPanel.hide();
        kingUpgradePanel.hide();
        selectedUnitKind = null;
    } else {
        gameOverOverlay.style.display = 'none';
    }
}

function applyFullState(newState: GameState) {
    unitMap = new Map(newState.units.map(u => [u.id, u]));
    lastSeq = newState.seq;
    currentPlayers = newState.players;

    // Phase must be set before refreshDerivedDisplays: the king upgrade panel
    // gates on the module-level gamePhase.
    setPhaseText(newState.phase, newState.phase_timer);
    refreshDerivedDisplays();
    panel.syncDynamicState(currentUnits(), gamePhase);
    updateGameOverOverlay(newState.winner_id);
    renderer.syncState(unitMap, currentPlayers, gamePhase, myPlayerId);
}

function applyDelta(d: GameStateDelta) {
    if (d.seq <= lastSeq) return; // stale delta (pre-snapshot): drop

    if (d.seq !== lastSeq + 1) {
        // Gap: a delta was missed. Ask for a direct resync and keep rendering
        // the old state until the full snapshot arrives.
        if (socket && socket.readyState === WebSocket.OPEN) {
            socket.send(JSON.stringify({ action: 'requestFullState' }));
        }
        return;
    }

    lastSeq = d.seq;
    for (const u of d.added) unitMap.set(u.id, u);
    for (const u of d.updated) unitMap.set(u.id, u);
    for (const id of d.removed) unitMap.delete(id);
    if (d.players) currentPlayers = d.players;

    // Phase must be set before refreshDerivedDisplays: the king upgrade panel
    // gates on the module-level gamePhase. updateGameOverOverlay must run AFTER
    // refreshDerivedDisplays (matching applyFullState's order): refreshDerivedDisplays
    // -> kingUpgradePanel.update() unconditionally shows the panel, which would
    // re-show it right after the overlay hid it if called in the other order.
    if (d.phase_info) {
        setPhaseText(d.phase_info.phase, d.phase_info.phase_timer);
    }
    refreshDerivedDisplays();
    panel.syncDynamicState(currentUnits(), gamePhase);
    if (d.phase_info) {
        updateGameOverOverlay(d.phase_info.winner_id);
    }
    renderer.syncState(unitMap, currentPlayers, gamePhase, myPlayerId);
}

function handleCombatEvents(events: CombatEvent[]) {
    renderer.playCombatEvents(events);
}

// --- ONE-TIME EVENT REGISTRATION ---
hireWorkerBtn.onclick = () => {
    if (socket && socket.readyState === WebSocket.OPEN) {
        socket.send(JSON.stringify({ action: 'hireWorker', payload: {} }));
    }
};
document.getElementById('skip-to-combat')!.onclick = () => {
    if (socket && socket.readyState === WebSocket.OPEN) {
        socket.send(JSON.stringify({ action: 'skipToCombat' }));
    }
};

renderer.onClick((hit: ClickHit) => {
    if (!isInGame) return;
    switch (hit.kind) {
        case 'unit': {
            const clickedUnit = unitMap.get(hit.unitId);
            if (!clickedUnit) return;
            panel.selectUnit(clickedUnit, myPlayerId!, gamePhase);
            if (!panel.staticInfoCache.has(clickedUnit.id)) {
                if (socket && socket.readyState === WebSocket.OPEN) {
                    socket.send(JSON.stringify({ action: 'requestUnitInfo', payload: { entity_id: clickedUnit.id } }));
                }
            }
            return;
        }
        case 'mercBuilding':
            mercPanel.toggle();
            return;
        case 'cell': {
            panel.clearSelection();
            if (hit.row >= 8) return; // king protection zone — no placement
            if (!selectedUnitKind) return; // no family/tower picked yet
            const placeMessage = { action: 'place', payload: { shape: selectedUnitKind, row: hit.row, col: hit.col } };
            if (socket && socket.readyState === WebSocket.OPEN) socket.send(JSON.stringify(placeMessage));
            return;
        }
        case 'empty':
            panel.clearSelection();
            return;
    }
});

function handleLeaveLobby() {
    isInGame = false;
    gameOverOverlay.style.display = 'none';
    socket?.send(JSON.stringify({ action: 'leaveLobby' }));
    panel.clearSelection();
    mercPanel.hide();
    kingUpgradePanel.hide();
    renderer.reset();
    resetQueueUi();
    showLobbyView();
}

leaveLobbyButton.onclick = handleLeaveLobby;
overlayLeaveLobbyButton.onclick = handleLeaveLobby;

showAuthView();
