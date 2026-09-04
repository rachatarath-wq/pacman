// Gamepad mapping + test/remap menu for the desktop (Tauri) build.
//
// The backend sends a raw snapshot (see `GamepadState` in `src-tauri`): for a
// raw-HID pad (`kind === 1`) it carries the button bitmap, hat nibble and stick
// values and the *frontend* resolves them against the user's saved mapping. For
// GameController/gilrs pads (`kind === 2/3`) the backend already maps
// `dir/start/back`, so those fields are used as-is.

// ---- config --------------------------------------------------------------

export interface RawGamepad {
  dir: number | null;
  start: boolean;
  back: boolean;
  connected: boolean;
  kind: number; // 0 none, 1 raw-HID, 2 GC, 3 gilrs
  buttons: number; // raw bitmap, bit0 = Button 1
  hat: number; // raw hat nibble: 0=N, 2=E, 4=S, 6=W, 15=centered
  stick_x: number; // 0..255, center 127
  stick_y: number; // 0..255, center 127
}

// Button indices are 1-based (bit0 = Button 1). `null` = unbound.
export interface GamepadMap {
  up: number | null;
  down: number | null;
  left: number | null;
  right: number | null;
  start: number | null;
  back: number | null;
}

export type MapKey = keyof GamepadMap;

export const DEFAULT_MAP: GamepadMap = {
  up: null,
  down: null,
  left: null,
  right: null,
  start: 10, // B10 = Start
  back: 9, // B9 = Select/Back
};

// SHANWAN "PS3/PC Gamepad" button order (index 0 = Button 1).
export const BTN_NAMES: string[] = [
  'Triangle',
  'Circle',
  'Cross',
  'Square',
  'L1',
  'R1',
  'L2',
  'R2',
  'Select',
  'Start',
  'L3',
  'R3',
  'Home',
];

const STORE_KEY = 'pacman.gamepad.map';
const STICK_DEADZONE = 40; // raw 8-bit units, center 127

export function loadMap(): GamepadMap {
  try {
    const raw = localStorage.getItem(STORE_KEY);
    if (!raw) return { ...DEFAULT_MAP };
    const parsed = JSON.parse(raw) as Partial<GamepadMap>;
    return { ...DEFAULT_MAP, ...parsed };
  } catch {
    return { ...DEFAULT_MAP };
  }
}

export function saveMap(map: GamepadMap): void {
  try {
    localStorage.setItem(STORE_KEY, JSON.stringify(map));
  } catch {
    // storage unavailable (private mode) — mapping is session-only
  }
}

export function buttonLabel(i: number | null): string {
  if (i == null) return '—';
  const name = BTN_NAMES[i - 1];
  return name ? `B${i} · ${name}` : `B${i}`;
}

// ---- input resolution ----------------------------------------------------

function bit(raw: number, idx: number): boolean {
  return (raw & (1 << (idx - 1))) !== 0;
}

function hatDir(hat: number): number | null {
  switch (hat) {
    case 0:
      return 0; // up
    case 2:
      return 3; // right
    case 4:
      return 2; // down
    case 6:
      return 1; // left
    default:
      return null;
  }
}

function stickDir(x: number, y: number): number | null {
  const dx = x - 127;
  const dy = 127 - y; // up = low byte
  if (Math.abs(dx) < STICK_DEADZONE && Math.abs(dy) < STICK_DEADZONE) return null;
  if (Math.abs(dx) > Math.abs(dy)) return dx > 0 ? 3 : 1;
  return dy > 0 ? 0 : 2;
}

// Resolve one snapshot into { dir, start, back }. For raw-HID pads the user
// mapping is authoritative; otherwise the backend's already-mapped fields pass
// through unchanged.
export function resolveInput(raw: RawGamepad, map: GamepadMap): { dir: number | null; start: boolean; back: boolean } {
  if (raw.kind !== 1) {
    return { dir: raw.dir, start: raw.start, back: raw.back };
  }

  let dir: number | null = null;
  if (map.up != null && bit(raw.buttons, map.up)) dir = 0;
  else if (map.down != null && bit(raw.buttons, map.down)) dir = 2;
  else if (map.left != null && bit(raw.buttons, map.left)) dir = 1;
  else if (map.right != null && bit(raw.buttons, map.right)) dir = 3;
  if (dir == null) dir = hatDir(raw.hat);
  if (dir == null) dir = stickDir(raw.stick_x, raw.stick_y);

  const start = map.start != null && bit(raw.buttons, map.start);
  const back = map.back != null && bit(raw.buttons, map.back);
  return { dir, start, back };
}

// ---- test / remap menu ---------------------------------------------------

const ACTION_LABELS: Record<MapKey, string> = {
  up: 'Up',
  down: 'Down',
  left: 'Left',
  right: 'Right',
  start: 'Start',
  back: 'Select / Back',
};

const HAT_GLYPH: Record<number, string> = {
  0: '↑',
  1: '↗',
  2: '→',
  3: '↘',
  4: '↓',
  5: '↙',
  6: '←',
  7: '↖',
  15: '·',
};

