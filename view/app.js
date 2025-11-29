// Views
const authView = document.getElementById('auth-view');
const lobbySelectionView = document.getElementById('lobby-selection');
const gameView = document.getElementById('game-view');

// Auth elements
const registerForm = document.getElementById('register-form');
const loginForm = document.getElementById('login-form');
const authStatus = document.getElementById('auth-status');

// Lobby elements
const lobbyList = document.getElementById('lobby-list');

// Game elements
const canvas = document.getElementById('gameCanvas');
const ctx = canvas.getContext('2d');
const leaveLobbyButton = document.getElementById('leave-lobby');
const uiPanel = document.getElementById('ui-panel');
const towerTypeEl = document.getElementById('tower-type');
const sellButton = document.getElementById('sell-button');
const gamePhaseEl = document.getElementById('game-phase');
const gameTimerEl = document.getElementById('game-timer');

const BOARD_SIZE = 10;
const SQUARE_SIZE = canvas.width / BOARD_SIZE;
let selectedShape = 'Square';
let gameState = [];
let selectedTower = null;
let myPlayerId = null;
let socket = null;

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
    initGameView();
}

// --- AUTHENTICATION LOGIC ---
registerForm.addEventListener('submit', async (e) => {
    e.preventDefault();
    const username = document.getElementById('register-username').value;
    const password = document.getElementById('register-password').value;

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
    const username = document.getElementById('login-username').value;
    const password = document.getElementById('login-password').value;

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

    socket.onopen = function() {
        showLobbyView();
    };

    socket.onmessage = function(event) {
        const serverMsg = JSON.parse(event.data);
        switch (serverMsg.type) {
            case 'LobbyStatus':
                renderLobbies(serverMsg.data);
                break;
            case 'GameState':
                if (gameView.style.display === 'none') showGameView();
                updateGameState(serverMsg.data);
                break;
            case 'PlayerId':
                myPlayerId = serverMsg.data;
                break;
            case 'Error':
                M.toast({html: serverMsg.data});
                break;
        }
    };

    socket.onclose = function() {
        M.toast({html: 'Disconnected from server.'});
        localStorage.removeItem('jwt');
        showAuthView();
    };

    socket.onerror = function() {
        M.toast({html: 'WebSocket error.'});
        localStorage.removeItem('jwt');
        showAuthView();
    };
}


function renderLobbies(lobbies) {
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
        if (!button.hasAttribute('disabled')) {
            button.addEventListener('click', (e) => {
                e.preventDefault();
                const lobbyId = parseInt(e.target.getAttribute('data-lobby-id'));
                socket.send(JSON.stringify({ action: 'joinLobby', payload: lobbyId }));
            });
        }
    });
}

function drawCheckerboard() {
    for (let row = 0; row < BOARD_SIZE; row++) {
        for (let col = 0; col < BOARD_SIZE; col++) {
            ctx.fillStyle = (row + col) % 2 === 0 ? '#EEE' : '#CCC';
            ctx.fillRect(col * SQUARE_SIZE, row * SQUARE_SIZE, SQUARE_SIZE, SQUARE_SIZE);
        }
    }
}

function drawUnits(units) {
    units.forEach(unit => {
        const { shape, x, y, owner_id, is_enemy } = unit;
        if (is_enemy) {
            ctx.fillStyle = '#2E8B57'; // SeaGreen for enemies
        } else {
            ctx.fillStyle = owner_id === myPlayerId ? '#88F' : '#F88'; // Blue for own, Red for other player
        }
        
        if (shape === 'Square') {
            ctx.fillRect(x - (SQUARE_SIZE / 2 - 10), y - (SQUARE_SIZE / 2 - 10), SQUARE_SIZE - 20, SQUARE_SIZE - 20);
        } else if (shape === 'Circle') {
            ctx.beginPath(); ctx.arc(x, y, SQUARE_SIZE / 2 - 10, 0, 2 * Math.PI); ctx.fill();
        } else if (shape === 'Triangle') {
            ctx.beginPath(); ctx.moveTo(x, y - (SQUARE_SIZE / 2 - 10)); ctx.lineTo(x - (SQUARE_SIZE / 2 - 10), y + (SQUARE_SIZE / 2 - 10)); ctx.lineTo(x + (SQUARE_SIZE / 2 - 10), y + (SQUARE_SIZE / 2 - 10)); ctx.closePath(); ctx.fill();
        }
    });
}

function updateGameState(newState) {
    gameState = newState.units;
    gamePhaseEl.textContent = newState.phase;
    gameTimerEl.textContent = newState.phase_timer.toFixed(1);
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    drawCheckerboard();
    drawUnits(gameState);
}

function showUiPanel(tower) {
    if (tower.is_enemy) return;
    selectedTower = tower;
    towerTypeEl.textContent = tower.shape;
    sellButton.disabled = tower.owner_id !== myPlayerId;
    uiPanel.style.display = 'block';
}

function hideUiPanel() {
    selectedTower = null;
    uiPanel.style.display = 'none';
}

function initGameView() {
    document.getElementById('selectSquare').onclick = () => { selectedShape = 'Square'; };
    document.getElementById('selectCircle').onclick = () => { selectedShape = 'Circle'; };
    document.getElementById('selectTriangle').onclick = () => { selectedShape = 'Triangle'; };
    document.getElementById('skip-to-combat').onclick = () => {
        socket.send(JSON.stringify({ action: 'skipToCombat' }));
    };

    canvas.addEventListener('click', function(event) {
        const rect = canvas.getBoundingClientRect();
        const clickX = event.clientX - rect.left;
        const clickY = event.clientY - rect.top;
        
        const towerSize = SQUARE_SIZE - 20;
        const clickedTower = gameState.find(s => {
            return !s.is_enemy && clickX >= s.x - towerSize / 2 && clickX <= s.x + towerSize / 2 &&
                   clickY >= s.y - towerSize / 2 && clickY <= s.y + towerSize / 2;
        });

        if (clickedTower) {
            showUiPanel(clickedTower);
        } else {
            hideUiPanel();
            const row = Math.floor(clickY / SQUARE_SIZE);
            const col = Math.floor(clickX / SQUARE_SIZE);
            const placeMessage = { action: 'place', payload: { shape: selectedShape, row, col } };
            if (socket.readyState === WebSocket.OPEN) socket.send(JSON.stringify(placeMessage));
        }
    });

    sellButton.addEventListener('click', function() {
        if (selectedTower && selectedTower.owner_id === myPlayerId) {
            const row = Math.floor(selectedTower.y / SQUARE_SIZE);
            const col = Math.floor(selectedTower.x / SQUARE_SIZE);
            const sellMessage = { action: 'sell', payload: { row, col } };
            if (socket.readyState === WebSocket.OPEN) socket.send(JSON.stringify(sellMessage));
            hideUiPanel();
        }
    });
}

leaveLobbyButton.onclick = () => {
    socket.send(JSON.stringify({ action: 'leaveLobby' }));
    hideUiPanel();
    showLobbyView();
};

// --- INITIALIZATION ---
// On page load, show the authentication view.
showAuthView();