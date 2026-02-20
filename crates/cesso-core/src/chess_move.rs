//! Chess move representation, bit-packed into a u16.

use std::fmt;

use crate::piece_kind::PieceKind;
use crate::square::Square;

// Private bit-field constants.
const SRC_MASK: u16 = 0x003F;
const DST_MASK: u16 = 0x0FC0;
const PROMO_MASK: u16 = 0x3000;
const KIND_MASK: u16 = 0xC000;
const SRC_SHIFT: u32 = 0;
const DST_SHIFT: u32 = 6;
const PROMO_SHIFT: u32 = 12;
const KIND_SHIFT: u32 = 14;

/// The category of a chess move.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum MoveKind {
    Normal = 0,
    Promotion = 1,
    EnPassant = 2,
    Castling = 3,
}

impl MoveKind {
    /// Return the bit pattern for this kind, shifted to position.
    const fn bits(self) -> u16 {
        (self as u16) << KIND_SHIFT
    }
}

/// The piece a pawn promotes to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PromotionPiece {
    Knight = 0,
    Bishop = 1,
    Rook = 2,
    Queen = 3,
}

impl PromotionPiece {
    /// All promotion pieces in index order.
    pub const ALL: [PromotionPiece; 4] = [
        PromotionPiece::Knight,
        PromotionPiece::Bishop,
        PromotionPiece::Rook,
        PromotionPiece::Queen,
    ];

    /// Convert to the corresponding [`PieceKind`].
    pub const fn to_piece_kind(self) -> PieceKind {
        match self {
            PromotionPiece::Knight => PieceKind::Knight,
            PromotionPiece::Bishop => PieceKind::Bishop,
            PromotionPiece::Rook => PieceKind::Rook,
            PromotionPiece::Queen => PieceKind::Queen,
        }
    }

    /// Return the UCI character for this promotion.
    pub const fn uci_char(self) -> char {
        match self {
            PromotionPiece::Knight => 'n',
            PromotionPiece::Bishop => 'b',
            PromotionPiece::Rook => 'r',
            PromotionPiece::Queen => 'q',
        }
    }

    /// Return the bit pattern for this promotion, shifted to position.
    const fn bits(self) -> u16 {
        (self as u16) << PROMO_SHIFT
    }
}

/// A chess move encoded in 16 bits.
///
/// ```text
/// bits  0-5:  source square      (0-63)
/// bits  6-11: destination square (0-63)
/// bits 12-13: promotion piece    (Knight=0, Bishop=1, Rook=2, Queen=3)
/// bits 14-15: move kind          (Normal=0, Promotion=1, EnPassant=2, Castling=3)
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Move(u16);

impl Move {
    /// Null move sentinel (A1→A1, Normal). Never a legal move.
    pub const NULL: Move = Move(0);

    /// Create a normal (quiet or capture) move.
    pub const fn new(source: Square, dest: Square) -> Move {
        let _ = SRC_SHIFT; // suppress unused-constant lint
        Move((source.index() as u16) | ((dest.index() as u16) << DST_SHIFT))
    }

    /// Create a promotion move.
    pub const fn new_promotion(source: Square, dest: Square, promo: PromotionPiece) -> Move {
        Move(
            (source.index() as u16)
                | ((dest.index() as u16) << DST_SHIFT)
                | promo.bits()
                | MoveKind::Promotion.bits(),
        )
    }

    /// Create an en passant capture.
    pub const fn new_en_passant(source: Square, dest: Square) -> Move {
        Move(
            (source.index() as u16)
                | ((dest.index() as u16) << DST_SHIFT)
                | MoveKind::EnPassant.bits(),
        )
    }

    /// Create a castling move using the king's source and destination squares.
    pub const fn new_castle(king_src: Square, king_dst: Square) -> Move {
        Move(
            (king_src.index() as u16)
                | ((king_dst.index() as u16) << DST_SHIFT)
                | MoveKind::Castling.bits(),
        )
    }

    /// Extract the source square.
    pub const fn source(self) -> Square {
        Square::from_index_unchecked((self.0 & SRC_MASK) as u8)
    }

    /// Extract the destination square.
    pub const fn dest(self) -> Square {
        Square::from_index_unchecked(((self.0 & DST_MASK) >> DST_SHIFT) as u8)
    }

