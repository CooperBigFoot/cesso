//! Negamax alpha-beta search with quiescence.

use cesso_core::{Board, Move, generate_legal_moves};

use crate::evaluate;
use crate::search::control::SearchControl;
use crate::search::ordering::MovePicker;
use crate::search::tt::{Bound, TranspositionTable};

/// Score representing an unreachable upper/lower bound.
pub const INF: i32 = 30_000;

/// Base score for checkmate (adjusted by ply for mate distance).
pub const MATE_SCORE: i32 = 29_000;

/// Scores above this threshold indicate a forced mate.
pub const MATE_THRESHOLD: i32 = 28_000;

/// Maximum search depth (in plies) for array sizing and recursion limits.
pub const MAX_PLY: usize = 128;

/// Negamax alpha-beta search.
///
/// Returns the best score for the side to move. The principal
/// variation is collected into `ctx.pv`.
pub(super) fn negamax(
    board: &Board,
    depth: u8,
    ply: u8,
    mut alpha: i32,
    beta: i32,
    ctx: &mut SearchContext<'_>,
) -> i32 {
    ctx.pv.clear_ply(ply as usize);
    ctx.nodes += 1;

    // Check stop condition (time limit, node limit, etc.)
    if ctx.control.should_stop(ctx.nodes) {
        return 0;
    }

    // Fifty-move rule draw
    if board.halfmove_clock() >= 100 {
        return 0;
    }

    // Probe transposition table
    let mut tt_move = Move::NULL;
    if let Some(tt_entry) = ctx.tt.probe(board.hash(), ply) {
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
                        ctx.pv.set_single(0, tt_entry.best_move);
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
        return qsearch(board, ply, alpha, beta, ctx);
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
        let score = -negamax(&child, depth - 1, ply + 1, -beta, -alpha, ctx);

        if score > best_score {
            best_score = score;
            best_move = mv;
            if score > alpha {
                alpha = score;
                ctx.pv.update(ply as usize, mv);
            }
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
    ctx.tt.store(board.hash(), depth, best_score, 0, store_move, bound, ply);

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
    ctx: &mut SearchContext<'_>,
) -> i32 {
    ctx.nodes += 1;

    // Check stop condition (time limit, node limit, etc.)
    if ctx.control.should_stop(ctx.nodes) {
        return 0;
    }

    // Ply ceiling to prevent runaway recursion
    if ply as usize >= MAX_PLY {
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
        let score = -qsearch(&child, ply + 1, -beta, -alpha, ctx);

        if score >= beta {
            return score;
        }
        if score > alpha {
            alpha = score;
        }
    }

    alpha
}

/// Triangular PV table for collecting principal variation lines.
///
/// Stored on the stack (~33 KB). Each row `ply` contains the PV
/// continuation from that ply onward.
pub struct PvTable {
    moves: [[Move; MAX_PLY]; MAX_PLY],
    len: [usize; MAX_PLY],
}

impl PvTable {
    /// Create a zeroed PV table.
    pub fn new() -> Self {
        Self {
            moves: [[Move::NULL; MAX_PLY]; MAX_PLY],
            len: [0; MAX_PLY],
        }
    }

    /// Clear the PV line at `ply` (called at the top of each node).
    pub fn clear_ply(&mut self, ply: usize) {
        if ply < MAX_PLY {
            self.len[ply] = 0;
        }
    }

    /// Update the PV at `ply`: set `mv` as the best move and copy
    /// the continuation from `ply + 1`.
    ///
    /// After this call, `self.moves[ply]` = `[mv, pv[ply+1]...]`.
    pub fn update(&mut self, ply: usize, mv: Move) {
        if ply >= MAX_PLY {
            return;
        }

        self.moves[ply][0] = mv;

        let child_ply = ply + 1;
        if child_ply < MAX_PLY {
            let child_len = self.len[child_ply];
            let copy_len = child_len.min(MAX_PLY - 1);

            // Use split_at_mut for safe simultaneous borrow of two rows
            if ply < child_ply {
                let (top, bottom) = self.moves.split_at_mut(child_ply);
                top[ply][1..1 + copy_len].copy_from_slice(&bottom[0][..copy_len]);
            }

            self.len[ply] = 1 + copy_len;
        } else {
            self.len[ply] = 1;
        }
    }

    /// Set a single move as the PV at `ply` (no continuation).
    ///
    /// Used for TT cutoffs at the root.
    pub fn set_single(&mut self, ply: usize, mv: Move) {
        if ply < MAX_PLY {
            self.moves[ply][0] = mv;
            self.len[ply] = 1;
        }
    }

    /// The principal variation from the root.
    pub fn root_pv(&self) -> &[Move] {
        &self.moves[0][..self.len[0]]
    }

    /// Length of the root PV line.
    pub fn root_len(&self) -> usize {
        self.len[0]
    }
}

impl Default for PvTable {
    fn default() -> Self {
        Self::new()
    }
}

/// Search state threaded through negamax calls.
pub(super) struct SearchContext<'a> {
    /// Total nodes visited.
    pub nodes: u64,
    /// Transposition table (shared, lockless).
    pub tt: &'a TranspositionTable,
    /// Principal variation table.
    pub pv: PvTable,
    /// Search control (stop flag + time limits).
    pub control: &'a SearchControl,
}
