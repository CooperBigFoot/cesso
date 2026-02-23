//! Move ordering via MVV-LVA, SEE, killers, history, and continuation history.
//!
//! Score bands ensure correct ordering:
//! - TT move:              100,000
//! - Queen promotion:       30,000
//! - Good captures (SEE >= 0): 10,000 + MVV_LVA (10,007..10,144)
//! - En passant:            10,015
//! - Killer moves:           9,000
//! - Quiet moves (history): bounded by ±HISTORY_MAX plus cont_hist
//! - Bad captures (SEE < 0): -50,000 + see_score (always very negative)

use std::sync::OnceLock;

use cesso_core::{Board, Move, MoveKind, MoveList, PieceKind, PromotionPiece};

use crate::search::heuristics::{cont_hist_score, ContinuationHistory, HistoryTable, KillerTable, StackEntry};
use crate::search::see::{see, see_ge};

/// MVV-LVA scores indexed by `[victim][attacker]`.
///
/// Weights: Pawn=1, Knight=3, Bishop=3, Rook=5, Queen=9, King=0.
/// Formula: `victim_weight * 16 - attacker_weight`.
const MVV_LVA: [[i32; 6]; 6] = [
    // victim = Pawn (weight 1)
    [15, 13, 13, 11, 7, 16],
    // victim = Knight (weight 3)
    [47, 45, 45, 43, 39, 48],
    // victim = Bishop (weight 3)
    [47, 45, 45, 43, 39, 48],
    // victim = Rook (weight 5)
    [79, 77, 77, 75, 71, 80],
    // victim = Queen (weight 9)
    [143, 141, 141, 139, 135, 144],
    // victim = King (weight 0)
    [-1, -3, -3, -5, -9, 0],
];

// ---------------------------------------------------------------------------
// LMR reduction table
// ---------------------------------------------------------------------------

static LMR_TABLE: OnceLock<[[i32; 64]; 64]> = OnceLock::new();

fn lmr_table() -> &'static [[i32; 64]; 64] {
    LMR_TABLE.get_or_init(|| {
        let mut t = [[0i32; 64]; 64];
        for i in 1..64usize {
            for d in 1..64usize {
                t[i][d] =
                    ((0.76 + (i as f64).ln() * (d as f64).ln() / 2.32) * 1024.0) as i32;
            }
        }
        t
    })
}

/// Get the LMR reduction for the given move index and depth (in 1024ths of a ply).
pub fn lmr_reduction(move_index: usize, depth: usize) -> i32 {
    lmr_table()[move_index.min(63)][depth.min(63)]
}

// ---------------------------------------------------------------------------
// Internal scoring helpers
// ---------------------------------------------------------------------------

/// Score a move for the main search using staged score bands and continuation history.
fn score_move_staged(
    board: &Board,
    mv: Move,
    killers: &KillerTable,
    history: &HistoryTable,
    cont_history: &ContinuationHistory,
    stack: &[StackEntry],
    ply: usize,
) -> i32 {
    match mv.kind() {
        MoveKind::Promotion => match mv.promotion_piece() {
            PromotionPiece::Queen => 30_000,
            PromotionPiece::Rook => 170,
            PromotionPiece::Bishop | PromotionPiece::Knight => 160,
        },
        MoveKind::EnPassant => 10_015,
        MoveKind::Castling => 1,
        MoveKind::Normal => {
            if let Some(victim) = board.piece_on(mv.dest()) {
                let see_score = see(board, mv);
                if see_score >= 0 {
                    let attacker = board.piece_on(mv.source()).unwrap_or(PieceKind::Pawn);
                    10_000 + MVV_LVA[victim.index()][attacker.index()]
                } else {
                    -50_000 + see_score
                }
            } else if killers.is_killer(ply, mv) {
                9_000
            } else {
                let piece = board.piece_on(mv.source()).unwrap_or(PieceKind::Pawn);
                let hist = history.score(piece, mv.dest().index());
                let cont = cont_hist_score(cont_history, stack, ply, piece, mv.dest().index());
                hist + cont / 2
            }
        }
    }
}

/// Score a move for quiescence search (no killers or history needed).
pub fn score_move(board: &Board, mv: Move) -> i32 {
    match mv.kind() {
        MoveKind::Promotion => match mv.promotion_piece() {
            PromotionPiece::Queen => 200,
            PromotionPiece::Rook => 170,
            PromotionPiece::Bishop | PromotionPiece::Knight => 160,
        },
        MoveKind::EnPassant => 15,
        MoveKind::Castling => 0,
        MoveKind::Normal => {
            if let Some(victim) = board.piece_on(mv.dest()) {
                let see_score = see(board, mv);
                if see_score >= 0 {
                    let attacker = board.piece_on(mv.source()).unwrap_or(PieceKind::Pawn);
                    1_000 + MVV_LVA[victim.index()][attacker.index()]
                } else {
                    -10_000 + see_score
                }
            } else {
                0
            }
        }
    }
}

// ---------------------------------------------------------------------------
// MovePicker
// ---------------------------------------------------------------------------

