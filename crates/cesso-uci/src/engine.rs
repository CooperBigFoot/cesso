//! UCI engine loop.

use std::io::{self, BufRead};

use tracing::{debug, info};

use cesso_core::Board;
use cesso_engine::Searcher;

use crate::command::{parse_command, Command};
use crate::error::UciError;

/// The UCI engine, holding current board state and searcher.
pub struct UciEngine {
    board: Board,
    searcher: Searcher,
}

impl UciEngine {
    /// Create a new engine with the starting position.
    pub fn new() -> Self {
        Self {
            board: Board::starting_position(),
            searcher: Searcher::new(),
        }
    }

    /// Run the UCI input loop, reading from stdin until `quit`.
    pub fn run(mut self) -> Result<(), UciError> {
        let stdin = io::stdin();
        let reader = stdin.lock();

        for line in reader.lines() {
            let line = line?;
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            debug!(cmd = %line, "received UCI command");

            match parse_command(line)? {
                Command::Uci => self.handle_uci(),
                Command::IsReady => self.handle_isready(),
                Command::UciNewGame => self.handle_ucinewgame(),
                Command::Position(board) => self.handle_position(board),
                Command::GoDepth { depth } => self.handle_go_depth(depth),
                Command::Stop => {} // no async search yet
                Command::Quit => break,
                Command::Unknown(_) => {} // silently ignore per UCI spec
            }
        }

        info!("cesso shutting down");
        Ok(())
    }

    fn handle_uci(&self) {
        println!("id name cesso");
        println!("id author Nicolas Lazaro");
        println!("uciok");
    }

    fn handle_isready(&self) {
        println!("readyok");
    }

    fn handle_ucinewgame(&mut self) {
        self.board = Board::starting_position();
        self.searcher.clear_tt();
    }

    fn handle_position(&mut self, board: Board) {
        self.board = board;
    }

    fn handle_go_depth(&mut self, depth: u8) {
        let result = self.searcher.search(&self.board, depth, |d, score, nodes, best_move| {
            if best_move.is_null() {
                println!("info depth {} score cp {} nodes {}", d, score, nodes);
            } else {
                println!(
                    "info depth {} score cp {} nodes {} pv {}",
                    d, score, nodes, best_move.to_uci()
                );
            }
        });
        if result.best_move.is_null() {
            println!("bestmove 0000");
        } else {
            println!("bestmove {}", result.best_move.to_uci());
        }
    }
}

impl Default for UciEngine {
    fn default() -> Self {
        Self::new()
    }
}
