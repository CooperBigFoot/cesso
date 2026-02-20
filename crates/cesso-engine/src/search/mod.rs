//! Search algorithms and move ordering.

pub mod negamax;
pub mod ordering;

use cesso_core::{Board, Move};

use negamax::{negamax, INF};

/// Result of a completed search.
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// Best move found at the highest completed depth.
    pub best_move: Move,
    /// Evaluation score in centipawns from the engine's perspective.
    pub score: i32,
    /// Total nodes visited during the search.
    pub nodes: u64,
    /// Depth reached.
    pub depth: u8,
}

/// Iterative-deepening searcher.
#[derive(Debug)]
pub struct Searcher {
    nodes: u64,
    best_move: Move,
}

impl Searcher {
    /// Create a fresh searcher.
    pub fn new() -> Self {
        Self {
            nodes: 0,
            best_move: Move::NULL,
        }
    }

    /// Run iterative-deepening search up to `max_depth`.
    ///
    /// Calls `on_iter(depth, score, nodes, best_move)` after each completed
    /// iteration, allowing the caller to emit UCI `info` lines.
    pub fn search<F>(
        &mut self,
        board: &Board,
        max_depth: u8,
        mut on_iter: F,
    ) -> SearchResult
    where
        F: FnMut(u8, i32, u64, Move),
    {
        self.nodes = 0;
        self.best_move = Move::NULL;

        let mut best_score = -INF;

        for depth in 1..=max_depth {
            let score = negamax(
                board,
                depth,
                0,
                -INF,
                INF,
                &mut self.nodes,
                &mut self.best_move,
            );
            best_score = score;
            on_iter(depth, score, self.nodes, self.best_move);
        }

        SearchResult {
            best_move: self.best_move,
            score: best_score,
            nodes: self.nodes,
            depth: max_depth,
        }
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
    use cesso_core::Board;

    #[test]
    fn depth_1_returns_legal_move() {
        let board = Board::starting_position();
        let mut searcher = Searcher::new();
        let result = searcher.search(&board, 1, |_, _, _, _| {});
        assert!(!result.best_move.is_null(), "should find a move at depth 1");
    }

    #[test]
    fn finds_mate_in_one() {
        // Scholar's mate setup: White Qh5, Bc4, black king exposed
        // After Qxf7# — white to move, mate in 1
        let board: Board = "r1bqkb1r/pppp1ppp/2n2n2/4p2Q/2B1P3/8/PPPP1PPP/RNB1K1NR w KQkq - 4 4"
            .parse()
            .unwrap();
        let mut searcher = Searcher::new();
        let result = searcher.search(&board, 2, |_, _, _, _| {});
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
        let mut searcher = Searcher::new();
        let result = searcher.search(&board, 1, |_, _, _, _| {});
        assert_eq!(result.score, 0, "stalemate should score 0");
    }

    #[test]
    fn mated_position_returns_negative() {
        // Black king on h8, white queen on g7, white king on f6 — black to move, checkmated
        let board: Board = "7k/6Q1/5K2/8/8/8/8/8 b - - 0 1".parse().unwrap();
        let mut searcher = Searcher::new();
        let result = searcher.search(&board, 1, |_, _, _, _| {});
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
        let mut searcher = Searcher::new();
        let mut depths_seen = Vec::new();
        searcher.search(&board, 3, |depth, _, _, _| {
            depths_seen.push(depth);
        });
        assert_eq!(depths_seen, vec![1, 2, 3]);
    }
}
