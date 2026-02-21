//! Integration tests for the Lazy SMP thread pool.
//!
//! Verifies correctness (legal moves, mate detection) and robustness
//! (stop-signal propagation, node counting) under various thread counts.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use cesso_core::Board;
use cesso_engine::{SearchControl, SearchResult, ThreadPool};

const SCHOLARS_MATE_FEN: &str =
    "r1bqkb1r/pppp1ppp/2n2n2/4p2Q/2B1P3/8/PPPP1PPP/RNB1K1NR w KQkq - 4 4";

const SICILIAN_FEN: &str =
    "rnbqkbnr/pp1ppppp/8/2p5/4P3/8/PPPP1PPP/RNBQKBNR w KQkq c6 0 2";

const RUY_LOPEZ_FEN: &str =
    "r1bqkbnr/pppp1ppp/2n5/1B2p3/4P3/5N2/PPPP1PPP/RNBQK2R b KQkq - 3 3";

const ENDGAME_FEN: &str =
    "8/8/8/3k4/8/3K4/4P3/8 w - - 0 1";

/// Helper: run a search on `board` at `depth` using `threads` helper threads.
fn search_with_threads(board: &Board, depth: u8, threads: usize) -> SearchResult {
    let mut pool = ThreadPool::new(16);
    pool.set_num_threads(threads);
    let stopped = Arc::new(AtomicBool::new(false));
    let control = SearchControl::new_infinite(stopped);
    pool.search(board, depth, &control, |_, _, _, _| {})
}

// ── Basic correctness ─────────────────────────────────────────────────────────

#[test]
fn single_thread_returns_legal_move() {
    let board = Board::starting_position();
    let result = search_with_threads(&board, 4, 1);
    assert!(
        !result.best_move.is_null(),
        "single-thread search on startpos should return a legal move"
    );
}

#[test]
fn single_thread_finds_mate_in_one() {
    let board: Board = SCHOLARS_MATE_FEN.parse().unwrap();
    let result = search_with_threads(&board, 2, 1);
    assert_eq!(
        result.best_move.to_uci(),
        "h5f7",
        "single-thread should find Qxf7# (h5f7) in Scholar's mate position"
    );
    assert!(
        result.score > 28_000,
        "score {} should indicate mate (> 28000)",
        result.score
    );
}

// ── Multi-thread correctness ──────────────────────────────────────────────────

#[test]
fn multi_thread_2_returns_legal_move() {
    let board = Board::starting_position();
    let result = search_with_threads(&board, 4, 2);
    assert!(
        !result.best_move.is_null(),
        "2-thread search on startpos should return a legal move"
    );
}

#[test]
fn multi_thread_4_returns_legal_move() {
    let board = Board::starting_position();
    let result = search_with_threads(&board, 4, 4);
    assert!(
        !result.best_move.is_null(),
        "4-thread search on startpos should return a legal move"
    );
}

#[test]
fn multi_thread_finds_mate_in_one() {
    let board: Board = SCHOLARS_MATE_FEN.parse().unwrap();
    let result = search_with_threads(&board, 2, 4);
    assert_eq!(
        result.best_move.to_uci(),
        "h5f7",
        "4-thread search should find Qxf7# (h5f7) in Scholar's mate position"
    );
    assert!(
        result.score > 28_000,
        "score {} should indicate mate (> 28000)",
        result.score
    );
}

#[test]
fn multi_thread_various_positions() {
    let positions = [
        ("Sicilian Defence", SICILIAN_FEN),
        ("Ruy Lopez", RUY_LOPEZ_FEN),
        ("King+pawn endgame", ENDGAME_FEN),
    ];

    for (name, fen) in positions {
        let board: Board = fen.parse().unwrap_or_else(|_| panic!("invalid FEN for {name}"));
        let result = search_with_threads(&board, 4, 4);
        assert!(
            !result.best_move.is_null(),
            "4-thread search on {name} ({fen}) returned null move"
        );
    }
}

// ── Stop-signal behaviour ─────────────────────────────────────────────────────

#[test]
fn stop_signal_terminates_all_threads() {
    use std::thread;

    let board = Board::starting_position();
    let mut pool = ThreadPool::new(16);
    pool.set_num_threads(4);

    let stopped = Arc::new(AtomicBool::new(false));
    let control = Arc::new(SearchControl::new_infinite(Arc::clone(&stopped)));

    // Set the stop flag after 50 ms from a background thread.
    let stop_clone = Arc::clone(&stopped);
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(50));
        stop_clone.store(true, Ordering::Release);
    });

    // Run the search in a dedicated thread so we can join with a timeout.
    let (tx, rx) = std::sync::mpsc::channel::<SearchResult>();
    let board_clone = board.clone();
    let control_clone = Arc::clone(&control);
    thread::spawn(move || {
        let mut pool_inner = ThreadPool::new(16);
        pool_inner.set_num_threads(4);
        let result =
            pool_inner.search(&board_clone, 100, &control_clone, |_, _, _, _| {});
        let _ = tx.send(result);
    });

    let deadline = Duration::from_secs(5);
    let result = rx
        .recv_timeout(deadline)
        .expect("search with stop signal did not complete within 5 seconds");

    assert!(
        result.depth < 100,
        "search should have been stopped before depth 100, got depth {}",
        result.depth
    );
}

#[test]
fn pre_set_stop_returns_immediately() {
    let board = Board::starting_position();
    let mut pool = ThreadPool::new(16);
    pool.set_num_threads(4);

    // Stop flag set BEFORE the search begins.
    let stopped = Arc::new(AtomicBool::new(true));
    let control = SearchControl::new_infinite(Arc::clone(&stopped));

    let result = pool.search(&board, 100, &control, |_, _, _, _| {});

    assert_eq!(
        result.depth, 0,
        "search with pre-set stop flag should complete depth 0 (no iteration)"
    );
}

// ── Node counting ─────────────────────────────────────────────────────────────

#[test]
fn multi_thread_reports_total_nodes() {
    let board = Board::starting_position();

    let single = search_with_threads(&board, 6, 1);
    let quad = search_with_threads(&board, 6, 4);

    assert!(
        single.nodes > 0,
        "single-thread search should report > 0 nodes"
    );
    assert!(
        quad.nodes > 0,
        "4-thread search should report > 0 nodes"
    );
}

// ── Callback behaviour ────────────────────────────────────────────────────────

#[test]
fn on_iter_callback_fires() {
    let board = Board::starting_position();
    let mut pool = ThreadPool::new(16);
    pool.set_num_threads(4);

    let stopped = Arc::new(AtomicBool::new(false));
    let control = SearchControl::new_infinite(stopped);

    let mut depths_seen: Vec<u8> = Vec::new();
    pool.search(&board, 3, &control, |depth, _, _, _| {
        depths_seen.push(depth);
    });

    assert_eq!(
        depths_seen,
        vec![1, 2, 3],
        "on_iter callback should fire exactly once per completed depth"
    );
}
