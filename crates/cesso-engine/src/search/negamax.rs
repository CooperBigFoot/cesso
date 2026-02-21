//! Negamax alpha-beta search with quiescence.

use cesso_core::{Board, Move, MoveKind, generate_legal_moves};

use crate::evaluate;
use crate::search::control::SearchControl;
use crate::search::heuristics::{HistoryTable, KillerTable};
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
    do_null: bool,
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

    // Repetition detection (twofold repetition = draw in search)
    if ply > 0 {
        let hash = board.hash();
        let hmc = board.halfmove_clock() as usize;
        let len = ctx.history.len();
        let lookback = hmc.min(len);
        for i in (len.saturating_sub(lookback)..len).rev() {
            if ctx.history[i] == hash {
                return 0;
            }
        }
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
            // Never cut off at the root — always search so the PV and
            // score reflect the current iteration's work.  The TT move
            // is still used for move ordering above.
            if cutoff && ply > 0 {
                return tt_entry.score;
            }
        }
    }

    // Leaf node — drop into quiescence search
    if depth == 0 {
        return qsearch(board, ply, alpha, beta, ctx);
    }

    // --- Null Move Pruning ---
    if do_null && ply > 0 && depth >= 3 && beta.abs() < MATE_THRESHOLD {
        let king_sq = board.king_square(board.side_to_move());
        let in_check = board.is_square_attacked(king_sq, !board.side_to_move());
        if !in_check {
            let r = if depth >= 6 { 3 } else { 2 };
            let null_board = board.make_null_move();
            ctx.history.push(board.hash());
            let null_score = -negamax(
                &null_board,
                depth.saturating_sub(1 + r),
                ply + 1,
                -beta,
                -beta + 1,
                false,
                ctx,
            );
            ctx.history.pop();
            if null_score >= beta {
                return beta;
            }
        }
    }

    let moves = generate_legal_moves(board);

    // Detect check once — used for stalemate/checkmate and LMR guard
    let king_sq = board.king_square(board.side_to_move());
    let in_check = board.is_square_attacked(king_sq, !board.side_to_move());

    // No legal moves: checkmate or stalemate
    if moves.is_empty() {
        return if in_check {
            -(MATE_SCORE - ply as i32)
        } else {
            0
        };
    }

    let original_alpha = alpha;
    let mut best_score = -INF;
    let mut best_move = Move::NULL;
    let mut picker = MovePicker::new(&moves, board, tt_move, &ctx.killers, &ctx.history_table, ply as usize);
    let mut searched_quiets = [Move::NULL; 64];
    let mut quiet_count: usize = 0;
    let mut move_count: usize = 0;

    while let Some(mv) = picker.pick_next() {
        // Track quiet moves for history penalty on cutoff
        let is_quiet_move = mv.kind() == MoveKind::Normal && board.piece_on(mv.dest()).is_none();
        if is_quiet_move && quiet_count < 64 {
            searched_quiets[quiet_count] = mv;
            quiet_count += 1;
        }

        let child = board.make_move(mv);
        move_count += 1;

        // Push current position hash so the child can detect repetitions
        // with ancestor positions (must NOT push child.hash() — the child
        // would immediately match itself).
        ctx.history.push(board.hash());

        // --- Late Move Reductions (LMR) ---
        let is_tactical = board.piece_on(mv.dest()).is_some()
            || mv.kind() == MoveKind::EnPassant
            || mv.kind() == MoveKind::Promotion;

        let mut score;
        if depth >= 3 && move_count >= 4 && !is_tactical && !in_check {
            let r: u8 = if move_count >= 7 { 2 } else { 1 };
            let reduced_depth = depth.saturating_sub(1 + r);
            // Reduced-depth search with full window
            score = -negamax(&child, reduced_depth, ply + 1, -beta, -alpha, true, ctx);

            // Re-search at full depth if the reduced search improved alpha
            if score > alpha {
                score = -negamax(&child, depth - 1, ply + 1, -beta, -alpha, true, ctx);
            }
        } else {
            score = -negamax(&child, depth - 1, ply + 1, -beta, -alpha, true, ctx);
        }

        // Pop child position hash after recursion
        ctx.history.pop();

        if score > best_score {
            best_score = score;
            best_move = mv;
            if score > alpha {
                alpha = score;
                ctx.pv.update(ply as usize, mv);
            }
        }

        if alpha >= beta {
            // Update killer and history for quiet moves that cause cutoffs
            let is_quiet = mv.kind() == MoveKind::Normal && board.piece_on(mv.dest()).is_none();
            if is_quiet {
                ctx.killers.store(ply as usize, mv);
                if let Some(piece) = board.piece_on(mv.source()) {
                    ctx.history_table.update_good(piece, mv.dest().index(), depth);
                    // Penalise all quiet moves searched before the cutoff move
                    for i in 0..quiet_count {
                        let bad_mv = searched_quiets[i];
                        if let Some(bad_piece) = board.piece_on(bad_mv.source()) {
                            ctx.history_table.update_bad(bad_piece, bad_mv.dest().index(), depth);
                        }
                    }
                }
            }
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

/// Aspiration window search — wraps [`negamax`] with a narrow window
/// that widens on fail-high/fail-low.
///
/// For depths 1-4 or near-mate scores, uses a full window.
/// For deeper searches, starts with `delta = 50` centered on `prev_score`.
pub(super) fn aspiration_search(
    board: &Board,
    depth: u8,
    prev_score: i32,
    ctx: &mut SearchContext<'_>,
) -> i32 {
    // Full window for shallow depths or near-mate scores
    if depth <= 4 || prev_score.abs() >= MATE_THRESHOLD {
        return negamax(board, depth, 0, -INF, INF, true, ctx);
    }

    let mut delta: i32 = 50;
    let mut alpha = (prev_score - delta).max(-INF);
    let mut beta = (prev_score + delta).min(INF);

    loop {
        let score = negamax(board, depth, 0, alpha, beta, true, ctx);

        // Abort immediately if the search was stopped
        if ctx.control.should_stop(ctx.nodes) {
            return score;
        }

        if score <= alpha {
            // Fail low — widen alpha
            delta *= 4;
            alpha = (prev_score - delta).max(-INF);
            if delta > INF {
                alpha = -INF;
                beta = INF;
            }
        } else if score >= beta {
            // Fail high — widen beta
            delta *= 4;
            beta = (prev_score + delta).min(INF);
            if delta > INF {
                alpha = -INF;
                beta = INF;
            }
        } else {
            // Score is within the window — done
            return score;
        }
    }
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
    /// Killer move table.
    pub killers: KillerTable,
    /// History heuristic table.
    pub history_table: HistoryTable,
    /// Zobrist hashes of positions visited during this search (for repetition detection).
    /// Grows/shrinks with the search stack via push/pop.
    pub history: Vec<u64>,
}
