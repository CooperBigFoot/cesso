//! Negamax alpha-beta search with quiescence, PVS, LMR, and advanced pruning.

use cesso_core::{Board, Color, Move, MoveKind, PieceKind, generate_legal_moves};

use crate::evaluate;
use crate::search::control::SearchControl;
use crate::search::heuristics::{
    ContHistIndex, ContinuationHistory, CorrectionHistory, HistoryTable, KillerTable,
    StackEntry, update_cont_history,
};
use crate::search::ordering::{MovePicker, lmr_reduction};
use crate::search::see::see_ge;
use crate::search::tt::{Bound, TranspositionTable};

/// Score representing an unreachable upper/lower bound.
pub const INF: i32 = 30_000;

/// Base score for checkmate (adjusted by ply for mate distance).
pub const MATE_SCORE: i32 = 29_000;

/// Scores above this threshold indicate a forced mate.
pub const MATE_THRESHOLD: i32 = 28_000;

/// Maximum search depth (in plies) for array sizing and recursion limits.
pub const MAX_PLY: usize = 128;

/// Maximum depth for futility pruning.
const FUTILITY_DEPTH: u8 = 3;

/// Forward futility margins indexed by depth.
const FUTILITY_MARGIN: [i32; 4] = [0, 200, 450, 700];

/// Reverse futility pruning margins indexed by depth.
const RFP_MARGIN: [i32; 4] = [0, 200, 450, 700];

/// Maximum depth for Late Move Pruning.
const LMP_MAX_DEPTH: u8 = 4;

/// Move count threshold for LMP by depth (formula: 3 + d*d).
const LMP_THRESHOLD: [usize; 5] = [0, 4, 7, 12, 19];

/// ProbCut threshold margin above beta.
const PROBCUT_MARGIN: i32 = 344;

/// Razoring margins indexed by depth (depth <= 3).
const RAZOR_MARGIN: [i32; 4] = [0, 300, 550, 900];

/// History pruning threshold: prune if hist < -(HISTORY_PRUNE_MARGIN * depth).
const HISTORY_PRUNE_MARGIN: i32 = 2711;

/// Minimum depth for singular extension.
const SE_DEPTH: u8 = 8;

/// Double extension threshold (singular_score < singular_beta - SE_DOUBLE_MARGIN).
const SE_DOUBLE_MARGIN: i32 = 23;

/// Depth threshold above which NMP verification is required.
const NMP_VERIFY_DEPTH: u8 = 12;

/// Maximum cumulative double extensions allowed per search path.
const MAX_DOUBLE_EXTENSIONS: u8 = 16;

/// Parameters passed to each negamax call beyond alpha/beta.
#[derive(Clone, Copy)]
pub(super) struct NodeParams {
    pub depth: u8,
    pub ply: u8,
    pub do_null: bool,
    pub excluded: Move,
    pub cutnode: bool,
    pub double_extensions: u8,
}

/// Check if the side to move has any non-pawn, non-king material.
fn has_non_pawn_material(board: &Board) -> bool {
    let us = board.side_to_move();
    let our_pieces = board.side(us);
    (board.pieces(PieceKind::Knight) & our_pieces).is_nonempty()
        || (board.pieces(PieceKind::Bishop) & our_pieces).is_nonempty()
        || (board.pieces(PieceKind::Rook) & our_pieces).is_nonempty()
        || (board.pieces(PieceKind::Queen) & our_pieces).is_nonempty()
}

