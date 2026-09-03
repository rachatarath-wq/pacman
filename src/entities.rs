//! Pac-Man and ghost entities: movement, collision, and ghost AI.

use crate::maze::Maze;
use crate::types::Direction;
use rand::random;

/// The row at which the tunnel wraps left/right across the screen edge.
pub const TUNNEL_ROW: i32 = 14;

pub const PACMAN_SPEED: f32 = 8.0;
pub const GHOST_SPEED_BASE: f32 = 7.3;
pub const FRIGHT_SPEED: f32 = 5.0;
pub const EATEN_SPEED: f32 = 14.0;

/// Is a tile blocked for a moving body? Out-of-bounds is treated as open only
/// on the tunnel row (so entities can exit and wrap).
#[inline]
pub fn is_blocked(maze: &Maze, ghost: bool, c: i32, r: i32) -> bool {
    if !maze.in_bounds(c, r) {
        return r != TUNNEL_ROW;
    }
    if ghost {
        maze.blocked_ghost(c, r)
    } else {
        maze.blocked_pacman(c, r)
    }
}

/// Snap a position to the nearest tile center (a lane intersection).
#[inline]
pub fn snap_to_center(x: &mut f32, y: &mut f32) {
    *x = (*x - 0.5).round() + 0.5;
    *y = (*y - 0.5).round() + 0.5;
}

/// Is the body within `tol` of a tile center along its travel axis?
#[inline]
pub fn near_center_along(x: f32, y: f32, dir: Direction, tol: f32) -> bool {
    match dir {
        Direction::Left | Direction::Right => {
            let c = x - 0.5;
            (c - c.round()).abs() <= tol
        }
        Direction::Up | Direction::Down => {
            let c = y - 0.5;
            (c - c.round()).abs() <= tol
        }
        Direction::None => true,
    }
}

/// Advance `(x, y)` by `step` tiles in `dir`, clamping against walls and
/// keeping the body aligned to the grid lane. Handles tunnel wrapping.
pub fn move_forward(
    x: &mut f32,
    y: &mut f32,
    dir: Direction,
    step: f32,
    maze: &Maze,
    ghost: bool,
) {
    if dir == Direction::None {
        return;
    }
    let (dx, dy) = dir.delta();
    let col = x.floor() as i32;
    let row = y.floor() as i32;

    if dx != 0 {
        let ahead = col + dx;
        if is_blocked(maze, ghost, ahead, row) {
            let center = col as f32 + 0.5;
            if dx > 0 {
                *x = (*x + step).min(center);
            } else {
                *x = (*x - step).max(center);
            }
        } else {
            *x += step * dx as f32;
        }
        *y = row as f32 + 0.5;
        // tunnel wrap
        if *x < 0.0 {
            *x += maze.width as f32;
        } else if *x >= maze.width as f32 {
            *x -= maze.width as f32;
        }
    } else if dy != 0 {
        let ahead = row + dy;
        if is_blocked(maze, ghost, col, ahead) {
            let center = row as f32 + 0.5;
            if dy > 0 {
                *y = (*y + step).min(center);
            } else {
                *y = (*y - step).max(center);
            }
        } else {
            *y += step * dy as f32;
        }
        *x = col as f32 + 0.5;
    }
}

// ---------------------------------------------------------------------------
// Pac-Man
// ---------------------------------------------------------------------------

pub struct Pacman {
    pub x: f32,
    pub y: f32,
    pub dir: Direction,
    pub next_dir: Option<Direction>,
    pub speed: f32,
    /// Mouth animation phase (radians); frontend turns it into openness.
    pub mouth: f32,
    pub dying: bool,
    pub spawn: (f32, f32),
}

impl Pacman {
    pub fn new(spawn: (f32, f32)) -> Self {
        Pacman {
            x: spawn.0,
            y: spawn.1,
            dir: Direction::Left,
            next_dir: None,
            speed: PACMAN_SPEED,
            mouth: 0.0,
            dying: false,
            spawn,
        }
    }

