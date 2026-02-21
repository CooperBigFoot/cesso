//! Negamax alpha-beta search with quiescence.

use cesso_core::{Board, Move, generate_legal_moves};

use crate::evaluate;
use crate::search::ordering::MovePicker;
use crate::search::tt::{Bound, TranspositionTable};

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
    tt: &mut TranspositionTable,
) -> i32 {
    *nodes += 1;

    // Fifty-move rule draw
    if board.halfmove_clock() >= 100 {
        return 0;
    }

    // Probe transposition table
    let mut tt_move = Move::NULL;
    if let Some(tt_entry) = tt.probe(board.hash(), ply) {
        tt_move = tt_entry.best_move;
        // Cutoff if the stored depth is sufficient
        if tt_entry.depth >= depth {
            let cutoff = match tt_entry.bound {
                Bound::Exact => true,
                Bound::LowerBound => tt_entry.score >= beta,
                Bound::UpperBound => tt_entry.score <= alpha,
                Bound::None => false,
            };
            if cutoff {
                if ply == 0 {
                    if !tt_entry.best_move.is_null() {
                        *root_best_move = tt_entry.best_move;
                        return tt_entry.score;
                    }
                    // No valid move in TT entry — fall through to search
                } else {
                    return tt_entry.score;
                }
            }
        }
    }

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

    let original_alpha = alpha;
    let mut best_score = -INF;
    let mut best_move = Move::NULL;
    let mut picker = MovePicker::new(&moves, board, tt_move);

    while let Some(mv) = picker.pick_next() {
        let child = board.make_move(mv);
        let score = -negamax(&child, depth - 1, ply + 1, -beta, -alpha, nodes, root_best_move, tt);

        if score > best_score {
            best_score = score;
            best_move = mv;
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

    // Determine bound type and store in TT
    let bound = if best_score <= original_alpha {
        Bound::UpperBound
    } else if best_score >= beta {
        Bound::LowerBound
    } else {
        Bound::Exact
    };

    let store_move = if bound == Bound::UpperBound && best_move.is_null() {
        tt_move // preserve ordering hint from prior entry
    } else {
        best_move
    };
    tt.store(board.hash(), depth, best_score, 0, store_move, bound, ply);

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
