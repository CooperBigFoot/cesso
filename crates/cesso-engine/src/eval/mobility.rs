//! Piece mobility evaluation for HCE (Handcrafted Evaluation).
//!
//! Mobility measures how many safe squares each piece can reach. Pieces with
//! greater freedom of movement receive a bonus proportional to their mobility.
//! Safe squares exclude friendly-occupied squares and squares controlled by
//! enemy pawns.

use cesso_core::{bishop_attacks, knight_attacks, queen_attacks, rook_attacks};
use cesso_core::{Bitboard, Board, Color, PieceKind};

use crate::eval::score::{Score, S};

// ---------------------------------------------------------------------------
// Mobility bonus tables
// ---------------------------------------------------------------------------

/// Per-square mobility bonus for knights.
const KNIGHT_MOBILITY: Score = S(4, 4);

/// Per-square mobility bonus for bishops.
const BISHOP_MOBILITY: Score = S(3, 5);

/// Per-square mobility bonus for rooks.
const ROOK_MOBILITY: Score = S(2, 3);

/// Per-square mobility bonus for queens.
const QUEEN_MOBILITY: Score = S(1, 2);

// ---------------------------------------------------------------------------
// Helper: bulk pawn attack span
// ---------------------------------------------------------------------------

/// Compute all squares attacked by pawns of the given color.
///
/// Uses bitboard shifts for O(1) bulk computation instead of per-pawn
/// iteration. Masks out wraparound across the A and H files.
///
/// - White pawns attack NE (`<< 9`, not FILE_A) and NW (`<< 7`, not FILE_H).
/// - Black pawns attack SE (`>> 7`, not FILE_A) and SW (`>> 9`, not FILE_H).
fn pawn_attack_span(pawns: Bitboard, color: Color) -> Bitboard {
    match color {
        Color::White => {
            let ne = (pawns << 9u8) & !Bitboard::FILE_A;
            let nw = (pawns << 7u8) & !Bitboard::FILE_H;
            ne | nw
        }
        Color::Black => {
            let se = (pawns >> 7u8) & !Bitboard::FILE_A;
            let sw = (pawns >> 9u8) & !Bitboard::FILE_H;
            se | sw
        }
    }
}

// ---------------------------------------------------------------------------
// Per-side evaluation
// ---------------------------------------------------------------------------

/// Evaluate piece mobility for one side, returning the raw mobility score.
///
/// Counts safe squares reachable by each knight, bishop, rook, and queen.
/// Safe squares exclude squares occupied by friendly pieces and squares
/// attacked by enemy pawns.
fn evaluate_mobility_for_side(board: &Board, color: Color) -> Score {
    let occupied = board.occupied();
    let friendly = board.side(color);
    let enemy_pawns = board.pieces(PieceKind::Pawn) & board.side(!color);
    let enemy_pawn_attacks = pawn_attack_span(enemy_pawns, !color);
    let safe = !friendly & !enemy_pawn_attacks;

    let mut score = Score::ZERO;

    let knights = board.pieces(PieceKind::Knight) & friendly;
    for sq in knights {
        let attacks = knight_attacks(sq) & safe;
        score += KNIGHT_MOBILITY * attacks.count() as i16;
    }

    let bishops = board.pieces(PieceKind::Bishop) & friendly;
    for sq in bishops {
        let attacks = bishop_attacks(sq, occupied) & safe;
        score += BISHOP_MOBILITY * attacks.count() as i16;
    }

    let rooks = board.pieces(PieceKind::Rook) & friendly;
    for sq in rooks {
        let attacks = rook_attacks(sq, occupied) & safe;
        score += ROOK_MOBILITY * attacks.count() as i16;
    }

    let queens = board.pieces(PieceKind::Queen) & friendly;
    for sq in queens {
        let attacks = queen_attacks(sq, occupied) & safe;
        score += QUEEN_MOBILITY * attacks.count() as i16;
    }

    score
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Evaluate piece mobility from White's perspective.
///
/// For each side, counts the number of safe squares each piece (knight,
/// bishop, rook, queen) can access. Safe squares exclude squares occupied by
/// friendly pieces and squares attacked by enemy pawns. Returns the difference
/// `white_mobility - black_mobility`.
pub fn evaluate_mobility(board: &Board) -> Score {
    evaluate_mobility_for_side(board, Color::White)
        - evaluate_mobility_for_side(board, Color::Black)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use cesso_core::Board;

    use super::evaluate_mobility;

    /// At the starting position both sides have identical piece placement and
    /// mobility constraints. All pieces except the two knights are completely
    /// blocked by pawns, so White and Black each contribute the same knight
    /// mobility. The net result is exactly zero.
    #[test]
    fn starting_position_is_zero() {
        let board = Board::starting_position();
        let score = evaluate_mobility(&board);
        assert_eq!(score.mg(), 0, "mg mobility should be 0 in starting position");
        assert_eq!(score.eg(), 0, "eg mobility should be 0 in starting position");
    }

    /// After 1.e4 Nf6 (`rnbqkb1r/pppppppp/5n2/8/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 1 2`),
    /// White has moved a pawn to e4, opening lines for the bishop and queen.
    /// White should have a positive (or at least non-negative) mobility advantage.
    #[test]
    fn after_e4_nf6_white_advantage() {
        let board: Board = "rnbqkb1r/pppppppp/5n2/8/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 1 2"
            .parse()
            .unwrap();
        let score = evaluate_mobility(&board);
        // White's bishop and queen gain safe squares after e4; Black's knight
        // on f6 is active but White's overall mobility should be at least as
        // good. In practice the opened diagonals give White a clear edge.
        assert!(
            score.mg() > 0,
            "expected positive mg mobility for White after 1.e4 Nf6, got {}",
            score.mg()
        );
    }

    /// A position with a fully open board for White's rook gives a large
    /// positive mobility score. We use a rook endgame where White has a
    /// centralized rook and Black's rook is trapped on the back rank.
    #[test]
    fn open_rook_gives_positive_score() {
        // White rook on e4 (open file/rank), Black rook on a8 (constrained).
        // Both kings are present; no pawns so no pawn attack penalties.
        let board: Board = "r3k3/8/8/8/4R3/8/8/4K3 w - - 0 1"
            .parse()
            .unwrap();
        let score = evaluate_mobility(&board);
        assert!(
            score.mg() > 0,
            "White's centralized rook should yield positive mobility (got {})",
            score.mg()
        );
    }
}
