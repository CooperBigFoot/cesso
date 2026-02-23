//! Lockless transposition table using atomic XOR-based torn-write detection.
//!
//! Two `AtomicU64` words per entry (16 bytes, one cache line per pair).
//!
//! ## Bit layout
//!
//! ```text
//! word0 (AtomicU64):
//!   bits 63-32: key           (upper 32 bits of Zobrist hash)
//!   bits 31-27: generation    (5 bits, wraps at 32)
//!   bits 26-26: is_pv         (1 bit)
//!   bits 25-24: bound         (2 bits)
//!   bits 23-16: depth         (8 bits)
//!   bits 15-0:  move          (16 bits)
//!
//! word1 (AtomicU64):
//!   bits 63-32: check         = key XOR (word0 & 0xFFFF_FFFF)
//!   bits 31-16: score         (i16 as u16)
//!   bits 15-0:  eval          (i16 as u16)
//! ```
//!
//! ## Torn-write detection
//!
//! On probe: `check_expected = (w0 >> 32) ^ (w0 & 0xFFFF_FFFF)`.
//! If `check_expected != (w1 >> 32)` the entry was written by another thread
//! mid-write and we return `None` rather than using garbage data.
//!
//! All atomic accesses use `Relaxed` ordering — the standard Stockfish technique.

use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};

use cesso_core::Move;

// ── Compile-time assertion: TT must be Send + Sync for Lazy SMP ─────────────
const _: () = {
    fn assert_send_sync<T: Send + Sync>() {}
    fn check() {
        assert_send_sync::<TranspositionTable>();
    }
    let _ = check;
};

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
    /// Whether this entry was written from a PV node.
    pub is_pv: bool,
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

// ── Internal entry type ──────────────────────────────────────────────────────

/// Two 64-bit atomic words — one logical TT slot.
struct AtomicEntry {
    word0: AtomicU64,
    word1: AtomicU64,
}

impl AtomicEntry {
    const fn new() -> Self {
        Self {
            word0: AtomicU64::new(0),
            word1: AtomicU64::new(0),
        }
    }

    /// Pack fields into word0.
    ///
    /// Layout:
    ///   [63:32] key | [31:27] generation | [26] is_pv | [25:24] bound | [23:16] depth | [15:0] mv
    fn pack_word0(key32: u32, generation: u8, is_pv: bool, bound: Bound, depth: u8, mv: Move) -> u64 {
        let key_bits = (key32 as u64) << 32;
        let gen_bits = ((generation & 0x1F) as u64) << 27;
        let pv_bit = (is_pv as u64) << 26;
        let bound_bits = ((bound as u8) as u64) << 24;
        let depth_bits = (depth as u64) << 16;
        let mv_bits = mv.raw() as u64;
        key_bits | gen_bits | pv_bit | bound_bits | depth_bits | mv_bits
    }

    /// Pack fields into word1.
    ///
    /// Layout:
    ///   [63:32] check (key XOR lower32 of word0) | [31:16] score | [15:0] eval
    fn pack_word1(w0: u64, score: i16, eval: i16) -> u64 {
        let key32 = (w0 >> 32) as u32;
        let data_lower = (w0 & 0xFFFF_FFFF) as u32;
        let check = (key32 ^ data_lower) as u64;
        let check_bits = check << 32;
        let score_bits = ((score as u16) as u64) << 16;
        let eval_bits = (eval as u16) as u64;
        check_bits | score_bits | eval_bits
    }

    /// Decode `word0` into its fields.
    fn decode_w0(w0: u64) -> (u32, u8, bool, Bound, u8, Move) {
        let key32 = (w0 >> 32) as u32;
        let generation = ((w0 >> 27) & 0x1F) as u8;
        let is_pv = ((w0 >> 26) & 0x01) != 0;
        let bound = Bound::from_bits(((w0 >> 24) & 0x03) as u8);
        let depth = ((w0 >> 16) & 0xFF) as u8;
        let mv = Move::from_raw((w0 & 0xFFFF) as u16);
        (key32, generation, is_pv, bound, depth, mv)
    }

    /// Load and verify the entry for `hash`.
    ///
    /// Returns `None` if the key does not match or the XOR check detects a torn write.
    fn load(&self, hash: u64) -> Option<(u8, bool, Bound, u8, Move, u64, u64)> {
        let w0 = self.word0.load(Ordering::Relaxed);
        let w1 = self.word1.load(Ordering::Relaxed);

        // XOR integrity check: detect torn writes from concurrent threads
        let key32_w0 = (w0 >> 32) as u32;
        let data_lower = (w0 & 0xFFFF_FFFF) as u32;
        let check_expected = key32_w0 ^ data_lower;
        let check_stored = (w1 >> 32) as u32;
        if check_expected != check_stored {
            return None;
        }

        // Key collision check
        let key32 = (hash >> 32) as u32;
        if key32_w0 != key32 {
            return None;
        }

        let (_, generation, is_pv, bound, depth, mv) = Self::decode_w0(w0);
        Some((generation, is_pv, bound, depth, mv, w0, w1))
    }

