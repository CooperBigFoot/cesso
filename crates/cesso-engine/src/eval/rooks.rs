//! Rook evaluation: open files, semi-open files, and rook on 7th rank.

use cesso_core::{Bitboard, Board, Color, PieceKind};

use crate::eval::score::{Score, S};

/// Bonus for a rook on a fully open file (no pawns of either color).
const ROOK_OPEN_FILE: Score = S(25, 15);

/// Bonus for a rook on a semi-open file (no friendly pawns, but enemy pawns present).
const ROOK_SEMI_OPEN_FILE: Score = S(15, 10);

/// Bonus for a rook on the 7th rank (2nd rank from the enemy's perspective).
const ROOK_ON_SEVENTH: Score = S(20, 30);

/// Evaluate rook placement for one side.
fn evaluate_rooks_for_side(board: &Board, color: Color) -> Score {
    let rooks = board.pieces(PieceKind::Rook) & board.side(color);
    let all_pawns = board.pieces(PieceKind::Pawn);
    let friendly_pawns = all_pawns & board.side(color);

    let seventh_rank = match color {
        Color::White => Bitboard::RANK_7,
        Color::Black => Bitboard::RANK_2,
    };

    let mut score = Score::ZERO;

    for sq in rooks {
        let file = sq.file();
        let file_mask = Bitboard::file_mask(file);

        // Open file: no pawns at all
        if (file_mask & all_pawns).is_empty() {
            score += ROOK_OPEN_FILE;
        }
        // Semi-open file: no friendly pawns but enemy pawns present
        else if (file_mask & friendly_pawns).is_empty() {
            score += ROOK_SEMI_OPEN_FILE;
        }

        // Rook on 7th rank
        if (sq.bitboard() & seventh_rank).is_nonempty() {
            score += ROOK_ON_SEVENTH;
        }
    }

    score
}

/// Evaluate rook placement from White's perspective.
pub fn evaluate_rooks(board: &Board) -> Score {
    evaluate_rooks_for_side(board, Color::White) - evaluate_rooks_for_side(board, Color::Black)
}

#[cfg(test)]
mod tests {
    use cesso_core::Board;

    use super::evaluate_rooks;

    #[test]
    fn starting_position_is_zero() {
        let board = Board::starting_position();
        let score = evaluate_rooks(&board);
        assert_eq!(score.mg(), 0);
        assert_eq!(score.eg(), 0);
    }

    #[test]
    fn rook_on_open_file() {
        // White rook on e1, no pawns on e-file
        let board: Board = "4k3/pppp1ppp/8/8/8/8/PPPP1PPP/4RK2 w - - 0 1".parse().unwrap();
        let score = evaluate_rooks(&board);
        // White has rook on open file, black has no rooks
        assert!(score.mg() > 0, "rook on open file should be positive, got {}", score.mg());
    }

    #[test]
    fn rook_on_seventh() {
        // White rook on d7
        let board: Board = "4k3/3R4/8/8/8/8/8/4K3 w - - 0 1".parse().unwrap();
        let score = evaluate_rooks(&board);
        assert!(score.mg() > 0, "rook on 7th should be positive, got {}", score.mg());
    }
}