    pub fn reset(&mut self) {
        self.x = self.spawn.0;
        self.y = self.spawn.1;
        self.dir = Direction::Left;
        self.next_dir = None;
        self.mouth = 0.0;
        self.dying = false;
    }

    /// Buffer the player's desired direction. Applied at the next tile center,
    /// or immediately if it is a reversal.
    pub fn set_dir(&mut self, d: Direction) {
        self.next_dir = Some(d);
    }

    pub fn update(&mut self, dt: f32, maze: &Maze) {
        self.mouth = (self.mouth + dt * 14.0) % (std::f32::consts::TAU);

        if let Some(nd) = self.next_dir {
            if nd == Direction::None {
                self.next_dir = None;
            } else if nd == self.dir.opposite() {
                // Always allow reversing.
                self.dir = nd;
                snap_to_center(&mut self.x, &mut self.y);
                self.next_dir = None;
            } else if nd != self.dir {
                // Perpendicular turns only at a tile center.
                if near_center_along(self.x, self.y, self.dir, 0.07) {
                    let (dx, dy) = nd.delta();
                    let col = self.x.floor() as i32;
                    let row = self.y.floor() as i32;
                    if !is_blocked(maze, false, col + dx, row + dy) {
                        self.dir = nd;
                        snap_to_center(&mut self.x, &mut self.y);
                        self.next_dir = None;
                    }
                }
            } else {
                self.next_dir = None;
            }
        }

        let step = self.speed * dt;
        move_forward(&mut self.x, &mut self.y, self.dir, step, maze, false);
    }
}

// ---------------------------------------------------------------------------
// Ghosts
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum GhostState {
    House = 0,
    Leaving = 1,
    Normal = 2,
    Frightened = 3,
    Eaten = 4,
}

pub struct Ghost {
    pub id: usize,
    pub x: f32,
    pub y: f32,
    pub dir: Direction,
    pub state: GhostState,
    /// Respawn position inside the ghost house.
    pub home: (f32, f32),
    /// Scatter-mode target corner.
    pub scatter: (f32, f32),
    /// Seconds until this ghost is released from the house.
    pub release: f32,
    pub speed: f32,
    /// Tile the ghost last chose a direction for.
    pub cur_tile: (i32, i32),
    /// Bob animation phase while in the house.
    pub bob: f32,
}

impl Ghost {
    pub fn new(id: usize, home: (f32, f32), scatter: (f32, f32)) -> Self {
        Ghost {
            id,
            x: home.0,
            y: home.1,
            dir: Direction::Up,
            state: GhostState::House,
            home,
            scatter,
            release: 0.0,
            speed: GHOST_SPEED_BASE,
            cur_tile: (home.0.floor() as i32, home.1.floor() as i32),
            bob: id as f32,
        }
    }

    pub fn place(&mut self, x: f32, y: f32, state: GhostState, release: f32) {
        self.x = x;
        self.y = y;
        self.state = state;
        self.release = release;
        self.dir = if state == GhostState::House {
            Direction::Up
        } else {
            Direction::Left
        };
        self.cur_tile = (x.floor() as i32, y.floor() as i32);
        self.bob = self.id as f32;
    }

    pub fn set_level(&mut self, level: u8) {
        self.speed = (GHOST_SPEED_BASE + (level as f32 - 1.0) * 0.25).min(GHOST_SPEED_BASE + 1.0);
    }

    pub fn reverse(&mut self) {
        self.dir = self.dir.opposite();
    }