/// Incremental move picker using selection sort.
///
/// Yields moves in descending score order. Score bands ensure TT move,
/// good captures, killers, quiets, and bad captures are searched in the
/// correct sequence. For quiescence search, only captures and promotions
/// (score >= 1) are yielded.
pub struct MovePicker {
    moves: [Move; 256],
    scores: [i32; 256],
    len: usize,
    cursor: usize,
    min_score: i32,
}

impl MovePicker {
    /// Create a staged picker that yields all legal moves ordered by priority.
    ///
    /// Scoring uses staged bands:
    /// TT move (100,000) > queen promotions (30,000) > good captures (10,007+) >
    /// killers (9,000) > quiets (history-based) > bad captures (-50,000+).
    pub fn new(
        moves: &MoveList,
        board: &Board,
        tt_move: Move,
        killers: &KillerTable,
        history: &HistoryTable,
        cont_history: &ContinuationHistory,
        stack: &[StackEntry],
        ply: usize,
    ) -> Self {
        let mut picker = Self {
            moves: [Move::NULL; 256],
            scores: [0; 256],
            len: moves.len(),
            cursor: 0,
            min_score: i32::MIN,
        };
        for i in 0..moves.len() {
            picker.moves[i] = moves[i];
            picker.scores[i] = if moves[i] == tt_move {
                100_000
            } else {
                score_move_staged(board, moves[i], killers, history, cont_history, stack, ply)
            };
        }
        picker
    }

    /// Create a picker for quiescence search (captures and promotions only).
    pub fn new_qsearch(moves: &MoveList, board: &Board) -> Self {
        let mut picker = Self {
            moves: [Move::NULL; 256],
            scores: [0; 256],
            len: moves.len(),
            cursor: 0,
            min_score: 1,
        };
        for i in 0..moves.len() {
            picker.moves[i] = moves[i];
            picker.scores[i] = score_move(board, moves[i]);
        }
        picker
    }

    /// Yield the next highest-scored move via selection sort.
    ///
    /// Returns `None` when all remaining moves score below `min_score`
    /// or all moves have been yielded.
    pub fn pick_next(&mut self) -> Option<Move> {
        if self.cursor >= self.len {
            return None;
        }

        let mut best_idx = self.cursor;
        let mut best_score = self.scores[self.cursor];
        for i in (self.cursor + 1)..self.len {
            if self.scores[i] > best_score {
                best_score = self.scores[i];
                best_idx = i;
            }
        }

        if best_score < self.min_score {
            return None;
        }

        self.moves.swap(self.cursor, best_idx);
        self.scores.swap(self.cursor, best_idx);

        let mv = self.moves[self.cursor];
        self.cursor += 1;
        Some(mv)
    }
}

// ---------------------------------------------------------------------------
// ProbCutPicker
// ---------------------------------------------------------------------------

/// Move picker for ProbCut — yields only captures and promotions with SEE >= threshold.
///
/// Ordered by MVV-LVA score. Quiet moves are excluded entirely.
pub struct ProbCutPicker {
    moves: [Move; 256],
    scores: [i32; 256],
    len: usize,
    cursor: usize,
}

impl ProbCutPicker {
    /// Create a ProbCut picker that yields captures/promotions with SEE >= `threshold`.
    pub fn new(moves: &MoveList, board: &Board, threshold: i32) -> Self {
        let mut picker = Self {
            moves: [Move::NULL; 256],
            scores: [0; 256],
            len: 0,
            cursor: 0,
        };

        for i in 0..moves.len() {
            let mv = moves[i];

            let is_tactical = board.piece_on(mv.dest()).is_some()
                || mv.kind() == MoveKind::EnPassant
                || mv.kind() == MoveKind::Promotion;

            if !is_tactical {
                continue;
            }

            if !see_ge(board, mv, threshold) {
                continue;
            }

            let idx = picker.len;
            picker.moves[idx] = mv;
            picker.scores[idx] = if let Some(victim) = board.piece_on(mv.dest()) {
                let attacker = board.piece_on(mv.source()).unwrap_or(PieceKind::Pawn);
                MVV_LVA[victim.index()][attacker.index()]
            } else if mv.kind() == MoveKind::Promotion {
                200
            } else {
                // En passant: pawn captures pawn
                15
            };
            picker.len += 1;
        }

        picker
    }

