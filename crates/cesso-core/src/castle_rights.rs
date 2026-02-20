//! Castling rights stored as a 4-bit field within a `u8`.

use std::fmt;
use std::ops::{BitAnd, BitOr, Not};

use crate::color::Color;
use crate::error::FenError;

/// Which side of the board to castle toward.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CastleSide {
    KingSide,
    QueenSide,
}

/// Castling rights encoded as a 4-bit field: bit 0 = WK, 1 = WQ, 2 = BK, 3 = BQ.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct CastleRights(u8);

impl CastleRights {
    /// No castling rights.
    pub const NONE: CastleRights = CastleRights(0);
    /// All castling rights.
    pub const ALL: CastleRights = CastleRights(0b1111);

    /// White king-side castling.
    pub const WHITE_KING: CastleRights = CastleRights(0b0001);
    /// White queen-side castling.
    pub const WHITE_QUEEN: CastleRights = CastleRights(0b0010);
    /// Black king-side castling.
    pub const BLACK_KING: CastleRights = CastleRights(0b0100);
    /// Black queen-side castling.
    pub const BLACK_QUEEN: CastleRights = CastleRights(0b1000);

    /// Both white castling rights.
    pub const WHITE_BOTH: CastleRights = CastleRights(0b0011);
    /// Both black castling rights.
    pub const BLACK_BOTH: CastleRights = CastleRights(0b1100);

    /// Create castling rights from a raw `u8`, masking to the lower 4 bits.
    #[inline]
    pub const fn new(bits: u8) -> CastleRights {
        CastleRights(bits & 0b1111)
    }

    /// Return the raw bits.
    #[inline]
    pub const fn bits(self) -> u8 {
        self.0
    }

    /// Return `true` if no castling rights remain.
    #[inline]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Return `true` if all bits in `other` are set in `self`.
    #[inline]
    pub const fn contains(self, other: CastleRights) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Return new rights with all bits from `other` added.
    #[inline]
    pub const fn insert(self, other: CastleRights) -> CastleRights {
        CastleRights(self.0 | other.0)
    }

    /// Return new rights with all bits from `other` removed.
    #[inline]
    pub const fn remove(self, other: CastleRights) -> CastleRights {
        CastleRights(self.0 & !other.0)
    }

    /// Check whether a specific color and side can castle.
    #[inline]
    pub const fn has(self, color: Color, side: CastleSide) -> bool {
        let bit = Self::flag(color, side).0;
        (self.0 & bit) != 0
    }

    /// Remove all castling rights for the given color.
    #[inline]
    pub const fn remove_color(self, color: Color) -> CastleRights {
        match color {
            Color::White => self.remove(Self::WHITE_BOTH),
            Color::Black => self.remove(Self::BLACK_BOTH),
        }
    }

    /// Return the single-bit flag for a color and side.
    #[inline]
    const fn flag(color: Color, side: CastleSide) -> CastleRights {
        match (color, side) {
            (Color::White, CastleSide::KingSide) => Self::WHITE_KING,
            (Color::White, CastleSide::QueenSide) => Self::WHITE_QUEEN,
            (Color::Black, CastleSide::KingSide) => Self::BLACK_KING,
            (Color::Black, CastleSide::QueenSide) => Self::BLACK_QUEEN,
        }
    }

    /// Parse castling rights from the FEN castling field (e.g. "KQkq", "Kq", "-").
    pub fn from_fen(s: &str) -> Result<CastleRights, FenError> {
        if s == "-" {
            return Ok(CastleRights::NONE);
        }

        let mut rights = CastleRights::NONE;
        for c in s.chars() {
            let flag = match c {
                'K' => Self::WHITE_KING,
                'Q' => Self::WHITE_QUEEN,
                'k' => Self::BLACK_KING,
                'q' => Self::BLACK_QUEEN,
                _ => return Err(FenError::InvalidCastlingChar { character: c }),
            };
            rights = rights.insert(flag);
        }
        Ok(rights)
    }

    /// Serialize castling rights to the FEN castling field.
    pub fn to_fen(self) -> String {
        if self.is_empty() {
            return "-".to_string();
        }

        let mut s = String::with_capacity(4);
        if self.contains(Self::WHITE_KING) {
            s.push('K');
        }
        if self.contains(Self::WHITE_QUEEN) {
            s.push('Q');
        }
        if self.contains(Self::BLACK_KING) {
            s.push('k');
        }
        if self.contains(Self::BLACK_QUEEN) {
            s.push('q');
        }
        s
    }
}

