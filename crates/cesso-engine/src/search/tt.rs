//! Transposition table with bounded-depth replacement.

use cesso_core::Move;

/// Bound type stored in a TT entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Bound {
    /// No bound information (empty entry).
    None = 0,
    /// The stored score is exact (PV node).
    Exact = 1,
    /// The stored score is a lower bound (failed high / beta cutoff).
    LowerBound = 2,
    /// The stored score is an upper bound (failed low / all-node).
    UpperBound = 3,
}

impl Bound {
    const fn from_bits(bits: u8) -> Self {
        match bits & 0x03 {
            1 => Bound::Exact,
            2 => Bound::LowerBound,
            3 => Bound::UpperBound,
            _ => Bound::None,
        }
    }
}

/// Scores above this threshold indicate a forced mate.
const MATE_THRESHOLD: i32 = 28_000;

/// A single transposition table entry — exactly 16 bytes.
///
/// Layout:
/// - `key`: upper 32 bits of the Zobrist hash (for collision detection)
/// - `data`: packed bits — move(16) | depth(8) | bound(2) | generation(6)
/// - `score`: mate-adjusted search score
/// - `eval`: static evaluation (for future pruning techniques)
#[derive(Clone, Copy)]
#[repr(C)]
pub struct TtEntry {
    key: u32,
    data: u32,
    score: i16,
    eval: i16,
    _padding: u32,
}

impl Default for TtEntry {
    fn default() -> Self {
        Self {
            key: 0,
            data: 0,
            score: 0,
            eval: 0,
            _padding: 0,
        }
    }
}

impl TtEntry {
    /// Pack data fields into the `data` u32.
    fn pack_data(mv: Move, depth: u8, bound: Bound, generation: u8) -> u32 {
        let mv_bits = mv.raw() as u32;
        let depth_bits = (depth as u32) << 16;
        let bound_bits = ((bound as u8) as u32) << 24;
        let gen_bits = ((generation & 0x3F) as u32) << 26;
        mv_bits | depth_bits | bound_bits | gen_bits
    }

    /// Extract the best move from the packed data.
    fn move_bits(&self) -> Move {
        Move::from_raw((self.data & 0xFFFF) as u16)
    }

    /// Extract the search depth from the packed data.
    fn depth(&self) -> u8 {
        ((self.data >> 16) & 0xFF) as u8
    }

    /// Extract the bound type from the packed data.
    fn bound(&self) -> Bound {
        Bound::from_bits(((self.data >> 24) & 0x03) as u8)
    }

    /// Extract the generation counter from the packed data.
    fn generation(&self) -> u8 {
        ((self.data >> 26) & 0x3F) as u8
    }
}

/// Result of a successful TT probe.
#[derive(Debug, Clone)]
pub struct TtProbeResult {
    /// Best move from a previous search of this position.
    pub best_move: Move,
    /// Search depth of the stored entry.
    pub depth: u8,
    /// Bound type (exact, lower, or upper).
    pub bound: Bound,
    /// Score (already adjusted from TT-relative back to root-relative).
    pub score: i32,
    /// Static evaluation.
    pub eval: i32,
}

/// Convert a search score to TT-storable form.
///
/// Mate scores are path-dependent: `MATE_SCORE - ply` changes based on
/// the search path. We store them as distance-from-node instead of
/// distance-from-root so they're path-independent.
pub fn score_to_tt(score: i32, ply: u8) -> i16 {
    let adjusted = if score > MATE_THRESHOLD {
        score + ply as i32
    } else if score < -MATE_THRESHOLD {
        score - ply as i32
    } else {
        score
    };
    adjusted as i16
}

/// Convert a TT-stored score back to search-usable form.
///
/// Reverses the mate-distance adjustment applied by [`score_to_tt`].
pub fn score_from_tt(score: i16, ply: u8) -> i32 {
    let score = score as i32;
    if score > MATE_THRESHOLD {
        score - ply as i32
    } else if score < -MATE_THRESHOLD {
        score + ply as i32
    } else {
        score
    }
}

/// Fixed-size transposition table with generation-based replacement.
pub struct TranspositionTable {
    entries: Box<[TtEntry]>,
    mask: u32,
    generation: u8,
}

impl TranspositionTable {
    /// Create a new transposition table with the given size in megabytes.
    ///
    /// The actual number of entries is rounded down to the nearest power of two.
    pub fn new(mb: usize) -> Self {
        let bytes = mb * 1024 * 1024;
        let entry_size = std::mem::size_of::<TtEntry>();
        let num_entries = (bytes / entry_size).next_power_of_two() >> 1; // round down
        let num_entries = num_entries.max(1); // at least 1 entry

        Self {
            entries: vec![TtEntry::default(); num_entries].into_boxed_slice(),
            mask: (num_entries - 1) as u32,
            generation: 0,
        }
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.fill(TtEntry::default());
        self.generation = 0;
    }

    /// Advance the generation counter. Call once per `go` command.
    pub fn new_generation(&mut self) {
        self.generation = self.generation.wrapping_add(1) & 0x3F;
    }

    /// Probe the table for a position.
    ///
    /// Returns `Some(TtProbeResult)` if a matching entry is found,
    /// with the score adjusted back from TT-relative to root-relative
    /// using the given `ply`.
    pub fn probe(&self, hash: u64, ply: u8) -> Option<TtProbeResult> {
        let index = (hash as u32 & self.mask) as usize;
        let entry = &self.entries[index];

        // Verify the upper 32 bits match to reduce collisions
        let key32 = (hash >> 32) as u32;
        if entry.key != key32 || entry.bound() == Bound::None {
            return None;
        }

        Some(TtProbeResult {
            best_move: entry.move_bits(),
            depth: entry.depth(),
            bound: entry.bound(),
            score: score_from_tt(entry.score, ply),
            eval: entry.eval as i32,
        })
    }

