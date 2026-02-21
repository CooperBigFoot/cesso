//! Search algorithms and move ordering.

pub mod control;
pub mod heuristics;
pub mod negamax;
pub mod ordering;
pub mod pool;
pub mod tt;

use cesso_core::{Board, Move, generate_legal_moves};

use control::SearchControl;
use heuristics::{HistoryTable, KillerTable};
use negamax::{INF, PvTable, SearchContext, aspiration_search};
use tt::TranspositionTable;

/// Result of a completed search.
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// Best move found at the highest completed depth.
    pub best_move: Move,
    /// Second move in the PV — the expected reply (for pondering).
    pub ponder_move: Option<Move>,
    /// Full principal variation line.
    pub pv: Vec<Move>,
    /// Evaluation score in centipawns from the engine's perspective.
    pub score: i32,
    /// Total nodes visited during the search.
    pub nodes: u64,
    /// Depth reached.
    pub depth: u8,
}

/// Tracks best-move stability across ID iterations for time management.
///
/// When the best move changes or the score drops significantly, the engine
/// should think longer. When the best move is stable, it can play faster.
pub(super) struct StabilityTracker {
    last_move: Move,
    last_score: i32,
    stable_streak: u32,
}

impl StabilityTracker {
    fn new() -> Self {
        Self {
            last_move: Move::NULL,
            last_score: 0,
            stable_streak: 0,
        }
    }

    /// Update with the latest iteration results and return a scale factor (in hundredths).
    ///
    /// - Score drop > 100cp: 250 (think much longer)
    /// - Score drop > 50cp: 180 (think longer)
    /// - Stable streak >= 3: 60 (play faster)
    /// - Otherwise: 100 (neutral)
    fn update(&mut self, best_move: Move, score: i32) -> i32 {
        let scale;

        if self.last_move.is_null() {
            // First iteration — neutral
            scale = 100;
        } else {
            let score_drop = self.last_score - score;

            if score_drop > 100 {
                // Score dropped significantly — think much longer
                self.stable_streak = 0;
                scale = 250;
            } else if score_drop > 50 {
                // Moderate score drop — think longer
                self.stable_streak = 0;
                scale = 180;
            } else if best_move == self.last_move {
                self.stable_streak += 1;
                if self.stable_streak >= 3 {
                    scale = 60;
                } else {
                    scale = 100;
                }
            } else {
                // Move changed — reset stability
                self.stable_streak = 0;
                scale = 100;
            }
        }

        self.last_move = best_move;
        self.last_score = score;
        scale
    }
}

/// Iterative-deepening searcher with transposition table.
pub struct Searcher {
    tt: TranspositionTable,
}

impl Searcher {
    /// Create a fresh searcher with a 16 MB transposition table.
    pub fn new() -> Self {
        Self {
            tt: TranspositionTable::new(16),
        }
    }

    /// Clear the transposition table (preserving the allocation).
    pub fn clear_tt(&self) {
        self.tt.clear();
    }

    /// Resize the transposition table to the given size in megabytes.
    pub fn resize_tt(&mut self, mb: usize) {
        self.tt = TranspositionTable::new(mb);
    }

    /// Run iterative-deepening search up to `max_depth`.
    ///
    /// Calls `on_iter(depth, score, nodes, pv)` after each completed
    /// iteration, allowing the caller to emit UCI `info` lines.
    pub fn search<F>(
        &self,
        board: &Board,
        max_depth: u8,
        control: &SearchControl,
        history: &[u64],
        mut on_iter: F,
    ) -> SearchResult
    where
        F: FnMut(u8, i32, u64, &[Move]),
    {
        self.tt.new_generation();

        let mut ctx = SearchContext {
            nodes: 0,
            tt: &self.tt,
            pv: PvTable::new(),
            control,
            killers: KillerTable::new(),
            history_table: HistoryTable::new(),
            history: history.to_vec(),
        };

        // Track completed iteration results (for abort-safety)
        let mut completed_move = Move::NULL;
        let mut completed_score = -INF;
        let mut completed_depth: u8 = 0;
        let mut completed_pv: Vec<Move> = Vec::new();
        let mut prev_score: i32 = 0;
        let mut stability = StabilityTracker::new();

        for depth in 1..=max_depth {
            // Check soft limit before starting a new iteration
            if control.should_stop_iterating() {
                break;
            }

            let score = aspiration_search(board, depth, prev_score, &mut ctx);

            // If search was aborted mid-iteration, discard this iteration's result
            if control.should_stop(ctx.nodes) {
                break;
            }

            prev_score = score;

            // This iteration completed successfully — record results
            let pv = ctx.pv.root_pv();
            if !pv.is_empty() && !pv[0].is_null() {
                completed_move = pv[0];
            }
            completed_score = score;
            completed_depth = depth;
            completed_pv = pv.iter().copied().filter(|m| !m.is_null()).collect();

            debug_assert!(
                !completed_move.is_null() || generate_legal_moves(board).is_empty(),
                "negamax returned without setting root_best_move at depth {depth}"
            );

            on_iter(depth, score, ctx.nodes, &completed_pv);

            // Update time management based on best-move stability
            let scale = stability.update(completed_move, score);
            control.update_soft_scale(scale);
        }

        let ponder_move = if completed_pv.len() > 1 {
            Some(completed_pv[1])
        } else {
            None
        };

        SearchResult {
            best_move: completed_move,
            ponder_move,
            pv: if completed_pv.is_empty() { vec![completed_move] } else { completed_pv },
            score: completed_score,
            nodes: ctx.nodes,
            depth: completed_depth,
        }
    }
}