    /// Store an entry atomically (word0 first, then word1).
    fn store(&self, w0: u64, w1: u64) {
        self.word0.store(w0, Ordering::Relaxed);
        self.word1.store(w1, Ordering::Relaxed);
    }

    /// Load word0 for replacement-policy inspection (no key check).
    fn peek_w0(&self) -> u64 {
        self.word0.load(Ordering::Relaxed)
    }
}

// ── Public API ───────────────────────────────────────────────────────────────

/// Lockless transposition table with atomic XOR integrity checking.
///
/// All method receivers are `&self` — the table is safe to share across threads.
pub struct TranspositionTable {
    entries: Box<[AtomicEntry]>,
    /// Index mask — `num_entries - 1` (power-of-two allocation).
    mask: u64,
    /// Current search generation (wraps every 64 searches).
    generation: AtomicU8,
}

impl TranspositionTable {
    /// Create a new transposition table with the given size in megabytes.
    ///
    /// The actual number of entries is rounded down to the nearest power of two.
    pub fn new(mb: usize) -> Self {
        let bytes = mb * 1024 * 1024;
        let entry_size = std::mem::size_of::<AtomicEntry>();
        let num_entries = (bytes / entry_size).next_power_of_two() >> 1;
        let num_entries = num_entries.max(1);

        let entries: Box<[AtomicEntry]> = (0..num_entries)
            .map(|_| AtomicEntry::new())
            .collect();

        Self {
            entries,
            mask: (num_entries - 1) as u64,
            generation: AtomicU8::new(0),
        }
    }

    /// Clear all entries and reset the generation counter.
    pub fn clear(&self) {
        for entry in self.entries.iter() {
            entry.word0.store(0, Ordering::Relaxed);
            entry.word1.store(0, Ordering::Relaxed);
        }
        self.generation.store(0, Ordering::Relaxed);
    }

    /// Advance the generation counter. Call once per `go` command.
    pub fn new_generation(&self) {
        let current = self.generation.load(Ordering::Relaxed);
        self.generation
            .store(current.wrapping_add(1) & 0x1F, Ordering::Relaxed);
    }

    /// Probe the table for a position.
    ///
    /// Returns `Some(TtProbeResult)` if a matching, intact entry is found.
    /// Returns `None` on a miss, key mismatch, or torn-write detection.
    pub fn probe(&self, hash: u64, ply: u8) -> Option<TtProbeResult> {
        let index = (hash & self.mask) as usize;
        let entry = &self.entries[index];

        let (_, is_pv, bound, depth, mv, _w0, w1) = entry.load(hash)?;

        if bound == Bound::None {
            return None;
        }

        let score_raw = ((w1 >> 16) & 0xFFFF) as u16 as i16;
        let eval_raw = (w1 & 0xFFFF) as u16 as i16;

        Some(TtProbeResult {
            best_move: mv,
            depth,
            bound,
            score: score_from_tt(score_raw, ply),
            eval: eval_raw as i32,
            is_pv,
        })
    }

    /// Store a position in the table.
    ///
    /// Replacement policy: replace if any of:
    /// - The slot is empty (bound is None)
    /// - The stored entry is from a different generation
    /// - The new depth >= stored depth
    /// - The new bound is Exact
    pub fn store(
        &self,
        hash: u64,
        depth: u8,
        score: i32,
        eval: i32,
        best_move: Move,
        bound: Bound,
        ply: u8,
        is_pv: bool,
    ) {
        let index = (hash & self.mask) as usize;
        let entry = &self.entries[index];
        let generation = self.generation.load(Ordering::Relaxed);

        // Replacement policy — inspect existing entry without key check
        let existing_w0 = entry.peek_w0();
        let (_, existing_generation, _existing_is_pv, existing_bound, existing_depth, _) =
            AtomicEntry::decode_w0(existing_w0);

        let dominated = existing_bound == Bound::None
            || existing_generation != generation
            || depth >= existing_depth
            || bound == Bound::Exact;

        if !dominated {
            return;
        }

        let key32 = (hash >> 32) as u32;
        let w0 = AtomicEntry::pack_word0(key32, generation, is_pv, bound, depth, best_move);
        let w1 = AtomicEntry::pack_word1(w0, score_to_tt(score, ply), eval as i16);
        entry.store(w0, w1);
    }
}

