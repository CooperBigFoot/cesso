//! Attack generation: precomputed tables for all piece types.

mod magic;
mod magic_data;
mod tables;

use crate::bitboard::Bitboard;
use crate::color::Color;
use crate::square::Square;

use self::magic::{bishop_attacks_lookup, rook_attacks_lookup};
use self::tables::{BETWEEN, KING_ATTACKS, KNIGHT_ATTACKS, LINE, PAWN_ATTACKS};

/// Return the squares a knight on `sq` attacks.
#[inline]
pub fn knight_attacks(sq: Square) -> Bitboard {
    KNIGHT_ATTACKS[sq.index()]
}

/// Return the squares a king on `sq` attacks.
#[inline]
pub fn king_attacks(sq: Square) -> Bitboard {
    KING_ATTACKS[sq.index()]
}

/// Return the squares a pawn of `color` on `sq` attacks.
#[inline]
pub fn pawn_attacks(color: Color, sq: Square) -> Bitboard {
    PAWN_ATTACKS[color.index()][sq.index()]
}

/// Return rook attacks from `sq` given `occupied` squares.
#[inline]
pub fn rook_attacks(sq: Square, occupied: Bitboard) -> Bitboard {
    rook_attacks_lookup(sq.index(), occupied)
}

/// Return bishop attacks from `sq` given `occupied` squares.
#[inline]
pub fn bishop_attacks(sq: Square, occupied: Bitboard) -> Bitboard {
    bishop_attacks_lookup(sq.index(), occupied)
}

/// Return queen attacks from `sq` given `occupied` squares.
#[inline]
pub fn queen_attacks(sq: Square, occupied: Bitboard) -> Bitboard {
    rook_attacks(sq, occupied) | bishop_attacks(sq, occupied)
}

/// Return squares strictly between `sq1` and `sq2` (exclusive of both endpoints).
///
/// Returns an empty bitboard if the two squares are not on the same rank, file,
/// or diagonal.
#[inline]
pub fn between(sq1: Square, sq2: Square) -> Bitboard {
    BETWEEN[sq1.index()][sq2.index()]
}

/// Return all squares on the line through `sq1` and `sq2`, including both endpoints
/// and extending to the board edges.
///
/// Returns an empty bitboard if the two squares are not on the same rank, file,
/// or diagonal.
#[inline]
pub fn line(sq1: Square, sq2: Square) -> Bitboard {
    LINE[sq1.index()][sq2.index()]
}

#[cfg(test)]
mod tests {
    use super::magic;
    use super::*;
    use crate::bitboard::Bitboard;
    use crate::color::Color;
    use crate::square::Square;

    // --- Leaper spot checks ---

    #[test]
    fn knight_e4_has_8_attacks() {
        assert_eq!(knight_attacks(Square::E4).count(), 8);
    }

    #[test]
    fn knight_a1_has_2_attacks() {
        assert_eq!(knight_attacks(Square::A1).count(), 2);
    }

    #[test]
    fn king_e1_has_5_attacks() {
        assert_eq!(king_attacks(Square::E1).count(), 5);
    }

    #[test]
    fn king_e4_has_8_attacks() {
        assert_eq!(king_attacks(Square::E4).count(), 8);
    }

    #[test]
    fn pawn_no_wrap_file_a() {
        // White pawn on A4 should attack B5 only (not wrap to H-file).
        let attacks = pawn_attacks(Color::White, Square::A4);
        assert_eq!(attacks.count(), 1);
        assert!(attacks.contains(Square::B5));
    }

    #[test]
    fn pawn_no_wrap_file_h() {
        let attacks = pawn_attacks(Color::White, Square::H4);
        assert_eq!(attacks.count(), 1);
        assert!(attacks.contains(Square::G5));
    }

    #[test]
    fn pawn_black_attacks_south() {
        let attacks = pawn_attacks(Color::Black, Square::E5);
        assert_eq!(attacks.count(), 2);
        assert!(attacks.contains(Square::D4));
        assert!(attacks.contains(Square::F4));
    }

    // --- Sliding piece on empty board ---

    #[test]
    fn rook_empty_board_always_14() {
        for sq in Square::all() {
            assert_eq!(
                rook_attacks(sq, Bitboard::EMPTY).count(),
                14,
                "rook on {} should have 14 attacks on empty board",
                sq
            );
        }
    }

    #[test]
    fn bishop_d4_empty_board_13() {
        assert_eq!(bishop_attacks(Square::D4, Bitboard::EMPTY).count(), 13);
    }

    // --- Blocker test ---

    #[test]
    fn rook_e4_blocked_e6() {
        let occupied = Square::E6.bitboard();
        let attacks = rook_attacks(Square::E4, occupied);
        assert!(attacks.contains(Square::E5));
        assert!(attacks.contains(Square::E6)); // blocker square included
        assert!(!attacks.contains(Square::E7)); // blocked beyond
    }

    // --- BETWEEN / LINE ---

    #[test]
    fn between_e1_e4() {
        let bb = between(Square::E1, Square::E4);
        assert_eq!(bb.count(), 2);
        assert!(bb.contains(Square::E2));
        assert!(bb.contains(Square::E3));
    }

    #[test]
    fn between_a1_h8() {
        let bb = between(Square::A1, Square::H8);
        assert_eq!(bb.count(), 6); // B2..G7
    }

    #[test]
    fn between_non_aligned_empty() {
        let bb = between(Square::A1, Square::B3);
        assert!(bb.is_empty());
    }

    #[test]
    fn line_a1_h8() {
        let bb = line(Square::A1, Square::H8);
        assert_eq!(bb.count(), 8); // full main diagonal
    }

    #[test]
    fn line_non_aligned_empty() {
        let bb = line(Square::A1, Square::B3);
        assert!(bb.is_empty());
    }

    // --- Cross-validation: magic lookup vs. on-the-fly ---

    #[test]
    fn rook_magic_vs_naive() {
        let mut rng: u64 = 0xDEADBEEF12345678;
        for sq_idx in 0..64usize {
            let sq = Square::from_index(sq_idx as u8).unwrap();
            for _ in 0..128 {
                // LCG PRNG
                rng = rng
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                let occupied = Bitboard::new(rng);
                let magic_result = rook_attacks(sq, occupied);
                let naive_result =
                    Bitboard::new(magic::rook_attacks_on_the_fly(sq_idx, rng));
                assert_eq!(
                    magic_result, naive_result,
                    "rook mismatch on sq {} with occ {:016x}",
                    sq, rng
                );
            }
        }
    }

    #[test]
    fn bishop_magic_vs_naive() {
        let mut rng: u64 = 0xCAFEBABE87654321;
        for sq_idx in 0..64usize {
            let sq = Square::from_index(sq_idx as u8).unwrap();
            for _ in 0..128 {
                rng = rng
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                let occupied = Bitboard::new(rng);
                let magic_result = bishop_attacks(sq, occupied);
                let naive_result =
                    Bitboard::new(magic::bishop_attacks_on_the_fly(sq_idx, rng));
                assert_eq!(
                    magic_result, naive_result,
                    "bishop mismatch on sq {} with occ {:016x}",
                    sq, rng
                );
            }
        }
    }
}