    pub fn update(&mut self, dt: f32, maze: &Maze, target: (f32, f32)) {
        match self.state {
            GhostState::House => {
                self.update_house(dt);
                return;
            }
            GhostState::Leaving => {
                self.update_leaving(dt, maze);
                return;
            }
            _ => {}
        }

        let speed = match self.state {
            GhostState::Frightened => FRIGHT_SPEED,
            GhostState::Eaten => EATEN_SPEED,
            _ => self.speed,
        };

        let col = self.x.floor() as i32;
        let row = self.y.floor() as i32;
        let (dx, dy) = self.dir.delta();
        let blocked = is_blocked(maze, true, col + dx, row + dy);

        // Decide a direction when entering a new tile, or when stuck (a dead
        // end forces a reversal).
        if (col, row) != self.cur_tile || blocked {
            self.cur_tile = (col, row);
            let door = (maze.door_cols.0 as f32 + 0.5, maze.door_row as f32 + 0.5);
            match self.state {
                GhostState::Frightened => self.choose_dir(maze, (0.0, 0.0), true, true),
                GhostState::Eaten => self.choose_dir(maze, door, true, false),
                _ => self.choose_dir(maze, target, false, false),
            }
        }

        // Eaten ghosts are steered down through the door, then respawn inside.
        if self.state == GhostState::Eaten {
            let dc = self.x.floor() as i32;
            let dr = self.y.floor() as i32;
            let at_door_x = dc == maze.door_cols.0 || dc == maze.door_cols.1;
            if dr == maze.door_row && at_door_x {
                self.dir = Direction::Down;
            } else if dr > maze.door_row && at_door_x {
                self.respawn();
                return;
            }
        }

        move_forward(&mut self.x, &mut self.y, self.dir, speed * dt, maze, true);
    }

    fn update_house(&mut self, dt: f32) {
        self.release -= dt;
        if self.release <= 0.0 {
            self.state = GhostState::Leaving;
            self.x = self.home.0;
            self.y = self.home.1;
            self.dir = Direction::Up;
            self.cur_tile = (self.x.floor() as i32, self.y.floor() as i32);
        } else {
            self.bob += dt * 3.0;
            self.x = self.home.0;
            self.y = self.home.1 + self.bob.sin() * 0.35;
            self.dir = if self.bob.sin() > 0.0 {
                Direction::Down
            } else {
                Direction::Up
            };
        }
    }

    fn update_leaving(&mut self, dt: f32, maze: &Maze) {
        move_forward(&mut self.x, &mut self.y, self.dir, self.speed * dt, maze, true);
        if self.y <= maze.door_row as f32 - 0.5 {
            self.state = GhostState::Normal;
            self.dir = Direction::Left;
            self.cur_tile = (self.x.floor() as i32, self.y.floor() as i32);
        }
    }

    fn respawn(&mut self) {
        self.state = GhostState::House;
        self.x = self.home.0;
        self.y = self.home.1;
        self.dir = Direction::Up;
        self.release = 0.8;
        self.cur_tile = (self.x.floor() as i32, self.y.floor() as i32);
        self.bob = self.id as f32;
    }

    /// Greedy "move toward target" direction choice, excluding the reverse
    /// direction unless allowed. `random` picks a random open direction
    /// (frightened mode).
    fn choose_dir(&mut self, maze: &Maze, target: (f32, f32), allow_reverse: bool, randomize: bool) {
        let col = self.x.floor() as i32;
        let row = self.y.floor() as i32;

        let mut options: [Direction; 4] = [Direction::None; 4];
        let mut n = 0usize;
        for d in [
            Direction::Up,
            Direction::Left,
            Direction::Down,
            Direction::Right,
        ] {
            let (dx, dy) = d.delta();
            if is_blocked(maze, true, col + dx, row + dy) {
                continue;
            }
            if d == self.dir.opposite() && !allow_reverse {
                continue;
            }
            options[n] = d;
            n += 1;
        }

        if n == 0 {
            // No legal forward move: dead end, must reverse.
            self.dir = self.dir.opposite();
            return;
        }

        if randomize {
            let i = (random::<f32>() * n as f32) as usize % n;
            self.dir = options[i];
            return;
        }

        let mut best = options[0];
        let mut best_d = f32::MAX;
        for i in 0..n {
            let d = options[i];
            let (dx, dy) = d.delta();
            let nx = col as f32 + 0.5 + dx as f32;
            let ny = row as f32 + 0.5 + dy as f32;
            let dd = (nx - target.0).powi(2) + (ny - target.1).powi(2);
            if dd < best_d {
                best_d = dd;
                best = d;
            }
        }
        self.dir = best;
    }
}