    /// Yield the next highest-scored move via selection sort.
    pub fn pick_next(&mut self) -> Option<Move> {
        if self.cursor >= self.len {
            return None;
        }

        let mut best_idx = self.cursor;
        let mut best_score = self.scores[self.cursor];
        for i in (self.cursor + 1)..self.len {
            if self.scores[i] > best_score {
                best_score = self.scores[i];
                best_idx = i;
            }
        }

        self.moves.swap(self.cursor, best_idx);
        self.scores.swap(self.cursor, best_idx);

        let mv = self.moves[self.cursor];
        self.cursor += 1;
        Some(mv)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use cesso_core::{generate_legal_moves, Board};
    use crate::search::heuristics::{ContinuationHistory, HistoryTable, KillerTable, StackEntry};

    #[test]
    fn pawn_takes_queen_scores_higher_than_queen_takes_pawn() {
        assert!(MVV_LVA[PieceKind::Queen.index()][PieceKind::Pawn.index()]
            > MVV_LVA[PieceKind::Pawn.index()][PieceKind::Queen.index()]);
    }

    #[test]
    fn lighter_attacker_preferred_for_same_victim() {
        // For capturing a rook: PxR (79) > NxR (77) > QxR (71)
        let pxr = MVV_LVA[PieceKind::Rook.index()][PieceKind::Pawn.index()];
        let nxr = MVV_LVA[PieceKind::Rook.index()][PieceKind::Knight.index()];
        let qxr = MVV_LVA[PieceKind::Rook.index()][PieceKind::Queen.index()];
        assert!(pxr > nxr);
        assert!(nxr > qxr);
    }

    #[test]
    fn queen_promotion_scores_highest() {
        let board: Board = "7k/4P3/8/8/8/8/8/4K3 w - - 0 1".parse().unwrap();
        let moves = generate_legal_moves(&board);
        let mut max_score = i32::MIN;
        for mv in &moves {
            let s = score_move(&board, *mv);
            if s > max_score {
                max_score = s;
            }
        }
        assert_eq!(max_score, 200);
    }

    #[test]
    fn en_passant_scores_correctly() {
        let board: Board = "rnbqkbnr/ppp1pppp/8/3pP3/8/8/PPPP1PPP/RNBQKBNR w KQkq d6 0 3"
            .parse()
            .unwrap();
        let moves = generate_legal_moves(&board);
        let ep_move = moves.as_slice().iter().find(|m| m.kind() == MoveKind::EnPassant);
        assert!(ep_move.is_some(), "should have en passant move available");
        assert_eq!(score_move(&board, *ep_move.unwrap()), 15);
    }

    #[test]
    fn qsearch_picker_empty_on_starting_position() {
        let board = Board::starting_position();
        let moves = generate_legal_moves(&board);
        let mut picker = MovePicker::new_qsearch(&moves, &board);
        assert!(picker.pick_next().is_none());
    }

    #[test]
    fn picker_yields_all_moves_in_starting_position() {
        let board = Board::starting_position();
        let moves = generate_legal_moves(&board);
        let cont_hist = ContinuationHistory::new();
        let stack = [StackEntry::EMPTY; 128];
        let mut picker = MovePicker::new(
            &moves,
            &board,
            Move::NULL,
            &KillerTable::new(),
            &HistoryTable::new(),
            &cont_hist,
            &stack,
            0,
        );
        let mut count = 0;
        while picker.pick_next().is_some() {
            count += 1;
        }
        assert_eq!(count, 20);
    }

    #[test]
    fn picker_yields_captures_before_quiet() {
        // White queen on d4, black pawn on e5 — QxP is a good capture
        let board: Board = "4k3/8/8/4p3/3Q4/8/8/4K3 w - - 0 1".parse().unwrap();
        let moves = generate_legal_moves(&board);
        let cont_hist = ContinuationHistory::new();
        let stack = [StackEntry::EMPTY; 128];
        let mut picker = MovePicker::new(
            &moves,
            &board,
            Move::NULL,
            &KillerTable::new(),
            &HistoryTable::new(),
            &cont_hist,
            &stack,
            0,
        );
        let first = picker.pick_next().unwrap();
        assert!(
            board.piece_on(first.dest()).is_some(),
            "first move from picker should be a capture"
        );
    }

    #[test]
    fn tt_move_yielded_first() {
        let board = Board::starting_position();
        let moves = generate_legal_moves(&board);
        let tt_move = moves[10];
        let cont_hist = ContinuationHistory::new();
        let stack = [StackEntry::EMPTY; 128];
        let mut picker = MovePicker::new(
            &moves,
            &board,
            tt_move,
            &KillerTable::new(),
            &HistoryTable::new(),
            &cont_hist,
            &stack,
            0,
        );
        let first = picker.pick_next().unwrap();
        assert_eq!(first, tt_move, "TT move should be yielded first");
    }

    #[test]
    fn probcut_picker_filters_by_see() {
        // White queen on d4, black pawn on e5 — QxP capture has positive SEE
        let board: Board = "4k3/8/8/4p3/3Q4/8/8/4K3 w - - 0 1".parse().unwrap();
        let moves = generate_legal_moves(&board);
        let mut picker = ProbCutPicker::new(&moves, &board, 0);
        let mut count = 0;
        while picker.pick_next().is_some() {
            count += 1;
        }
        assert!(count >= 1, "should have at least one good capture");
    }

    #[test]
    fn lmr_reduction_increases_with_depth_and_move_count() {
        let r_low = lmr_reduction(2, 3);
        let r_high = lmr_reduction(10, 10);
        assert!(r_high > r_low, "deeper searches with more moves should reduce more");
        assert!(r_low > 0, "should have some reduction at depth 3, move 2");
    }
}