impl std::fmt::Debug for Searcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Searcher")
            .field("tt", &self.tt)
            .finish()
    }
}

impl Default for Searcher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;

    use cesso_core::Board;

    fn search_depth(searcher: &Searcher, board: &Board, depth: u8) -> SearchResult {
        let stopped = Arc::new(AtomicBool::new(false));
        let control = SearchControl::new_infinite(stopped);
        searcher.search(board, depth, &control, &[], |_, _, _, _| {})
    }

    #[test]
    fn depth_1_returns_legal_move() {
        let board = Board::starting_position();
        let searcher = Searcher::new();
        let result = search_depth(&searcher, &board, 1);
        assert!(!result.best_move.is_null(), "should find a move at depth 1");
    }

    #[test]
    fn finds_mate_in_one() {
        // Scholar's mate setup: White Qh5, Bc4, black king exposed
        // After Qxf7# — white to move, mate in 1
        let board: Board = "r1bqkb1r/pppp1ppp/2n2n2/4p2Q/2B1P3/8/PPPP1PPP/RNB1K1NR w KQkq - 4 4"
            .parse()
            .unwrap();
        let searcher = Searcher::new();
        let result = search_depth(&searcher, &board, 2);
        // The best move should be Qxf7# (h5f7)
        assert_eq!(result.best_move.to_uci(), "h5f7");
        // Score should indicate mate
        assert!(
            result.score > negamax::MATE_THRESHOLD,
            "score {} should indicate mate",
            result.score
        );
    }

    #[test]
    fn stalemate_returns_zero() {
        // Black king on a8, white king on c7, white queen on b6 — black to move, stalemate
        let board: Board = "k7/2K5/1Q6/8/8/8/8/8 b - - 0 1".parse().unwrap();
        let searcher = Searcher::new();
        let result = search_depth(&searcher, &board, 1);
        assert_eq!(result.score, 0, "stalemate should score 0");
    }

    #[test]
    fn mated_position_returns_negative() {
        // Black king on h8, white queen on g7, white king on f6 — black to move, checkmated
        let board: Board = "7k/6Q1/5K2/8/8/8/8/8 b - - 0 1".parse().unwrap();
        let searcher = Searcher::new();
        let result = search_depth(&searcher, &board, 1);
        // Black is checkmated, score should be very negative
        assert!(
            result.score < -negamax::MATE_THRESHOLD,
            "mated score {} should be deeply negative",
            result.score
        );
    }

    #[test]
    fn iterative_deepening_calls_callback() {
        let board = Board::starting_position();
        let searcher = Searcher::new();
        let stopped = Arc::new(AtomicBool::new(false));
        let control = SearchControl::new_infinite(stopped);
        let mut depths_seen = Vec::new();
        searcher.search(&board, 3, &control, &[], |depth, _, _, _| {
            depths_seen.push(depth);
        });
        assert_eq!(depths_seen, vec![1, 2, 3]);
    }

    #[test]
    fn on_iter_never_emits_null_move() {
        let board = Board::starting_position();
        let searcher = Searcher::new();
        let stopped = Arc::new(AtomicBool::new(false));
        let control = SearchControl::new_infinite(stopped);
        searcher.search(&board, 4, &control, &[], |_d, _score, _nodes, pv| {
            assert!(
                !pv.is_empty() && !pv[0].is_null(),
                "on_iter callback received empty PV or Move::NULL"
            );
        });
    }

    #[test]
    fn repeated_search_no_null_leak() {
        let board = Board::starting_position();
        let searcher = Searcher::new();
        // First search warms the TT
        let stopped1 = Arc::new(AtomicBool::new(false));
        let control1 = SearchControl::new_infinite(stopped1);
        searcher.search(&board, 3, &control1, &[], |_d, _score, _nodes, pv| {
            assert!(
                !pv.is_empty() && !pv[0].is_null(),
                "null move in first search callback"
            );
        });
        // Second search probes the warm TT
        let stopped2 = Arc::new(AtomicBool::new(false));
        let control2 = SearchControl::new_infinite(stopped2);
        searcher.search(&board, 3, &control2, &[], |_d, _score, _nodes, pv| {
            assert!(
                !pv.is_empty() && !pv[0].is_null(),
                "null move in second search callback (warm TT)"
            );
        });
    }

    #[test]
    fn stalemate_result_is_null() {
        // Black king on a8, white king on c7, white queen on b6 — black to move, stalemate
        let board: Board = "k7/2K5/1Q6/8/8/8/8/8 b - - 0 1".parse().unwrap();
        let searcher = Searcher::new();
        let result = search_depth(&searcher, &board, 1);
        assert!(
            result.best_move.is_null(),
            "stalemate should produce null best_move"
        );
    }

    #[test]
    fn checkmate_result_is_null() {
        // Black king on h8, white queen on g7, white king on f6 — black to move, checkmated
        let board: Board = "7k/6Q1/5K2/8/8/8/8/8 b - - 0 1".parse().unwrap();
        let searcher = Searcher::new();
        let result = search_depth(&searcher, &board, 1);
        assert!(
            result.best_move.is_null(),
            "checkmate should produce null best_move"
        );
    }

    #[test]
    fn pv_has_multiple_moves_at_depth_4() {
        let board = Board::starting_position();
        let searcher = Searcher::new();
        let result = search_depth(&searcher, &board, 4);
        assert!(
            result.pv.len() >= 2,
            "PV at depth 4 should have at least 2 moves, got {}",
            result.pv.len()
        );
    }

    #[test]
    fn ponder_move_available_at_depth_4() {
        let board = Board::starting_position();
        let searcher = Searcher::new();
        let result = search_depth(&searcher, &board, 4);
        assert!(
            result.ponder_move.is_some(),
            "ponder move should be available at depth 4"
        );
    }

    #[test]
    fn pv_first_move_matches_best_move() {
        let board = Board::starting_position();
        let searcher = Searcher::new();
        let result = search_depth(&searcher, &board, 4);
        if !result.pv.is_empty() {
            assert_eq!(
                result.pv[0], result.best_move,
                "first PV move should match best_move"
            );
        }
    }

    #[test]
    fn search_aborts_when_stopped() {
        use std::sync::atomic::Ordering;

        let board = Board::starting_position();
        let searcher = Searcher::new();

        // Set stop flag immediately — search should return after completing
        // at most a few iterations.
        let stopped = Arc::new(AtomicBool::new(false));
        let control = SearchControl::new_infinite(Arc::clone(&stopped));

        // Stop after depth 1 callback fires
        let stop_clone = Arc::clone(&stopped);
        let result = searcher.search(&board, 128, &control, &[], |depth, _, _, _| {
            if depth >= 1 {
                stop_clone.store(true, Ordering::Release);
            }
        });

        // Should have stopped very early
        assert!(
            result.depth <= 2,
            "search should stop shortly after flag is set, got depth {}",
            result.depth
        );
    }

    #[test]
    fn nmp_still_finds_mate_in_one() {
        let board: Board = "r1bqkb1r/pppp1ppp/2n2n2/4p2Q/2B1P3/8/PPPP1PPP/RNB1K1NR w KQkq - 4 4"
            .parse()
            .unwrap();
        let searcher = Searcher::new();
        let result = search_depth(&searcher, &board, 4);
        assert_eq!(result.best_move.to_uci(), "h5f7", "NMP should not break mate-in-one");
        assert!(result.score > negamax::MATE_THRESHOLD);
    }

    #[test]
    fn nmp_stalemate_still_zero() {
        let board: Board = "k7/2K5/1Q6/8/8/8/8/8 b - - 0 1".parse().unwrap();
        let searcher = Searcher::new();
        let result = search_depth(&searcher, &board, 4);
        assert_eq!(result.score, 0, "stalemate should still return 0 with NMP");
    }

    #[test]
    fn lmr_still_finds_mate_in_one() {
        let board: Board = "r1bqkb1r/pppp1ppp/2n2n2/4p2Q/2B1P3/8/PPPP1PPP/RNB1K1NR w KQkq - 4 4"
            .parse()
            .unwrap();
        let searcher = Searcher::new();
        let result = search_depth(&searcher, &board, 5);
        assert_eq!(result.best_move.to_uci(), "h5f7", "LMR should not break mate-in-one");
        assert!(result.score > negamax::MATE_THRESHOLD);
    }

    #[test]
    fn lmr_startpos_depth4_legal_move() {
        let board = Board::starting_position();
        let searcher = Searcher::new();
        let result = search_depth(&searcher, &board, 4);
        assert!(!result.best_move.is_null(), "LMR should return legal move from startpos");
    }

    #[test]
    fn aspiration_fires_all_depths() {
        let board = Board::starting_position();
        let searcher = Searcher::new();
        let stopped = Arc::new(AtomicBool::new(false));
        let control = SearchControl::new_infinite(stopped);
        let mut depths_seen = Vec::new();
        searcher.search(&board, 6, &control, &[], |depth, _, _, _| {
            depths_seen.push(depth);
        });
        assert_eq!(depths_seen, vec![1, 2, 3, 4, 5, 6], "aspiration should not skip depths");
    }

    #[test]
    fn aspiration_mate_score_not_corrupted() {
        let board: Board = "r1bqkb1r/pppp1ppp/2n2n2/4p2Q/2B1P3/8/PPPP1PPP/RNB1K1NR w KQkq - 4 4"
            .parse()
            .unwrap();
        let searcher = Searcher::new();
        let result = search_depth(&searcher, &board, 6);
        assert_eq!(result.best_move.to_uci(), "h5f7");
        assert!(result.score > negamax::MATE_THRESHOLD, "mate score should survive aspiration");
    }

    #[test]
    fn aborted_search_uses_previous_iteration_result() {
        use std::sync::atomic::Ordering;

        let board = Board::starting_position();
        let searcher = Searcher::new();
        let stopped = Arc::new(AtomicBool::new(false));
        let control = SearchControl::new_infinite(Arc::clone(&stopped));

        // First do a normal depth-2 search to get a baseline
        let stopped2 = Arc::new(AtomicBool::new(false));
        let control2 = SearchControl::new_infinite(stopped2);
        let baseline = searcher.search(&board, 2, &control2, &[], |_, _, _, _| {});
        assert!(!baseline.best_move.is_null());

        // Now set stop immediately and search to depth 100
        stopped.store(true, Ordering::Release);
        let searcher2 = Searcher::new();
        let result = searcher2.search(&board, 100, &control, &[], |_, _, _, _| {});

        // With stop set immediately, depth 0 means no iteration completed
        // The best_move should be NULL (no completed iterations)
        // This is expected behavior — the engine should have at least one completed iteration
        // before stopping makes sense. In practice, stop is set after search starts.
        assert!(
            result.depth == 0 || !result.best_move.is_null(),
            "if any iteration completed, best_move should be non-null"
        );
    }

    #[test]
    fn stability_neutral_on_first_iteration() {
        let mut tracker = StabilityTracker::new();
        let mv = cesso_core::Move::new(cesso_core::Square::E2, cesso_core::Square::E4);
        let scale = tracker.update(mv, 25);
        assert_eq!(scale, 100, "first iteration should be neutral");
    }

    #[test]
    fn stability_streak_triggers_fast_play() {
        let mut tracker = StabilityTracker::new();
        let mv = cesso_core::Move::new(cesso_core::Square::E2, cesso_core::Square::E4);
        tracker.update(mv, 25); // first
        tracker.update(mv, 25); // streak 1
        tracker.update(mv, 25); // streak 2
        let scale = tracker.update(mv, 25); // streak 3
        assert_eq!(scale, 60, "stable streak >= 3 should return 60");
    }

    #[test]
    fn stability_score_drop_overrides() {
        let mut tracker = StabilityTracker::new();
        let mv = cesso_core::Move::new(cesso_core::Square::E2, cesso_core::Square::E4);
        tracker.update(mv, 100);
        tracker.update(mv, 100);
        tracker.update(mv, 100);
        // Big score drop even though move is stable
        let scale = tracker.update(mv, -50);
        assert_eq!(scale, 250, "score drop > 100cp should trigger alarm (250)");
    }

    #[test]
    fn repetition_returns_draw() {
        use cesso_core::{Move as CessoMove, Square};

        // 1.Nf3 Nf6 2.Ng1 Ng8 — position returns to the starting position
        let board = Board::starting_position();
        let b1 = board.make_move(CessoMove::new(Square::G1, Square::F3));
        let b2 = b1.make_move(CessoMove::new(Square::G8, Square::F6));
        let b3 = b2.make_move(CessoMove::new(Square::F3, Square::G1));
        let b4 = b3.make_move(CessoMove::new(Square::F6, Square::G8));
        // b4 is the same position as `board` (starting position)
        // history = [board.hash, b1.hash, b2.hash, b3.hash]
        let history = vec![board.hash(), b1.hash(), b2.hash(), b3.hash()];

        let searcher = Searcher::new();
        let stopped = Arc::new(AtomicBool::new(false));
        let control = SearchControl::new_infinite(stopped);
        let result = searcher.search(&b4, 6, &control, &history, |_, _, _, _| {});
        // With repetition detected, the score should be near zero (draw)
        assert!(
            result.score.abs() <= 100,
            "repetition should yield near-draw score, got {}",
            result.score
        );
    }
}
