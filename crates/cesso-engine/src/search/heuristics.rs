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

    /// Apply a gravity-adjusted bonus to a history entry.
    fn apply_gravity(entry: &mut i32, bonus: i32) {
        *entry += bonus - (*entry).abs() * bonus / HISTORY_MAX;
    }

    /// Update a history entry with a raw bonus (positive = reward, negative = penalise).
    pub fn update(&mut self, piece: PieceKind, to: usize, bonus: i32) {
        let entry = &mut self.table[piece.index()][to];
        Self::apply_gravity(entry, bonus);
        *entry = (*entry).clamp(-HISTORY_MAX, HISTORY_MAX);
    }

    /// Reward a quiet move that caused a beta cutoff.
    pub fn update_good(&mut self, piece: PieceKind, to: usize, depth: u8) {
        let bonus = (depth as i32) * (depth as i32);
        self.update(piece, to, bonus);
    }

    /// Penalise a quiet move that was searched but did not cause a cutoff.
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

/// Index into the continuation history table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContHistIndex {
    pub side: Color,
    pub piece: PieceKind,
    pub to: Square,
}

/// Per-move continuation history scores — indexed by `[piece][to_square]`.
pub struct ContHistEntry {
    table: [[i32; 64]; 6],
}

impl ContHistEntry {
    /// Get the continuation history score for a (piece, to) pair.
    pub fn score(&self, piece: PieceKind, to: Square) -> i32 {
        self.table[piece.index()][to.index()]
    }

    /// Get a mutable reference to the continuation history entry.
    pub fn entry_mut(&mut self, piece: PieceKind, to: Square) -> &mut i32 {
        &mut self.table[piece.index()][to.index()]
    }
}

/// Continuation history table.
///
/// Indexed by the previous move's (side, piece, to) to score the current move's
/// (piece, to) pair. Used to improve move ordering by conditioning on predecessor moves.
pub struct ContinuationHistory {
    /// [side][piece][to][piece][to] — flat 2D array of ContHistEntry.
    /// Outer dim: Color::COUNT * 6 * 64, inner dim: ContHistEntry.
    table: Box<[[[ContHistEntry; 64]; 6]; 2]>,
}

impl ContinuationHistory {
    /// Create a zeroed continuation history table.
    pub fn new() -> Self {
        // Can't use derive Default for large arrays, so initialise manually via unsafe zero-fill
        // Safety: ContHistEntry is POD (all i32), zeroing is valid.
        let table = unsafe {
            let layout = std::alloc::Layout::new::<[[[ContHistEntry; 64]; 6]; 2]>();
            let ptr = std::alloc::alloc_zeroed(layout);
            if ptr.is_null() {
                std::alloc::handle_alloc_error(layout);
            }
            Box::from_raw(ptr as *mut [[[ContHistEntry; 64]; 6]; 2])
        };
        Self { table }
    }

    /// Get the entry for a given predecessor index.
    pub fn entry(&self, idx: &ContHistIndex) -> &ContHistEntry {
        &self.table[idx.side.index()][idx.piece.index()][idx.to.index()]
    }

    /// Get a mutable entry for a given predecessor index.
    pub fn entry_mut(&mut self, idx: &ContHistIndex) -> &mut ContHistEntry {
        &mut self.table[idx.side.index()][idx.piece.index()][idx.to.index()]
    }
}

impl Default for ContinuationHistory {
    fn default() -> Self {
        Self::new()
    }
}

/// Stack entry tracking per-ply search information.
#[derive(Clone, Copy)]
pub struct StackEntry {
    /// Static evaluation at this ply.
    pub static_eval: i32,
    /// The move made to reach this position.
    pub current_move: Move,
    /// The piece that moved.
    pub moved_piece: PieceKind,
    /// Excluded move (for singular extension search).
    pub excluded_move: Move,
    /// Number of beta cutoffs at ply+1 during this node's search.
    pub cutoff_count: u32,
    /// Index for continuation history lookup.
    pub cont_hist_index: Option<ContHistIndex>,
}

impl StackEntry {
    /// Empty stack entry for initialisation.
    pub const EMPTY: Self = Self {
        static_eval: 0,
        current_move: Move::NULL,
        moved_piece: PieceKind::Pawn,
        excluded_move: Move::NULL,
        cutoff_count: 0,
        cont_hist_index: None,
    };
}

/// Compute the continuation history score for the current (piece, to) pair.
///
/// Looks back 1 and 2 plies and sums the scores from those predecessor entries.
pub fn cont_hist_score(
    cont_history: &ContinuationHistory,
    stack: &[StackEntry],
    ply: usize,
    piece: PieceKind,
    to: usize,
) -> i32 {
    let to_sq = match Square::from_index(to as u8) {
        Some(sq) => sq,
        None => return 0,
    };
    let mut score = 0i32;
    // 1-ply continuation
    if ply >= 1 {
        if let Some(idx) = stack[ply - 1].cont_hist_index {
            score += cont_history.entry(&idx).score(piece, to_sq);
        }
    }
    // 2-ply continuation
    if ply >= 2 {
        if let Some(idx) = stack[ply - 2].cont_hist_index {
            score += cont_history.entry(&idx).score(piece, to_sq);
        }
    }
    score
}

