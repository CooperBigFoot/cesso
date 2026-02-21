//! Lazy SMP thread pool for parallel search.

use std::sync::atomic::{AtomicU64, Ordering};

use cesso_core::{Board, Move};

use crate::search::control::SearchControl;
use crate::search::heuristics::{HistoryTable, KillerTable};
use crate::search::negamax::{INF, PvTable, SearchContext, aspiration_search};
use crate::search::tt::TranspositionTable;
use crate::search::SearchResult;
use crate::search::StabilityTracker;

/// Lazy SMP thread pool — owns the shared transposition table.
pub struct ThreadPool {
    tt: TranspositionTable,
    num_threads: usize,
}

impl ThreadPool {
    /// Create a new thread pool with `hash_mb` MB transposition table.
    pub fn new(hash_mb: usize) -> Self {
        Self {
            tt: TranspositionTable::new(hash_mb),
            num_threads: 1,
        }
    }

    /// Set the number of search threads.
    pub fn set_num_threads(&mut self, n: usize) {
        self.num_threads = n.max(1);
    }

    /// Resize the transposition table.
    pub fn resize_tt(&mut self, mb: usize) {
        self.tt = TranspositionTable::new(mb);
    }

    /// Clear the transposition table.
    pub fn clear_tt(&self) {
        self.tt.clear();
    }

    /// Run a Lazy SMP search.
    ///
    /// Thread 0 runs full iterative deepening with the `on_iter` callback for UCI output.
    /// Threads 1..N-1 run silent iterative deepening, contributing only to the shared TT.
    /// Uses `std::thread::scope` — no `Arc` needed on the TT.
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

        if self.num_threads <= 1 {
            // Single-thread fast path — no scope overhead
            return self.search_single(board, max_depth, control, history, on_iter);
        }

        // Shared node counters — one AtomicU64 per thread to avoid contention
        let node_counters: Vec<AtomicU64> = (0..self.num_threads)
            .map(|_| AtomicU64::new(0))
            .collect();

        let mut result = SearchResult {
            best_move: Move::NULL,
            ponder_move: None,
            pv: vec![Move::NULL],
            score: -INF,
            nodes: 0,
            depth: 0,
        };

        std::thread::scope(|s| {
            // Spawn N-1 helper threads (thread_id 1..num_threads)
            for (thread_id, node_counter) in node_counters.iter().enumerate().skip(1) {
                let tt = &self.tt;
                s.spawn(move || {
                    run_helper(thread_id, tt, board, max_depth, control, node_counter, history);
                });
            }

            // Thread 0 runs on this thread (the coordinator)
            result = self.search_main(board, max_depth, control, history, &mut on_iter, &node_counters[0]);
        });
        // scope auto-joins all helpers here

        // Sum node counts from all threads
        let total_nodes: u64 = node_counters
            .iter()
            .map(|c| c.load(Ordering::Relaxed))
            .sum();
        result.nodes = total_nodes;

