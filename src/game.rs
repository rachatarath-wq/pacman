//! Game orchestration: state machine, scoring, collision, level progression.

use crate::entities::{Ghost, GhostState, Pacman};
use crate::maze::Maze;
use crate::types::Direction;
use serde::Serialize;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum GameState {
    Ready = 0,
    Playing = 1,
    Paused = 2,
    Dying = 3,
    LevelComplete = 4,
    GameOver = 5,
}

pub const PACMAN_SPAWN: (f32, f32) = (13.5, 23.5);
const SCATTER_TIME: f32 = 7.0;
const CHASE_TIME: f32 = 20.0;
const DEATH_TIME: f32 = 1.6;
const LEVEL_COMPLETE_TIME: f32 = 2.0;

pub struct Game {
    pub maze: Maze,
    pub pacman: Pacman,
    pub ghosts: Vec<Ghost>,
    pub score: u32,
    pub lives: u8,
    pub level: u8,
    pub state: GameState,
    pub scatter: bool,
    pub mode_timer: f32,
    pub frightened: bool,
    pub fright_timer: f32,
    pub dots_remaining: u32,
    pub ghost_combo: u32,
    pub state_timer: f32,
    pub time: f32,
    // one-shot sound/effect events for the frontend, cleared each update
    pub event_dot: bool,
    pub event_pellet: bool,
    pub event_ghost_eaten: bool,
    pub event_death: bool,
    pub event_level: bool,
    pub event_fright: bool,
}

impl Game {
    pub fn new() -> Self {
        let maze = Maze::new();
        let pacman = Pacman::new(PACMAN_SPAWN);
        let mut g = Game {
            maze,
            pacman,
            ghosts: Self::make_ghosts(),
            score: 0,
            lives: 3,
            level: 1,
            state: GameState::Ready,
            scatter: true,
            mode_timer: SCATTER_TIME,
            frightened: false,
            fright_timer: 0.0,
            dots_remaining: 0,
            ghost_combo: 0,
            state_timer: 0.0,
            time: 0.0,
            event_dot: false,
            event_pellet: false,
            event_ghost_eaten: false,
            event_death: false,
            event_level: false,
            event_fright: false,
        };
        g.dots_remaining = g.maze.total_dots;
        g
    }

    fn make_ghosts() -> Vec<Ghost> {
        vec![
            // Blinky (red) — starts outside the house
            Ghost::new(0, (13.5, 13.5), (25.5, 1.5)),
            // Pinky (pink)
            Ghost::new(1, (13.5, 14.5), (1.5, 1.5)),
            // Inky (cyan)
            Ghost::new(2, (14.5, 13.5), (25.5, 28.5)),
            // Clyde (orange)
            Ghost::new(3, (14.5, 15.5), (1.5, 28.5)),
        ]
    }

    /// Full reset: score, lives, level, dots, and positions.
    pub fn reset_full(&mut self) {
        self.score = 0;
        self.lives = 3;
        self.level = 1;
        self.maze.reset_dots();
        self.dots_remaining = self.maze.total_dots;
        self.reset_positions();
        self.state = GameState::Ready;
    }

    /// Reset entity positions (after a death or on level start).
    fn reset_positions(&mut self) {
        self.pacman.reset();
        self.ghosts[0].place(13.5, 11.5, GhostState::Normal, 0.0); // Blinky
        self.ghosts[1].place(13.5, 14.5, GhostState::House, 1.0); // Pinky
        self.ghosts[2].place(14.5, 13.5, GhostState::House, 3.0); // Inky
        self.ghosts[3].place(14.5, 15.5, GhostState::House, 5.0); // Clyde
        for g in self.ghosts.iter_mut() {
            g.set_level(self.level);
        }
        self.scatter = true;
        self.mode_timer = SCATTER_TIME;
        self.frightened = false;
        self.fright_timer = 0.0;
        self.ghost_combo = 0;
    }

    pub fn start(&mut self) {
        if matches!(self.state, GameState::Ready | GameState::Paused) {
            self.state = GameState::Playing;
        }
    }

