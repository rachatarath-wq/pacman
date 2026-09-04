# Gamepad subsystem

How controller input flows through the app, and everything needed to modify it
without re-discovering the quirks from scratch.

## Input pipeline

```
pad ──► backend thread (8 ms poll) ──► shared snapshot ──► `gamepad-state` event ──► frontend ──► game.set_dir()
                                                           (pushed on change only)
```

- Backend: `src-tauri/src/gamepad.rs` (platform dispatch), `gamepad_hid.rs`
  (macOS raw-HID fallback). Spawned in `main.rs` `.setup()`, which
  `.manage()`s a `GamepadHub` and passes the `AppHandle` into `gamepad::init`.
- The backend pushes snapshots to the webview via `app.emit("gamepad-state", &state)`
  **only when the state changes**, at a ~120 Hz poll (8 ms). The frontend keeps the
  latest snapshot and reads it in the `requestAnimationFrame` loop.
- The `gamepad_state` Tauri command still exists for a one-shot snapshot (used to
  be the polling path; kept for convenience).

## Why raw HID for the SHANWAN clone

The user's pad is a **SHANWAN "PS3/PC Gamepad"** (an Xbox 360-style clone,
VID `2563` : PID `0575`). Apple's GameController.framework and gilrs both come up
empty on it:

- gilrs' IOKit backend enumerates elements with `IOHIDDeviceCopyMatchingElements`,
  which returns **zero** elements for pads handled by Apple's DriverKit
  game-controller driver — so it reports `raw_buttons=0 raw_axes=0` (connected but
  unreadable).
- GameController.framework likewise exposes no profile for it in this mode.

The pad's raw interrupt report is a standard 27-byte layout that `hidapi` can read
directly, so the macOS path runs **both**: GameController.framework for official
pads, and a raw-HID reader as a fallback. The raw snapshot is merged into the same
`GamepadState`.

> The pad has a MODE button. In "Xbox 360" mode macOS hides its raw HID behind
> DriverKit; in "PS3/PC Gamepad" mode (VID 2563:PID 0575) the raw report is visible.
> If the raw reader stops working, the pad may have switched modes.

## Report layout (raw HID, 27 bytes)

```
bytes 0–1 : 13 buttons + 3 pad bits   (bit0 = Button 1 … bit12 = Button 13)
byte 2    : hat switch, LOW nibble    (0=N, 1=NE, 2=E, 3=SE, 4=S, 5=SW, 6=W, 7=NW, 15=centered)
byte 3    : left stick X              (8-bit, center 0x7F)
byte 4    : left stick Y              (8-bit, center 0x7F; LOW = up)
byte 5/6  : Z/Rz (right stick)        (unused)
bytes 7+  : vendor-specific           (unused; they change every poll)
```

### Confirmed button mapping (bit index → button)

| Bit | Button | Label | Bit | Button | Label |
|----:|:------:|:------|----:|:------:|:------|
| 0 | 1 | Triangle | 7 | 8 | R2 |
| 1 | 2 | Circle | 8 | 9 | **Select/Back** |
| 2 | 3 | Cross | 9 | 10 | **Start** |
| 3 | 4 | Square | 10 | 11 | L3 |
| 4 | 5 | L1 | 11 | 12 | R3 |
| 5 | 6 | R1 | 12 | 13 | Home |
| 6 | 7 | L2 | — | — | — |

Backend constants: `BTN_START = 1 << 9` (Button 10), `BTN_BACK = 1 << 8` (Button 9)
in `gamepad_hid.rs`.

### Direction codes (shared everywhere)

`0 = up, 1 = left, 2 = down, 3 = right` — same in the WASM core, `gamepad.rs`,
`gamepad_hid.rs`, and `web/src/gamepad.ts`.

## `GamepadState` shape (over the wire)

```rust
pub struct GamepadState {
    pub dir: Option<u8>,   // mapped dir (set by GC/gilrs; raw-HID also fills via hat/stick)
    pub start: bool,       // mapped
    pub back: bool,        // mapped
    pub connected: bool,
    pub kind: u8,          // 0 none, 1 raw-HID, 2 GameController, 3 gilrs
    pub buttons: u16,      // raw bitmap (raw-HID only)
    pub hat: u8,           // raw hat nibble 0..15 (raw-HID only)
    pub stick_x: i32,      // 0..255, center 127 (raw-HID only)
    pub stick_y: i32,      // 0..255, center 127 (raw-HID only)
}
```

`kind` tells the frontend how to resolve input:

- `1` (raw-HID): the frontend resolves `dir/start/back` from `buttons/hat/stick_*`
  against the user's remap config (see below).
- `2` / `3` (GC / gilrs): standard layouts — the backend's `dir/start/back` are
  used as-is, raw fields stay neutral.

## Remap config (frontend)

Stored in `localStorage["pacman.gamepad.map"]`, schema in `web/src/gamepad.ts`:

```ts
interface GamepadMap {
  up: number | null;    // button index 1..13, null = use D-pad/stick
  down: number | null;
  left: number | null;
  right: number | null;
  start: number | null;
  back: number | null;
}
// DEFAULT_MAP = { up:null, down:null, left:null, right:null, start:10, back:9 }
```

Resolution order for `dir` on a raw-HID pad:

1. bound directional button (first match, order up/down/left/right),
2. hat switch (`0=up, 2=right, 4=down, 6=left`),
3. analog stick (deadzone 40 raw units; x `0..255` = left→right, y `0..255` = up→down).

Remapping only applies to raw-HID pads. The 🎮 menu's Remap tab explains this.

## The latency fix (why events, not polling)

Earlier the frontend polled `invoke('gamepad_state')` every 16 ms, on top of the
backend's 16 ms poll — so input took `pad → 16 ms → backend → 16 ms → invoke → rAF`
(2–3 frames), which the user felt as imprecise steering. The fix:

- backend poll 16 ms → **8 ms**, and
- backend **pushes** `gamepad-state` events on change; the frontend no longer polls.

Input now reaches the frame loop within ~one frame, matching keyboard (whose
`keydown` handler calls `game.set_dir` immediately).

## Testing tools

- `tools/gamepad-detect/` — a standalone Rust utility (`cargo run --release`) that
  lists HID devices, opens the first gamepad-like one, and live-decodes button/hat/
  stick reports. Dedupes on input bytes 0–6 (the vendor tail changes every poll).
  Use it to sniff an unknown pad's mapping before editing `BTN_NAMES` /
  `BTN_START` / `BTN_BACK`.
- In-app **🎮 menu** (desktop build only) — a live Test tab (button bits, hat
  arrow, stick crosshair) and a Remap tab (click an action, press a button to
  rebind). Opens from the 🎮 button in the on-screen controls.
