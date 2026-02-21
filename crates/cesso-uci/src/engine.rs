//! Event-driven, multi-threaded UCI engine with pondering support.

use std::io::{self, BufRead};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};

use tracing::{debug, info, warn};

use cesso_core::Board;
use cesso_engine::{SearchControl, SearchResult, Searcher, limits_from_go};

use crate::command::{GoParams, parse_command, Command};
use crate::error::UciError;

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
    searcher: Searcher,
}

/// The UCI engine, holding current board state and searcher.
///
/// Runs an event-driven loop on the main thread, dispatching searches
/// to a worker thread and processing UCI commands concurrently.
pub struct UciEngine {
    board: Board,
    searcher: Option<Searcher>,
    state: EngineState,
    stop_flag: Arc<AtomicBool>,
    control: Option<Arc<SearchControl>>,
    pending_clear_tt: bool,
}

impl UciEngine {
    /// Create a new engine with the starting position.
    pub fn new() -> Self {
        Self {
            board: Board::starting_position(),
            searcher: Some(Searcher::new()),
            state: EngineState::Idle,
            stop_flag: Arc::new(AtomicBool::new(false)),
            control: None,
            pending_clear_tt: false,
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
                    Command::Position(board) => self.handle_position(board),
                    Command::Go(params) => self.handle_go(params, &tx),
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
        println!("option name Ponder type check default false");
        println!("uciok");
    }

    fn handle_isready(&self) {
        println!("readyok");
    }

    fn handle_ucinewgame(&mut self) {
        self.board = Board::starting_position();
        if let Some(ref mut searcher) = self.searcher {
            searcher.clear_tt();
        } else {
            // Search thread owns the searcher — defer clear until it comes back
            self.pending_clear_tt = true;
        }
    }

    fn handle_position(&mut self, board: Board) {
        self.board = board;
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
        ));

        let max_depth = params.depth.unwrap_or(128);

        // Take the searcher — the search thread will own it
        let mut searcher = self.searcher.take().unwrap_or_default();

        let board = self.board;
        let search_control = Arc::clone(&control);
        let tx = tx.clone();

        std::thread::spawn(move || {
            let result =
                searcher.search(&board, max_depth, &search_control, |d, score, nodes, pv| {
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
            let _ = tx.send(EngineEvent::SearchDone(SearchDone { result, searcher }));
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
        let mut searcher = done.searcher;

        if self.pending_clear_tt {
            searcher.clear_tt();
            self.pending_clear_tt = false;
        }

        self.searcher = Some(searcher);
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
