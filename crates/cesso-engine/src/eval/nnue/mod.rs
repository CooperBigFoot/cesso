//! NNUE evaluation using a (768->1024)x2->1x8 SCReLU network.

mod accumulator;
mod features;
mod network;

use cesso_core::{Board, Color};

use self::accumulator::Accumulator;
use self::network::Network;
use self::network::NUM_BUCKETS;

/// Compute the output bucket index from material count.
///
/// Must match Bullet's `MaterialCount<8>`:
/// `bucket = (occupied_count - 2) / (32.div_ceil(8))` = `(occ - 2) / 4`.
#[inline]
fn output_bucket(board: &Board) -> usize {
    let piece_count = board.occupied().count() as usize;
    (piece_count.saturating_sub(2)) / 4
}

/// Evaluate the board using NNUE.
///
/// Returns a centipawn score from the side-to-move's perspective
/// (positive = good for the side to move).
pub fn evaluate(board: &Board) -> i32 {
    let net = Network::get();
    let bucket = output_bucket(board);

    let white_acc = Accumulator::refresh(board, Color::White, net);
    let black_acc = Accumulator::refresh(board, Color::Black, net);

    let (us, them) = match board.side_to_move() {
        Color::White => (&white_acc, &black_acc),
        Color::Black => (&black_acc, &white_acc),
    };

    net.evaluate(us, them, bucket)
}

#[cfg(test)]
mod tests {
    use cesso_core::{Board, Color, PieceKind, Square};

    use super::evaluate;
    use super::features::feature_index;
    use super::network::Network;
    use super::NUM_BUCKETS;

    /// Network struct size must match the binary file exactly.
    #[test]
    fn network_size_matches_binary() {
        assert_eq!(
            std::mem::size_of::<Network>(),
            1_607_744,
            "Network struct size must match new bucketed binary"
        );
    }

    /// Starting position is symmetric -- NNUE eval should be near zero.
    #[test]
    fn starting_position_near_zero() {
        let board = Board::starting_position();
        let score = evaluate(&board);
        assert!(
            score.abs() <= 100,
            "starting position should be near 0, got {score}"
        );
    }

    /// Missing a queen should produce a large score difference.
    #[test]
    fn material_asymmetry() {
        // White has queen, Black does not
        let with_queen: Board = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
            .parse()
            .unwrap();
        let without_queen: Board = "rnb1kbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
            .parse()
            .unwrap();

        let score_full = evaluate(&with_queen);
        let score_missing = evaluate(&without_queen);

        // White should benefit significantly when Black is missing a queen
        assert!(
            score_missing - score_full > 300,
            "missing queen should cause large score difference, full={score_full}, missing={score_missing}"
        );
    }

    /// All feature indices must be in range [0, 768).
    #[test]
    fn feature_index_bounds() {
        for &perspective in &Color::ALL {
            for &piece_color in &Color::ALL {
                for kind in PieceKind::ALL {
                    for sq in Square::all() {
                        let idx = feature_index(perspective, piece_color, kind, sq);
                        assert!(
                            idx < 768,
                            "feature_index out of bounds: perspective={perspective:?}, \
                             color={piece_color:?}, kind={kind:?}, sq={sq:?}, idx={idx}"
                        );
                    }
                }
            }
        }
    }

    /// In a symmetric starting position, NNUE eval from the side-to-move's
    /// perspective should be approximately equal regardless of which side is
    /// to move, because the position is mirror-symmetric and `evaluate`
    /// already returns a score relative to the side to move.
    #[test]
    fn perspective_symmetry() {
        let white_to_move: Board = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
            .parse()
            .unwrap();
        let black_to_move: Board = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR b KQkq - 0 1"
            .parse()
            .unwrap();

        let w_score = evaluate(&white_to_move);
        let b_score = evaluate(&black_to_move);

        // For a symmetric position, both sides should see the same score
        // since evaluate returns from the side-to-move's perspective
        assert!(
            (w_score - b_score).abs() <= 5,
            "symmetric position scores should be equal: white={w_score}, black={b_score}"
        );
    }
}
