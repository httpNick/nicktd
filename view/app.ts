import { AnimationManager, DamageType, Position } from './animations';
import { UnitInfoPanel } from './unit_info_panel';
import { MercenaryPanel } from './mercenary_panel';
import { KingUpgradePanel } from './king_upgrade_panel';

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
    is_king: boolean;
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
    | { action: 'requestUnitInfo'; payload: { entity_id: number } }
    | { action: 'sendUnit'; payload: { shape: string } }
    | { action: 'upgradeKing'; payload: Record<string, never> };

interface Player {
    id: number;
    username: string;
    gold: number;
    income: number;
    king_tier: number;
    spawning_queue: ('Square' | 'Circle' | 'Triangle')[];
}

interface GameState {
    units: Unit[];
    players: Player[];
    phase: string;
    phase_timer: number;
    winner_id: number | null;
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
const lobbyList = document.getElementById('lobby-list') as HTMLDivElement;

// Game elements
const canvas = document.getElementById('gameCanvas') as HTMLCanvasElement;
const ctx = canvas.getContext('2d')!;
const leaveLobbyButton = document.getElementById('leave-lobby') as HTMLButtonElement;
const gamePhaseEl = document.getElementById('game-phase') as HTMLSpanElement;
const gameTimerEl = document.getElementById('game-timer') as HTMLSpanElement;
const goldDisplay = document.getElementById('gold-display') as HTMLSpanElement;
const livesDisplay = document.getElementById('lives-display') as HTMLSpanElement;
const hireWorkerBtn = document.getElementById('hire-worker-btn') as HTMLButtonElement;

const BOARD_SIZE = 10;
const SQUARE_SIZE = 60;
const GAP_SIZE = 200;
const LEFT_BOARD_END = 600;
const RIGHT_BOARD_START = 800;

// King zone constants
const TOTAL_HEIGHT = 600;
const KING_ZONE_HEIGHT = 120;
const CANVAS_HEIGHT = 720; // TOTAL_HEIGHT + KING_ZONE_HEIGHT

const KING_Y = 660;           // TOTAL_HEIGHT + 60
const KING_LEFT_X = 300;      // BOARD_SIZE / 2 * SQUARE_SIZE
const KING_RIGHT_X = 1100;    // RIGHT_BOARD_START + BOARD_SIZE / 2 * SQUARE_SIZE
const KING_RADIUS = 30;

const MERC_BUILDING_X = LEFT_BOARD_END + GAP_SIZE / 2;  // 700
const MERC_BUILDING_Y = [150, 450] as const;
const MERC_BUILDING_HALF = 18;

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

function drawKingZone() {
    // Left board king zone: x = 0..600, y = 600..720
    ctx.fillStyle = '#1a1a2e';
    ctx.fillRect(0, TOTAL_HEIGHT, LEFT_BOARD_END, KING_ZONE_HEIGHT);

    // Right board king zone: x = 800..1400, y = 600..720
    ctx.fillRect(RIGHT_BOARD_START, TOTAL_HEIGHT, LEFT_BOARD_END, KING_ZONE_HEIGHT);

    // Border / separator lines at y = 600
    ctx.strokeStyle = '#444488';
    ctx.lineWidth = 2;
    ctx.beginPath();
    ctx.moveTo(0, TOTAL_HEIGHT);
    ctx.lineTo(LEFT_BOARD_END, TOTAL_HEIGHT);
    ctx.stroke();

    ctx.beginPath();
    ctx.moveTo(RIGHT_BOARD_START, TOTAL_HEIGHT);
    ctx.lineTo(RIGHT_BOARD_START + LEFT_BOARD_END, TOTAL_HEIGHT);
    ctx.stroke();
}

function drawCheckerboard() {
    // Left Board
    for (let row = 0; row < BOARD_SIZE; row++) {
        for (let col = 0; col < BOARD_SIZE; col++) {
            // Rows 8 and 9 are king protection zone — draw dimmed
            if (row >= 8) {
                ctx.fillStyle = (row + col) % 2 === 0 ? '#888' : '#666';
            } else {
                ctx.fillStyle = (row + col) % 2 === 0 ? '#EEE' : '#CCC';
            }
            ctx.fillRect(col * SQUARE_SIZE, row * SQUARE_SIZE, SQUARE_SIZE, SQUARE_SIZE);
        }
    }
    // Overlay dim for rows 8-9 on left board
    ctx.fillStyle = 'rgba(0,0,0,0.4)';
    ctx.fillRect(0, 8 * SQUARE_SIZE, LEFT_BOARD_END, 2 * SQUARE_SIZE);

    // Right Board
    for (let row = 0; row < BOARD_SIZE; row++) {
        for (let col = 0; col < BOARD_SIZE; col++) {
            if (row >= 8) {
                ctx.fillStyle = (row + col) % 2 === 0 ? '#888' : '#666';
            } else {
                ctx.fillStyle = (row + col) % 2 === 0 ? '#EEE' : '#CCC';
            }
            ctx.fillRect(RIGHT_BOARD_START + col * SQUARE_SIZE, row * SQUARE_SIZE, SQUARE_SIZE, SQUARE_SIZE);
        }
    }
    // Overlay dim for rows 8-9 on right board
    ctx.fillStyle = 'rgba(0,0,0,0.4)';
    ctx.fillRect(RIGHT_BOARD_START, 8 * SQUARE_SIZE, LEFT_BOARD_END, 2 * SQUARE_SIZE);
}

function drawWorkerArea() {
    ctx.strokeStyle = '#FFF';

    // Board boundaries
    ctx.beginPath();
    ctx.moveTo(LEFT_BOARD_END, 0);
    ctx.lineTo(LEFT_BOARD_END, TOTAL_HEIGHT);
    ctx.stroke();

    ctx.beginPath();
    ctx.moveTo(RIGHT_BOARD_START, 0);
    ctx.lineTo(RIGHT_BOARD_START, TOTAL_HEIGHT);
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
        const { shape, x, y, owner_id, is_enemy, is_king } = unit;

        if (is_king) {
            // Kings get a large distinct render
            const isMyKing = owner_id === myPlayerId;
            ctx.fillStyle = isMyKing ? '#FFD700' : '#8B0000';

            ctx.beginPath();
            ctx.arc(x, y, KING_RADIUS, 0, 2 * Math.PI);
            ctx.fill();

            // King crown outline
            ctx.strokeStyle = isMyKing ? '#FFF' : '#FF6666';
            ctx.lineWidth = 2;
            ctx.stroke();
            ctx.lineWidth = 1;

            // King label
            ctx.fillStyle = isMyKing ? '#000' : '#FFF';
            ctx.font = 'bold 10px Arial';
            ctx.textAlign = 'center';
            ctx.fillText('KING', x, y + 4);
            ctx.textAlign = 'left';
        } else {
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
        }
    });

    units.forEach(unit => {
        const { x, y, current_hp, max_hp, is_worker, is_king } = unit;
        if (!is_worker && current_hp !== undefined && max_hp !== undefined) {
            if (is_king) {
                // Wider HP bar for king
                const barWidth = KING_RADIUS * 2 + 10;
                const barHeight = 8;
                const barX = x - barWidth / 2;
                const barY = y - KING_RADIUS - 14;

                ctx.fillStyle = '#F00';
                ctx.fillRect(barX, barY, barWidth, barHeight);

                const healthPercent = Math.max(0, Math.min(1, current_hp / max_hp));
                ctx.fillStyle = '#0F0';
                ctx.fillRect(barX, barY, barWidth * healthPercent, barHeight);

                // HP text
                ctx.fillStyle = '#FFF';
                ctx.font = '9px Arial';
                ctx.textAlign = 'center';
                ctx.fillText(`${current_hp}/${max_hp}`, x, barY - 2);
                ctx.textAlign = 'left';
            } else {
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
        if (me) {
            goldDisplay.textContent = me.income > 0
                ? `${me.gold} (+${me.income}/round)`
                : me.gold.toString();
            mercPanel.updateGold(me.gold);
        }

        // Update king HP display
        const myKing = newState.units.find(u => u.is_king && u.owner_id === myPlayerId);
        if (myKing) {
            livesDisplay.textContent = `${myKing.current_hp}/${myKing.max_hp}`;
        } else {
            livesDisplay.textContent = '--';
        }

        // Update king upgrade panel
        if (me && myKing) {
            kingUpgradePanel.update(
                { currentTier: me.king_tier, currentHp: myKing.current_hp, maxHp: myKing.max_hp },
                me.gold,
                newState.phase
            );
        }

        applyPanelBoardSide();
    }
    gamePhase = newState.phase;
    gameTimer = Math.max(0, newState.phase_timer);

    gamePhaseEl.textContent = gamePhase;
    gameTimerEl.textContent = gameTimer.toFixed(1);

    panel.syncDynamicState(gameState, gamePhase);

    if (newState.phase === 'GameOver') {
        const isLoser = newState.winner_id !== null && newState.winner_id !== myPlayerId;
        const isDraw = newState.winner_id === null;

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
        selectedShape = 'Square';
    } else if (newState.phase === 'Victory') {
        gameResultTitle.textContent = 'Victory!';
        gameResultTitle.className = 'victory';
        gameResultSubtitle.textContent = 'All waves defeated!';
        gameOverOverlay.style.display = 'flex';
        mercPanel.hide();
        kingUpgradePanel.hide();
        selectedShape = 'Square';
    } else {
        gameOverOverlay.style.display = 'none';
    }
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

function drawMercenaryBuildings() {
    currentPlayers.forEach((_player, index) => {
        const buildingY = MERC_BUILDING_Y[index];
        const bx = MERC_BUILDING_X - MERC_BUILDING_HALF;
        const by = buildingY - MERC_BUILDING_HALF;
        const bSize = MERC_BUILDING_HALF * 2;

        ctx.fillStyle = '#8B0000';
        ctx.fillRect(bx, by, bSize, bSize);

        ctx.strokeStyle = '#FFD700';
        ctx.lineWidth = 2;
        ctx.strokeRect(bx, by, bSize, bSize);

        ctx.fillStyle = '#FFD700';
        ctx.font = 'bold 9px Arial';
        ctx.textAlign = 'center';
        ctx.fillText('⚔', MERC_BUILDING_X, buildingY - 2);
        ctx.font = '8px Arial';
        ctx.fillText('Merc', MERC_BUILDING_X, buildingY + 9);
        ctx.textAlign = 'left';
    });
}

function drawSpawningGrounds() {
    currentPlayers.forEach((player, index) => {
        if (!player.spawning_queue || player.spawning_queue.length === 0) return;
        const groundY = MERC_BUILDING_Y[index] + MERC_BUILDING_HALF + 14;
        const startX = MERC_BUILDING_X - (player.spawning_queue.length * 12) / 2;

        player.spawning_queue.forEach((shape, i) => {
            const iconX = startX + i * 14;
            const radius = shape === 'Circle' ? 8 : shape === 'Triangle' ? 6 : 4;
            ctx.fillStyle = '#FFA500';
            ctx.beginPath();
            ctx.arc(iconX, groundY, radius, 0, 2 * Math.PI);
            ctx.fill();
        });
    });
}

function render() {
    if (isInGame) {
        ctx.clearRect(0, 0, canvas.width, canvas.height);
        drawKingZone();
        drawCheckerboard();
        drawWorkerArea();
        drawUnits(gameState);
        drawMercenaryBuildings();
        drawSpawningGrounds();

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
        // Check if a Mercenary Building was clicked (gap area)
        if (clickX >= LEFT_BOARD_END && clickX < RIGHT_BOARD_START && myPlayerId !== null) {
            const myIndex = currentPlayers.findIndex(p => p.id === myPlayerId);
            if (myIndex !== -1) {
                const buildingY = MERC_BUILDING_Y[myIndex];
                const dx = clickX - MERC_BUILDING_X;
                const dy = clickY - buildingY;
                if (dx * dx + dy * dy <= MERC_BUILDING_HALF * MERC_BUILDING_HALF) {
                    mercPanel.toggle();
                    return;
                }
            }
        }
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
        // Rows 8 and 9 are king protection zone — no placement allowed
        if (row >= 8) return;
        const placeMessage = { action: 'place', payload: { shape: selectedShape, row, col } };
        if (socket && socket.readyState === WebSocket.OPEN) socket.send(JSON.stringify(placeMessage));
    }
});

function handleLeaveLobby() {
    isInGame = false;
    gameOverOverlay.style.display = 'none';
    socket?.send(JSON.stringify({ action: 'leaveLobby' }));
    panel.clearSelection();
    mercPanel.hide();
    kingUpgradePanel.hide();
    showLobbyView();
}

leaveLobbyButton.onclick = handleLeaveLobby;
overlayLeaveLobbyButton.onclick = handleLeaveLobby;

showAuthView();
