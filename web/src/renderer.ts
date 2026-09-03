// Canvas rendering: static walls (cached to an offscreen canvas), dots,
// pellets, Pac-Man, and the four ghosts.

import { COLS, ROWS, GhostState, type RenderState } from './types';

const COLORS = {
  wall: '#1b1bd8',
  wallGlow: '#4f4fff',
  dot: '#ffd9b0',
  pellet: '#ffb8ae',
  pacman: '#ffe000',
  ghost: ['#ff0000', '#ffb8de', '#00ffff', '#ffb852'], // Blinky, Pinky, Inky, Clyde
  frightBody: '#2b2bff',
  frightBlink: '#ffffff',
  frightFace: '#ffb8ae',
};

// dir (0 up, 1 left, 2 down, 3 right) -> canvas angle (y grows downward)
const DIR_ANGLE = [-Math.PI / 2, Math.PI, Math.PI / 2, 0];
const DIR_DELTA: [number, number][] = [
  [0, -1],
  [-1, 0],
  [0, 1],
  [1, 0],
];

function roundRect(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  w: number,
  h: number,
  r: number,
): void {
  const rr = Math.min(r, w / 2, h / 2);
  ctx.beginPath();
  ctx.moveTo(x + rr, y);
  ctx.arcTo(x + w, y, x + w, y + h, rr);
  ctx.arcTo(x + w, y + h, x, y + h, rr);
  ctx.arcTo(x, y + h, x, y, rr);
  ctx.arcTo(x, y, x + w, y, rr);
  ctx.closePath();
}

function ellipse(
  ctx: CanvasRenderingContext2D,
  cx: number,
  cy: number,
  rx: number,
  ry: number,
): void {
  ctx.beginPath();
  ctx.ellipse(cx, cy, rx, ry, 0, 0, Math.PI * 2);
}

export class Renderer {
  private wallCanvas: HTMLCanvasElement | null = null;
  private wallSize = 0;

  /** Rebuild the cached wall layer when the tile size changes. */
  buildWalls(mazeRows: string[], tile: number): void {
    if (this.wallCanvas && tile === this.wallSize) return;
    this.wallSize = tile;
    const c = document.createElement('canvas');
    c.width = COLS * tile;
    c.height = ROWS * tile;
    const ctx = c.getContext('2d')!;
    ctx.fillStyle = '#05050d';
    ctx.fillRect(0, 0, c.width, c.height);

    for (let r = 0; r < ROWS; r++) {
      const row = mazeRows[r] ?? '';
      for (let col = 0; col < COLS; col++) {
        const ch = row[col];
        if (ch !== '#' && ch !== '=') continue;
        const x = col * tile;
        const y = r * tile;
        ctx.fillStyle = COLORS.wall;
        ctx.fillRect(x, y, tile, tile);
        ctx.strokeStyle = COLORS.wallGlow;
        ctx.lineWidth = Math.max(1, tile * 0.1);
        roundRect(ctx, x + tile * 0.14, y + tile * 0.14, tile * 0.72, tile * 0.72, tile * 0.16);
        ctx.stroke();
      }
    }

    this.wallCanvas = c;
  }

  draw(
    ctx: CanvasRenderingContext2D,
    state: RenderState,
    tile: number,
    ox: number,
    oy: number,
    now: number,
  ): void {
    ctx.fillStyle = '#05050d';
    ctx.fillRect(0, 0, ctx.canvas.width, ctx.canvas.height);

    if (this.wallCanvas) {
      ctx.drawImage(this.wallCanvas, ox, oy);
    }

    this.drawDots(ctx, state, tile, ox, oy, now);
    this.drawGhosts(ctx, state, tile, ox, oy);

    const p = state.pacman;
    this.drawPacman(
      ctx,
      ox + p.x * tile,
      oy + p.y * tile,
      tile * 0.72,
      p.dir,
      p.mouth,
      p.dying_progress,
    );
  }

  private drawDots(
    ctx: CanvasRenderingContext2D,
    state: RenderState,
    tile: number,
    ox: number,
    oy: number,
    now: number,
  ): void {
    const grid = state.grid;
    const half = tile / 2;
    const pulse = 0.75 + 0.25 * Math.sin(now / 140);

    for (let i = 0; i < grid.length; i++) {
      const ch = grid[i];
      if (ch === '0') continue;
      const col = i % COLS;
      const row = (i / COLS) | 0;
      const px = ox + col * tile + half;
      const py = oy + row * tile + half;
      if (ch === '1') {
        ctx.fillStyle = COLORS.dot;
        ctx.beginPath();
        ctx.arc(px, py, tile * 0.1, 0, Math.PI * 2);
        ctx.fill();
      } else {
        ctx.fillStyle = COLORS.pellet;
        ctx.beginPath();
        ctx.arc(px, py, tile * 0.28 * pulse, 0, Math.PI * 2);
        ctx.fill();
      }
    }
  }

  private drawGhosts(
    ctx: CanvasRenderingContext2D,
    state: RenderState,
    tile: number,
    ox: number,
    oy: number,
  ): void {
    for (const g of state.ghosts) {
      this.drawGhost(ctx, ox + g.x * tile, oy + g.y * tile, tile * 0.68, g);
    }
  }

