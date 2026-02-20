//! King safety evaluation based on pawn shield coverage.
//!
//! V1 evaluates only the pawn shield directly in front of the king.
//! Each missing pawn in the shield incurs a middlegame-only penalty.

use cesso_core::{Bitboard, Board, Color, PieceKind, Square};

use crate::eval::score::{Score, S};

/// Penalty for each missing pawn in the king's shield (middlegame only).
const MISSING_SHIELD_PAWN_PENALTY: Score = S(-30, 0);

/// Compute the pawn shield mask for a king on the given square.
///
/// The shield consists of the 2-3 squares directly in front of the king
/// (one rank forward from the king's perspective). For kings on edge files,
/// this is 2 squares; otherwise 3.
///
/// For White, "in front" means one rank higher (shift left by 8 bits).
/// For Black, "in front" means one rank lower (shift right by 8 bits).
fn shield_mask(king_sq: Square, color: Color) -> Bitboard {
    let king_bb = king_sq.bitboard();

    let shifted = match color {
        Color::White => king_bb << 8,
        Color::Black => king_bb >> 8,
    };

    if shifted.is_empty() {
        return Bitboard::EMPTY;
    }

    // Expand one file left and right, masking out file-wrap artifacts.
    // Shifting a bitboard by 1 bit moves one file left (toward A) or right (toward H).
    // FILE_A mask prevents a piece on the A file from wrapping to H when shifted right by 1.
    // FILE_H mask prevents a piece on the H file from wrapping to A when shifted left by 1.
    shifted | ((shifted << 1) & !Bitboard::FILE_A) | ((shifted >> 1) & !Bitboard::FILE_H)
}

/// Evaluate king safety (pawn shield) from White's perspective.
///
/// Counts missing friendly pawns in the king's shield for each side.
/// Each missing shield pawn incurs [`MISSING_SHIELD_PAWN_PENALTY`].
///
/// Returns a positive score when Black has the weaker shield, and a negative
/// score when White has the weaker shield.
pub fn evaluate_king_safety(board: &Board) -> Score {
    let mut white_penalty = Score::ZERO;
    let mut black_penalty = Score::ZERO;

    let pawns = board.pieces(PieceKind::Pawn);

    for color in Color::ALL {
        let king_sq = board.king_square(color);
        let shield = shield_mask(king_sq, color);
        let friendly_pawns = pawns & board.side(color);
        let shield_pawns = shield & friendly_pawns;
        let missing = shield.count() - shield_pawns.count();
        let penalty = MISSING_SHIELD_PAWN_PENALTY * missing as i16;

        match color {
            Color::White => white_penalty = penalty,
            Color::Black => black_penalty = penalty,
        }
    }

    white_penalty - black_penalty
}

#[cfg(test)]
mod tests {
    use cesso_core::Board;

    use super::evaluate_king_safety;
    use crate::eval::score::{Score, S};

    #[test]
    fn starting_position_is_zero() {
        // Both kings have full pawn shields in the starting position:
        // White: d2, e2, f2 all occupied. Black: d7, e7, f7 all occupied.
        let board = Board::starting_position();
        assert_eq!(evaluate_king_safety(&board), Score::ZERO);
    }

    #[test]
    fn missing_white_shield_pawn() {
        // White king on g1, pawns on f2 and h2 but not g2.
        // Black king on e8 with d7/e7/f7 all occupied.
        // White shield: f2, g2, h2 — g2 missing = 1 missing pawn.
        // Black shield: d7, e7, f7 — all present = 0 missing.
        // Expected: S(-30, 0) - S(0, 0) = S(-30, 0).
        let board = "4k3/pppppppp/8/8/8/8/PPPPP1PP/6K1 w - - 0 1"
            .parse::<Board>()
            .unwrap();
        assert_eq!(evaluate_king_safety(&board), S(-30, 0));
    }

    #[test]
    fn edge_king_a1_full_shield() {
        // White king on a1. Shield squares are a2 and b2 (2 squares — no square to the left).
        // Both a2 and b2 are occupied. Black king on e8 with full d7/e7/f7 shield.
        // Expected: 0 missing for both sides = S(0, 0).
        let board = "4k3/pppppppp/8/8/8/8/PP6/K7 w - - 0 1"
            .parse::<Board>()
            .unwrap();
        assert_eq!(evaluate_king_safety(&board), Score::ZERO);
    }

    #[test]
    fn symmetry_starting_position() {
        // Both sides have identical king safety in the starting position.
        let board = Board::starting_position();
        let score = evaluate_king_safety(&board);
        assert_eq!(score.mg(), 0);
        assert_eq!(score.eg(), 0);
    }
}
