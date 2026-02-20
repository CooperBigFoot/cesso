//! Bitboard representation for chess — a 64-bit integer where each bit maps to a square.

use std::fmt;
use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Mul, Not, Shl, Shr};

use crate::file::File;
use crate::rank::Rank;
use crate::square::Square;

/// A 64-bit board where each bit represents a square (LERF mapping).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Bitboard(u64);

impl Bitboard {
    /// Empty bitboard (no squares set).
    pub const EMPTY: Bitboard = Bitboard(0);

    /// Full bitboard (all 64 squares set).
    pub const FULL: Bitboard = Bitboard(!0);

    // Rank masks
    pub const RANK_1: Bitboard = Bitboard(0x0000_0000_0000_00FF);
    pub const RANK_2: Bitboard = Bitboard(0x0000_0000_0000_FF00);
    pub const RANK_3: Bitboard = Bitboard(0x0000_0000_00FF_0000);
    pub const RANK_4: Bitboard = Bitboard(0x0000_0000_FF00_0000);
    pub const RANK_5: Bitboard = Bitboard(0x0000_00FF_0000_0000);
    pub const RANK_6: Bitboard = Bitboard(0x0000_FF00_0000_0000);
    pub const RANK_7: Bitboard = Bitboard(0x00FF_0000_0000_0000);
    pub const RANK_8: Bitboard = Bitboard(0xFF00_0000_0000_0000);

    /// All rank masks indexed by rank index.
    pub const RANKS: [Bitboard; 8] = [
        Self::RANK_1, Self::RANK_2, Self::RANK_3, Self::RANK_4,
        Self::RANK_5, Self::RANK_6, Self::RANK_7, Self::RANK_8,
    ];

    // File masks
    pub const FILE_A: Bitboard = Bitboard(0x0101_0101_0101_0101);
    pub const FILE_B: Bitboard = Bitboard(0x0202_0202_0202_0202);
    pub const FILE_C: Bitboard = Bitboard(0x0404_0404_0404_0404);
    pub const FILE_D: Bitboard = Bitboard(0x0808_0808_0808_0808);
    pub const FILE_E: Bitboard = Bitboard(0x1010_1010_1010_1010);
    pub const FILE_F: Bitboard = Bitboard(0x2020_2020_2020_2020);
    pub const FILE_G: Bitboard = Bitboard(0x4040_4040_4040_4040);
    pub const FILE_H: Bitboard = Bitboard(0x8080_8080_8080_8080);

    /// All file masks indexed by file index.
    pub const FILES: [Bitboard; 8] = [
        Self::FILE_A, Self::FILE_B, Self::FILE_C, Self::FILE_D,
        Self::FILE_E, Self::FILE_F, Self::FILE_G, Self::FILE_H,
    ];

    /// Create a bitboard from a raw `u64`.
    #[inline]
    pub const fn new(bits: u64) -> Bitboard {
        Bitboard(bits)
    }

    /// Return the underlying `u64`.
    #[inline]
    pub const fn inner(self) -> u64 {
        self.0
    }

    /// Return `true` if no bits are set.
    #[inline]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Return `true` if at least one bit is set.
    #[inline]
    pub const fn is_nonempty(self) -> bool {
        self.0 != 0
    }

    /// Count the number of set bits.
    #[inline]
    pub const fn count(self) -> u32 {
        self.0.count_ones()
    }

    /// Return `true` if the given square's bit is set.
    #[inline]
    pub const fn contains(self, sq: Square) -> bool {
        (self.0 & (1u64 << sq.index())) != 0
    }

    /// Return a new bitboard with the given square set.
    #[inline]
    pub const fn with(self, sq: Square) -> Bitboard {
        Bitboard(self.0 | (1u64 << sq.index()))
    }

    /// Return a new bitboard with the given square cleared.
    #[inline]
    pub const fn without(self, sq: Square) -> Bitboard {
        Bitboard(self.0 & !(1u64 << sq.index()))
    }

    /// Return a new bitboard with the given square toggled.
    #[inline]
    pub const fn toggle(self, sq: Square) -> Bitboard {
        Bitboard(self.0 ^ (1u64 << sq.index()))
    }

    /// Return the least significant set bit as a square, or `None` if empty.
    #[inline]
    pub const fn lsb(self) -> Option<Square> {
        if self.0 == 0 {
            None
        } else {
            Some(Square::from_index_unchecked(self.0.trailing_zeros() as u8))
        }
    }

