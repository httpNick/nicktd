import { AnimationManager, DamageType, Position } from './animations';
import { UnitInfoPanel } from './unit_info_panel';

// --- TYPES & INTERFACES ---
interface Unit {
    id: number;
    shape: 'Square' | 'Circle' | 'Triangle';
    x: number;
    y: number;
    owner_id: number;
    is_enemy: boolean;
    current_hp: number;
    max_hp: number;
    is_worker: boolean;
    current_mana?: number;
    max_mana?: number;
    worker_state?: 'MovingToVein' | 'Mining' | 'MovingToCart';
}

interface UnitStaticInfo {
    entity_id: number;
    attack_damage: number | null;
    attack_rate: number | null;
    attack_range: number | null;
    damage_type: 'PhysicalBasic' | 'PhysicalPierce' | 'FireMagical' | null;
    armor: number | null;
    is_boss: boolean;
    sell_value: number | null;
}

type ClientMessagePayload =
    | { action: 'joinLobby'; payload: number }
    | { action: 'place'; payload: { shape: string; row: number; col: number } }
    | { action: 'sellById'; payload: { entity_id: number } }
    | { action: 'skipToCombat' }
    | { action: 'leaveLobby' }
    | { action: 'hireWorker'; payload: Record<string, never> }
    | { action: 'requestUnitInfo'; payload: { entity_id: number } };

interface Player {
    id: number;
    username: string;
    gold: number;
}

interface GameState {
    units: Unit[];
    players: Player[];
    phase: string;
    phase_timer: number;
}

interface CombatEvent {
    attacker_id: number;
    target_id: number;
    attack_type: DamageType;
    start_pos: Position;
    end_pos: Position;
}

type ServerMessage =
    | { type: 'LobbyStatus'; data: any[] }
    | { type: 'GameState'; data: GameState }
    | { type: 'CombatEvents'; data: CombatEvent[] }
    | { type: 'PlayerId'; data: number }
    | { type: 'Error'; data: string }
    | { type: 'UnitInfo'; data: UnitStaticInfo };

// Views
const authView = document.getElementById('auth-view') as HTMLDivElement;
const lobbySelectionView = document.getElementById('lobby-selection') as HTMLDivElement;
const gameView = document.getElementById('game-view') as HTMLDivElement;

// Auth elements
const registerForm = document.getElementById('register-form') as HTMLFormElement;
const loginForm = document.getElementById('login-form') as HTMLFormElement;
const authStatus = document.getElementById('auth-status') as HTMLDivElement;

// Lobby elements
const lobbyList = document.getElementById('lobby-list') as HTMLDivElement;

// Game elements
const canvas = document.getElementById('gameCanvas') as HTMLCanvasElement;
const ctx = canvas.getContext('2d')!;
const leaveLobbyButton = document.getElementById('leave-lobby') as HTMLButtonElement;
const gamePhaseEl = document.getElementById('game-phase') as HTMLSpanElement;
const gameTimerEl = document.getElementById('game-timer') as HTMLSpanElement;
const goldDisplay = document.getElementById('gold-display') as HTMLSpanElement;
const hireWorkerBtn = document.getElementById('hire-worker-btn') as HTMLButtonElement;

const BOARD_SIZE = 10;
const SQUARE_SIZE = 60;
const GAP_SIZE = 200;
const LEFT_BOARD_END = 600;
const RIGHT_BOARD_START = 800;

let selectedShape: 'Square' | 'Circle' | 'Triangle' = 'Square';
let gameState: Unit[] = [];
let currentPlayers: Player[] = [];
let myPlayerId: number | null = null;
let socket: WebSocket | null = null;
let isInGame = false;
let gamePhase = '';
let gameTimer = 0;

const animationManager = new AnimationManager();

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
            case 'LobbyStatus':
                renderLobbies(serverMsg.data);
                break;
            case 'GameState':
                if (!isInGame) return;
                if (gameView.style.display === 'none') showGameView();
                updateGameState(serverMsg.data);
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


function renderLobbies(lobbies: any[]) {
    lobbyList.innerHTML = '';
    lobbies.forEach(lobby => {
        const lobbyEl = document.createElement('div');
        lobbyEl.className = 'col s12 m6 l4';
        lobbyEl.innerHTML = `
            <div class="card blue-grey darken-1">
                <div class="card-content white-text"><span class="card-title">Lobby ${lobby.id + 1}</span><p>Players: ${lobby.player_count} / 2</p></div>
                <div class="card-action"><a href="#" class="join-lobby-btn" data-lobby-id="${lobby.id}" ${lobby.player_count >= 2 ? 'disabled' : ''}>Join</a></div>
            </div>`;
        lobbyList.appendChild(lobbyEl);
    });
    document.querySelectorAll('.join-lobby-btn').forEach(button => {
        const btn = button as HTMLAnchorElement;
        if (!btn.hasAttribute('disabled')) {
            btn.onclick = (e) => {
                e.preventDefault();
                isInGame = true;
                const lobbyId = parseInt(btn.getAttribute('data-lobby-id')!);
                socket?.send(JSON.stringify({ action: 'joinLobby', payload: lobbyId }));
            };
        }
    });
}

