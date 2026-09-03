# Pac-Man — Rust (WebAssembly) + TypeScript

A complete, playable Pac-Man clone. The entire game simulation — movement,
collision detection, ghost AI, scoring, frightened mode, lives, and level
progression — runs in **Rust compiled to WebAssembly**. A **TypeScript + Vite**
frontend renders the game to a single HTML5 canvas and plays synthesized sound
effects through the Web Audio API.

## Project structure

```
pacman/
├── Cargo.toml              # Rust crate manifest
├── src/                    # Rust game core
│   ├── lib.rs              # wasm-bindgen exports (the JS-facing API)
│   ├── maze.rs             # fixed 28×31 maze, dots, ghost-house door
│   ├── types.rs            # Direction
│   ├── entities.rs         # Pac-Man + ghost movement & AI
│   └── game.rs             # state machine, scoring, collision, levels
├── web/                    # TypeScript / Vite frontend
│   ├── index.html          # page, HUD, buttons, overlay
│   ├── package.json
│   ├── tsconfig.json
│   ├── vite.config.ts
│   ├── pkg/                # (generated) wasm-pack output — gitignored
│   └── src/
│       ├── main.ts         # boot WASM, input, game loop
│       ├── renderer.ts     # canvas drawing
│       ├── audio.ts        # Web Audio sound effects
│       └── types.ts        # state shapes shared with Rust
├── build.sh                # one-command build & run
└── .gitignore
```

## Prerequisites

| Tool          | Version  | Install                                     |
| ------------- | -------- | ------------------------------------------- |
| Rust          | ≥ 1.70   | https://rustup.rs                           |
| wasm32 target | —        | `rustup target add wasm32-unknown-unknown`  |
| wasm-pack     | latest   | `cargo install wasm-pack`                   |
| Node.js + npm | ≥ 18     | https://nodejs.org                          |

## Build & run

**One command** (builds the WASM core, installs deps, starts the dev server):

```bash
./build.sh
```

Or step by step:

```bash
# 1. Build the Rust crate to WebAssembly (outputs web/pkg/)
wasm-pack build --target web --out-dir web/pkg --out-name pacman --release

# 2. Install frontend dependencies
cd web
npm install

# 3. Start the Vite dev server (opens http://localhost:5173)
npm run dev
```

For a production build: `cd web && npm run build` (outputs `web/dist/`).

> **Why `--target web`?** It emits an ES module that loads the `.wasm` file via
> `import.meta.url`, which Vite serves and bundles natively — no extra plugins.

## How to play

- **Move** — arrow keys or `W`/`A`/`S`/`D`
- **Pause / resume** — `Space` or `P` (or the Start/Pause button)
- **Start** — `Enter` (or the Start button)
- **Reset** — the Reset button
- **Mute** — the Mute button

Eat every dot to clear the level. Power pellets (the larger, blinking dots) turn
the ghosts blue for a few seconds — run into them for 200/400/800/1600 points.
Touching a non-frightened ghost costs a life. You start with three.

## How it works

**Rust side** exposes a single `PacmanGame` class via `wasm-bindgen`:

| Method            | Purpose                                              |
| ----------------- | ---------------------------------------------------- |
| `maze_json()`     | static maze layout (called once)                     |
| `state_json()`    | full per-frame state for rendering                   |
| `set_dir(dir)`    | buffer the player's direction (0–3)                  |
| `update(dt)`      | advance the simulation by `dt` seconds               |
| `start`/`toggle_pause`/`reset` | control the state machine             |

The maze is the classic 28×31 layout (270 dots). Ghost AI uses the familiar
greedy "move toward target" rule with a no-reverse constraint, per-ghost chase
targets (Blinky chases directly, Pinky aims ahead, Inky flanks, Clyde switches
on distance), and alternating scatter/chase phases. Frightened mode picks random
directions; eaten ghosts become "eyes" and path back through the ghost-house
door to respawn.

**TypeScript side** runs a `requestAnimationFrame` loop: each frame it calls
`update(dt)`, reads `state_json()`, draws the cached wall layer plus dots,
Pac-Man and ghosts, and triggers one-shot sound events.

## Desktop app (Tauri)

The same game is also packaged as a native desktop app with
[Tauri](https://tauri.app) (Rust + the system webview). The app lives in
`src-tauri/` and reuses the existing `web/` frontend — `tauri.conf.json` points
`frontendDist` at `web/dist`, so the desktop window renders exactly what the
browser serves.

Requires the [Tauri prerequisites](https://tauri.app/start/prerequisites/) for
your OS, plus the CLI (`npm i -g @tauri-apps/cli`). Build from the repo root:

```bash
tauri build
```

This builds the WASM core + frontend first, then compiles and bundles the app.
Installers land in `src-tauri/target/release/bundle/` (`.dmg` on macOS, `.exe`
(NSIS) on Windows, `.deb` on Linux).

## Releasing

- The web build deploys to GitHub Pages automatically on every push to `main`.
- Desktop binaries are built in CI and attached to a GitHub Release whenever a
  `v*` tag is pushed (see `.github/workflows/release.yml`). Cut a release by
  tagging a commit:

```bash
git tag v1.1.0 && git push origin v1.1.0
```
