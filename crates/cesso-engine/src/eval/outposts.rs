//! Knight and bishop outpost evaluation.
//!
//! An outpost is a square on ranks 4-6 (from the piece's perspective) that
//! cannot be attacked by enemy pawns (no enemy pawns on adjacent files that
//! could advance to attack the square).

use cesso_core::{Bitboard, Board, Color, File, PieceKind, Square, pawn_attacks};

use crate::eval::pawns::PASSED_PAWN_MASK;
use crate::eval::score::{Score, S};

/// Bonus for a knight on an outpost.
const KNIGHT_OUTPOST: Score = S(20, 15);

/// Bonus for a knight on an outpost supported by a friendly pawn.
const KNIGHT_OUTPOST_SUPPORTED: Score = S(30, 20);

/// Bonus for a bishop on an outpost.
const BISHOP_OUTPOST: Score = S(10, 8);

/// Bonus for a bishop on an outpost supported by a friendly pawn.
const BISHOP_OUTPOST_SUPPORTED: Score = S(18, 12);

/// Outpost-eligible ranks from each side's perspective.
///
/// White: ranks 4-6 (indices 3-5), Black: ranks 3-5 (indices 2-4).
fn outpost_ranks(color: Color) -> Bitboard {
    match color {
        Color::White => Bitboard::RANK_4 | Bitboard::RANK_5 | Bitboard::RANK_6,
        Color::Black => Bitboard::RANK_3 | Bitboard::RANK_4 | Bitboard::RANK_5,
    }
}

/// Check if a square is an outpost for the given color.
///
/// A square is an outpost if no enemy pawn can advance to attack it.
/// Uses the passed pawn mask on adjacent files from the piece's color perspective
/// to check if any enemy pawn could reach a square that attacks this one.
fn is_outpost(sq: Square, color: Color, enemy_pawns: Bitboard) -> bool {
    let mask = PASSED_PAWN_MASK[color.index()][sq.index()];

    // Only care about adjacent files (not same file) since pawns attack diagonally
    let file_idx = sq.file().index();
    let mut adj_files = Bitboard::EMPTY;
    if file_idx > 0 {
        if let Some(f) = File::from_index(file_idx as u8 - 1) {
            adj_files = adj_files | Bitboard::file_mask(f);
        }
    }
    if file_idx < 7 {
        if let Some(f) = File::from_index(file_idx as u8 + 1) {
            adj_files = adj_files | Bitboard::file_mask(f);
        }
    }

    let relevant_mask = mask & adj_files;
    (relevant_mask & enemy_pawns).is_empty()
}

/// Evaluate outposts for one side.
fn evaluate_outposts_for_side(board: &Board, color: Color) -> Score {
    let friendly = board.side(color);
    let friendly_pawns = board.pieces(PieceKind::Pawn) & friendly;
    let enemy_pawns = board.pieces(PieceKind::Pawn) & board.side(!color);
    let eligible = outpost_ranks(color);

    let mut score = Score::ZERO;

    // Knights on outposts
    let knights = board.pieces(PieceKind::Knight) & friendly & eligible;
    for sq in knights {
        if is_outpost(sq, color, enemy_pawns) {
            // Check if supported by a friendly pawn
            let supported = (pawn_attacks(!color, sq) & friendly_pawns).is_nonempty();
            if supported {
                score += KNIGHT_OUTPOST_SUPPORTED;
            } else {
                score += KNIGHT_OUTPOST;
            }
        }
    }

    // Bishops on outposts
    let bishops = board.pieces(PieceKind::Bishop) & friendly & eligible;
    for sq in bishops {
        if is_outpost(sq, color, enemy_pawns) {
            let supported = (pawn_attacks(!color, sq) & friendly_pawns).is_nonempty();
            if supported {
                score += BISHOP_OUTPOST_SUPPORTED;
            } else {
                score += BISHOP_OUTPOST;
            }
        }
    }

    score
}

/// Evaluate outposts from White's perspective.
pub fn evaluate_outposts(board: &Board) -> Score {
    evaluate_outposts_for_side(board, Color::White) - evaluate_outposts_for_side(board, Color::Black)
}

#[cfg(test)]
mod tests {
    use cesso_core::Board;

    use super::evaluate_outposts;

    #[test]
    fn starting_position_is_zero() {
        let board = Board::starting_position();
        let score = evaluate_outposts(&board);
        assert_eq!(score.mg(), 0);
        assert_eq!(score.eg(), 0);
    }

    #[test]
    fn knight_outpost_positive() {
        // White knight on d5, no black pawns on c or e files that could attack d5
        // White pawn on e4 supports the knight
        let board: Board = "4k3/pp1p1ppp/8/3N4/4P3/8/PPP2PPP/4K3 w - - 0 1"
            .parse()
            .unwrap();
        let score = evaluate_outposts(&board);
        assert!(score.mg() > 0, "knight outpost should give positive score, got {}", score.mg());
    }
}