        result
    }

    /// Single-thread fast path — no scope overhead.
    fn search_single<F>(
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
        let mut ctx = SearchContext {
            nodes: 0,
            tt: &self.tt,
            pv: PvTable::new(),
            control,
            killers: KillerTable::new(),
            history_table: HistoryTable::new(),
            history: history.to_vec(),
        };

        let mut completed_move = Move::NULL;
        let mut completed_score = -INF;
        let mut completed_depth: u8 = 0;
        let mut completed_pv: Vec<Move> = Vec::new();
        let mut prev_score: i32 = 0;
        let mut stability = StabilityTracker::new();

        for depth in 1..=max_depth {
            if control.should_stop_iterating() {
                break;
            }

            let score = aspiration_search(board, depth, prev_score, &mut ctx);

            if control.should_stop(ctx.nodes) {
                break;
            }

            prev_score = score;

            let pv = ctx.pv.root_pv();
            if !pv.is_empty() && !pv[0].is_null() {
                completed_move = pv[0];
            }
            completed_score = score;
            completed_depth = depth;
            completed_pv = pv.iter().copied().filter(|m| !m.is_null()).collect();

            on_iter(depth, score, ctx.nodes, &completed_pv);

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
            pv: if completed_pv.is_empty() {
                vec![completed_move]
            } else {
                completed_pv
            },
            score: completed_score,
            nodes: ctx.nodes,
            depth: completed_depth,
        }
    }

    /// Thread 0 search — same as single, but stores final node count to an atomic counter.
    fn search_main<F>(
        &self,
        board: &Board,
        max_depth: u8,
        control: &SearchControl,
        history: &[u64],
        on_iter: &mut F,
        node_counter: &AtomicU64,
    ) -> SearchResult
    where
        F: FnMut(u8, i32, u64, &[Move]),
    {
        let mut ctx = SearchContext {
            nodes: 0,
            tt: &self.tt,
            pv: PvTable::new(),
            control,
            killers: KillerTable::new(),
            history_table: HistoryTable::new(),
            history: history.to_vec(),
        };

        let mut completed_move = Move::NULL;
        let mut completed_score = -INF;
        let mut completed_depth: u8 = 0;
        let mut completed_pv: Vec<Move> = Vec::new();
        let mut prev_score: i32 = 0;
        let mut stability = StabilityTracker::new();

        for depth in 1..=max_depth {
            if control.should_stop_iterating() {
                break;
            }

            let score = aspiration_search(board, depth, prev_score, &mut ctx);

            if control.should_stop(ctx.nodes) {
                break;
            }

            prev_score = score;

            let pv = ctx.pv.root_pv();
            if !pv.is_empty() && !pv[0].is_null() {
                completed_move = pv[0];
            }
            completed_score = score;
            completed_depth = depth;
            completed_pv = pv.iter().copied().filter(|m| !m.is_null()).collect();

            on_iter(depth, score, ctx.nodes, &completed_pv);

            let scale = stability.update(completed_move, score);
            control.update_soft_scale(scale);
        }

        node_counter.store(ctx.nodes, Ordering::Relaxed);

        let ponder_move = if completed_pv.len() > 1 {
            Some(completed_pv[1])
        } else {
            None
        };

        SearchResult {
            best_move: completed_move,
            ponder_move,
            pv: if completed_pv.is_empty() {
                vec![completed_move]
            } else {
                completed_pv
            },
            score: completed_score,
            nodes: ctx.nodes,
            depth: completed_depth,
        }
    }
}

/// Silent helper thread for Lazy SMP — writes to TT only, no UCI output.
fn run_helper(
    thread_id: usize,
    tt: &TranspositionTable,
    board: &Board,
    max_depth: u8,
    control: &SearchControl,
    node_counter: &AtomicU64,
    history: &[u64],
) {
    let mut ctx = SearchContext {
        nodes: 0,
        tt,
        pv: PvTable::new(),
        control,
        killers: KillerTable::new(),
        history_table: HistoryTable::new(),
        history: history.to_vec(),
    };

    // Depth offset: helpers start at different depths to increase search divergence.
    // Helper i starts at depth 1 + (i % 2), so odd helpers skip depth 1.
    let start_depth: u8 = 1 + (thread_id % 2) as u8;

    let mut prev_score: i32 = 0;

    for depth in start_depth..=max_depth {
        if control.should_stop_iterating() {
            break;
        }

        let score = aspiration_search(board, depth, prev_score, &mut ctx);

        if control.should_stop(ctx.nodes) {
            break;
        }

        prev_score = score;
    }

    node_counter.store(ctx.nodes, Ordering::Relaxed);
}

impl std::fmt::Debug for ThreadPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ThreadPool")
            .field("tt", &self.tt)
            .field("num_threads", &self.num_threads)
            .finish()
    }
}

impl Default for ThreadPool {
    fn default() -> Self {
        Self::new(16)
    }
}
