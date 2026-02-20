//! Chess piece colors.

use std::fmt;
use std::ops::Not;

/// A chess piece color: White or Black.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Color {
    White = 0,
    Black = 1,
}

impl Color {
    /// Total number of colors.
    pub const COUNT: usize = 2;

    /// All colors in index order.
    pub const ALL: [Color; 2] = [Color::White, Color::Black];

    /// Return the index (0 for White, 1 for Black).
    #[inline]
    pub const fn index(self) -> usize {
        self as usize
    }

    /// Return the opposite color.
    #[inline]
    pub const fn flip(self) -> Color {
        match self {
            Color::White => Color::Black,
            Color::Black => Color::White,
        }
    }
}

impl Not for Color {
    type Output = Color;

    #[inline]
    fn not(self) -> Color {
        self.flip()
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Color::White => write!(f, "w"),
            Color::Black => write!(f, "b"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Color;

    #[test]
    fn index_values() {
        assert_eq!(Color::White.index(), 0);
        assert_eq!(Color::Black.index(), 1);
    }

    #[test]
    fn flip_roundtrip() {
        assert_eq!(Color::White.flip(), Color::Black);
        assert_eq!(Color::Black.flip(), Color::White);
        assert_eq!(Color::White.flip().flip(), Color::White);
    }

    #[test]
    fn not_operator() {
        assert_eq!(!Color::White, Color::Black);
        assert_eq!(!Color::Black, Color::White);
    }

    #[test]
    fn display() {
        assert_eq!(format!("{}", Color::White), "w");
        assert_eq!(format!("{}", Color::Black), "b");
    }

    #[test]
    fn all_and_count() {
        assert_eq!(Color::COUNT, 2);
        assert_eq!(Color::ALL.len(), Color::COUNT);
        assert_eq!(Color::ALL[0], Color::White);
        assert_eq!(Color::ALL[1], Color::Black);
    }
}
