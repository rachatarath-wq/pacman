// Frontend entry point: boots the WASM module, wires input + rendering + audio,
// and runs the requestAnimationFrame game loop.

import init, { PacmanGame } from '../pkg/pacman.js';
import { Renderer } from './renderer';
import { Audio } from './audio';
import { State, COLS, ROWS, type RenderState, type Events } from './types';
import { resolveInput, GamepadMenu, loadMap, type RawGamepad, type GamepadMap } from './gamepad';

function $(id: string): HTMLElement {
  return document.getElementById(id)!;
}

// Direction code -> (dx, dy) in tile space (0 up, 1 left, 2 down, 3 right).
const DIR_VEC: [number, number][] = [
  [0, -1],
  [-1, 0],
  [0, 1],
  [1, 0],
];

// The maze row whose side edges are open tunnels (matches Rust `TUNNEL_ROW`).
const TUNNEL_ROW = 14;
// Minimum stick deflection before a gamepad axis counts as input.
const GAMEPAD_DEADZONE = 0.35;

async function main(): Promise<void> {
  await init();
  const game = new PacmanGame();
  const mazeRows = JSON.parse(game.maze_json()) as string[];

  const canvas = $('game') as HTMLCanvasElement;
  const wrap = $('canvas-wrap');
  const overlayText = $('overlay-text');
  const scoreEl = $('score');
  const livesEl = $('lives');
  const levelEl = $('level');
  const btnStart = $('btn-start') as HTMLButtonElement;
  const btnReset = $('btn-reset') as HTMLButtonElement;
  const btnMute = $('btn-mute') as HTMLButtonElement;
  const btnGamepad = $('btn-gamepad') as HTMLButtonElement;

  const ctx = canvas.getContext('2d')!;
  const audio = new Audio();
  const renderer = new Renderer();

  let tile = 16;
  let ox = 0;
  let oy = 0;
  let lastState: number = State.Ready;

  // Steering state: last known Pac-Man position and an optional mouse/touch
  // target tile. Keyboard/gamepad input clears the target.
  let lastPacman = { x: 13.5, y: 23.5 };
  let mouseTarget: { x: number; y: number } | null = null;
  let pointerActive = false;

  function resize(): void {
    const dpr = window.devicePixelRatio || 1;
    const w = wrap.clientWidth;
    const h = wrap.clientHeight;
    canvas.width = Math.round(w * dpr);
    canvas.height = Math.round(h * dpr);
    canvas.style.width = w + 'px';
    canvas.style.height = h + 'px';
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

    const pad = 12;
    tile = Math.floor(Math.min((w - pad * 2) / COLS, (h - pad * 2) / ROWS));
    tile = Math.max(4, tile);
    ox = Math.floor((w - COLS * tile) / 2);
    oy = Math.floor((h - ROWS * tile) / 2);
    renderer.buildWalls(mazeRows, tile);
  }

  window.addEventListener('resize', resize);
  resize();

  // ---- helpers -----------------------------------------------------------

  function startGame(): void {
    audio.ensure();
    if (game.is_over()) game.reset();
    game.start();
  }

  // Start (from Ready/GameOver) or pause/resume (while playing).
  function pressStart(): void {
    audio.ensure();
    if (lastState === State.Playing) {
      game.toggle_pause();
    } else {
      startGame();
    }
  }

  // Is a tile passable for Pac-Man? Walls and the ghost-house door block;
  // out-of-bounds tiles are open only on the tunnel row (where the maze wraps).
  function canEnter(c: number, r: number, dir: number): boolean {
    const [dx, dy] = DIR_VEC[dir];
    const nc = c + dx;
    const nr = r + dy;
    if (nr < 0 || nr >= ROWS) return false;
    if (nc < 0 || nc >= COLS) return nr === TUNNEL_ROW;
    const ch = mazeRows[nr][nc];
    return ch !== '#' && ch !== '=';
  }

  // Pick the cardinal direction that moves Pac-Man toward the target tile,
  // preferring the dominant axis but falling back to the other axis when the
  // preferred one is blocked right now. Returns null when neither axis is open
  // (Pac-Man keeps its heading until a turn opens up).
  function steerToward(p: { x: number; y: number }, t: { x: number; y: number }): number | null {
    const pc = Math.floor(p.x);
    const pr = Math.floor(p.y);
    const dx = t.x - pc;
    const dy = t.y - pr;

    const h: number | null = dx > 0 ? 3 : dx < 0 ? 1 : null;
    const v: number | null = dy > 0 ? 2 : dy < 0 ? 0 : null;
    const primary = Math.abs(dx) >= Math.abs(dy) ? h : v;
    const secondary = Math.abs(dx) >= Math.abs(dy) ? v : h;

    if (primary !== null && canEnter(pc, pr, primary)) return primary;
    if (secondary !== null && canEnter(pc, pr, secondary)) return secondary;
    return null;
  }

  // ---- input: keyboard ---------------------------------------------------
  const DIR: Record<string, number> = {
    ArrowUp: 0,
    KeyW: 0,
    ArrowLeft: 1,
    KeyA: 1,
    ArrowDown: 2,
    KeyS: 2,
    ArrowRight: 3,
    KeyD: 3,
  };

  window.addEventListener('keydown', (e) => {
    audio.ensure();
    if (e.code in DIR) {
      e.preventDefault();
      mouseTarget = null;
      game.set_dir(DIR[e.code]);
    } else if (e.code === 'Space' || e.code === 'KeyP') {
      e.preventDefault();
      game.toggle_pause();
    } else if (e.code === 'Enter') {
      e.preventDefault();
      startGame();
    }
  });

  // ---- input: mouse / touch ---------------------------------------------
  function mazePoint(e: PointerEvent): { x: number; y: number } {
    const rect = canvas.getBoundingClientRect();
    const col = Math.floor((e.clientX - rect.left - ox) / tile);
    const row = Math.floor((e.clientY - rect.top - oy) / tile);
    return {
      x: Math.max(0, Math.min(COLS - 1, col)),
      y: Math.max(0, Math.min(ROWS - 1, row)),
    };
  }

  canvas.addEventListener('pointerdown', (e) => {
    audio.ensure();
    e.preventDefault();
    pointerActive = true;
    mouseTarget = mazePoint(e);
    canvas.setPointerCapture(e.pointerId);
  });
  canvas.addEventListener('pointermove', (e) => {
    if (pointerActive) mouseTarget = mazePoint(e);
  });
  const endPointer = (): void => {
    pointerActive = false;
  };
  canvas.addEventListener('pointerup', endPointer);
  canvas.addEventListener('pointercancel', endPointer);

  // ---- input: gamepad ----------------------------------------------------
  // Two sources: the Web Gamepad API (browser) and a native `gilrs` bridge
  // (Tauri desktop, whose webview exposes no Gamepad API). Both funnel into the
  // same snapshot shape below.
  const gpStatus = $('gp-status');
  const useNativeGamepad = '__TAURI_INTERNALS__' in window;
  let gpStartWasPressed = false;
  let gpBackWasPressed = false;

  // The resolved movement/action state fed to the game each frame.
  interface ResolvedInput {
    dir: number | null;
    start: boolean;
    back: boolean;
    connected: boolean;
  }

  // ---- Web Gamepad API (browser) ----------------------------------------
  let sawGamepad = false;
  window.addEventListener('gamepadconnected', (e) => {
    sawGamepad = true;
    gpStatus.style.display = '';
    console.info(
      '[gamepad] connected:',
      e.gamepad.id,
      `(buttons=${e.gamepad.buttons.length}, axes=${e.gamepad.axes.length})`,
    );
  });
  window.addEventListener('gamepaddisconnected', () => {
    sawGamepad = false;
    gpStatus.style.display = 'none';
    console.info('[gamepad] disconnected');
  });

  function readBrowserGamepad(): ResolvedInput {
    if (typeof navigator.getGamepads !== 'function') {
      return { dir: null, start: false, back: false, connected: false };
    }
    const pads = navigator.getGamepads();
    let dir: number | null = null;
    let start = false;
    let back = false;
    let connected = false;

    for (const pad of pads) {
      if (!pad) continue;
      connected = true;
      const b = pad.buttons;

      // d-pad (standard mapping: buttons 12=up, 13=down, 14=left, 15=right).
      let dpad: number | null = null;
      if (b[12]?.pressed) dpad = 0;
      else if (b[13]?.pressed) dpad = 2;
      else if (b[14]?.pressed) dpad = 1;
      else if (b[15]?.pressed) dpad = 3;

      if (dpad !== null) {
        dir = dpad;
      } else {
        // left analog stick (axes 0 = x, 1 = y) with a deadzone.
        const ax = pad.axes[0] ?? 0;
        const ay = pad.axes[1] ?? 0;
        if (Math.abs(ax) > GAMEPAD_DEADZONE || Math.abs(ay) > GAMEPAD_DEADZONE) {
          dir = Math.abs(ax) >= Math.abs(ay) ? (ax > 0 ? 3 : 1) : (ay > 0 ? 2 : 0);
        }
      }

      start = start || (b[9]?.pressed ?? false); // Start
      back = back || (b[8]?.pressed ?? false); // Select / Back
    }

    if (connected && !sawGamepad) {
      sawGamepad = true;
      gpStatus.style.display = '';
    }
    return { dir, start, back, connected };
  }

  // ---- native bridge (Tauri desktop) -------------------------------------
  // The backend pushes snapshots as `gamepad-state` events (low latency), so
  // the frontend just keeps the latest and resolves it against the user map.
  let nativeGamepad: RawGamepad = {
    dir: null,
    start: false,
    back: false,
    connected: false,
    kind: 0,
    buttons: 0,
    hat: 15,
    stick_x: 127,
    stick_y: 127,
  };
  const gamepadMap: GamepadMap = loadMap();

  async function runNativeGamepad(): Promise<void> {
    const { listen } = await import('@tauri-apps/api/event');
    await listen('gamepad-state', (e) => {
      nativeGamepad = e.payload as RawGamepad;
    });
  }
  if (useNativeGamepad) void runNativeGamepad();

  // Apply one snapshot: edge-trigger Start/Back, update the indicator, return
  // the movement direction.
  function consumeGamepad(input: ResolvedInput): number | null {
    if (input.start && !gpStartWasPressed) {
      gpStartWasPressed = true;
      pressStart();
    } else if (!input.start) {
      gpStartWasPressed = false;
    }

    if (input.back && !gpBackWasPressed) {
      gpBackWasPressed = true;
      audio.ensure();
      game.reset();
    } else if (!input.back) {
      gpBackWasPressed = false;
    }

    if (input.connected) gpStatus.style.display = '';
    return input.dir;
  }

  // ---- buttons -----------------------------------------------------------
  btnStart.addEventListener('click', pressStart);
  btnReset.addEventListener('click', () => {
    audio.ensure();
    game.reset();
  });
  btnMute.addEventListener('click', () => {
    const muted = audio.toggleMute();
    btnMute.textContent = muted ? 'Unmute' : 'Mute';
  });

  // Gamepad test/remap menu. The 🎮 button is only meaningful in the desktop
  // build (where the raw-HID pad is read natively), so hide it in the browser.
  const gamepadMenu = new GamepadMenu();
  btnGamepad.style.display = useNativeGamepad ? '' : 'none';
  btnGamepad.addEventListener('click', () => gamepadMenu.open());

  // ---- frame loop --------------------------------------------------------
  function handleEvents(ev: Events): void {
    if (ev.death) audio.death();
    else if (ev.ghost) audio.ghost();
    else if (ev.pellet) audio.pellet();
    else if (ev.level) audio.level();
    else if (ev.fright) audio.fright();
    else if (ev.dot) audio.dot();
  }

  function updateHud(state: RenderState): void {
    scoreEl.textContent = String(state.score).padStart(6, '0');
    livesEl.textContent = String(state.lives);
    levelEl.textContent = String(state.level);
  }

  function updateOverlay(state: number): void {
    lastState = state;
    let text = '';
    switch (state) {
      case State.Ready:
        text = 'READY!';
        break;
      case State.Paused:
        text = 'PAUSED';
        break;
      case State.LevelComplete:
        text = 'LEVEL COMPLETE!';
        break;
      case State.GameOver:
        text = 'GAME OVER';
        break;
      default:
        text = '';
    }
    overlayText.textContent = text;
    overlayText.style.display = text ? '' : 'none';
    btnStart.textContent = state === State.Playing ? 'Pause' : 'Start';
  }

  // Faint pulsing marker at the current mouse/touch steer target.
  function drawTarget(now: number): void {
    if (!mouseTarget) return;
    const cx = ox + (mouseTarget.x + 0.5) * tile;
    const cy = oy + (mouseTarget.y + 0.5) * tile;
    const r = tile * 0.3 * (0.85 + 0.15 * Math.sin(now / 120));
    ctx.strokeStyle = 'rgba(255,224,0,0.55)';
    ctx.lineWidth = Math.max(1, tile * 0.07);
    ctx.beginPath();
    ctx.arc(cx, cy, r, 0, Math.PI * 2);
    ctx.stroke();
  }

  let last = performance.now();
  function loop(now: number): void {
    const dt = Math.min(0.1, (now - last) / 1000);
    last = now;

    // Gamepad direction takes priority over the mouse steer target; otherwise
    // keep steering toward the last clicked/tapped tile until it is reached.
    // While the gamepad menu is open, input is suppressed so remapping doesn't
    // steer or reset the game.
    gamepadMenu.setSnapshot(nativeGamepad);
    gamepadMenu.update();
    const menuOpen = gamepadMenu.isOpen();

    if (!menuOpen) {
      const gpInput: ResolvedInput = useNativeGamepad
        ? { ...resolveInput(nativeGamepad, gamepadMap), connected: nativeGamepad.connected }
        : readBrowserGamepad();
      const gpDir = consumeGamepad(gpInput);
      if (gpDir !== null) {
        mouseTarget = null;
        game.set_dir(gpDir);
      } else if (mouseTarget) {
        const pc = Math.floor(lastPacman.x);
        const pr = Math.floor(lastPacman.y);
        if (pc === mouseTarget.x && pr === mouseTarget.y) {
          mouseTarget = null; // reached
        } else {
          const d = steerToward(lastPacman, mouseTarget);
          if (d !== null) game.set_dir(d);
        }
      }
    }

    game.update(dt);
    const state = JSON.parse(game.state_json()) as RenderState;
    lastPacman = { x: state.pacman.x, y: state.pacman.y };

    handleEvents(state.events);
    updateHud(state);
    updateOverlay(state.state);
    renderer.draw(ctx, state, tile, ox, oy, now);
    drawTarget(now);

    requestAnimationFrame(loop);
  }

  updateOverlay(State.Ready);
  requestAnimationFrame(loop);
}

void main();