function drawCheckerboard() {
    // Left Board
    for (let row = 0; row < BOARD_SIZE; row++) {
        for (let col = 0; col < BOARD_SIZE; col++) {
            ctx.fillStyle = (row + col) % 2 === 0 ? '#EEE' : '#CCC';
            ctx.fillRect(col * SQUARE_SIZE, row * SQUARE_SIZE, SQUARE_SIZE, SQUARE_SIZE);
        }
    }
    // Right Board
    for (let row = 0; row < BOARD_SIZE; row++) {
        for (let col = 0; col < BOARD_SIZE; col++) {
            ctx.fillStyle = (row + col) % 2 === 0 ? '#EEE' : '#CCC';
            ctx.fillRect(RIGHT_BOARD_START + col * SQUARE_SIZE, row * SQUARE_SIZE, SQUARE_SIZE, SQUARE_SIZE);
        }
    }
}

function drawWorkerArea() {
    ctx.strokeStyle = '#FFF';

    // Board boundaries
    ctx.beginPath();
    ctx.moveTo(LEFT_BOARD_END, 0);
    ctx.lineTo(LEFT_BOARD_END, 600);
    ctx.stroke();

    ctx.beginPath();
    ctx.moveTo(RIGHT_BOARD_START, 0);
    ctx.lineTo(RIGHT_BOARD_START, 600);
    ctx.stroke();

    // Central horizontal separator
    ctx.beginPath();
    ctx.moveTo(LEFT_BOARD_END, 300);
    ctx.lineTo(RIGHT_BOARD_START, 300);
    ctx.stroke();

    currentPlayers.forEach((player, index) => {
        const veinY = index === 0 ? 50 : 350;
        const cartY = index === 0 ? 250 : 550;
        const labelY = index === 0 ? 20 : 320;
        const textX = LEFT_BOARD_END + 10;

        ctx.fillStyle = '#FFF';
        ctx.font = '16px Arial';
        ctx.fillText(player.username || `Player ${index + 1}`, textX, labelY);
        ctx.fillText(`Gold: ${player.gold}`, textX + 110, labelY);

        ctx.fillStyle = '#FFD700';
        ctx.beginPath();
        ctx.arc(LEFT_BOARD_END + (GAP_SIZE / 2), veinY, 20, 0, 2 * Math.PI);
        ctx.fill();
        ctx.fillStyle = '#000';
        ctx.fillText("Vein", LEFT_BOARD_END + (GAP_SIZE / 2) - 15, veinY + 5);

        ctx.fillStyle = '#8B4513';
        ctx.fillRect(LEFT_BOARD_END + (GAP_SIZE / 2) - 20, cartY - 20, 40, 40);
        ctx.fillStyle = '#FFF';
        ctx.fillText("Cart", LEFT_BOARD_END + (GAP_SIZE / 2) - 15, cartY + 5);
    });
}

function drawUnits(units: Unit[]) {
    units.forEach(unit => {
        const { shape, x, y, owner_id, is_enemy } = unit;
        if (is_enemy) {
            ctx.fillStyle = '#2E8B57';
        } else {
            ctx.fillStyle = owner_id === myPlayerId ? '#88F' : '#F88';
        }

        if (shape === 'Square') {
            ctx.fillRect(x - (SQUARE_SIZE / 2 - 10), y - (SQUARE_SIZE / 2 - 10), SQUARE_SIZE - 20, SQUARE_SIZE - 20);
        } else if (shape === 'Circle') {
            ctx.beginPath(); ctx.arc(x, y, SQUARE_SIZE / 2 - 10, 0, 2 * Math.PI); ctx.fill();
        } else if (shape === 'Triangle') {
            ctx.beginPath(); ctx.moveTo(x, y - (SQUARE_SIZE / 2 - 10)); ctx.lineTo(x - (SQUARE_SIZE / 2 - 10), y + (SQUARE_SIZE / 2 - 10)); ctx.lineTo(x + (SQUARE_SIZE / 2 - 10), y + (SQUARE_SIZE / 2 - 10)); ctx.closePath(); ctx.fill();
        }
    });

    units.forEach(unit => {
        const { x, y, current_hp, max_hp, is_worker } = unit;
        if (!is_worker && current_hp !== undefined && max_hp !== undefined) {
            const barWidth = SQUARE_SIZE - 20;
            const barHeight = 6;
            const barX = x - barWidth / 2;
            const barY = y - (SQUARE_SIZE / 2);

            ctx.fillStyle = '#F00';
            ctx.fillRect(barX, barY, barWidth, barHeight);

            const healthPercent = Math.max(0, Math.min(1, current_hp / max_hp));
            ctx.fillStyle = '#0F0';
            ctx.fillRect(barX, barY, barWidth * healthPercent, barHeight);
        }
    });
}

