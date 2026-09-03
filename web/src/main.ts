// Frontend entry point: boots the WASM module, wires input + rendering + audio,
// and runs the requestAnimationFrame game loop.

import init, { PacmanGame } from '../pkg/pacman.js';
import { Renderer } from './renderer';
import { Audio } from './audio';
import { State, COLS, ROWS, type RenderState, type Events } from './types';

function $(id: string): HTMLElement {
  return document.getElementById(id)!;
}

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

  const ctx = canvas.getContext('2d')!;
  const audio = new Audio();
  const renderer = new Renderer();

  let tile = 16;
  let ox = 0;
  let oy = 0;
  let lastState: number = State.Ready;

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

  // ---- input -------------------------------------------------------------
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
      game.set_dir(DIR[e.code]);
    } else if (e.code === 'Space' || e.code === 'KeyP') {
      e.preventDefault();
      game.toggle_pause();
    } else if (e.code === 'Enter') {
      e.preventDefault();
      startGame();
    }
  });

  function startGame(): void {
    audio.ensure();
    if (game.is_over()) game.reset();
    game.start();
  }

  btnStart.addEventListener('click', () => {
    audio.ensure();
    if (lastState === State.Playing) {
      game.toggle_pause();
    } else {
      startGame();
    }
  });
  btnReset.addEventListener('click', () => {
    audio.ensure();
    game.reset();
  });
  btnMute.addEventListener('click', () => {
    const muted = audio.toggleMute();
    btnMute.textContent = muted ? 'Unmute' : 'Mute';
  });

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

  let last = performance.now();
  function loop(now: number): void {
    const dt = Math.min(0.1, (now - last) / 1000);
    last = now;

    game.update(dt);
    const state = JSON.parse(game.state_json()) as RenderState;

    handleEvents(state.events);
    updateHud(state);
    updateOverlay(state.state);
    renderer.draw(ctx, state, tile, ox, oy, now);

    requestAnimationFrame(loop);
  }

  updateOverlay(State.Ready);
  requestAnimationFrame(loop);
}

void main();