    /// Pop the least significant set bit, returning the square and the remaining bitboard.
    #[inline]
    pub const fn pop_lsb(self) -> Option<(Square, Bitboard)> {
        if self.0 == 0 {
            None
        } else {
            let sq = Square::from_index_unchecked(self.0.trailing_zeros() as u8);
            Some((sq, Bitboard(self.0 & (self.0 - 1))))
        }
    }

    /// Return the rank mask for the given rank.
    #[inline]
    pub const fn rank_mask(rank: Rank) -> Bitboard {
        Self::RANKS[rank.index()]
    }

    /// Return the file mask for the given file.
    #[inline]
    pub const fn file_mask(file: File) -> Bitboard {
        Self::FILES[file.index()]
    }
}

// --- Operator impls ---

impl BitAnd for Bitboard {
    type Output = Bitboard;
    #[inline]
    fn bitand(self, rhs: Bitboard) -> Bitboard {
        Bitboard(self.0 & rhs.0)
    }
}

impl BitAndAssign for Bitboard {
    #[inline]
    fn bitand_assign(&mut self, rhs: Bitboard) {
        self.0 &= rhs.0;
    }
}

impl BitOr for Bitboard {
    type Output = Bitboard;
    #[inline]
    fn bitor(self, rhs: Bitboard) -> Bitboard {
        Bitboard(self.0 | rhs.0)
    }
}

impl BitOrAssign for Bitboard {
    #[inline]
    fn bitor_assign(&mut self, rhs: Bitboard) {
        self.0 |= rhs.0;
    }
}

impl BitXor for Bitboard {
    type Output = Bitboard;
    #[inline]
    fn bitxor(self, rhs: Bitboard) -> Bitboard {
        Bitboard(self.0 ^ rhs.0)
    }
}

impl BitXorAssign for Bitboard {
    #[inline]
    fn bitxor_assign(&mut self, rhs: Bitboard) {
        self.0 ^= rhs.0;
    }
}

impl Not for Bitboard {
    type Output = Bitboard;
    #[inline]
    fn not(self) -> Bitboard {
        Bitboard(!self.0)
    }
}

impl Shl<u8> for Bitboard {
    type Output = Bitboard;
    #[inline]
    fn shl(self, rhs: u8) -> Bitboard {
        Bitboard(self.0 << rhs)
    }
}

impl Shr<u8> for Bitboard {
    type Output = Bitboard;
    #[inline]
    fn shr(self, rhs: u8) -> Bitboard {
        Bitboard(self.0 >> rhs)
    }
}

impl Mul for Bitboard {
    type Output = Bitboard;
    #[inline]
    fn mul(self, rhs: Bitboard) -> Bitboard {
        Bitboard(self.0.wrapping_mul(rhs.0))
    }
}

// --- Iterator ---

impl Iterator for Bitboard {
    type Item = Square;

    #[inline]
    fn next(&mut self) -> Option<Square> {
        if self.0 == 0 {
            None
        } else {
            let sq = Square::from_index_unchecked(self.0.trailing_zeros() as u8);
            self.0 &= self.0 - 1;
            Some(sq)
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let count = self.count() as usize;
        (count, Some(count))
    }
}

impl ExactSizeIterator for Bitboard {}

// --- Debug (8x8 grid) ---

impl fmt::Debug for Bitboard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f)?;
        for rank in (0..8).rev() {
            write!(f, "  {} ", rank + 1)?;
            for file in 0..8 {
                let sq_index = rank * 8 + file;
                if (self.0 >> sq_index) & 1 == 1 {
                    write!(f, "1 ")?;
                } else {
                    write!(f, ". ")?;
                }
            }
            writeln!(f)?;
        }
        write!(f, "    a b c d e f g h")
    }
}

#[cfg(test)]
mod tests {
    use super::Bitboard;
    use crate::file::File;
    use crate::rank::Rank;
    use crate::square::Square;

    #[test]
    fn empty_and_full() {
        assert!(Bitboard::EMPTY.is_empty());
        assert!(!Bitboard::FULL.is_empty());
        assert!(!Bitboard::EMPTY.is_nonempty());
        assert!(Bitboard::FULL.is_nonempty());
        assert_eq!(!Bitboard::EMPTY, Bitboard::FULL);
        assert_eq!(!Bitboard::FULL, Bitboard::EMPTY);
    }