/// Update the continuation history entries for the current (piece, to) pair.
pub fn update_cont_history(
    cont_history: &mut ContinuationHistory,
    stack: &[StackEntry],
    ply: usize,
    piece: PieceKind,
    to: usize,
    bonus: i32,
) {
    let to_sq = match Square::from_index(to as u8) {
        Some(sq) => sq,
        None => return,
    };
    // 1-ply continuation
    if ply >= 1 {
        if let Some(idx) = stack[ply - 1].cont_hist_index {
            let entry = cont_history.entry_mut(&idx).entry_mut(piece, to_sq);
            *entry += bonus - (*entry).abs() * bonus.abs() / HISTORY_MAX;
            *entry = (*entry).clamp(-HISTORY_MAX, HISTORY_MAX);
        }
    }
    // 2-ply continuation
    if ply >= 2 {
        if let Some(idx) = stack[ply - 2].cont_hist_index {
            let entry = cont_history.entry_mut(&idx).entry_mut(piece, to_sq);
            *entry += bonus - (*entry).abs() * bonus.abs() / HISTORY_MAX;
            *entry = (*entry).clamp(-HISTORY_MAX, HISTORY_MAX);
        }
    }
}

// ── Correction History ────────────────────────────────────────────────────────

/// Number of buckets in each correction history sub-table.
const CORR_SIZE: usize = 16384;

/// Correction history sub-table indexed by a hash bucket.
struct CorrTable {
    data: Box<[i32; CORR_SIZE]>,
}

impl CorrTable {
    fn new() -> Self {
        Self {
            data: Box::new([0i32; CORR_SIZE]),
        }
    }

    fn get(&self, hash: u64) -> i32 {
        self.data[(hash as usize) & (CORR_SIZE - 1)]
    }

    fn update(&mut self, hash: u64, delta: i32, weight: i32) {
        let idx = (hash as usize) & (CORR_SIZE - 1);
        let entry = &mut self.data[idx];
        // Weighted update with gravity
        *entry = (*entry * (256 - weight) + delta * weight) / 256;
        *entry = (*entry).clamp(-HISTORY_MAX, HISTORY_MAX);
    }
}

/// Correction history for refining static evaluation.
///
/// Uses pawn structure hash, non-pawn material hashes, major/minor piece hashes,
/// and a "continuation correction" (conditioned on the previous move) to
/// adjust the raw static evaluation toward the true search score.
pub struct CorrectionHistory {
    pawn: [CorrTable; 2],
    non_pawn_white: [CorrTable; 2],
    non_pawn_black: [CorrTable; 2],
    major: [CorrTable; 2],
    minor: [CorrTable; 2],
    cont: [CorrTable; 2],
}

impl CorrectionHistory {
    /// Create a zeroed correction history.
    pub fn new() -> Self {
        Self {
            pawn: [CorrTable::new(), CorrTable::new()],
            non_pawn_white: [CorrTable::new(), CorrTable::new()],
            non_pawn_black: [CorrTable::new(), CorrTable::new()],
            major: [CorrTable::new(), CorrTable::new()],
            minor: [CorrTable::new(), CorrTable::new()],
            cont: [CorrTable::new(), CorrTable::new()],
        }
    }

    /// Correct a raw static evaluation using the accumulated correction data.
    #[allow(clippy::too_many_arguments)]
    pub fn correct_eval(
        &self,
        side: Color,
        pawn_hash: u64,
        non_pawn_hash_white: u64,
        non_pawn_hash_black: u64,
        major_hash: u64,
        minor_hash: u64,
        prev_piece: Option<PieceKind>,
        prev_dest: Option<Square>,
        raw_eval: i32,
    ) -> i32 {
        let si = side.index();
        let mut correction = 0i32;

        correction += self.pawn[si].get(pawn_hash);
        correction += self.non_pawn_white[si].get(non_pawn_hash_white);
        correction += self.non_pawn_black[si].get(non_pawn_hash_black);
        correction += self.major[si].get(major_hash);
        correction += self.minor[si].get(minor_hash);

        // Continuation correction
        if let (Some(piece), Some(dest)) = (prev_piece, prev_dest) {
            let cont_key = (piece.index() as u64) * 64 + dest.index() as u64;
            correction += self.cont[si].get(cont_key);
        }

        // Scale: correction values are in units of 1/256 centipawns
        raw_eval + correction / 256
    }

    /// Update correction history after a completed search node.
    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &mut self,
        side: Color,
        pawn_hash: u64,
        non_pawn_hash_white: u64,
        non_pawn_hash_black: u64,
        major_hash: u64,
        minor_hash: u64,
        prev_piece: Option<PieceKind>,
        prev_dest: Option<Square>,
        score_diff: i32,
    ) {
        let si = side.index();
        let delta = score_diff * 256;
        let weight = 16;

        self.pawn[si].update(pawn_hash, delta, weight);
        self.non_pawn_white[si].update(non_pawn_hash_white, delta, weight);
        self.non_pawn_black[si].update(non_pawn_hash_black, delta, weight);
        self.major[si].update(major_hash, delta, weight);
        self.minor[si].update(minor_hash, delta, weight);

        if let (Some(piece), Some(dest)) = (prev_piece, prev_dest) {
            let cont_key = (piece.index() as u64) * 64 + dest.index() as u64;
            self.cont[si].update(cont_key, delta, weight);
        }
    }
}

impl Default for CorrectionHistory {
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
        assert!(ht.score(PieceKind::Knight, 20) > 0);

        ht.update_bad(PieceKind::Knight, 20, 3);
        // Score should have decreased
        let s = ht.score(PieceKind::Knight, 20);
        assert!(s < 16);
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

    #[test]
    fn cont_hist_score_zero_at_ply_0() {
        let cont = ContinuationHistory::new();
        let stack = [StackEntry::EMPTY; MAX_PLY];
        let score = cont_hist_score(&cont, &stack, 0, PieceKind::Knight, 20);
        assert_eq!(score, 0);
    }

    #[test]
    fn correction_history_default_is_zero() {
        let corr = CorrectionHistory::new();
        let result = corr.correct_eval(
            Color::White, 0, 0, 0, 0, 0, None, None, 100,
        );
        assert_eq!(result, 100);
    }
}
