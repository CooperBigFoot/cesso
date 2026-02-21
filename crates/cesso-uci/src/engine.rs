//! Event-driven, multi-threaded UCI engine with pondering support.

use std::io::{self, BufRead};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};

use tracing::{debug, info, warn};

use cesso_core::Board;
use cesso_engine::{SearchControl, SearchResult, ThreadPool, limits_from_go};

use crate::command::{GoParams, UciOption, parse_command, Command, PositionInfo};
use crate::error::UciError;

/// Configuration knobs adjustable via `setoption`.
struct EngineConfig {
    /// Transposition table size in megabytes.
    hash_mb: u32,
    /// Number of search threads.
    threads: u16,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            hash_mb: 16,
            threads: 1,
        }
    }
}

/// Internal engine state — tracks whether the engine is idle, searching, or pondering.
enum EngineState {
    Idle,
    Searching,
    Pondering,
}

/// Events processed by the main engine loop.
enum EngineEvent {
    UciCommand(Result<Command, UciError>),
    SearchDone(SearchDone),
    InputClosed,
}

/// Payload returned by the search thread when it finishes.
struct SearchDone {
    result: SearchResult,
    pool: ThreadPool,
}

/// The UCI engine, holding current board state and thread pool.
///
/// Runs an event-driven loop on the main thread, dispatching searches
/// to a worker thread and processing UCI commands concurrently.
pub struct UciEngine {
    board: Board,
    history: Vec<u64>,
    pool: Option<ThreadPool>,
    state: EngineState,
    stop_flag: Arc<AtomicBool>,
    control: Option<Arc<SearchControl>>,
    config: EngineConfig,
    pending_clear_tt: bool,
    /// Pending TT resize (MB) to apply when the search thread returns the pool.
    pending_resize_tt: Option<u32>,
}

impl UciEngine {
    /// Create a new engine with the starting position.
    pub fn new() -> Self {
        Self {
            board: Board::starting_position(),
            history: Vec::new(),
            pool: Some(ThreadPool::new(16)),
            state: EngineState::Idle,
            stop_flag: Arc::new(AtomicBool::new(false)),
            control: None,
            config: EngineConfig::default(),
            pending_clear_tt: false,
            pending_resize_tt: None,
        }
    }

    /// Run the UCI event loop, reading from stdin until `quit` or input closes.
    pub fn run(mut self) -> Result<(), UciError> {
        let (tx, rx) = mpsc::channel::<EngineEvent>();

        // Spawn stdin reader thread
        let stdin_tx = tx.clone();
        std::thread::spawn(move || {
            let stdin = io::stdin();
            let reader = stdin.lock();
            for line in reader.lines() {
                match line {
                    Ok(line) => {
                        let trimmed = line.trim().to_string();
                        if trimmed.is_empty() {
                            continue;
                        }
                        debug!(cmd = %trimmed, "received UCI command");
                        let cmd = parse_command(&trimmed);
                        if stdin_tx.send(EngineEvent::UciCommand(cmd)).is_err() {
                            break;
                        }
                    }
                    Err(_) => {
                        let _ = stdin_tx.send(EngineEvent::InputClosed);
                        break;
                    }
                }
            }
            let _ = stdin_tx.send(EngineEvent::InputClosed);
        });

        for event in &rx {
            match event {
                EngineEvent::UciCommand(Ok(cmd)) => match cmd {
                    Command::Uci => self.handle_uci(),
                    Command::IsReady => self.handle_isready(),
                    Command::UciNewGame => self.handle_ucinewgame(),
                    Command::Position(info) => self.handle_position(info),
                    Command::Go(params) => self.handle_go(params, &tx),
                    Command::SetOption(opt) => self.handle_setoption(opt),
                    Command::PonderHit => self.handle_ponderhit(),
                    Command::Stop => self.handle_stop(),
                    Command::Quit => {
                        // Stop any active search and wait for it to finish
                        if !matches!(self.state, EngineState::Idle) {
                            self.handle_stop();
                            // Drain events until we get SearchDone
                            for ev in &rx {
                                if let EngineEvent::SearchDone(done) = ev {
                                    self.finish_search(done);
                                    break;
                                }
                            }
                        }
                        break;
                    }
                    Command::Unknown(_) => {}
                },
                EngineEvent::UciCommand(Err(e)) => {
                    warn!(error = %e, "UCI parse error");
                }
                EngineEvent::SearchDone(done) => {
                    self.finish_search(done);
                }
                EngineEvent::InputClosed => break,
            }
        }

        info!("cesso shutting down");
        Ok(())
    }