    pub fn toggle_pause(&mut self) {
        match self.state {
            GameState::Playing => self.state = GameState::Paused,
            GameState::Paused => self.state = GameState::Playing,
            _ => {}
        }
    }

    pub fn set_dir(&mut self, d: Direction) {
        self.pacman.set_dir(d);
    }

    pub fn update(&mut self, dt: f32) {
        let dt = dt.min(0.1);
        // Clear one-shot events; they are re-set during `step` as needed.
        self.event_dot = false;
        self.event_pellet = false;
        self.event_ghost_eaten = false;
        self.event_death = false;
        self.event_level = false;
        self.event_fright = false;

        match self.state {
            GameState::Playing => self.step(dt),
            GameState::Dying => {
                self.state_timer -= dt;
                if self.state_timer <= 0.0 {
                    if self.lives == 0 {
                        self.state = GameState::GameOver;
                    } else {
                        self.reset_positions();
                        self.state = GameState::Ready;
                    }
                }
            }
            GameState::LevelComplete => {
                self.state_timer -= dt;
                if self.state_timer <= 0.0 {
                    self.level += 1;
                    self.maze.reset_dots();
                    self.dots_remaining = self.maze.total_dots;
                    self.reset_positions();
                    self.state = GameState::Ready;
                }
            }
            _ => {}
        }
    }

    fn step(&mut self, dt: f32) {
        self.time += dt;

        // Alternate scatter / chase.
        self.mode_timer -= dt;
        if self.mode_timer <= 0.0 {
            self.scatter = !self.scatter;
            self.mode_timer = if self.scatter { SCATTER_TIME } else { CHASE_TIME };
            if !self.frightened {
                for g in self.ghosts.iter_mut() {
                    if g.state == GhostState::Normal {
                        g.reverse();
                    }
                }
            }
        }

        // Frightened countdown.
        if self.frightened {
            self.fright_timer -= dt;
            if self.fright_timer <= 0.0 {
                self.frightened = false;
                for g in self.ghosts.iter_mut() {
                    if g.state == GhostState::Frightened {
                        g.state = GhostState::Normal;
                    }
                }
            }
        }

        self.pacman.update(dt, &self.maze);
        self.eat_dots();

        let targets: Vec<(f32, f32)> = (0..self.ghosts.len())
            .map(|i| self.ghost_target(i))
            .collect();
        for (i, t) in targets.into_iter().enumerate() {
            self.ghosts[i].update(dt, &self.maze, t);
        }

        self.check_collisions();

        if self.dots_remaining == 0 {
            self.state = GameState::LevelComplete;
            self.state_timer = LEVEL_COMPLETE_TIME;
            self.event_level = true;
        }
    }

    fn eat_dots(&mut self) {
        let col = self.pacman.x.floor() as i32;
        let row = self.pacman.y.floor() as i32;
        match self.maze.eat_dot(col, row) {
            1 => {
                self.score += 10;
                self.dots_remaining -= 1;
                self.event_dot = true;
            }
            2 => {
                self.score += 50;
                self.dots_remaining -= 1;
                self.event_pellet = true;
                self.start_frightened();
            }
            _ => {}
        }
    }

    fn start_frightened(&mut self) {
        self.frightened = true;
        self.fright_timer = (7.0 - self.level as f32).max(2.0);
        self.ghost_combo = 0;
        self.event_fright = true;
        for g in self.ghosts.iter_mut() {
            if g.state == GhostState::Normal {
                g.state = GhostState::Frightened;
                g.reverse();
            }
        }
    }

    fn check_collisions(&mut self) {
        let px = self.pacman.x;
        let py = self.pacman.y;
        let mut death = false;
        let mut eaten: Vec<usize> = Vec::new();

        for (i, g) in self.ghosts.iter().enumerate() {
            let dx = px - g.x;
            let dy = py - g.y;
            if dx * dx + dy * dy < 0.36 {
                match g.state {
                    GhostState::Frightened => eaten.push(i),
                    GhostState::Normal | GhostState::Leaving => death = true,
                    _ => {}
                }
            }
        }

        if death {
            self.pacman_dies();
            return;
        }

        for i in eaten {
            self.score += 200u32 << self.ghost_combo.min(3);
            self.ghost_combo += 1;
            self.ghosts[i].state = GhostState::Eaten;
            self.event_ghost_eaten = true;
        }
    }