impl BitAnd for CastleRights {
    type Output = CastleRights;
    #[inline]
    fn bitand(self, rhs: CastleRights) -> CastleRights {
        CastleRights(self.0 & rhs.0)
    }
}

impl BitOr for CastleRights {
    type Output = CastleRights;
    #[inline]
    fn bitor(self, rhs: CastleRights) -> CastleRights {
        CastleRights(self.0 | rhs.0)
    }
}

impl Not for CastleRights {
    type Output = CastleRights;
    #[inline]
    fn not(self) -> CastleRights {
        CastleRights(!self.0 & 0b1111)
    }
}

impl fmt::Display for CastleRights {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_fen())
    }
}

impl fmt::Debug for CastleRights {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CastleRights({})", self.to_fen())
    }
}

#[cfg(test)]
mod tests {
    use super::{CastleRights, CastleSide};
    use crate::color::Color;

    #[test]
    fn insert_remove_roundtrip() {
        let rights = CastleRights::NONE
            .insert(CastleRights::WHITE_KING)
            .insert(CastleRights::BLACK_QUEEN);
        assert!(rights.contains(CastleRights::WHITE_KING));
        assert!(rights.contains(CastleRights::BLACK_QUEEN));
        assert!(!rights.contains(CastleRights::WHITE_QUEEN));

        let removed = rights.remove(CastleRights::WHITE_KING);
        assert!(!removed.contains(CastleRights::WHITE_KING));
        assert!(removed.contains(CastleRights::BLACK_QUEEN));
    }

    #[test]
    fn from_fen_to_fen_roundtrip() {
        let cases = ["KQkq", "Kq", "k", "-", "KQ", "kq", "Qk"];
        for fen in &cases {
            let rights = CastleRights::from_fen(fen).unwrap();
            let output = rights.to_fen();
            let reparsed = CastleRights::from_fen(&output).unwrap();
            assert_eq!(rights, reparsed, "roundtrip failed for {fen}");
        }
    }

    #[test]
    fn from_fen_starting() {
        let rights = CastleRights::from_fen("KQkq").unwrap();
        assert_eq!(rights, CastleRights::ALL);
    }

    #[test]
    fn from_fen_none() {
        let rights = CastleRights::from_fen("-").unwrap();
        assert_eq!(rights, CastleRights::NONE);
        assert!(rights.is_empty());
    }

    #[test]
    fn from_fen_invalid() {
        assert!(CastleRights::from_fen("KQxq").is_err());
        assert!(CastleRights::from_fen("1").is_err());
    }

    #[test]
    fn has_color_side() {
        let rights = CastleRights::from_fen("Kq").unwrap();
        assert!(rights.has(Color::White, CastleSide::KingSide));
        assert!(!rights.has(Color::White, CastleSide::QueenSide));
        assert!(!rights.has(Color::Black, CastleSide::KingSide));
        assert!(rights.has(Color::Black, CastleSide::QueenSide));
    }

    #[test]
    fn remove_color() {
        let rights = CastleRights::ALL.remove_color(Color::White);
        assert_eq!(rights, CastleRights::BLACK_BOTH);

        let rights2 = CastleRights::ALL.remove_color(Color::Black);
        assert_eq!(rights2, CastleRights::WHITE_BOTH);
    }

    #[test]
    fn contains_checks() {
        assert!(CastleRights::ALL.contains(CastleRights::WHITE_BOTH));
        assert!(CastleRights::ALL.contains(CastleRights::BLACK_BOTH));
        assert!(!CastleRights::NONE.contains(CastleRights::WHITE_KING));
    }

    #[test]
    fn not_operator() {
        assert_eq!(!CastleRights::NONE, CastleRights::ALL);
        assert_eq!(!CastleRights::ALL, CastleRights::NONE);
        assert_eq!(!CastleRights::WHITE_BOTH, CastleRights::BLACK_BOTH);
    }

    #[test]
    fn display() {
        assert_eq!(format!("{}", CastleRights::ALL), "KQkq");
        assert_eq!(format!("{}", CastleRights::NONE), "-");
    }

    #[test]
    fn new_masks_to_four_bits() {
        let rights = CastleRights::new(0xFF);
        assert_eq!(rights.bits(), 0b1111);
    }
}