    /// Extract the move kind.
    pub const fn kind(self) -> MoveKind {
        match (self.0 & KIND_MASK) >> KIND_SHIFT {
            0 => MoveKind::Normal,
            1 => MoveKind::Promotion,
            2 => MoveKind::EnPassant,
            _ => MoveKind::Castling,
        }
    }

    /// Extract the promotion piece.
    ///
    /// Only meaningful when `kind() == MoveKind::Promotion`.
    pub const fn promotion_piece(self) -> PromotionPiece {
        match (self.0 & PROMO_MASK) >> PROMO_SHIFT {
            0 => PromotionPiece::Knight,
            1 => PromotionPiece::Bishop,
            2 => PromotionPiece::Rook,
            _ => PromotionPiece::Queen,
        }
    }

    /// Return `true` if this is the null move sentinel.
    pub const fn is_null(self) -> bool {
        self.0 == 0
    }

    /// Return `true` if this is a promotion move.
    pub const fn is_promotion(self) -> bool {
        (self.0 & KIND_MASK) >> KIND_SHIFT == MoveKind::Promotion as u16
    }

    /// Return `true` if this is an en passant capture.
    pub const fn is_en_passant(self) -> bool {
        (self.0 & KIND_MASK) >> KIND_SHIFT == MoveKind::EnPassant as u16
    }

    /// Return `true` if this is a castling move.
    pub const fn is_castle(self) -> bool {
        (self.0 & KIND_MASK) >> KIND_SHIFT == MoveKind::Castling as u16
    }

    /// Return `true` if this is a normal (quiet or capture) move.
    pub const fn is_quiet(self) -> bool {
        (self.0 & KIND_MASK) >> KIND_SHIFT == MoveKind::Normal as u16
    }

    /// Return the UCI string representation.
    ///
    /// # Panics
    ///
    /// Debug-asserts that the move is not null.
    pub fn to_uci(self) -> String {
        debug_assert!(!self.is_null(), "to_uci called on null move");
        if self.is_promotion() {
            format!("{}{}{}", self.source(), self.dest(), self.promotion_piece().uci_char())
        } else {
            format!("{}{}", self.source(), self.dest())
        }
    }
}

impl fmt::Display for Move {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_null() {
            write!(f, "0000")
        } else if self.is_promotion() {
            write!(f, "{}{}{}", self.source(), self.dest(), self.promotion_piece().uci_char())
        } else {
            write!(f, "{}{}", self.source(), self.dest())
        }
    }
}

impl fmt::Debug for Move {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Move({} kind={:?})", self, self.kind())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{Move, MoveKind, PromotionPiece};
    use crate::piece_kind::PieceKind;
    use crate::square::Square;

    #[test]
    fn size_of_move() {
        assert_eq!(std::mem::size_of::<Move>(), 2);
    }

    #[test]
    fn normal_move_roundtrip() {
        let mv = Move::new(Square::E2, Square::E4);
        assert_eq!(mv.source(), Square::E2);
        assert_eq!(mv.dest(), Square::E4);
        assert_eq!(mv.kind(), MoveKind::Normal);
        assert!(mv.is_quiet());
        assert!(!mv.is_promotion());
        assert!(!mv.is_en_passant());
        assert!(!mv.is_castle());
        assert!(!mv.is_null());
    }

    #[test]
    fn edge_squares() {
        let mv1 = Move::new(Square::A1, Square::H8);
        assert_eq!(mv1.source(), Square::A1);
        assert_eq!(mv1.dest(), Square::H8);
        assert_eq!(mv1.kind(), MoveKind::Normal);

        let mv2 = Move::new(Square::H1, Square::A8);
        assert_eq!(mv2.source(), Square::H1);
        assert_eq!(mv2.dest(), Square::A8);
        assert_eq!(mv2.kind(), MoveKind::Normal);
    }

    #[test]
    fn promotion_all_pieces() {
        for promo in PromotionPiece::ALL {
            let mv = Move::new_promotion(Square::E7, Square::E8, promo);
            assert_eq!(mv.source(), Square::E7);
            assert_eq!(mv.dest(), Square::E8);
            assert_eq!(mv.kind(), MoveKind::Promotion);
            assert_eq!(mv.promotion_piece(), promo);
            assert!(mv.is_promotion());
            assert!(!mv.is_quiet());
        }
    }

    #[test]
    fn promotion_a_file() {
        let mv = Move::new_promotion(Square::A7, Square::A8, PromotionPiece::Queen);
        assert_eq!(mv.source(), Square::A7);
        assert_eq!(mv.dest(), Square::A8);
        assert_eq!(mv.kind(), MoveKind::Promotion);
        assert_eq!(mv.promotion_piece(), PromotionPiece::Queen);
    }

