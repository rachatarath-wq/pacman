//! The fixed Pac-Man maze.
//!
//! The maze is 28 columns x 31 rows and mirrors the classic arcade layout. Tile
//! characters are:
//!   `#` wall, `.` dot, `o` power pellet, `_` empty, `=` ghost-house door.

/// Structural (static) tile kinds.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cell {
    Wall,
    Empty,
    Door,
}

pub struct Maze {
    pub width: usize,
    pub height: usize,
    cells: Vec<Cell>,
    /// Per-tile pellet state: 0 = none/eaten, 1 = dot, 2 = power pellet.
    pub dots: Vec<u8>,
    /// Total dots (dots + pellets) for one level.
    pub total_dots: u32,
    /// Row of the ghost-house door, and its two columns.
    pub door_row: i32,
    pub door_cols: (i32, i32),
}

const MAZE: [&str; 31] = [
    "############################",
    "#............##............#",
    "#.####.#####.##.#####.####.#",
    "#o####.#####.##.#####.####o#",
    "#.####.#####.##.#####.####.#",
    "#..........................#",
    "#.####.##.########.##.####.#",
    "#.####.##.########.##.####.#",
    "#......##....##....##......#",
    "######.#####.##.#####.######",
    "######.#####.##.#####.######",
    "######.##..........##.######",
    "######.######==######.######",
    "######.###________###.######",
    "_________#________#_________",
    "######.###________###.######",
    "######.##############.######",
    "######.##..........##.######",
    "######.##.########.##.######",
    "######.##.########.##.######",
    "#............##............#",
    "#.####.#####.##.#####.####.#",
    "#.####.#####.##.#####.####.#",
    "#o..##.......__.......##..o#",
    "###.##.##.########.##.##.###",
    "###.##.##.########.##.##.###",
    "#......##....##....##......#",
    "#.##########.##.##########.#",
    "#.##########.##.##########.#",
    "#..........................#",
    "############################",
];

impl Maze {
    pub fn new() -> Self {
        let mut cells = Vec::with_capacity(28 * 31);
        let mut dots = Vec::with_capacity(28 * 31);
        let mut total_dots = 0u32;

        for row in MAZE.iter() {
            debug_assert_eq!(row.len(), 28, "every maze row must be 28 wide");
            for ch in row.chars() {
                match ch {
                    '#' => {
                        cells.push(Cell::Wall);
                        dots.push(0);
                    }
                    '.' => {
                        cells.push(Cell::Empty);
                        dots.push(1);
                        total_dots += 1;
                    }
                    'o' => {
                        cells.push(Cell::Empty);
                        dots.push(2);
                        total_dots += 1;
                    }
                    '=' => {
                        cells.push(Cell::Door);
                        dots.push(0);
                    }
                    _ => {
                        cells.push(Cell::Empty);
                        dots.push(0);
                    }
                }
            }
        }

        Maze {
            width: 28,
            height: 31,
            cells,
            dots,
            total_dots,
            door_row: 12,
            door_cols: (13, 14),
        }
    }

    #[inline]
    pub fn idx(&self, c: i32, r: i32) -> usize {
        (r as usize) * self.width + (c as usize)
    }

    #[inline]
    pub fn in_bounds(&self, c: i32, r: i32) -> bool {
        c >= 0 && r >= 0 && (c as usize) < self.width && (r as usize) < self.height
    }

    #[inline]
    pub fn cell(&self, c: i32, r: i32) -> Cell {
        if self.in_bounds(c, r) {
            self.cells[self.idx(c, r)]
        } else {
            Cell::Wall
        }
    }

    /// Pac-Man cannot pass walls or the ghost-house door.
    #[inline]
    pub fn blocked_pacman(&self, c: i32, r: i32) -> bool {
        matches!(self.cell(c, r), Cell::Wall | Cell::Door)
    }

    /// Ghosts pass the door but not walls.
    #[inline]
    pub fn blocked_ghost(&self, c: i32, r: i32) -> bool {
        self.cell(c, r) == Cell::Wall
    }

    /// Eat the dot/pellet at a tile. Returns what was eaten (0/1/2).
    pub fn eat_dot(&mut self, c: i32, r: i32) -> u8 {
        if self.in_bounds(c, r) {
            let i = self.idx(c, r);
            let d = self.dots[i];
            if d != 0 {
                self.dots[i] = 0;
            }
            d
        } else {
            0
        }
    }

    /// Restore every dot/pellet (used when a new level begins).
    pub fn reset_dots(&mut self) {
        for (r, row) in MAZE.iter().enumerate() {
            for (c, ch) in row.chars().enumerate() {
                let i = r * self.width + c;
                self.dots[i] = match ch {
                    '.' => 1,
                    'o' => 2,
                    _ => 0,
                };
            }
        }
    }

    /// Raw maze rows, exported to JS once so the frontend can draw walls.
    pub fn rows(&self) -> &'static [&'static str] {
        &MAZE
    }
}

impl Default for Maze {
    fn default() -> Self {
        Self::new()
    }
}