export class GamepadMenu {
  private map: GamepadMap;
  private raw: RawGamepad = {
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
  private prevButtons = 0;
  private capturing: MapKey | null = null;

  private root: HTMLElement;
  private testBody: HTMLElement;
  private remapBody: HTMLElement;
  private connEl!: HTMLElement;
  private hatEl!: HTMLElement;
  private stickDot!: HTMLElement;
  private btnCells: HTMLElement[] = [];
  private actionRows: Partial<Record<MapKey, { row: HTMLElement; binding: HTMLElement }>> = {};

  constructor() {
    this.map = loadMap();
    this.root = document.getElementById('gp-menu')!;
    this.testBody = document.getElementById('gp-test')!;
    this.remapBody = document.getElementById('gp-remap')!;
    this.buildTest();
    this.buildRemap();

    document.getElementById('gp-close')!.addEventListener('click', () => this.close());
    document.querySelectorAll<HTMLElement>('[data-tab]').forEach((tab) => {
      tab.addEventListener('click', () => this.showTab(tab.dataset.tab!));
    });
    window.addEventListener('keydown', (e) => {
      if (!this.root.hidden && e.code === 'Escape') this.close();
    });
  }

  isOpen(): boolean {
    return !this.root.hidden;
  }

  open(): void {
    this.root.hidden = false;
    this.showTab('test');
  }

  close(): void {
    this.root.hidden = true;
    this.capturing = null;
    this.refreshCapturing();
  }

  setSnapshot(raw: RawGamepad): void {
    this.raw = raw;
  }

  // Call once per frame while the menu is open.
  update(): void {
    if (this.root.hidden) {
      // Still track the button edge so a held button isn't captured on open.
      this.prevButtons = this.raw.buttons;
      return;
    }
    this.refreshTest();
    this.runCapture();
    this.prevButtons = this.raw.buttons;
  }

  private showTab(name: string): void {
    const test = name === 'test';
    this.testBody.hidden = !test;
    this.remapBody.hidden = test;
    document.querySelectorAll<HTMLElement>('[data-tab]').forEach((tab) => {
      tab.classList.toggle('active', tab.dataset.tab === name);
    });
  }

  private buildTest(): void {
    const conn = document.createElement('div');
    conn.className = 'conn';
    this.testBody.appendChild(conn);
    this.connEl = conn;

    const hat = document.createElement('div');
    hat.className = 'hat';
    this.testBody.appendChild(hat);
    this.hatEl = hat;

    const stick = document.createElement('div');
    stick.className = 'stick-box';
    const dot = document.createElement('div');
    dot.className = 'stick-dot';
    stick.appendChild(dot);
    this.testBody.appendChild(stick);
    this.stickDot = dot;

    const grid = document.createElement('div');
    grid.className = 'btn-grid';
    for (let i = 1; i <= 13; i++) {
      const cell = document.createElement('span');
      cell.className = 'btn-cell';
      const name = BTN_NAMES[i - 1];
      cell.textContent = `B${i}`;
      cell.title = name;
      grid.appendChild(cell);
      this.btnCells.push(cell);
    }
    this.testBody.appendChild(grid);
  }

  private refreshTest(): void {
    const r = this.raw;
    const kindName = ['none', 'raw-HID', 'GameController', 'gilrs'][r.kind] ?? '?';
    this.connEl.textContent = r.connected
      ? `connected · ${kindName}`
      : 'not connected';
    this.connEl.classList.toggle('on', r.connected);

    this.hatEl.textContent = `hat ${HAT_GLYPH[r.hat] ?? '?'} (${r.hat})`;

    const x = (r.stick_x / 255) * 100;
    const y = (r.stick_y / 255) * 100;
    this.stickDot.style.left = `${x}%`;
    this.stickDot.style.top = `${y}%`;

    for (let i = 0; i < 13; i++) {
      const on = bit(r.buttons, i + 1);
      this.btnCells[i].classList.toggle('on', on);
    }
  }

  private buildRemap(): void {
    const note = document.createElement('p');
    note.className = 'remap-note';
    note.textContent =
      'Remapping needs the raw-HID pad (SHANWAN in PS3/PC mode). Bind the four directions to buttons to override the D-pad/stick.';
    this.remapBody.appendChild(note);

    const keys: MapKey[] = ['up', 'down', 'left', 'right', 'start', 'back'];
    for (const key of keys) {
      const row = document.createElement('div');
      row.className = 'action-row';

      const name = document.createElement('span');
      name.className = 'action-name';
      name.textContent = ACTION_LABELS[key];

      const binding = document.createElement('span');
      binding.className = 'action-binding';
      binding.textContent = this.bindingText(key);

      const btn = document.createElement('button');
      btn.className = 'rebind';
      btn.textContent = 'Rebind';
      btn.addEventListener('click', () => this.toggleCapture(key));

      row.append(name, binding, btn);
      this.remapBody.appendChild(row);
      this.actionRows[key] = { row, binding };
    }
  }

  private bindingText(key: MapKey): string {
    const v = this.map[key];
    if (v == null) {
      return key === 'start' || key === 'back' ? 'unbound' : 'D-pad / stick';
    }
    return buttonLabel(v);
  }

  private toggleCapture(key: MapKey): void {
    if (this.capturing === key) {
      this.capturing = null;
    } else {
      this.capturing = key;
    }
    this.refreshCapturing();
  }

  private refreshCapturing(): void {
    for (const [key, r] of Object.entries(this.actionRows)) {
      const active = this.capturing === key;
      r.row!.classList.toggle('capturing', active);
      const btn = r.row!.querySelector('.rebind') as HTMLButtonElement;
      btn.textContent = active ? 'press a button…' : 'Rebind';
    }
  }

  private runCapture(): void {
    if (this.capturing == null) return;
    if (this.raw.kind !== 1) return; // nothing to read from a non-raw pad

    const rising = this.raw.buttons & ~this.prevButtons;
    if (rising === 0) return;
    // Lowest set bit -> button index (1-based).
    const idx = 1 + Math.round(Math.log2(rising & -rising));
    if (idx < 1 || idx > 13) return;

    this.map[this.capturing] = idx;
    saveMap(this.map);
    this.capturing = null;
    this.refreshCapturing();

    for (const [key, r] of Object.entries(this.actionRows)) {
      r.binding!.textContent = this.bindingText(key as MapKey);
    }
  }
}