function applyPanelBoardSide(): void {
    if (myPlayerId === null) return;
    const idx = currentPlayers.findIndex(p => p.id === myPlayerId);
    if (idx === -1) return;
    const uiPanel = document.getElementById('ui-panel') as HTMLElement;
    uiPanel.style.marginLeft = idx === 0 ? '0px' : '700px';
}

function updateGameState(newState: GameState) {
    gameState = newState.units;
    if (newState.players) {
        currentPlayers = newState.players;
        const me = newState.players.find(p => p.id === myPlayerId);
        if (me) goldDisplay.textContent = me.gold.toString();
        applyPanelBoardSide();
    }
    gamePhase = newState.phase;
    gameTimer = Math.max(0, newState.phase_timer);

    gamePhaseEl.textContent = gamePhase;
    gameTimerEl.textContent = gameTimer.toFixed(1);

    panel.syncDynamicState(gameState, gamePhase);
}

function handleCombatEvents(events: CombatEvent[]) {
    events.forEach(event => {
        const isRanged = event.attack_type === 'FireMagical' || event.attack_type === 'PhysicalPierce';
        const type = isRanged ? 'projectile' : 'melee';
        const duration = isRanged ? 300 : 200;

        animationManager.addAnimation(
            type,
            event.start_pos,
            event.end_pos,
            duration,
            event.attack_type
        );
    });
}

function render() {
    if (isInGame) {
        ctx.clearRect(0, 0, canvas.width, canvas.height);
        drawCheckerboard();
        drawWorkerArea();
        drawUnits(gameState);

        animationManager.update();
        animationManager.draw(ctx);
    }
    requestAnimationFrame(render);
}

requestAnimationFrame(render);

// --- ONE-TIME EVENT REGISTRATION ---
document.getElementById('selectSquare')!.onclick = () => { selectedShape = 'Square'; };
document.getElementById('selectCircle')!.onclick = () => { selectedShape = 'Circle'; };
document.getElementById('selectTriangle')!.onclick = () => { selectedShape = 'Triangle'; };
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

canvas.addEventListener('click', function (event) {
    if (!isInGame) return;
    const rect = canvas.getBoundingClientRect();
    const clickX = event.clientX - rect.left;
    const clickY = event.clientY - rect.top;

    // Identify which board was clicked
    let boardIdx: number | null = null;
    let localX = clickX;
    if (clickX < LEFT_BOARD_END) {
        boardIdx = 0;
    } else if (clickX >= RIGHT_BOARD_START && clickX < RIGHT_BOARD_START + LEFT_BOARD_END) {
        boardIdx = 1;
        localX = clickX - RIGHT_BOARD_START;
    }

    if (boardIdx === null) {
        panel.clearSelection();
        return;
    }

    const unitHitSize = SQUARE_SIZE - 20;
    const clickedUnit = gameState.find(s => {
        return clickX >= s.x - unitHitSize / 2 && clickX <= s.x + unitHitSize / 2 &&
            clickY >= s.y - unitHitSize / 2 && clickY <= s.y + unitHitSize / 2;
    });

    if (clickedUnit) {
        panel.selectUnit(clickedUnit, myPlayerId!, gamePhase);
        if (!panel.staticInfoCache.has(clickedUnit.id)) {
            if (socket && socket.readyState === WebSocket.OPEN) {
                socket.send(JSON.stringify({ action: 'requestUnitInfo', payload: { entity_id: clickedUnit.id } }));
            }
        }
    } else {
        panel.clearSelection();
        const row = Math.floor(clickY / SQUARE_SIZE);
        const col = Math.floor(localX / SQUARE_SIZE);
        const placeMessage = { action: 'place', payload: { shape: selectedShape, row, col } };
        if (socket && socket.readyState === WebSocket.OPEN) socket.send(JSON.stringify(placeMessage));
    }
});

leaveLobbyButton.onclick = () => {
    isInGame = false;
    socket?.send(JSON.stringify({ action: 'leaveLobby' }));
    panel.clearSelection();
    showLobbyView();
};

showAuthView();
