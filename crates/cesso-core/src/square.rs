//! Chess board squares using Little-Endian Rank-File (LERF) encoding.

use std::fmt;

use crate::bitboard::Bitboard;
use crate::file::File;
use crate::rank::Rank;

/// A square on the chess board, encoded as a `u8` in LERF format.
///
/// Index = rank * 8 + file, so A1 = 0, B1 = 1, ..., H8 = 63.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Square(u8);

impl Square {
    /// Total number of squares.
    pub const COUNT: usize = 64;

    /// Create a square from a rank and file.
    #[inline]
    pub const fn new(rank: Rank, file: File) -> Square {
        Square(rank.index() as u8 * 8 + file.index() as u8)
    }

    /// Create a square from a zero-based index, returning `None` if out of range.
    #[inline]
    pub const fn from_index(index: u8) -> Option<Square> {
        if index < 64 {
            Some(Square(index))
        } else {
            None
        }
    }

    /// Create a square from a zero-based index without bounds checking.
    ///
    /// # Panics
    ///
    /// Debug-asserts that `index < 64`.
    #[inline]
    pub(crate) const fn from_index_unchecked(index: u8) -> Square {
        debug_assert!(index < 64);
        Square(index)
    }

    /// Parse an algebraic notation string (e.g. "e4") into a square.
    pub fn from_algebraic(s: &str) -> Option<Square> {
        let bytes = s.as_bytes();
        if bytes.len() != 2 {
            return None;
        }

        let file_byte = bytes[0];
        let rank_byte = bytes[1];

        if !(b'a'..=b'h').contains(&file_byte) || !(b'1'..=b'8').contains(&rank_byte) {
            return None;
        }

        let file = File::from_index(file_byte - b'a')?;
        let rank = Rank::from_index(rank_byte - b'1')?;
        Some(Square::new(rank, file))
    }

    /// Return the zero-based index (0..63).
    #[inline]
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    /// Return the rank of this square.
    #[inline]
    pub const fn rank(self) -> Rank {
        // Safety: self.0 / 8 is always 0..7 for valid squares
        match self.0 / 8 {
            0 => Rank::Rank1,
            1 => Rank::Rank2,
            2 => Rank::Rank3,
            3 => Rank::Rank4,
            4 => Rank::Rank5,
            5 => Rank::Rank6,
            6 => Rank::Rank7,
            _ => Rank::Rank8,
        }
    }

    /// Return the file of this square.
    #[inline]
    pub const fn file(self) -> File {
        match self.0 % 8 {
            0 => File::FileA,
            1 => File::FileB,
            2 => File::FileC,
            3 => File::FileD,
            4 => File::FileE,
            5 => File::FileF,
            6 => File::FileG,
            _ => File::FileH,
        }
    }

    /// Return a bitboard with only this square set.
    #[inline]
    pub const fn bitboard(self) -> Bitboard {
        Bitboard::new(1u64 << self.0)
    }

    /// Iterate over all 64 squares in index order (A1, B1, ..., H8).
    pub fn all() -> impl Iterator<Item = Square> {
        (0u8..64).map(Square)
    }

