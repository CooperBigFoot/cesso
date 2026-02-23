//! Killer move table, history heuristic, continuation history, and correction history.

use cesso_core::{Color, Move, PieceKind, Square};

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
pub const HISTORY_MAX: i32 = 16_384;

/// Apply gravity update: `entry += bonus - entry * |bonus| / HISTORY_MAX`.
///
/// Keeps history scores bounded without a hard clamp by pulling values
/// toward zero at a rate proportional to their magnitude.
fn apply_gravity(entry: &mut i32, bonus: i32) {
    *entry += bonus - *entry * bonus.abs() / HISTORY_MAX;
}

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

    /// Update history score using gravity formula.
    pub fn update(&mut self, piece: PieceKind, to: usize, bonus: i32) {
        apply_gravity(&mut self.table[piece.index()][to], bonus);
    }

    /// Deprecated: use `update` with a positive bonus instead.
    pub fn update_good(&mut self, piece: PieceKind, to: usize, depth: u8) {
        let bonus = (depth as i32) * (depth as i32);
        self.update(piece, to, bonus);
    }

    /// Deprecated: use `update` with a negative bonus instead.
    pub fn update_bad(&mut self, piece: PieceKind, to: usize, depth: u8) {
        let penalty = (depth as i32) * (depth as i32);
        self.update(piece, to, -penalty);
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

// ---------------------------------------------------------------------------
// Continuation history
// ---------------------------------------------------------------------------

/// Key for continuation history lookup.
#[derive(Debug, Clone, Copy)]
pub struct ContHistIndex {
    pub side: Color,
    pub piece: PieceKind,
    pub to: Square,
}

/// Inner leaf table `[6][64]` of `i32` for continuation history.
pub struct ContHistEntry {
    table: [[i32; 64]; 6],
}

impl ContHistEntry {
    /// Get the history score for a piece moving to a square.
    #[inline]
    pub fn score(&self, piece: PieceKind, to: usize) -> i32 {
        self.table[piece.index()][to]
    }

    /// Get a mutable reference to a history entry.
    #[inline]
    pub fn entry_mut(&mut self, piece: PieceKind, to: usize) -> &mut i32 {
        &mut self.table[piece.index()][to]
    }
}

/// Continuation history table: `[color][piece][square] -> ContHistEntry`.
///
/// ~1.125 MB — must be heap-allocated.
pub struct ContinuationHistory {
    table: Box<[[[ContHistEntry; 64]; 6]; 2]>,
}

impl ContinuationHistory {
    /// Create a zeroed continuation history.
    pub fn new() -> Self {
        use std::alloc::{Layout, alloc_zeroed};
        let layout = Layout::new::<[[[ContHistEntry; 64]; 6]; 2]>();
        let ptr = unsafe { alloc_zeroed(layout) as *mut [[[ContHistEntry; 64]; 6]; 2] };
        if ptr.is_null() {
            std::alloc::handle_alloc_error(layout);
        }
        Self {
            table: unsafe { Box::from_raw(ptr) },
        }
    }

    /// Get a reference to the [`ContHistEntry`] for the given index.
    #[inline]
    pub fn entry(&self, idx: &ContHistIndex) -> &ContHistEntry {
        &self.table[idx.side.index()][idx.piece.index()][idx.to.index()]
    }

    /// Get a mutable reference to the [`ContHistEntry`] for the given index.
    #[inline]
    pub fn entry_mut(&mut self, idx: &ContHistIndex) -> &mut ContHistEntry {
        &mut self.table[idx.side.index()][idx.piece.index()][idx.to.index()]
    }
}

impl Default for ContinuationHistory {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Per-ply search stack entry
// ---------------------------------------------------------------------------

/// Per-ply search state, stored in an array on SearchContext.
#[derive(Clone, Copy)]
pub struct StackEntry {
    /// Static evaluation at this ply.
    pub static_eval: i32,
    /// Move being searched at this ply.
    pub current_move: Move,
    /// Piece kind that moved.
    pub moved_piece: PieceKind,
    /// Move excluded from search (for singular extensions).
    pub excluded_move: Move,
    /// Number of beta cutoffs found at this ply.
    pub cutoff_count: u16,
    /// Key for continuation history lookup.
    pub cont_hist_index: Option<ContHistIndex>,
}

impl StackEntry {
    /// Empty stack entry (all zeroed/null).
    pub const EMPTY: Self = Self {
        static_eval: 0,
        current_move: Move::NULL,
        moved_piece: PieceKind::Pawn,
        excluded_move: Move::NULL,
        cutoff_count: 0,
        cont_hist_index: None,
    };
}

// ---------------------------------------------------------------------------
// Correction history
// ---------------------------------------------------------------------------

/// Maximum absolute value for correction history entries.
const MAX_CORRHIST: i32 = 1024;
/// Number of correction history buckets (hash & 0x3FFF).
const CORR_BUCKETS: usize = 16384;
/// Correction history weights for combining multiple tables.
const CORR_WEIGHTS: [i32; 6] = [117, 134, 134, 61, 67, 140];
/// Divisor for weighted correction sum.
const CORR_DIVISOR: i32 = 2048;

/// Eval correction history tables.
///
/// ~643 KB — must be heap-allocated.
pub struct CorrectionHistory {
    pawn: Box<[[i32; CORR_BUCKETS]; 2]>,
    non_pawn: Box<[[[i32; CORR_BUCKETS]; 2]; 2]>,
    major: Box<[[i32; CORR_BUCKETS]; 2]>,
    minor: Box<[[i32; CORR_BUCKETS]; 2]>,
    cont: Box<[[[i32; 64]; 6]; 2]>,
}

impl CorrectionHistory {
    /// Create a zeroed correction history.
    pub fn new() -> Self {
        Self {
            pawn: Self::alloc_zeroed_box(),
            non_pawn: Self::alloc_zeroed_box(),
            major: Self::alloc_zeroed_box(),
            minor: Self::alloc_zeroed_box(),
            cont: Self::alloc_zeroed_box(),
        }
    }

    /// Allocate a zeroed box of any sized type.
    fn alloc_zeroed_box<T>() -> Box<T> {
        use std::alloc::{Layout, alloc_zeroed};
        let layout = Layout::new::<T>();
        let ptr = unsafe { alloc_zeroed(layout) as *mut T };
        if ptr.is_null() {
            std::alloc::handle_alloc_error(layout);
        }
        unsafe { Box::from_raw(ptr) }
    }

    /// Apply correction to a raw static eval.
    pub fn correct_eval(
        &self,
        side: Color,
        pawn_hash: u64,
        np_white_hash: u64,
        np_black_hash: u64,
        major_hash: u64,
        minor_hash: u64,
        prev_piece: Option<PieceKind>,
        prev_dest: Option<Square>,
        raw_eval: i32,
    ) -> i32 {
        let s = side.index();
        let ph = (pawn_hash & (CORR_BUCKETS as u64 - 1)) as usize;
        let nph_w = (np_white_hash & (CORR_BUCKETS as u64 - 1)) as usize;
        let nph_b = (np_black_hash & (CORR_BUCKETS as u64 - 1)) as usize;
        let majh = (major_hash & (CORR_BUCKETS as u64 - 1)) as usize;
        let minh = (minor_hash & (CORR_BUCKETS as u64 - 1)) as usize;

        let mut correction = 0i32;
        correction += CORR_WEIGHTS[0] * self.pawn[s][ph];
        correction += CORR_WEIGHTS[1] * self.non_pawn[s][0][nph_w];
        correction += CORR_WEIGHTS[2] * self.non_pawn[s][1][nph_b];
        correction += CORR_WEIGHTS[3] * self.major[s][majh];
        correction += CORR_WEIGHTS[4] * self.minor[s][minh];

        if let (Some(piece), Some(dest)) = (prev_piece, prev_dest) {
            correction += CORR_WEIGHTS[5] * self.cont[s][piece.index()][dest.index()];
        }

        raw_eval + correction / CORR_DIVISOR
    }

    /// Update correction history tables after a search.
    pub fn update(
        &mut self,
        side: Color,
        pawn_hash: u64,
        np_white_hash: u64,
        np_black_hash: u64,
        major_hash: u64,
        minor_hash: u64,
        prev_piece: Option<PieceKind>,
        prev_dest: Option<Square>,
        score_diff: i32,
    ) {
        let bonus = score_diff.clamp(-256, 256);
        let s = side.index();
        let ph = (pawn_hash & (CORR_BUCKETS as u64 - 1)) as usize;
        let nph_w = (np_white_hash & (CORR_BUCKETS as u64 - 1)) as usize;
        let nph_b = (np_black_hash & (CORR_BUCKETS as u64 - 1)) as usize;
        let majh = (major_hash & (CORR_BUCKETS as u64 - 1)) as usize;
        let minh = (minor_hash & (CORR_BUCKETS as u64 - 1)) as usize;

        Self::apply_corr_gravity(&mut self.pawn[s][ph], bonus);
        Self::apply_corr_gravity(&mut self.non_pawn[s][0][nph_w], bonus);
        Self::apply_corr_gravity(&mut self.non_pawn[s][1][nph_b], bonus);
        Self::apply_corr_gravity(&mut self.major[s][majh], bonus);
        Self::apply_corr_gravity(&mut self.minor[s][minh], bonus);

        if let (Some(piece), Some(dest)) = (prev_piece, prev_dest) {
            Self::apply_corr_gravity(&mut self.cont[s][piece.index()][dest.index()], bonus);
        }
    }

    fn apply_corr_gravity(entry: &mut i32, bonus: i32) {
        *entry += bonus - *entry * bonus.abs() / MAX_CORRHIST;
        *entry = (*entry).clamp(-MAX_CORRHIST, MAX_CORRHIST);
    }
}

impl Default for CorrectionHistory {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Continuation history helpers
// ---------------------------------------------------------------------------

/// Sum continuation history scores from plies -1, -2, -3, -4, -6 relative to current ply.
pub fn cont_hist_score(
    cont_history: &ContinuationHistory,
    stack: &[StackEntry],
    ply: usize,
    piece: PieceKind,
    to: usize,
) -> i32 {
    let offsets: [usize; 5] = [1, 2, 3, 4, 6];
    let mut score = 0i32;
    for &offset in &offsets {
        if ply >= offset {
            if let Some(idx) = &stack[ply - offset].cont_hist_index {
                score += cont_history.entry(idx).score(piece, to);
            }
        }
    }
    score
}

/// Update continuation history at plies -1, -2, -3, -4, -6 relative to current ply.
pub fn update_cont_history(
    cont_history: &mut ContinuationHistory,
    stack: &[StackEntry],
    ply: usize,
    piece: PieceKind,
    to: usize,
    bonus: i32,
) {
    let offsets: [usize; 5] = [1, 2, 3, 4, 6];
    for &offset in &offsets {
        if ply >= offset {
            if let Some(idx) = &stack[ply - offset].cont_hist_index {
                let idx_owned = *idx;
                let entry = cont_history.entry_mut(&idx_owned);
                apply_gravity(entry.entry_mut(piece, to), bonus);
            }
        }
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
    fn history_update_and_score() {
        let mut ht = HistoryTable::new();
        assert_eq!(ht.score(PieceKind::Knight, 20), 0);

        // Positive bonus (like depth^2 for good move)
        ht.update(PieceKind::Knight, 20, 16);
        assert!(ht.score(PieceKind::Knight, 20) > 0);

        // Negative bonus (penalty for bad move)
        ht.update(PieceKind::Knight, 20, -9);
        // Score should have decreased
    }

    #[test]
    fn history_gravity_bounded() {
        let mut ht = HistoryTable::new();
        // Spam positive updates
        for _ in 0..200 {
            ht.update(PieceKind::Pawn, 0, 100);
        }
        assert!(ht.score(PieceKind::Pawn, 0) <= HISTORY_MAX);
        assert!(ht.score(PieceKind::Pawn, 0) > 0);

        // Spam negative updates
        for _ in 0..400 {
            ht.update(PieceKind::Pawn, 0, -100);
        }
        assert!(ht.score(PieceKind::Pawn, 0) >= -HISTORY_MAX);
    }

    #[test]
    fn apply_gravity_converges() {
        let mut entry = 0i32;
        // Repeated positive bonuses should converge toward HISTORY_MAX
        for _ in 0..1000 {
            apply_gravity(&mut entry, 400);
        }
        // Should be close to HISTORY_MAX but not exceed it
        assert!(entry > HISTORY_MAX / 2);
        assert!(entry <= HISTORY_MAX);
    }

    #[test]
    fn stack_entry_empty_is_zeroed() {
        let entry = StackEntry::EMPTY;
        assert_eq!(entry.static_eval, 0);
        assert!(entry.current_move.is_null());
        assert_eq!(entry.moved_piece, PieceKind::Pawn);
        assert!(entry.excluded_move.is_null());
        assert_eq!(entry.cutoff_count, 0);
        assert!(entry.cont_hist_index.is_none());
    }

    #[test]
    fn cont_hist_entry_read_write() {
        let mut ch = ContinuationHistory::new();
        let idx = ContHistIndex {
            side: Color::White,
            piece: PieceKind::Knight,
            to: Square::from_index(28).unwrap(), // e4
        };

        // Initially zero
        assert_eq!(ch.entry(&idx).score(PieceKind::Pawn, 20), 0);

        // Write and read back
        *ch.entry_mut(&idx).entry_mut(PieceKind::Pawn, 20) = 42;
        assert_eq!(ch.entry(&idx).score(PieceKind::Pawn, 20), 42);
    }

    #[test]
    fn correction_history_zeroed_gives_no_correction() {
        let ch = CorrectionHistory::new();
        let corrected = ch.correct_eval(
            Color::White, 0x1234, 0x5678, 0x9ABC, 0xDEF0, 0x1111,
            None, None, 100,
        );
        assert_eq!(corrected, 100, "zeroed correction should not modify eval");
    }

    #[test]
    fn correction_history_update_then_correct() {
        let mut ch = CorrectionHistory::new();
        // Update with a positive score diff
        ch.update(
            Color::White, 0x1234, 0x5678, 0x9ABC, 0xDEF0, 0x1111,
            None, None, 200,
        );
        // Now correction should shift eval upward
        let corrected = ch.correct_eval(
            Color::White, 0x1234, 0x5678, 0x9ABC, 0xDEF0, 0x1111,
            None, None, 100,
        );
        assert!(corrected > 100, "positive correction should increase eval, got {corrected}");
    }
}
