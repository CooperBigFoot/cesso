//! Material balance evaluation.
//!
//! Counts weighted piece material for each side and adds a bishop-pair bonus.
//! All scores are returned from White's perspective (positive = White ahead).

use cesso_core::{Board, Color, PieceKind};

use crate::eval::score::{Score, S};

/// Base material values indexed by [`PieceKind::index()`].
///
/// | Piece  | mg  | eg  |
/// |--------|-----|-----|
/// | Pawn   | 100 | 120 |
/// | Knight | 320 | 310 |
/// | Bishop | 330 | 320 |
/// | Rook   | 500 | 520 |
/// | Queen  | 900 | 950 |
/// | King   |   0 |   0 |
pub const MATERIAL_VALUE: [Score; PieceKind::COUNT] = [
    S(100, 120), // Pawn
    S(320, 310), // Knight
    S(330, 320), // Bishop
    S(500, 520), // Rook
    S(900, 950), // Queen
    S(0, 0),     // King
];

/// Bonus awarded to a side that has two or more bishops.
const BISHOP_PAIR_BONUS: Score = S(50, 60);

/// Evaluate material balance from White's perspective.
///
/// For each piece kind the function counts White pieces and Black pieces,
/// accumulates `MATERIAL_VALUE[kind] * (white_count - black_count)`, then
/// applies a [`BISHOP_PAIR_BONUS`] if either side owns two or more bishops.
///
/// Returns a positive score when White has more material, negative when Black does.
pub fn material(board: &Board) -> Score {
    let mut score = Score::ZERO;

    for kind in PieceKind::ALL {
        let piece_bb = board.pieces(kind);
        let white_count = (piece_bb & board.side(Color::White)).count() as i16;
        let black_count = (piece_bb & board.side(Color::Black)).count() as i16;
        score += MATERIAL_VALUE[kind.index()] * (white_count - black_count);
    }

    // Bishop pair bonus
    let white_bishops = (board.pieces(PieceKind::Bishop) & board.side(Color::White)).count();
    let black_bishops = (board.pieces(PieceKind::Bishop) & board.side(Color::Black)).count();

    if white_bishops >= 2 {
        score += BISHOP_PAIR_BONUS;
    }
    if black_bishops >= 2 {
        score -= BISHOP_PAIR_BONUS;
    }

    score
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use cesso_core::Board;

    use super::{material, BISHOP_PAIR_BONUS, MATERIAL_VALUE};
    use crate::eval::score::{Score, S};
    use cesso_core::PieceKind;

    #[test]
    fn starting_position_is_zero() {
        let board = Board::starting_position();
        // Both sides have identical material, so the balance is zero.
        // However, both sides also have 2 bishops each, so the bishop-pair
        // bonuses cancel and the result is still zero.
        assert_eq!(material(&board), Score::ZERO);
    }

    #[test]
    fn missing_black_queen_gives_queen_advantage() {
        // Black is missing the queen on d8.
        let board = "rnb1kbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
            .parse::<Board>()
            .unwrap();
        let score = material(&board);
        // White has 1 extra queen; both sides still have 2 bishops so the
        // bishop-pair bonuses cancel.
        let queen_value = MATERIAL_VALUE[PieceKind::Queen.index()];
        assert_eq!(score, queen_value);
    }

    #[test]
    fn missing_black_queen_mg_eg() {
        let board = "rnb1kbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
            .parse::<Board>()
            .unwrap();
        let score = material(&board);
        assert_eq!(score.mg(), 900);
        assert_eq!(score.eg(), 950);
    }

    #[test]
    fn bishop_pair_bonus_white_only() {
        // White has 2 bishops; Black has lost both. Both sides still have
        // symmetric other material, so the only difference is the bishop-pair
        // bonus for White and the two extra bishops White has.
        //
        // FEN: Starting position minus both Black bishops (c8 and f8).
        // "rn1qk1nr" keeps Black's queen on d8 and removes only the bishops.
        let board = "rn1qk1nr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
            .parse::<Board>()
            .unwrap();
        let score = material(&board);
        // White has 2 extra bishops + the bishop-pair bonus; Black has none.
        let bishop_value = MATERIAL_VALUE[PieceKind::Bishop.index()];
        let expected = bishop_value * 2 + BISHOP_PAIR_BONUS;
        assert_eq!(score, expected);
    }

    #[test]
    fn bishop_pair_bonus_both_sides_cancels() {
        // Starting position: both sides have 2 bishops, so bonuses cancel.
        let board = Board::starting_position();
        let score = material(&board);
        // Material is balanced; bishop-pair bonuses cancel.
        assert_eq!(score, Score::ZERO);
    }

    #[test]
    fn extra_white_rook() {
        // White has an extra rook compared to Black.
        // FEN: remove one Black rook (a8).
        let board = "1nbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
            .parse::<Board>()
            .unwrap();
        let score = material(&board);
        let rook_value = MATERIAL_VALUE[PieceKind::Rook.index()];
        assert_eq!(score, rook_value);
    }

    #[test]
    fn score_is_negated_when_black_is_ahead() {
        // Black has an extra queen; result should be negative (Black ahead).
        let board = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNB1KBNR w KQkq - 0 1"
            .parse::<Board>()
            .unwrap();
        let score = material(&board);
        let queen_value = MATERIAL_VALUE[PieceKind::Queen.index()];
        // Black is up a queen: White minus Black = -queen_value
        assert_eq!(score, -queen_value);
    }

    #[test]
    fn material_value_table_king_is_zero() {
        assert_eq!(MATERIAL_VALUE[PieceKind::King.index()], S(0, 0));
    }
}