/// Negamax alpha-beta search with PVS, LMR, and all advanced pruning techniques.
///
/// Returns the best score for the side to move. The principal
/// variation is collected into `ctx.pv`.
pub(super) fn negamax(
    board: &Board,
    mut alpha: i32,
    beta: i32,
    params: NodeParams,
    ctx: &mut SearchContext<'_>,
) -> i32 {
    let NodeParams { mut depth, ply, do_null, excluded, cutnode, double_extensions } = params;
    let is_pv = alpha + 1 < beta;
    let is_root = ply == 0;

    ctx.pv.clear_ply(ply as usize);
    ctx.nodes += 1;

    // Ply ceiling to prevent out-of-bounds access and runaway recursion
    if ply as usize >= MAX_PLY {
        return evaluate(board);
    }

    // Reset cutoff count for this node
    ctx.stack[ply as usize].cutoff_count = 0;

    // Check stop condition (time limit, node limit, etc.)
    if ctx.control.should_stop(ctx.nodes) {
        return 0;
    }

    // Fifty-move rule draw
    if board.halfmove_clock() >= 100 {
        return ctx.draw_score(board);
    }

    // Repetition detection (twofold repetition = draw in search)
    if ply > 0 {
        let hash = board.hash();
        let hmc = board.halfmove_clock() as usize;
        let len = ctx.history.len();
        let lookback = hmc.min(len);
        for i in (len.saturating_sub(lookback)..len).rev() {
            if ctx.history[i] == hash {
                return ctx.draw_score(board);
            }
        }
    }

    // Mate Distance Pruning
    if !is_root {
        alpha = alpha.max(-MATE_SCORE + ply as i32);
        let new_beta = beta.min(MATE_SCORE - ply as i32 - 1);
        if alpha >= new_beta {
            return alpha;
        }
    }

    // TT probe — skip if we have an excluded move (singular extension search)
    let mut tt_move = Move::NULL;
    let mut tt_score = 0i32;
    let mut tt_depth: u8 = 0;
    let mut tt_bound = Bound::None;
    let mut tt_is_pv = is_pv;
    let mut tt_eval: i32 = 0;

    if excluded.is_null() {
        if let Some(tt_entry) = ctx.tt.probe(board.hash(), ply) {
            tt_move = tt_entry.best_move;
            tt_score = tt_entry.score;
            tt_depth = tt_entry.depth;
            tt_bound = tt_entry.bound;
            tt_is_pv = tt_is_pv || tt_entry.is_pv;
            tt_eval = tt_entry.eval;

            // TT cutoff (not at root, not in PV)
            if !is_root && tt_depth >= depth {
                let cutoff = match tt_bound {
                    Bound::Exact => true,
                    Bound::LowerBound => tt_score >= beta,
                    Bound::UpperBound => tt_score <= alpha,
                    Bound::None => false,
                };
                if cutoff {
                    return tt_score;
                }
            }
        }
    }

    // Compute check status
    let king_sq = board.king_square(board.side_to_move());
    let in_check = board.is_square_attacked(king_sq, !board.side_to_move());

    // IIR — Internal Iterative Reduction
    if (is_pv || cutnode) && depth > 4 && tt_move.is_null() {
        depth = depth.saturating_sub(2);
    }

    // Check extension
    if in_check && (ply as usize) < MAX_PLY - 1 {
        depth += 1;
    }

    // Drop to qsearch at depth 0
    if depth == 0 {
        return qsearch(board, ply, alpha, beta, ctx);
    }

    // Static eval with correction history
    let raw_eval = if tt_eval != 0 { tt_eval } else { evaluate(board) };

    // Get previous move info for correction history
    let (prev_piece, prev_dest) = if ply >= 1 {
        let prev = &ctx.stack[ply as usize - 1];
        if !prev.current_move.is_null() {
            (Some(prev.moved_piece), Some(prev.current_move.dest()))
        } else {
            (None, None)
        }
    } else {
        (None, None)
    };

    let static_eval = if !in_check {
        ctx.correction_history.correct_eval(
            board.side_to_move(),
            board.pawn_hash(),
            board.non_pawn_hash(Color::White),
            board.non_pawn_hash(Color::Black),
            board.major_hash(),
            board.minor_hash(),
            prev_piece,
            prev_dest,
            raw_eval,
        )
    } else {
        raw_eval
    };

    // Store static eval in stack
    ctx.stack[ply as usize].static_eval = static_eval;

    // Compute improving flag
    let improving = if ply >= 2 && !in_check {
        static_eval > ctx.stack[ply as usize - 2].static_eval
    } else {
        false
    };

    // Razoring
    if !is_pv && !in_check && depth <= 3
        && static_eval + RAZOR_MARGIN[depth as usize] < alpha
    {
        let razor_score = qsearch(board, ply, alpha, beta, ctx);
        if razor_score <= alpha {
            return razor_score;
        }
    }

    // Reverse Futility Pruning
    if !is_pv && !in_check && excluded.is_null()
        && depth >= 1 && depth <= FUTILITY_DEPTH
        && beta.abs() < MATE_THRESHOLD
    {
        let margin = RFP_MARGIN[depth as usize] - if improving { 0 } else { 100 };
        if static_eval - margin >= beta {
            return static_eval;
        }
    }

    // Null Move Pruning
    if do_null && !is_pv && ply > 0 && excluded.is_null()
        && depth >= 3 && beta.abs() < MATE_THRESHOLD
        && !in_check && has_non_pawn_material(board)
        && static_eval >= beta
    {
        let r = if depth >= 6 { 3 } else { 2 };
        let null_board = board.make_null_move();
        ctx.history.push(board.hash());

        // Clear stack entry for null move
        ctx.stack[ply as usize].current_move = Move::NULL;
        ctx.stack[ply as usize].cont_hist_index = None;

        let null_score = -negamax(
            &null_board,
            -beta,
            -beta + 1,
            NodeParams {
                depth: depth.saturating_sub(1 + r),
                ply: ply + 1,
                do_null: false,
                excluded: Move::NULL,
                cutnode: !cutnode,
                double_extensions,
            },
            ctx,
        );
        ctx.history.pop();

        if null_score >= beta {
            // NMP Verification at high depths
            if depth > NMP_VERIFY_DEPTH {
                let verify_score = negamax(
                    board,
                    alpha,
                    beta,
                    NodeParams {
                        depth: depth.saturating_sub(1 + r),
                        ply,
                        do_null: false,
                        excluded: Move::NULL,
                        cutnode: false,
                        double_extensions,
                    },
                    ctx,
                );
                if verify_score >= beta {
                    return beta;
                }
                // Fall through to full search if verification fails
            } else {
                return beta;
            }
        }
    }

    // ProbCut
    if !is_pv && !in_check && depth >= 7 && beta.abs() < MATE_THRESHOLD {
        let probcut_beta = beta + PROBCUT_MARGIN;
        let moves = generate_legal_moves(board);

        for i in 0..moves.len() {
            let mv = moves[i];
            let is_tactical = board.piece_on(mv.dest()).is_some()
                || mv.kind() == MoveKind::EnPassant
                || mv.kind() == MoveKind::Promotion;
            if !is_tactical || !see_ge(board, mv, probcut_beta - static_eval) {
                continue;
            }

            let child = board.make_move(mv);
            ctx.history.push(board.hash());

            // qsearch to verify
            let mut score = -qsearch(&child, ply + 1, -probcut_beta, -probcut_beta + 1, ctx);

            if score >= probcut_beta {
                // Verify with reduced negamax
                score = -negamax(
                    &child,
                    -probcut_beta,
                    -probcut_beta + 1,
                    NodeParams {
                        depth: depth.saturating_sub(5),
                        ply: ply + 1,
                        do_null: true,
                        excluded: Move::NULL,
                        cutnode: !cutnode,
                        double_extensions,
                    },
                    ctx,
                );
            }

            ctx.history.pop();

            if score >= probcut_beta {
                ctx.tt.store(
                    board.hash(),
                    depth.saturating_sub(3),
                    score,
                    raw_eval,
                    mv,
                    Bound::LowerBound,
                    ply,
                    false,
                );
                return score;
            }
        }
    }

    // Move generation
    let moves = generate_legal_moves(board);

    if moves.is_empty() {
        return if in_check {
            -(MATE_SCORE - ply as i32)
        } else {
            ctx.draw_score(board)
        };
    }

    let original_alpha = alpha;
    let mut best_score = -INF;
    let mut best_move = Move::NULL;
    let mut picker = MovePicker::new(
        &moves,
        board,
        tt_move,
        &ctx.killers,
        &ctx.history_table,
        &ctx.cont_history,
        &ctx.stack,
        ply as usize,
    );
    let mut searched_quiets = [Move::NULL; 64];
    let mut quiet_count: usize = 0;
    let mut move_count: usize = 0;

    while let Some(mv) = picker.pick_next() {
        // Skip excluded move (singular extension search)
        if mv == excluded {
            continue;
        }

        let is_tactical = board.piece_on(mv.dest()).is_some()
            || mv.kind() == MoveKind::EnPassant
            || mv.kind() == MoveKind::Promotion;

        let moved_piece = board.piece_on(mv.source()).unwrap_or(PieceKind::Pawn);

        // ── Pruning (skip non-first moves in some conditions) ──────────────

        if move_count > 0 && !is_root {
            // Forward Futility Pruning
            if !in_check && depth <= FUTILITY_DEPTH && !is_tactical
                && alpha.abs() < MATE_THRESHOLD
            {
                let margin = FUTILITY_MARGIN[depth as usize] - if improving { 0 } else { 50 };
                if static_eval + margin <= alpha {
                    continue;
                }
            }

            // History pruning
            if !in_check && !is_tactical && depth <= 5 {
                let hist = ctx.history_table.score(moved_piece, mv.dest().index());
                if hist < -(HISTORY_PRUNE_MARGIN * depth as i32) {
                    continue;
                }
            }

            // SEE pruning
            if depth <= 5 && mv.kind() != MoveKind::Promotion {
                if is_tactical {
                    if !see_ge(board, mv, -(27 * depth as i32 * depth as i32)) {
                        continue;
                    }
                } else if !in_check && !see_ge(board, mv, -(59 * depth as i32)) {
                    continue;
                }
            }

            // Late Move Pruning
            let lmp_threshold = if improving {
                LMP_THRESHOLD[depth.min(LMP_MAX_DEPTH) as usize]
            } else {
                LMP_THRESHOLD[depth.min(LMP_MAX_DEPTH) as usize] / 2
            };
            if !in_check && depth <= LMP_MAX_DEPTH && move_count >= lmp_threshold
                && !is_tactical && best_score > -MATE_THRESHOLD
            {
                continue;
            }
        }

        // Track quiet moves searched before cutoff (for history penalty)
        let is_quiet_move = mv.kind() == MoveKind::Normal && board.piece_on(mv.dest()).is_none();
        if is_quiet_move && quiet_count < 64 {
            searched_quiets[quiet_count] = mv;
            quiet_count += 1;
        }

        // Set stack entry before make_move
        ctx.stack[ply as usize].current_move = mv;
        ctx.stack[ply as usize].moved_piece = moved_piece;
        ctx.stack[ply as usize].cont_hist_index = Some(ContHistIndex {
            side: board.side_to_move(),
            piece: moved_piece,
            to: mv.dest(),
        });

        let child = board.make_move(mv);
        move_count += 1;
        ctx.history.push(board.hash());

        // ── Extensions ──────────────────────────────────────────────────────
        let mut extension: i32 = 0;

        // Singular Extension — for TT move only
        if mv == tt_move && !is_root && depth >= SE_DEPTH
            && tt_depth >= depth.saturating_sub(3) && tt_bound != Bound::UpperBound
            && excluded.is_null()
        {
            let singular_beta = tt_score - 2 * depth as i32;
            let singular_score = negamax(
                board,
                singular_beta - 1,
                singular_beta,
                NodeParams {
                    depth: (depth - 1) / 2,
                    ply,
                    do_null: false,
                    excluded: mv,
                    cutnode,
                    double_extensions,
                },
                ctx,
            );

            if singular_score < singular_beta {
                extension = 1;
                // Double extension
                if singular_score < singular_beta - SE_DOUBLE_MARGIN
                    && double_extensions < MAX_DOUBLE_EXTENSIONS
                {
                    extension = 2;
                }
            } else if singular_score >= beta {
                // Multicut: not singular, another move also beats beta
                ctx.history.pop();
                return singular_score;
            } else if tt_score >= beta {
                // TT score beats beta but isn't singular — negative extension
                extension = -3;
            } else if cutnode {
                extension = -2;
            }
        }

        let new_depth = ((depth as i32 - 1) + extension).max(0) as u8;
        let child_double_ext = double_extensions + (extension == 2) as u8;

        // ── PVS + LMR ───────────────────────────────────────────────────────
        let score;
        if move_count == 1 {
            // First move: full window, full depth
            score = -negamax(
                &child,
                -beta,
                -alpha,
                NodeParams {
                    depth: new_depth,
                    ply: ply + 1,
                    do_null: true,
                    excluded: Move::NULL,
                    cutnode: false,
                    double_extensions: child_double_ext,
                },
                ctx,
            );
        } else {
            let do_lmr = depth >= 3 && move_count >= 4 && !is_tactical && !in_check;

            let mut searched_depth = new_depth;

            if do_lmr {
                // Base LMR reduction (in 1024ths of a ply)
                let mut r = lmr_reduction(move_count, depth as usize);

                // Adjustments (in 1024ths)
                r -= 372; // Base offset
                if is_pv { r -= 1062; }
                if cutnode { r += 1303; }
                if tt_is_pv { r -= 975; }
                let is_killer = ctx.killers.is_killer(ply as usize, mv);
                if is_killer { r -= 932; }

                // History-based reduction for quiets
                if is_quiet_move {
                    let hist = ctx.history_table.score(moved_piece, mv.dest().index());
                    // hist ranges -16384..16384, divide by 8 to get adjustment in 1024ths
                    r -= hist / 8;
                }

                // Convert from 1024ths to plies, clamped to at least 1
                let r_plies = (r / 1024).max(0) as u8;
                searched_depth = new_depth.saturating_sub(r_plies).max(1);
            }

            // Null-window search at (possibly reduced) depth
            let mut sc = -negamax(
                &child,
                -alpha - 1,
                -alpha,
                NodeParams {
                    depth: searched_depth,
                    ply: ply + 1,
                    do_null: true,
                    excluded: Move::NULL,
                    cutnode: !cutnode,
                    double_extensions: child_double_ext,
                },
                ctx,
            );

            // Re-search at full depth if LMR reduced and score beats alpha
            if do_lmr && sc > alpha && searched_depth < new_depth {
                sc = -negamax(
                    &child,
                    -alpha - 1,
                    -alpha,
                    NodeParams {
                        depth: new_depth,
                        ply: ply + 1,
                        do_null: true,
                        excluded: Move::NULL,
                        cutnode: !cutnode,
                        double_extensions: child_double_ext,
                    },
                    ctx,
                );
            }

            // Full window re-search for PV nodes
            if sc > alpha && is_pv {
                sc = -negamax(
                    &child,
                    -beta,
                    -alpha,
                    NodeParams {
                        depth: new_depth,
                        ply: ply + 1,
                        do_null: true,
                        excluded: Move::NULL,
                        cutnode: false,
                        double_extensions: child_double_ext,
                    },
                    ctx,
                );
            }

            score = sc;
        }

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
            // Cutoff — update heuristics
            ctx.stack[ply as usize].cutoff_count += 1;

            if is_quiet_move {
                ctx.killers.store(ply as usize, mv);
                let bonus = (depth as i32) * (depth as i32);

                // Reward cutoff move
                ctx.history_table.update(moved_piece, mv.dest().index(), bonus);
                update_cont_history(
                    &mut ctx.cont_history,
                    &ctx.stack,
                    ply as usize,
                    moved_piece,
                    mv.dest().index(),
                    bonus,
                );

                // Penalise all previously searched quiets
                for i in 0..quiet_count.saturating_sub(1) {
                    let bad_mv = searched_quiets[i];
                    if let Some(bad_piece) = board.piece_on(bad_mv.source()) {
                        ctx.history_table.update(bad_piece, bad_mv.dest().index(), -bonus);
                        update_cont_history(
                            &mut ctx.cont_history,
                            &ctx.stack,
                            ply as usize,
                            bad_piece,
                            bad_mv.dest().index(),
                            -bonus,
                        );
                    }
                }
            }
            break;
        }
    }

    // TT store — skip during singular extension search
    if excluded.is_null() {
        let bound = if best_score <= original_alpha {
            Bound::UpperBound
        } else if best_score >= beta {
            Bound::LowerBound
        } else {
            Bound::Exact
        };

        let store_move = if bound == Bound::UpperBound && best_move.is_null() {
            tt_move
        } else {
            best_move
        };
        ctx.tt.store(
            board.hash(),
            depth,
            best_score,
            raw_eval,
            store_move,
            bound,
            ply,
            is_pv || tt_is_pv,
        );

        // Update correction history
        if !in_check && !best_move.is_null()
            && (bound == Bound::Exact || bound == Bound::LowerBound)
        {
            let score_diff = best_score - raw_eval;
            ctx.correction_history.update(
                board.side_to_move(),
                board.pawn_hash(),
                board.non_pawn_hash(Color::White),
                board.non_pawn_hash(Color::Black),
                board.major_hash(),
                board.minor_hash(),
                prev_piece,
                prev_dest,
                score_diff,
            );
        }
    }

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
    let base_params = NodeParams {
        depth,
        ply: 0,
        do_null: true,
        excluded: Move::NULL,
        cutnode: false,
        double_extensions: 0,
    };

    // Full window for shallow depths or near-mate scores
    if depth <= 4 || prev_score.abs() >= MATE_THRESHOLD {
        return negamax(board, -INF, INF, base_params, ctx);
    }

    let mut delta: i32 = 50;
    let mut alpha = (prev_score - delta).max(-INF);
    let mut beta = (prev_score + delta).min(INF);

    loop {
        let score = negamax(board, alpha, beta, base_params, ctx);

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
        return ctx.draw_score(board);
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
        // Skip captures with negative SEE (losing exchanges), but never skip promotions.
        if mv.kind() != MoveKind::Promotion && !see_ge(board, mv, 0) {
            continue;
        }

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
    /// Continuation history table.
    pub cont_history: Box<ContinuationHistory>,
    /// Correction history for static eval adjustment.
    pub correction_history: Box<CorrectionHistory>,
    /// Per-ply search stack.
    pub stack: [StackEntry; MAX_PLY],
    /// Zobrist hashes of positions visited during this search (for repetition detection).
    pub history: Vec<u64>,
    /// Contempt factor in centipawns — biases draw evaluation.
    pub contempt: i32,
    /// The color the engine is playing (for contempt sign).
    pub engine_color: Color,
}

impl SearchContext<'_> {
    /// Contempt-aware draw score for negamax.
    ///
    /// When the engine is to move, a draw scores `-contempt` (bad when
    /// contempt > 0). When the opponent is to move, it scores `+contempt`.
    #[inline]
    fn draw_score(&self, board: &Board) -> i32 {
        if board.side_to_move() == self.engine_color {
            -self.contempt
        } else {
            self.contempt
        }
    }
}
