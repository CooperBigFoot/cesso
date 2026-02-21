//! Killer move table and history heuristic for quiet move ordering.

use cesso_core::{Move, PieceKind};

use crate::search::negamax::MAX_PLY;

/// Two killer moves per ply — quiet moves that caused beta cutoffs.
pub struct KillerTable {
    slots: [[Move; 2]; MAX_PLY],
}

impl KillerTable {
    /// Create an empty killer table.
    pub fn new() -> Self {
        Self {
            slots: [[Move::NULL; 2]; MAX_PLY],
        }
    }

    /// Store a killer move at the given ply.
    ///
    /// Shifts slot 0 to slot 1 if the new move differs from slot 0.
    pub fn store(&mut self, ply: usize, mv: Move) {
        if ply >= MAX_PLY {
            return;
        }
        if self.slots[ply][0] != mv {
            self.slots[ply][1] = self.slots[ply][0];
            self.slots[ply][0] = mv;
        }
    }

    /// Check if a move is a killer at the given ply.
    pub fn is_killer(&self, ply: usize, mv: Move) -> bool {
        if ply >= MAX_PLY {
            return false;
        }
        self.slots[ply][0] == mv || self.slots[ply][1] == mv
    }
}

impl Default for KillerTable {
    fn default() -> Self {
        Self::new()
    }
}

/// Maximum absolute value for history scores (prevents overflow).
const HISTORY_MAX: i32 = 16_384;

/// History heuristic table — indexed by `[piece_kind][to_square]`.
///
/// Rewards quiet moves that cause beta cutoffs, penalises those that don't.
pub struct HistoryTable {
    table: [[i32; 64]; 6],
}

impl HistoryTable {
    /// Create a zeroed history table.
    pub fn new() -> Self {
        Self {
            table: [[0; 64]; 6],
        }
    }

    /// Reward a quiet move that caused a beta cutoff.
    pub fn update_good(&mut self, piece: PieceKind, to: usize, depth: u8) {
        let bonus = (depth as i32) * (depth as i32);
        let entry = &mut self.table[piece.index()][to];
        *entry = (*entry + bonus).min(HISTORY_MAX);
    }

    /// Penalise a quiet move that was searched but did not cause a cutoff.
    pub fn update_bad(&mut self, piece: PieceKind, to: usize, depth: u8) {
        let penalty = (depth as i32) * (depth as i32);
        let entry = &mut self.table[piece.index()][to];
        *entry = (*entry - penalty).max(-HISTORY_MAX);
    }

    /// Get the history score for a quiet move.
    pub fn score(&self, piece: PieceKind, to: usize) -> i32 {
        self.table[piece.index()][to]
    }
}

impl Default for HistoryTable {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cesso_core::{Move, PieceKind, Square};

    #[test]
    fn killer_store_and_check() {
        let mut kt = KillerTable::new();
        let mv1 = Move::new(Square::E2, Square::E4);
        let mv2 = Move::new(Square::D2, Square::D4);

        kt.store(5, mv1);
        assert!(kt.is_killer(5, mv1));
        assert!(!kt.is_killer(5, mv2));

        // Store a second killer — mv1 should shift to slot 1
        kt.store(5, mv2);
        assert!(kt.is_killer(5, mv1));
        assert!(kt.is_killer(5, mv2));
    }

    #[test]
    fn killer_same_move_no_shift() {
        let mut kt = KillerTable::new();
        let mv1 = Move::new(Square::E2, Square::E4);
        let mv2 = Move::new(Square::D2, Square::D4);

        kt.store(0, mv1);
        kt.store(0, mv2);
        // Storing mv2 again should not shift
        kt.store(0, mv2);
        // mv1 should still be in slot 1
        assert!(kt.is_killer(0, mv1));
        assert!(kt.is_killer(0, mv2));
    }

    #[test]
    fn killer_different_plies_independent() {
        let mut kt = KillerTable::new();
        let mv = Move::new(Square::E2, Square::E4);
        kt.store(3, mv);
        assert!(kt.is_killer(3, mv));
        assert!(!kt.is_killer(4, mv));
    }

    #[test]
    fn history_update_good_and_bad() {
        let mut ht = HistoryTable::new();
        assert_eq!(ht.score(PieceKind::Knight, 20), 0);

        ht.update_good(PieceKind::Knight, 20, 4);
        assert_eq!(ht.score(PieceKind::Knight, 20), 16); // 4*4

        ht.update_bad(PieceKind::Knight, 20, 3);
        assert_eq!(ht.score(PieceKind::Knight, 20), 7); // 16 - 9
    }

    #[test]
    fn history_clamped() {
        let mut ht = HistoryTable::new();
        // Spam enough updates to exceed HISTORY_MAX
        for _ in 0..200 {
            ht.update_good(PieceKind::Pawn, 0, 10);
        }
        assert!(ht.score(PieceKind::Pawn, 0) <= 16_384);

        // And negative clamping
        for _ in 0..400 {
            ht.update_bad(PieceKind::Pawn, 0, 10);
        }
        assert!(ht.score(PieceKind::Pawn, 0) >= -16_384);
    }
}
