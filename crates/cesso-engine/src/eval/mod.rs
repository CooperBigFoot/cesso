//! Hand-crafted evaluation (HCE) with tapered eval.
//!
//! Evaluation terms: material, piece-square tables, pawn structure,
//! piece mobility, and king safety (pawn shield).
//!
//! All individual terms return [`score::Score`] from White's perspective.
//! The orchestrator tapers the combined mg/eg values based on game phase
//! and flips the sign for Black.

pub mod king_safety;
pub mod material;
pub mod mobility;
pub mod pawns;
pub mod phase;
pub mod pst;
pub mod score;

use cesso_core::{Board, Color, PieceKind};

use self::king_safety::evaluate_king_safety;
use self::material::material;
use self::mobility::evaluate_mobility;
use self::pawns::evaluate_pawns;
use self::phase::{game_phase, MAX_PHASE};
use self::pst::pst_value;
use self::score::Score;

/// Evaluate the board position and return a centipawn score from the
/// side-to-move's perspective (positive = good for the side to move).
///
/// The evaluation:
/// 1. Computes all terms from White's perspective as packed [`Score`] values.
/// 2. Tapers the combined mg/eg values using the game phase.
/// 3. Flips the sign when Black is to move.
pub fn evaluate(board: &Board) -> i32 {
    let white_score = evaluate_white(board);
    let phase = game_phase(board);
    let tapered = taper(white_score, phase);

    match board.side_to_move() {
        Color::White => tapered,
        Color::Black => -tapered,
    }
}

/// Taper a packed Score into a single centipawn value using the game phase.
///
/// Formula: `(mg * phase + eg * (MAX_PHASE - phase)) / MAX_PHASE`
fn taper(score: Score, phase: i32) -> i32 {
    let mg = score.mg() as i32;
    let eg = score.eg() as i32;
    (mg * phase + eg * (MAX_PHASE - phase)) / MAX_PHASE
}

/// Compute the total evaluation from White's perspective as a packed Score.
///
/// Sums material, piece-square tables, pawn structure, mobility, and
/// king safety.
fn evaluate_white(board: &Board) -> Score {
    let mut score = Score::ZERO;

    score += material(board);
    score += pst_total(board);
    score += evaluate_pawns(board);
    score += evaluate_mobility(board);
    score += evaluate_king_safety(board);

    score
}

/// Sum piece-square table values for all pieces on the board.
///
/// White pieces contribute positively; Black pieces contribute negatively.
fn pst_total(board: &Board) -> Score {
    let mut score = Score::ZERO;

    for kind in PieceKind::ALL {
        let piece_bb = board.pieces(kind);

        // White pieces — add PST values
        let white_pieces = piece_bb & board.side(Color::White);
        for sq in white_pieces {
            score += pst_value(kind, Color::White, sq);
        }

        // Black pieces — subtract PST values
        let black_pieces = piece_bb & board.side(Color::Black);
        for sq in black_pieces {
            score -= pst_value(kind, Color::Black, sq);
        }
    }

    score
}

#[cfg(test)]
mod tests {
    use cesso_core::Board;
    use super::evaluate;

    /// The starting position is symmetric, so evaluate should return
    /// approximately 0 from White's perspective. Due to PST differences
    /// and mobility, the result may not be exactly 0, but it should be
    /// very close (within ±50 centipawns).
    #[test]
    fn starting_position_near_zero() {
        let board = Board::starting_position();
        let score = evaluate(&board);
        assert!(
            score.abs() <= 50,
            "starting position should be near 0, got {score}"
        );
    }

    /// White with an extra queen should evaluate strongly positive when
    /// it's White to move.
    #[test]
    fn extra_white_queen_is_positive() {
        // Black is missing queen
        let board: Board = "rnb1kbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
            .parse()
            .unwrap();
        let score = evaluate(&board);
        assert!(
            score > 500,
            "extra queen should give White a large advantage, got {score}"
        );
    }

    /// Same position but with Black to move — the returned score should be
    /// the negation (approximately) of the White-to-move version.
    #[test]
    fn extra_white_queen_black_to_move() {
        let board: Board = "rnb1kbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR b KQkq - 0 1"
            .parse()
            .unwrap();
        let score = evaluate(&board);
        assert!(
            score < -500,
            "extra queen for opponent should give Black a negative score, got {score}"
        );
    }

    /// An endgame position should weight endgame values more heavily.
    /// Kings and pawns only: phase should be 0, so eval = pure eg values.
    #[test]
    fn endgame_uses_eg_values() {
        // Kings + pawns only — phase = 0 (pure endgame)
        let board: Board = "4k3/pppppppp/8/8/8/8/PPPPPPPP/4K3 w - - 0 1"
            .parse()
            .unwrap();
        let score = evaluate(&board);
        // Symmetric pawn structure — should be close to 0
        assert!(
            score.abs() <= 50,
            "symmetric KP endgame should be near 0, got {score}"
        );
    }

    /// Test tapering: middlegame position should use mg values more.
    #[test]
    fn taper_function_works() {
        use super::score::S;
        use super::phase::MAX_PHASE;
        use super::taper;

        // Full middlegame: phase = 24, should return mg value
        let s = S(100, 50);
        assert_eq!(taper(s, MAX_PHASE), 100);

        // Pure endgame: phase = 0, should return eg value
        assert_eq!(taper(s, 0), 50);

        // Half phase: (100*12 + 50*12) / 24 = 1800/24 = 75
        assert_eq!(taper(s, 12), 75);
    }
}
