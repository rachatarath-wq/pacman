//! Shared small types: directions and coordinates.

/// A cardinal direction. Serialized to JS as a plain integer (`as u8`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Direction {
    Up = 0,
    Left = 1,
    Down = 2,
    Right = 3,
    None = 4,
}

impl Direction {
    #[inline]
    pub fn opposite(self) -> Direction {
        match self {
            Direction::Up => Direction::Down,
            Direction::Down => Direction::Up,
            Direction::Left => Direction::Right,
            Direction::Right => Direction::Left,
            Direction::None => Direction::None,
        }
    }

    /// Unit delta in (col, row) space.
    #[inline]
    pub fn delta(self) -> (i32, i32) {
        match self {
            Direction::Up => (0, -1),
            Direction::Down => (0, 1),
            Direction::Left => (-1, 0),
            Direction::Right => (1, 0),
            Direction::None => (0, 0),
        }
    }

    #[inline]
    pub fn from_u8(v: u8) -> Direction {
        match v {
            0 => Direction::Up,
            1 => Direction::Left,
            2 => Direction::Down,
            3 => Direction::Right,
            _ => Direction::None,
        }
    }

    #[inline]
    pub fn as_u8(self) -> u8 {
        self as u8
    }
}
