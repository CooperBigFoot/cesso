//! Chess piece kinds.

use std::fmt;

/// The kind of a chess piece, without color information.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PieceKind {
    Pawn = 0,
    Knight = 1,
    Bishop = 2,
    Rook = 3,
    Queen = 4,
    King = 5,
}

impl PieceKind {
    /// Total number of piece kinds.
    pub const COUNT: usize = 6;

    /// All piece kinds in index order.
    pub const ALL: [PieceKind; 6] = [
        PieceKind::Pawn,
        PieceKind::Knight,
        PieceKind::Bishop,
        PieceKind::Rook,
        PieceKind::Queen,
        PieceKind::King,
    ];

    /// Return the index (0..5).
    #[inline]
    pub const fn index(self) -> usize {
        self as usize
    }

    /// Return the FEN character for this piece kind (lowercase).
    #[inline]
    pub const fn fen_char(self) -> char {
        match self {
            PieceKind::Pawn => 'p',
            PieceKind::Knight => 'n',
            PieceKind::Bishop => 'b',
            PieceKind::Rook => 'r',
            PieceKind::Queen => 'q',
            PieceKind::King => 'k',
        }
    }

    /// Parse a FEN character (case-insensitive) into a piece kind.
    #[inline]
    pub fn from_fen_char(c: char) -> Option<PieceKind> {
        match c.to_ascii_lowercase() {
            'p' => Some(PieceKind::Pawn),
            'n' => Some(PieceKind::Knight),
            'b' => Some(PieceKind::Bishop),
            'r' => Some(PieceKind::Rook),
            'q' => Some(PieceKind::Queen),
            'k' => Some(PieceKind::King),
            _ => None,
        }
    }
}

impl fmt::Display for PieceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.fen_char())
    }
}

#[cfg(test)]
mod tests {
    use super::PieceKind;

    #[test]
    fn index_values() {
        assert_eq!(PieceKind::Pawn.index(), 0);
        assert_eq!(PieceKind::Knight.index(), 1);
        assert_eq!(PieceKind::Bishop.index(), 2);
        assert_eq!(PieceKind::Rook.index(), 3);
        assert_eq!(PieceKind::Queen.index(), 4);
        assert_eq!(PieceKind::King.index(), 5);
    }

    #[test]
    fn fen_char_roundtrip() {
        for kind in PieceKind::ALL {
            let c = kind.fen_char();
            assert_eq!(PieceKind::from_fen_char(c), Some(kind));
            assert_eq!(PieceKind::from_fen_char(c.to_ascii_uppercase()), Some(kind));
        }
    }

    #[test]
    fn from_fen_char_invalid() {
        assert_eq!(PieceKind::from_fen_char('x'), None);
        assert_eq!(PieceKind::from_fen_char('1'), None);
    }

    #[test]
    fn display() {
        assert_eq!(format!("{}", PieceKind::Pawn), "p");
        assert_eq!(format!("{}", PieceKind::King), "k");
    }

    #[test]
    fn all_and_count() {
        assert_eq!(PieceKind::COUNT, 6);
        assert_eq!(PieceKind::ALL.len(), PieceKind::COUNT);
    }
}