impl std::fmt::Debug for TranspositionTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TranspositionTable")
            .field("entries", &self.entries.len())
            .field("generation", &self.generation.load(Ordering::Relaxed))
            .finish()
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use cesso_core::{Move, Square};

    #[test]
    fn atomic_entry_is_16_bytes() {
        assert_eq!(std::mem::size_of::<AtomicEntry>(), 16);
    }

    #[test]
    fn store_and_probe_roundtrip() {
        let tt = TranspositionTable::new(1);
        let hash: u64 = 0xDEAD_BEEF_1234_5678;
        let mv = Move::new(Square::E2, Square::E4);

        tt.store(hash, 5, 100, 50, mv, Bound::Exact, 0, false);

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

        let tt_score = score_to_tt(mate_score, ply);
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
        let score = 150;
        let ply: u8 = 10;

        let tt_score = score_to_tt(score, ply);
        let restored = score_from_tt(tt_score, ply);
        assert_eq!(restored, score);
    }

    #[test]
    fn generation_replacement_policy() {
        let tt = TranspositionTable::new(1);
        let hash: u64 = 0xAAAA_BBBB_CCCC_DDDD;
        let mv1 = Move::new(Square::E2, Square::E4);
        let mv2 = Move::new(Square::D2, Square::D4);

        // Store at depth 10 in generation 0
        tt.store(hash, 10, 100, 50, mv1, Bound::Exact, 0, false);

        // Advance generation
        tt.new_generation();

        // Store at depth 1 in generation 1 — should replace (different generation)
        tt.store(hash, 1, 200, 60, mv2, Bound::LowerBound, 0, false);

        let result = tt.probe(hash, 0).unwrap();
        assert_eq!(result.best_move, mv2);
        assert_eq!(result.score, 200);
    }

    #[test]
    fn depth_replacement_policy() {
        let tt = TranspositionTable::new(1);
        let hash: u64 = 0x1111_2222_3333_4444;
        let mv1 = Move::new(Square::E2, Square::E4);
        let mv2 = Move::new(Square::D2, Square::D4);

        // Store at depth 5
        tt.store(hash, 5, 100, 50, mv1, Bound::LowerBound, 0, false);

        // Try to store at depth 3 (same generation) — should NOT replace
        tt.store(hash, 3, 200, 60, mv2, Bound::LowerBound, 0, false);

        let result = tt.probe(hash, 0).unwrap();
        assert_eq!(result.best_move, mv1); // original entry preserved
    }

    #[test]
    fn clear_removes_all_entries() {
        let tt = TranspositionTable::new(1);
        let hash: u64 = 0xAAAA_BBBB_CCCC_DDDD;
        let mv = Move::new(Square::E2, Square::E4);

        tt.store(hash, 5, 100, 50, mv, Bound::Exact, 0, false);
        assert!(tt.probe(hash, 0).is_some());

        tt.clear();
        assert!(tt.probe(hash, 0).is_none());
    }

    #[test]
    fn xor_integrity_detects_torn_write() {
        let tt = TranspositionTable::new(1);
        let hash: u64 = 0xDEAD_BEEF_1234_5678;
        let mv = Move::new(Square::E2, Square::E4);

        tt.store(hash, 5, 100, 50, mv, Bound::Exact, 0, false);
        assert!(tt.probe(hash, 0).is_some(), "entry should be found before corruption");

        // Corrupt the check bits in word1 to simulate a torn write
        let index = (hash & tt.mask) as usize;
        let entry = &tt.entries[index];
        let w1 = entry.word1.load(Ordering::Relaxed);
        // Flip all bits in the check field (upper 32 bits of word1)
        let corrupted_w1 = w1 ^ 0xFFFF_FFFF_0000_0000;
        entry.word1.store(corrupted_w1, Ordering::Relaxed);

        assert!(
            tt.probe(hash, 0).is_none(),
            "probe should return None after XOR corruption"
        );
    }

    #[test]
    fn concurrent_stress_no_panics() {
        use std::thread;

        let tt = std::sync::Arc::new(TranspositionTable::new(4));

        thread::scope(|s| {
            for t in 0..8u64 {
                let tt = std::sync::Arc::clone(&tt);
                s.spawn(move || {
                    let mv = Move::new(Square::E2, Square::E4);
                    for i in 0u64..10_000 {
                        // Mix of different hashes so threads collide on some entries
                        let hash = (t.wrapping_mul(6364136223846793005))
                            .wrapping_add(i.wrapping_mul(2862933555777941757))
                            ^ 0xDEAD_BEEF_CAFE_F00D;
                        tt.store(hash, 5, 100, 50, mv, Bound::Exact, 0, false);
                        let _ = tt.probe(hash, 0);
                    }
                });
            }
        });
    }
}
