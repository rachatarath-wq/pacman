//! WebAssembly bindings — the public API consumed by the TypeScript frontend.

mod entities;
mod game;
mod maze;
mod types;

use game::Game;
use types::Direction;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct PacmanGame {
    inner: Game,
}

#[wasm_bindgen]
impl PacmanGame {
    #[wasm_bindgen(constructor)]
    pub fn new() -> PacmanGame {
        PacmanGame {
            inner: Game::new(),
        }
    }

    /// Static maze layout as a JSON array of 31 strings. Call once at startup.
    pub fn maze_json(&self) -> String {
        serde_json::to_string(self.inner.maze.rows()).expect("serialize maze")
    }

    pub fn width(&self) -> usize {
        self.inner.maze.width
    }

    pub fn height(&self) -> usize {
        self.inner.maze.height
    }

    /// Full dynamic state for one frame (score, entities, pellet grid, events).
    pub fn state_json(&self) -> String {
        self.inner.render_state_json()
    }

    /// Buffer a direction: 0 up, 1 left, 2 down, 3 right.
    pub fn set_dir(&mut self, dir: u8) {
        self.inner.set_dir(Direction::from_u8(dir));
    }

    /// Advance the simulation by `dt` seconds.
    pub fn update(&mut self, dt: f32) {
        self.inner.update(dt);
    }

    pub fn start(&mut self) {
        self.inner.start();
    }

    pub fn toggle_pause(&mut self) {
        self.inner.toggle_pause();
    }

    pub fn reset(&mut self) {
        self.inner.reset_full();
    }

    pub fn is_over(&self) -> bool {
        self.inner.state == game::GameState::GameOver
    }

    pub fn score(&self) -> u32 {
        self.inner.score
    }

    pub fn lives(&self) -> u8 {
        self.inner.lives
    }

    pub fn level(&self) -> u8 {
        self.inner.level
    }
}

impl Default for PacmanGame {
    fn default() -> Self {
        Self::new()
    }
}