    fn pacman_dies(&mut self) {
        if self.lives > 0 {
            self.lives -= 1;
        }
        self.state = GameState::Dying;
        self.state_timer = DEATH_TIME;
        self.pacman.dying = true;
        self.frightened = false;
        self.event_death = true;
    }

    /// Current chase/scatter target for ghost `i`.
    fn ghost_target(&self, i: usize) -> (f32, f32) {
        let g = &self.ghosts[i];
        if self.scatter {
            return g.scatter;
        }
        let px = self.pacman.x;
        let py = self.pacman.y;
        match g.id {
            0 => (px, py), // Blinky: straight at Pac-Man
            1 => {
                // Pinky: 4 tiles ahead
                let (dx, dy) = self.pacman.dir.delta();
                (px + 4.0 * dx as f32, py + 4.0 * dy as f32)
            }
            2 => {
                // Inky: reflect Blinky across a point 2 tiles ahead
                let (dx, dy) = self.pacman.dir.delta();
                let ax = px + 2.0 * dx as f32;
                let ay = py + 2.0 * dy as f32;
                let b = &self.ghosts[0];
                (2.0 * ax - b.x, 2.0 * ay - b.y)
            }
            _ => {
                // Clyde: chase when far, scatter when close
                let dx = px - g.x;
                let dy = py - g.y;
                if dx * dx + dy * dy > 64.0 {
                    (px, py)
                } else {
                    g.scatter
                }
            }
        }
    }

    /// Serialize everything the frontend needs to draw one frame.
    pub fn render_state_json(&self) -> String {
        let grid: String = self
            .maze
            .dots
            .iter()
            .map(|&d| match d {
                1 => '1',
                2 => '2',
                _ => '0',
            })
            .collect();

        let blink = self.frightened && self.fright_timer < 2.0 && (self.time * 4.0) as i32 % 2 == 0;
        let dying_progress = if self.state == GameState::Dying {
            1.0 - self.state_timer / DEATH_TIME
        } else {
            0.0
        };

        let state = RenderState {
            score: self.score,
            lives: self.lives,
            level: self.level,
            state: self.state as u8,
            fright_timer: self.fright_timer,
            frightened: self.frightened,
            dots_remaining: self.dots_remaining,
            dots_total: self.maze.total_dots,
            pacman: PacmanView {
                x: self.pacman.x,
                y: self.pacman.y,
                dir: self.pacman.dir.as_u8(),
                mouth: self.pacman.mouth,
                dying: self.pacman.dying,
                dying_progress,
            },
            ghosts: self
                .ghosts
                .iter()
                .map(|g| GhostView {
                    id: g.id as u8,
                    x: g.x,
                    y: g.y,
                    dir: g.dir.as_u8(),
                    state: g.state as u8,
                    blink,
                })
                .collect(),
            grid,
            events: Events {
                dot: self.event_dot,
                pellet: self.event_pellet,
                ghost: self.event_ghost_eaten,
                death: self.event_death,
                level: self.event_level,
                fright: self.event_fright,
            },
            time: self.time,
        };

        serde_json::to_string(&state).expect("serialize render state")
    }
}

impl Default for Game {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Serialize)]
struct RenderState {
    score: u32,
    lives: u8,
    level: u8,
    state: u8,
    fright_timer: f32,
    frightened: bool,
    dots_remaining: u32,
    dots_total: u32,
    pacman: PacmanView,
    ghosts: Vec<GhostView>,
    grid: String,
    events: Events,
    time: f32,
}

#[derive(Serialize)]
struct PacmanView {
    x: f32,
    y: f32,
    dir: u8,
    mouth: f32,
    dying: bool,
    dying_progress: f32,
}

#[derive(Serialize)]
struct GhostView {
    id: u8,
    x: f32,
    y: f32,
    dir: u8,
    state: u8,
    blink: bool,
}

#[derive(Serialize)]
struct Events {
    dot: bool,
    pellet: bool,
    ghost: bool,
    death: bool,
    level: bool,
    fright: bool,
}
