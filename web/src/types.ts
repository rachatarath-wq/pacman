// Shapes matching the JSON produced by the Rust `render_state_json` method.

export interface PacmanView {
  x: number;
  y: number;
  dir: number; // 0 up, 1 left, 2 down, 3 right
  mouth: number; // animation phase in radians
  dying: boolean;
  dying_progress: number; // 0..1 during the death animation
}

export interface GhostView {
  id: number; // 0 Blinky, 1 Pinky, 2 Inky, 3 Clyde
  x: number;
  y: number;
  dir: number;
  state: number; // 0 house, 1 leaving, 2 normal, 3 frightened, 4 eaten
  blink: boolean; // white-flash near the end of frightened mode
}

export interface Events {
  dot: boolean;
  pellet: boolean;
  ghost: boolean;
  death: boolean;
  level: boolean;
  fright: boolean;
}

export interface RenderState {
  score: number;
  lives: number;
  level: number;
  state: number; // 0 ready, 1 playing, 2 paused, 3 dying, 4 level complete, 5 game over
  fright_timer: number;
  frightened: boolean;
  dots_remaining: number;
  dots_total: number;
  pacman: PacmanView;
  ghosts: GhostView[];
  grid: string; // 868 chars: '0' none, '1' dot, '2' pellet
  events: Events;
  time: number;
}

export const State = {
  Ready: 0,
  Playing: 1,
  Paused: 2,
  Dying: 3,
  LevelComplete: 4,
  GameOver: 5,
} as const;

export const GhostState = {
  House: 0,
  Leaving: 1,
  Normal: 2,
  Frightened: 3,
  Eaten: 4,
} as const;

export const COLS = 28;
export const ROWS = 31;