    #[test]
    fn set_contains_clear() {
        let bb = Bitboard::EMPTY.with(Square::E4);
        assert!(bb.contains(Square::E4));
        assert!(!bb.contains(Square::D4));
        assert_eq!(bb.count(), 1);

        let bb2 = bb.without(Square::E4);
        assert!(!bb2.contains(Square::E4));
        assert!(bb2.is_empty());
    }

    #[test]
    fn toggle() {
        let bb = Bitboard::EMPTY.toggle(Square::A1).toggle(Square::H8);
        assert!(bb.contains(Square::A1));
        assert!(bb.contains(Square::H8));
        assert_eq!(bb.count(), 2);

        let bb2 = bb.toggle(Square::A1);
        assert!(!bb2.contains(Square::A1));
        assert!(bb2.contains(Square::H8));
    }

    #[test]
    fn count() {
        assert_eq!(Bitboard::EMPTY.count(), 0);
        assert_eq!(Bitboard::FULL.count(), 64);
        assert_eq!(Bitboard::RANK_1.count(), 8);
        assert_eq!(Bitboard::FILE_A.count(), 8);
    }

    #[test]
    fn rank_masks() {
        for rank in Rank::ALL {
            let mask = Bitboard::rank_mask(rank);
            assert_eq!(mask.count(), 8);
            // Every square on this rank should be in the mask
            for file in File::ALL {
                let sq = Square::new(rank, file);
                assert!(mask.contains(sq));
            }
        }
    }

    #[test]
    fn file_masks() {
        for file in File::ALL {
            let mask = Bitboard::file_mask(file);
            assert_eq!(mask.count(), 8);
            for rank in Rank::ALL {
                let sq = Square::new(rank, file);
                assert!(mask.contains(sq));
            }
        }
    }

    #[test]
    fn lsb() {
        assert_eq!(Bitboard::EMPTY.lsb(), None);
        let bb = Bitboard::EMPTY.with(Square::C3).with(Square::F6);
        assert_eq!(bb.lsb(), Some(Square::C3));
    }

    #[test]
    fn pop_lsb() {
        let bb = Bitboard::EMPTY.with(Square::A1).with(Square::H8);
        let (sq, rest) = bb.pop_lsb().unwrap();
        assert_eq!(sq, Square::A1);
        assert_eq!(rest.count(), 1);
        let (sq2, rest2) = rest.pop_lsb().unwrap();
        assert_eq!(sq2, Square::H8);
        assert!(rest2.is_empty());
    }

    #[test]
    fn iterator_order_and_count() {
        let bb = Bitboard::EMPTY
            .with(Square::A1)
            .with(Square::E4)
            .with(Square::H8);
        let squares: Vec<_> = bb.collect();
        assert_eq!(squares.len(), 3);
        assert_eq!(squares[0], Square::A1);
        assert_eq!(squares[1], Square::E4);
        assert_eq!(squares[2], Square::H8);
    }

    #[test]
    fn exact_size_iterator() {
        let bb = Bitboard::RANK_1;
        assert_eq!(bb.len(), 8);
    }

    #[test]
    fn operator_commutativity() {
        let a = Bitboard::RANK_1;
        let b = Bitboard::FILE_A;
        assert_eq!(a & b, b & a);
        assert_eq!(a | b, b | a);
        assert_eq!(a ^ b, b ^ a);
    }

    #[test]
    fn shift_operators() {
        let bb = Bitboard::RANK_1;
        let shifted = bb << 8;
        assert_eq!(shifted, Bitboard::RANK_2);

        let back = shifted >> 8;
        assert_eq!(back, Bitboard::RANK_1);
    }

    #[test]
    fn default_is_empty() {
        assert_eq!(Bitboard::default(), Bitboard::EMPTY);
    }

    #[test]
    fn assign_operators() {
        let mut bb = Bitboard::RANK_1;
        bb |= Bitboard::RANK_2;
        assert_eq!(bb.count(), 16);

        bb &= Bitboard::FILE_A;
        assert_eq!(bb.count(), 2);

        bb ^= Bitboard::EMPTY.with(Square::A1);
        assert_eq!(bb.count(), 1);
    }
}
