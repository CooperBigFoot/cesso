//! Move ordering via MVV-LVA (Most Valuable Victim - Least Valuable Attacker).

use cesso_core::{Board, Move, MoveKind, MoveList, PieceKind, PromotionPiece};

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

/// Score a move for ordering purposes.
///
/// Higher scores are searched first. Score bands:
/// - Queen promotion: 200
/// - Rook promotion: 170
/// - Bishop/Knight promotion: 160
/// - Captures (MVV-LVA): 7..144
/// - En passant: 15
/// - Quiet / Castling: 0
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
                let attacker = board.piece_on(mv.source()).unwrap_or(PieceKind::Pawn);
                MVV_LVA[victim.index()][attacker.index()]
            } else {
                0
            }
        }
    }
}

/// Incremental move picker using selection sort.
///
/// Yields moves in descending score order. For quiescence search,
/// only yields moves with `score >= 1` (captures and promotions).
pub struct MovePicker {
    moves: [Move; 256],
    scores: [i32; 256],
    len: usize,
    cursor: usize,
    min_score: i32,
}

impl MovePicker {
    /// Create a picker that yields all legal moves, ordered by score.
    ///
    /// If `tt_move` is not null and matches a move in the list, it receives
    /// the highest priority score (10,000), ensuring it is searched first.
    pub fn new(moves: &MoveList, board: &Board, tt_move: Move) -> Self {
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
                10_000
            } else {
                score_move(board, moves[i])
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

        // Find the index of the maximum score in cursor..len
        let mut best_idx = self.cursor;
        let mut best_score = self.scores[self.cursor];
        for i in (self.cursor + 1)..self.len {
            if self.scores[i] > best_score {
                best_score = self.scores[i];
                best_idx = i;
            }
        }

        // Check minimum score threshold
        if best_score < self.min_score {
            return None;
        }

        // Swap the best to cursor position
        self.moves.swap(self.cursor, best_idx);
        self.scores.swap(self.cursor, best_idx);

        let mv = self.moves[self.cursor];
        self.cursor += 1;
        Some(mv)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cesso_core::{generate_legal_moves, Board};

    #[test]
    fn pawn_takes_queen_scores_higher_than_queen_takes_pawn() {
        // PxQ should score 143, QxP should score 7
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
        // Find the en passant move
        let ep_move = moves.as_slice().iter().find(|m| m.kind() == MoveKind::EnPassant);
        assert!(ep_move.is_some(), "should have en passant move available");
        assert_eq!(score_move(&board, *ep_move.unwrap()), 15);
    }

    #[test]
    fn qsearch_picker_empty_on_starting_position() {
        let board = Board::starting_position();
        let moves = generate_legal_moves(&board);
        let mut picker = MovePicker::new_qsearch(&moves, &board);
        // Starting position has no captures or promotions
        assert!(picker.pick_next().is_none());
    }

    #[test]
    fn picker_yields_all_moves_in_starting_position() {
        let board = Board::starting_position();
        let moves = generate_legal_moves(&board);
        let mut picker = MovePicker::new(&moves, &board, Move::NULL);
        let mut count = 0;
        while picker.pick_next().is_some() {
            count += 1;
        }
        assert_eq!(count, 20); // 20 legal moves in starting position
    }

    #[test]
    fn picker_yields_captures_before_quiet() {
        // Position with both captures and quiet moves
        // White queen on d4, black pawn on e5 — QxP is a capture
        let board: Board = "4k3/8/8/4p3/3Q4/8/8/4K3 w - - 0 1".parse().unwrap();
        let moves = generate_legal_moves(&board);
        let mut picker = MovePicker::new(&moves, &board, Move::NULL);
        let first = picker.pick_next().unwrap();
        // First move should be the capture (highest scored)
        assert!(
            board.piece_on(first.dest()).is_some(),
            "first move from picker should be a capture"
        );
    }
}