    /// Store a position in the table.
    ///
    /// Replacement policy: always replace if:
    /// - The entry is from an older generation, OR
    /// - The new depth >= stored depth, OR
    /// - The new bound is Exact
    pub fn store(
        &mut self,
        hash: u64,
        depth: u8,
        score: i32,
        eval: i32,
        best_move: Move,
        bound: Bound,
        ply: u8,
    ) {
        let index = (hash as u32 & self.mask) as usize;
        let key32 = (hash >> 32) as u32;
        let existing = &self.entries[index];

        // Replacement policy
        let dominated = existing.bound() == Bound::None
            || existing.generation() != self.generation
            || depth >= existing.depth()
            || bound == Bound::Exact;

        if !dominated {
            return;
        }

        self.entries[index] = TtEntry {
            key: key32,
            data: TtEntry::pack_data(best_move, depth, bound, self.generation),
            score: score_to_tt(score, ply),
            eval: eval as i16,
            _padding: 0,
        };
    }
}

impl std::fmt::Debug for TranspositionTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TranspositionTable")
            .field("entries", &self.entries.len())
            .field("generation", &self.generation)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cesso_core::{Move, Square};

    #[test]
    fn entry_is_16_bytes() {
        assert_eq!(std::mem::size_of::<TtEntry>(), 16);
    }

    #[test]
    fn store_and_probe_roundtrip() {
        let mut tt = TranspositionTable::new(1);
        let hash: u64 = 0xDEAD_BEEF_1234_5678;
        let mv = Move::new(Square::E2, Square::E4);

        tt.store(hash, 5, 100, 50, mv, Bound::Exact, 0);

        let result = tt.probe(hash, 0).expect("should find stored entry");
        assert_eq!(result.best_move, mv);
        assert_eq!(result.depth, 5);
        assert_eq!(result.bound, Bound::Exact);
        assert_eq!(result.score, 100);
        assert_eq!(result.eval, 50);
    }

    #[test]
    fn probe_miss_returns_none() {
        let tt = TranspositionTable::new(1);
        assert!(tt.probe(0x1234_5678_9ABC_DEF0, 0).is_none());
    }

    #[test]
    fn mate_score_adjustment_roundtrip() {
        // Mate in 3 from root (ply 0): MATE_SCORE - 3 = 28997
        let mate_score = 29_000 - 3;
        let ply: u8 = 5;

        // Store at ply 5: should add ply to make it distance-from-node
        let tt_score = score_to_tt(mate_score, ply);
        // Retrieve at ply 5: should subtract ply to get back distance-from-root
        let restored = score_from_tt(tt_score, ply);
        assert_eq!(restored, mate_score);
    }

    #[test]
    fn negative_mate_score_adjustment_roundtrip() {
        // Being mated in 3 from root: -(MATE_SCORE - 3) = -28997
        let mated_score = -(29_000 - 3);
        let ply: u8 = 7;

        let tt_score = score_to_tt(mated_score, ply);
        let restored = score_from_tt(tt_score, ply);
        assert_eq!(restored, mated_score);
    }

    #[test]
    fn normal_score_not_adjusted() {
        let score = 150; // regular centipawn score
        let ply: u8 = 10;

        let tt_score = score_to_tt(score, ply);
        let restored = score_from_tt(tt_score, ply);
        assert_eq!(restored, score);
    }

    #[test]
    fn generation_replacement_policy() {
        let mut tt = TranspositionTable::new(1);
        let hash: u64 = 0xAAAA_BBBB_CCCC_DDDD;
        let mv1 = Move::new(Square::E2, Square::E4);
        let mv2 = Move::new(Square::D2, Square::D4);

        // Store at depth 10 in generation 0
        tt.store(hash, 10, 100, 50, mv1, Bound::Exact, 0);

        // Advance generation
        tt.new_generation();

        // Store at depth 1 in generation 1 — should replace because different generation
        tt.store(hash, 1, 200, 60, mv2, Bound::LowerBound, 0);

        let result = tt.probe(hash, 0).unwrap();
        assert_eq!(result.best_move, mv2);
        assert_eq!(result.score, 200);
    }

    #[test]
    fn depth_replacement_policy() {
        let mut tt = TranspositionTable::new(1);
        let hash: u64 = 0x1111_2222_3333_4444;
        let mv1 = Move::new(Square::E2, Square::E4);
        let mv2 = Move::new(Square::D2, Square::D4);

        // Store at depth 5
        tt.store(hash, 5, 100, 50, mv1, Bound::LowerBound, 0);

        // Try to store at depth 3 (same generation) — should NOT replace
        tt.store(hash, 3, 200, 60, mv2, Bound::LowerBound, 0);

        let result = tt.probe(hash, 0).unwrap();
        assert_eq!(result.best_move, mv1); // original entry preserved
    }

    #[test]
    fn clear_removes_all_entries() {
        let mut tt = TranspositionTable::new(1);
        let hash: u64 = 0xAAAA_BBBB_CCCC_DDDD;
        let mv = Move::new(Square::E2, Square::E4);

        tt.store(hash, 5, 100, 50, mv, Bound::Exact, 0);
        assert!(tt.probe(hash, 0).is_some());

        tt.clear();
        assert!(tt.probe(hash, 0).is_none());
    }
}
