//! Game phase calculation based on remaining non-pawn material.

use cesso_core::{Board, PieceKind};

/// Maximum game phase value, corresponding to a full starting-position complement
/// of non-pawn material.
///
/// Weights: Knight=1, Bishop=1, Rook=2, Queen=4.
/// Starting totals: 4×1 + 4×1 + 4×2 + 2×4 = 24.
pub const MAX_PHASE: i32 = 24;

/// Calculate the game phase from non-pawn, non-king material on the board.
///
/// Returns a value in `0..=MAX_PHASE`. A value of [`MAX_PHASE`] indicates a
/// full middlegame material set; 0 indicates a pure king-and-pawn ending.
/// The result is clamped so that promoted pieces cannot push the phase above
/// the maximum.
///
/// # Phase weights
///
/// | Piece  | Weight |
/// |--------|--------|
/// | Knight | 1      |
/// | Bishop | 1      |
/// | Rook   | 2      |
/// | Queen  | 4      |
pub fn game_phase(board: &Board) -> i32 {
    let knights = board.pieces(PieceKind::Knight).count() as i32;
    let bishops = board.pieces(PieceKind::Bishop).count() as i32;
    let rooks = board.pieces(PieceKind::Rook).count() as i32;
    let queens = board.pieces(PieceKind::Queen).count() as i32;

    let phase = knights * 1 + bishops * 1 + rooks * 2 + queens * 4;
    phase.min(MAX_PHASE)
}

#[cfg(test)]
mod tests {
    use cesso_core::Board;

    use super::{game_phase, MAX_PHASE};

    #[test]
    fn starting_position_is_max_phase() {
        let board = Board::starting_position();
        assert_eq!(game_phase(&board), MAX_PHASE);
    }

    #[test]
    fn bare_kings_is_zero_phase() {
        let board = "8/8/4k3/8/8/4K3/8/8 w - - 0 1"
            .parse::<Board>()
            .unwrap();
        assert_eq!(game_phase(&board), 0);
    }

    #[test]
    fn missing_one_queen_is_20() {
        // Starting position minus one queen (Black's queen on d8 is absent).
        let board = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNB1KBNR w KQkq - 0 1"
            .parse::<Board>()
            .unwrap();
        // Missing one queen (weight 4): 24 - 4 = 20.
        assert_eq!(game_phase(&board), 20);
    }
}