  private drawGhost(
    ctx: CanvasRenderingContext2D,
    cx: number,
    cy: number,
    r: number,
    g: RenderState['ghosts'][number],
  ): void {
    const eyesOnly = g.state === GhostState.Eaten;

    if (!eyesOnly) {
      let body = COLORS.ghost[g.id];
      if (g.state === GhostState.Frightened) {
        body = g.blink ? COLORS.frightBlink : COLORS.frightBody;
      }
      this.ghostBody(ctx, cx, cy, r, body);
    }

    if (g.state === GhostState.Frightened) {
      this.ghostFrightFace(ctx, cx, cy, r, g.blink);
    } else {
      this.ghostEyes(ctx, cx, cy, r, g.dir, eyesOnly);
    }
  }

  private ghostBody(ctx: CanvasRenderingContext2D, cx: number, cy: number, r: number, color: string): void {
    ctx.beginPath();
    ctx.moveTo(cx - r, cy + r);
    ctx.lineTo(cx - r, cy);
    ctx.arc(cx, cy, r, Math.PI, 0, false);
    ctx.lineTo(cx + r, cy + r);
    const teeth = 3;
    const tw = (2 * r) / teeth;
    for (let i = 0; i < teeth; i++) {
      const x0 = cx + r - i * tw;
      const x1 = x0 - tw;
      ctx.quadraticCurveTo(x0 - tw / 2, cy + r + r * 0.32, x1, cy + r);
    }
    ctx.closePath();
    ctx.fillStyle = color;
    ctx.fill();
  }

  private ghostEyes(
    ctx: CanvasRenderingContext2D,
    cx: number,
    cy: number,
    r: number,
    dir: number,
    eyesOnly: boolean,
  ): void {
    const eyeY = cy - r * 0.18;
    const rx = r * 0.26;
    const ry = r * 0.34;
    const off = r * 0.4;
    const [pdx, pdy] = DIR_DELTA[dir] ?? [0, 0];

    for (const s of [-1, 1]) {
      const ex = cx + s * off;
      ctx.fillStyle = '#fff';
      ellipse(ctx, ex, eyeY, rx, ry);
      ctx.fill();
      ctx.fillStyle = '#1b1bd8';
      ctx.beginPath();
      ctx.arc(ex + pdx * rx * 0.4, eyeY + pdy * ry * 0.4, rx * 0.42, 0, Math.PI * 2);
      ctx.fill();
    }
    if (!eyesOnly) {
      // small "shoulder" shading to suggest a brow
      ctx.strokeStyle = 'rgba(0,0,0,0.25)';
      ctx.lineWidth = Math.max(1, r * 0.06);
      for (const s of [-1, 1]) {
        ctx.beginPath();
        ctx.moveTo(cx + s * off - rx, eyeY - ry * 1.3);
        ctx.lineTo(cx + s * off + rx, eyeY - ry * 1.3);
        ctx.stroke();
      }
    }
  }

  private ghostFrightFace(
    ctx: CanvasRenderingContext2D,
    cx: number,
    cy: number,
    r: number,
    blink: boolean,
  ): void {
    const eyeY = cy - r * 0.2;
    const off = r * 0.4;
    const faceColor = blink ? COLORS.frightBody : COLORS.frightFace;
    ctx.fillStyle = faceColor;
    for (const s of [-1, 1]) {
      ctx.beginPath();
      ctx.arc(cx + s * off, eyeY, r * 0.13, 0, Math.PI * 2);
      ctx.fill();
    }
    // wavy mouth
    ctx.strokeStyle = faceColor;
    ctx.lineWidth = Math.max(1, r * 0.09);
    ctx.lineJoin = 'round';
    ctx.beginPath();
    const my = cy + r * 0.35;
    const mw = r * 0.45;
    ctx.moveTo(cx - mw, my);
    const segs = 4;
    for (let i = 1; i <= segs; i++) {
      const x = cx - mw + ((mw * 2) / segs) * i;
      const y = my + (i % 2 === 0 ? -r * 0.12 : r * 0.12);
      ctx.lineTo(x, y);
    }
    ctx.stroke();
  }

  private drawPacman(
    ctx: CanvasRenderingContext2D,
    cx: number,
    cy: number,
    r: number,
    dir: number,
    mouthPhase: number,
    dying: number,
  ): void {
    const dirAngle = DIR_ANGLE[dir] ?? 0;
    let radius = r;
    let mouth: number;

    if (dying > 0) {
      const p = Math.min(1, dying);
      mouth = Math.PI * (0.12 + 0.88 * Math.min(1, p * 1.6));
      radius = r * Math.max(0, 1 - Math.max(0, p - 0.62) / 0.38);
      if (radius <= 0.01) return;
    } else {
      mouth = 0.12 + 0.24 * (0.5 + 0.5 * Math.sin(mouthPhase));
    }

    ctx.fillStyle = COLORS.pacman;
    ctx.beginPath();
    ctx.moveTo(cx, cy);
    ctx.arc(cx, cy, radius, dirAngle + mouth, dirAngle - mouth);
    ctx.closePath();
    ctx.fill();
  }
}
