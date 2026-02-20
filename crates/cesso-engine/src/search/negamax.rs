//! Negamax alpha-beta search with quiescence.

use cesso_core::{Board, Move, generate_legal_moves};

use crate::evaluate;
use crate::search::ordering::MovePicker;

/// Score representing an unreachable upper/lower bound.
pub const INF: i32 = 30_000;

/// Base score for checkmate (adjusted by ply for mate distance).
pub const MATE_SCORE: i32 = 29_000;

/// Scores above this threshold indicate a forced mate.
pub const MATE_THRESHOLD: i32 = 28_000;

/// Maximum ply depth for quiescence search to prevent stack overflow.
const QSEARCH_MAX_PLY: u8 = 64;

/// Negamax alpha-beta search.
///
/// Returns the best score for the side to move. At `ply == 0`,
/// writes the best root move into `root_best_move`.
pub(super) fn negamax(
    board: &Board,
    depth: u8,
    ply: u8,
    mut alpha: i32,
    beta: i32,
    nodes: &mut u64,
    root_best_move: &mut Move,
) -> i32 {
    *nodes += 1;

    // Fifty-move rule draw
    if board.halfmove_clock() >= 100 {
        return 0;
    }

    // TODO(Phase 3): threefold repetition via Zobrist hash

    // Leaf node — drop into quiescence search
    if depth == 0 {
        return qsearch(board, ply, alpha, beta, nodes);
    }

    let moves = generate_legal_moves(board);

    // No legal moves: checkmate or stalemate
    if moves.is_empty() {
        let king_sq = board.king_square(board.side_to_move());
        let in_check = board.is_square_attacked(king_sq, !board.side_to_move());
        return if in_check {
            -(MATE_SCORE - ply as i32)
        } else {
            0
        };
    }

    let mut best_score = -INF;
    let mut picker = MovePicker::new(&moves, board);

    while let Some(mv) = picker.pick_next() {
        let child = board.make_move(mv);
        let score = -negamax(&child, depth - 1, ply + 1, -beta, -alpha, nodes, root_best_move);

        if score > best_score {
            best_score = score;
            if ply == 0 {
                *root_best_move = mv;
            }
        }

        if score > alpha {
            alpha = score;
        }

        if alpha >= beta {
            break;
        }
    }

    best_score
}

/// Quiescence search — resolve tactical sequences before evaluating.
///
/// Only considers captures and promotions (via [`MovePicker::new_qsearch`])
/// to avoid the horizon effect.
fn qsearch(
    board: &Board,
    ply: u8,
    mut alpha: i32,
    beta: i32,
    nodes: &mut u64,
) -> i32 {
    *nodes += 1;

    // Ply ceiling to prevent runaway recursion
    if ply >= QSEARCH_MAX_PLY {
        return evaluate(board);
    }

    // Fifty-move rule draw
    if board.halfmove_clock() >= 100 {
        return 0;
    }

    // Stand-pat: the side to move can choose not to capture
    let stand_pat = evaluate(board);
    if stand_pat >= beta {
        return stand_pat;
    }
    if stand_pat > alpha {
        alpha = stand_pat;
    }

    let moves = generate_legal_moves(board);
    let mut picker = MovePicker::new_qsearch(&moves, board);

    while let Some(mv) = picker.pick_next() {
        let child = board.make_move(mv);
        let score = -qsearch(&child, ply + 1, -beta, -alpha, nodes);

        if score >= beta {
            return score;
        }
        if score > alpha {
            alpha = score;
        }
    }

    alpha
}