    fn handle_uci(&self) {
        println!("id name cesso");
        println!("id author Nicolas Lazaro");
        println!("option name Hash type spin default 16 min 1 max 65536");
        println!("option name Threads type spin default 1 min 1 max 256");
        println!("option name Ponder type check default false");
        println!("uciok");
    }

    fn handle_isready(&self) {
        println!("readyok");
    }

    fn handle_ucinewgame(&mut self) {
        self.board = Board::starting_position();
        self.history.clear();
        if let Some(ref pool) = self.pool {
            pool.clear_tt();
        } else {
            // Search thread owns the pool — defer clear until it comes back
            self.pending_clear_tt = true;
        }
    }

    fn handle_setoption(&mut self, option: UciOption) {
        match option {
            UciOption::Hash(mb) => {
                self.config.hash_mb = mb;
                if let Some(ref mut pool) = self.pool {
                    pool.resize_tt(mb as usize);
                } else {
                    self.pending_resize_tt = Some(mb);
                }
            }
            UciOption::Threads(threads) => {
                self.config.threads = threads;
                if let Some(ref mut pool) = self.pool {
                    pool.set_num_threads(threads as usize);
                }
            }
            UciOption::Ponder(_) => {
                // Ponder option acknowledged — actual pondering is handled by the go ponder protocol
            }
        }
    }

    fn handle_position(&mut self, info: PositionInfo) {
        self.board = info.board;
        self.history = info.history;
    }

    fn handle_go(&mut self, params: GoParams, tx: &mpsc::Sender<EngineEvent>) {
        if !matches!(self.state, EngineState::Idle) {
            warn!("go received while not idle, ignoring");
            return;
        }

        // Reset stop flag
        self.stop_flag = Arc::new(AtomicBool::new(false));

        let side = self.board.side_to_move();
        let control = Arc::new(limits_from_go(
            params.wtime,
            params.btime,
            params.winc,
            params.binc,
            params.movestogo,
            params.movetime,
            params.infinite,
            params.ponder,
            side,
            Arc::clone(&self.stop_flag),
            &self.board,
        ));

        let max_depth = params.depth.unwrap_or(128);

        // Take the pool — the search thread will own it
        let pool = self.pool.take().unwrap_or_default();

        let board = self.board;
        let history = self.history.clone();
        let search_control = Arc::clone(&control);
        let tx = tx.clone();

        std::thread::spawn(move || {
            let result = pool.search(&board, max_depth, &search_control, &history, |d, score, nodes, pv| {
                let elapsed = search_control.elapsed();
                let elapsed_ms = elapsed.as_millis().max(1);
                let nps = (nodes as u128 * 1000) / elapsed_ms;

                let pv_str: String = pv
                    .iter()
                    .filter(|m| !m.is_null())
                    .map(|m| m.to_uci())
                    .collect::<Vec<_>>()
                    .join(" ");

                println!(
                    "info depth {} score cp {} nodes {} nps {} time {} pv {}",
                    d, score, nodes, nps, elapsed_ms, pv_str
                );
            });
            let _ = tx.send(EngineEvent::SearchDone(SearchDone { result, pool }));
        });

        self.state = if params.ponder {
            EngineState::Pondering
        } else {
            EngineState::Searching
        };
        self.control = Some(control);
    }

    fn handle_ponderhit(&mut self) {
        if !matches!(self.state, EngineState::Pondering) {
            warn!("ponderhit received while not pondering, ignoring");
            return;
        }
        if let Some(ref control) = self.control {
            control.activate();
        }
        self.state = EngineState::Searching;
    }

    fn handle_stop(&mut self) {
        self.stop_flag.store(true, Ordering::Release);
    }

    fn finish_search(&mut self, done: SearchDone) {
        let mut pool = done.pool;

        if let Some(mb) = self.pending_resize_tt.take() {
            // Resize supersedes clear — a fresh allocation is already empty
            pool.resize_tt(mb as usize);
            self.pending_clear_tt = false;
        } else if self.pending_clear_tt {
            pool.clear_tt();
            self.pending_clear_tt = false;
        }

        self.pool = Some(pool);
        self.control = None;

        let result = &done.result;
        if result.best_move.is_null() {
            println!("bestmove 0000");
        } else {
            match result.ponder_move {
                Some(pm) if !pm.is_null() => {
                    println!(
                        "bestmove {} ponder {}",
                        result.best_move.to_uci(),
                        pm.to_uci()
                    );
                }
                _ => {
                    println!("bestmove {}", result.best_move.to_uci());
                }
            }
        }

        self.state = EngineState::Idle;
    }
}

impl Default for UciEngine {
    fn default() -> Self {
        Self::new()
    }
}