    #[test]
    fn promotion_h_file() {
        let mv = Move::new_promotion(Square::H7, Square::H8, PromotionPiece::Rook);
        assert_eq!(mv.source(), Square::H7);
        assert_eq!(mv.dest(), Square::H8);
        assert_eq!(mv.kind(), MoveKind::Promotion);
        assert_eq!(mv.promotion_piece(), PromotionPiece::Rook);
    }

    #[test]
    fn en_passant_roundtrip() {
        let mv = Move::new_en_passant(Square::E5, Square::D6);
        assert_eq!(mv.source(), Square::E5);
        assert_eq!(mv.dest(), Square::D6);
        assert_eq!(mv.kind(), MoveKind::EnPassant);
        assert!(mv.is_en_passant());
        assert!(!mv.is_quiet());
        assert!(!mv.is_promotion());
        assert!(!mv.is_castle());
        assert!(!mv.is_null());
    }

    #[test]
    fn castling_all_four() {
        let cases = [
            (Square::E1, Square::G1),
            (Square::E1, Square::C1),
            (Square::E8, Square::G8),
            (Square::E8, Square::C8),
        ];
        for (src, dst) in cases {
            let mv = Move::new_castle(src, dst);
            assert_eq!(mv.source(), src);
            assert_eq!(mv.dest(), dst);
            assert_eq!(mv.kind(), MoveKind::Castling);
            assert!(mv.is_castle());
            assert!(!mv.is_quiet());
            assert!(!mv.is_promotion());
            assert!(!mv.is_en_passant());
        }
    }

    #[test]
    fn null_move() {
        let mv = Move::NULL;
        assert!(mv.is_null());
        assert_eq!(mv.source(), Square::A1);
        assert_eq!(mv.dest(), Square::A1);
        assert_eq!(mv.kind(), MoveKind::Normal);
    }

    #[test]
    fn uci_normal() {
        assert_eq!(Move::new(Square::E2, Square::E4).to_uci(), "e2e4");
    }

    #[test]
    fn uci_promotion() {
        let mv = Move::new_promotion(Square::E7, Square::E8, PromotionPiece::Queen);
        assert_eq!(mv.to_uci(), "e7e8q");
    }

    #[test]
    fn display_null() {
        assert_eq!(format!("{}", Move::NULL), "0000");
    }

    #[test]
    fn debug_contains_kind() {
        let mv = Move::new(Square::D2, Square::D4);
        let debug_str = format!("{:?}", mv);
        assert!(debug_str.contains("d2d4"), "debug should contain UCI: {debug_str}");
        assert!(debug_str.contains("Normal"), "debug should contain kind name: {debug_str}");
    }

    #[test]
    fn equality_and_hash() {
        let mv1 = Move::new(Square::E2, Square::E4);
        let mv2 = Move::new(Square::E2, Square::E4);
        let mv3 = Move::new(Square::D2, Square::D4);

        assert_eq!(mv1, mv2);
        assert_ne!(mv1, mv3);

        let mut set = HashSet::new();
        set.insert(mv1);
        set.insert(mv2);
        assert_eq!(set.len(), 1);
        set.insert(mv3);
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn promotion_piece_to_piece_kind() {
        assert_eq!(PromotionPiece::Knight.to_piece_kind(), PieceKind::Knight);
        assert_eq!(PromotionPiece::Bishop.to_piece_kind(), PieceKind::Bishop);
        assert_eq!(PromotionPiece::Rook.to_piece_kind(), PieceKind::Rook);
        assert_eq!(PromotionPiece::Queen.to_piece_kind(), PieceKind::Queen);
    }

    #[test]
    fn exhaustive_normal_roundtrip() {
        for src in 0u8..64 {
            for dst in 0u8..64 {
                let src_sq = Square::from_index(src).unwrap();
                let dst_sq = Square::from_index(dst).unwrap();
                let mv = Move::new(src_sq, dst_sq);
                assert_eq!(mv.source(), src_sq, "source mismatch for {src}→{dst}");
                assert_eq!(mv.dest(), dst_sq, "dest mismatch for {src}→{dst}");
                assert_eq!(mv.kind(), MoveKind::Normal, "kind mismatch for {src}→{dst}");
            }
        }
    }
}