    // Named square constants
    pub const A1: Square = Square(0);
    pub const B1: Square = Square(1);
    pub const C1: Square = Square(2);
    pub const D1: Square = Square(3);
    pub const E1: Square = Square(4);
    pub const F1: Square = Square(5);
    pub const G1: Square = Square(6);
    pub const H1: Square = Square(7);
    pub const A2: Square = Square(8);
    pub const B2: Square = Square(9);
    pub const C2: Square = Square(10);
    pub const D2: Square = Square(11);
    pub const E2: Square = Square(12);
    pub const F2: Square = Square(13);
    pub const G2: Square = Square(14);
    pub const H2: Square = Square(15);
    pub const A3: Square = Square(16);
    pub const B3: Square = Square(17);
    pub const C3: Square = Square(18);
    pub const D3: Square = Square(19);
    pub const E3: Square = Square(20);
    pub const F3: Square = Square(21);
    pub const G3: Square = Square(22);
    pub const H3: Square = Square(23);
    pub const A4: Square = Square(24);
    pub const B4: Square = Square(25);
    pub const C4: Square = Square(26);
    pub const D4: Square = Square(27);
    pub const E4: Square = Square(28);
    pub const F4: Square = Square(29);
    pub const G4: Square = Square(30);
    pub const H4: Square = Square(31);
    pub const A5: Square = Square(32);
    pub const B5: Square = Square(33);
    pub const C5: Square = Square(34);
    pub const D5: Square = Square(35);
    pub const E5: Square = Square(36);
    pub const F5: Square = Square(37);
    pub const G5: Square = Square(38);
    pub const H5: Square = Square(39);
    pub const A6: Square = Square(40);
    pub const B6: Square = Square(41);
    pub const C6: Square = Square(42);
    pub const D6: Square = Square(43);
    pub const E6: Square = Square(44);
    pub const F6: Square = Square(45);
    pub const G6: Square = Square(46);
    pub const H6: Square = Square(47);
    pub const A7: Square = Square(48);
    pub const B7: Square = Square(49);
    pub const C7: Square = Square(50);
    pub const D7: Square = Square(51);
    pub const E7: Square = Square(52);
    pub const F7: Square = Square(53);
    pub const G7: Square = Square(54);
    pub const H7: Square = Square(55);
    pub const A8: Square = Square(56);
    pub const B8: Square = Square(57);
    pub const C8: Square = Square(58);
    pub const D8: Square = Square(59);
    pub const E8: Square = Square(60);
    pub const F8: Square = Square(61);
    pub const G8: Square = Square(62);
    pub const H8: Square = Square(63);
}

impl fmt::Display for Square {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.file(), self.rank())
    }
}

impl fmt::Debug for Square {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Square({})", self)
    }
}

#[cfg(test)]
mod tests {
    use super::Square;
    use crate::file::File;
    use crate::rank::Rank;

    #[test]
    fn new_and_accessors() {
        let sq = Square::new(Rank::Rank1, File::FileA);
        assert_eq!(sq, Square::A1);
        assert_eq!(sq.rank(), Rank::Rank1);
        assert_eq!(sq.file(), File::FileA);
        assert_eq!(sq.index(), 0);
    }

    #[test]
    fn rank_file_roundtrip() {
        for sq in Square::all() {
            let reconstructed = Square::new(sq.rank(), sq.file());
            assert_eq!(sq, reconstructed);
        }
    }

    #[test]
    fn from_index_valid() {
        for i in 0u8..64 {
            assert!(Square::from_index(i).is_some());
        }
    }

    #[test]
    fn from_index_invalid() {
        assert!(Square::from_index(64).is_none());
        assert!(Square::from_index(255).is_none());
    }

    #[test]
    fn algebraic_notation() {
        assert_eq!(Square::from_algebraic("a1"), Some(Square::A1));
        assert_eq!(Square::from_algebraic("e4"), Some(Square::E4));
        assert_eq!(Square::from_algebraic("h8"), Some(Square::H8));
        assert_eq!(format!("{}", Square::E4), "e4");
        assert_eq!(format!("{}", Square::A1), "a1");
        assert_eq!(format!("{}", Square::H8), "h8");
    }

    #[test]
    fn algebraic_invalid() {
        assert!(Square::from_algebraic("i1").is_none());
        assert!(Square::from_algebraic("a9").is_none());
        assert!(Square::from_algebraic("").is_none());
        assert!(Square::from_algebraic("a").is_none());
        assert!(Square::from_algebraic("a1b").is_none());
    }

    #[test]
    fn named_constants() {
        assert_eq!(Square::A1.index(), 0);
        assert_eq!(Square::H1.index(), 7);
        assert_eq!(Square::A8.index(), 56);
        assert_eq!(Square::H8.index(), 63);
        assert_eq!(Square::E1.index(), 4);
        assert_eq!(Square::E8.index(), 60);
    }

    #[test]
    fn bitboard_single_bit() {
        let bb = Square::A1.bitboard();
        assert_eq!(bb.count(), 1);
        assert!(bb.contains(Square::A1));
        assert!(!bb.contains(Square::B1));
    }

    #[test]
    fn all_iterator_count() {
        assert_eq!(Square::all().count(), 64);
    }

    #[test]
    fn debug_shows_algebraic() {
        assert_eq!(format!("{:?}", Square::E4), "Square(e4)");
    }
}
