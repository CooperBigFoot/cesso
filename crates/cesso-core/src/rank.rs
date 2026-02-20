//! Chess board ranks (rows 1–8).

use std::fmt;

/// A rank (row) on the chess board, from Rank1 (White's back rank) to Rank8.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Rank {
    Rank1 = 0,
    Rank2 = 1,
    Rank3 = 2,
    Rank4 = 3,
    Rank5 = 4,
    Rank6 = 5,
    Rank7 = 6,
    Rank8 = 7,
}

impl Rank {
    /// Total number of ranks.
    pub const COUNT: usize = 8;

    /// All ranks in index order.
    pub const ALL: [Rank; 8] = [
        Rank::Rank1,
        Rank::Rank2,
        Rank::Rank3,
        Rank::Rank4,
        Rank::Rank5,
        Rank::Rank6,
        Rank::Rank7,
        Rank::Rank8,
    ];

    /// Return the index (0..7).
    #[inline]
    pub const fn index(self) -> usize {
        self as usize
    }

    /// Create a rank from a zero-based index (0 = Rank1, 7 = Rank8).
    #[inline]
    pub const fn from_index(index: u8) -> Option<Rank> {
        match index {
            0 => Some(Rank::Rank1),
            1 => Some(Rank::Rank2),
            2 => Some(Rank::Rank3),
            3 => Some(Rank::Rank4),
            4 => Some(Rank::Rank5),
            5 => Some(Rank::Rank6),
            6 => Some(Rank::Rank7),
            7 => Some(Rank::Rank8),
            _ => None,
        }
    }
}

impl fmt::Display for Rank {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.index() + 1)
    }
}

#[cfg(test)]
mod tests {
    use super::Rank;

    #[test]
    fn index_values() {
        assert_eq!(Rank::Rank1.index(), 0);
        assert_eq!(Rank::Rank8.index(), 7);
    }

    #[test]
    fn from_index_roundtrip() {
        for rank in Rank::ALL {
            assert_eq!(Rank::from_index(rank.index() as u8), Some(rank));
        }
    }

    #[test]
    fn from_index_out_of_range() {
        assert_eq!(Rank::from_index(8), None);
        assert_eq!(Rank::from_index(255), None);
    }

    #[test]
    fn display() {
        assert_eq!(format!("{}", Rank::Rank1), "1");
        assert_eq!(format!("{}", Rank::Rank8), "8");
    }

    #[test]
    fn ordering() {
        assert!(Rank::Rank1 < Rank::Rank8);
        assert!(Rank::Rank3 < Rank::Rank5);
    }

    #[test]
    fn all_and_count() {
        assert_eq!(Rank::COUNT, 8);
        assert_eq!(Rank::ALL.len(), Rank::COUNT);
    }
}
