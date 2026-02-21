//! Pawn structure evaluation for HCE (Handcrafted Evaluation).
//!
//! Evaluates passed pawns, isolated pawns, doubled pawns, and backward pawns.
//! All scores are from White's perspective (positive = White advantage).

use cesso_core::{Bitboard, Board, Color, File, PieceKind, Square, pawn_attacks};

use crate::eval::score::{Score, S};

// ---------------------------------------------------------------------------
// Precomputed tables
// ---------------------------------------------------------------------------

/// For each file index 0–7, the bitboard of the adjacent files.
///
/// File A → FILE_B only; File H → FILE_G only; all others get both neighbours.
pub(crate) static ADJACENT_FILES: [Bitboard; 8] = compute_adjacent_files();

/// For each `[color][square]`, the mask of squares ahead of the pawn on the
/// same file and adjacent files.
///
/// A pawn is passed if `PASSED_PAWN_MASK[color][sq] & enemy_pawns` is empty.
pub(crate) static PASSED_PAWN_MASK: [[Bitboard; 64]; 2] = compute_passed_pawn_masks();

const fn compute_adjacent_files() -> [Bitboard; 8] {
    let mut table = [Bitboard::EMPTY; 8];
    let mut f = 0usize;
    while f < 8 {
        let mut bits = 0u64;
        // Left neighbour (file index - 1), if in range
        if f > 0 {
            bits |= Bitboard::FILES[f - 1].inner();
        }
        // Right neighbour (file index + 1), if in range
        if f < 7 {
            bits |= Bitboard::FILES[f + 1].inner();
        }
        table[f] = Bitboard::new(bits);
        f += 1;
    }
    table
}

const fn compute_passed_pawn_masks() -> [[Bitboard; 64]; 2] {
    let mut table = [[Bitboard::EMPTY; 64]; 2];

    let mut sq = 0usize;
    while sq < 64 {
        let rank = sq / 8; // 0 = rank 1, 7 = rank 8
        let file = sq % 8;

        // The file mask for this square plus both adjacent files
        let file_mask = Bitboard::FILES[file].inner();
        let adj_mask = ADJACENT_FILES[file].inner();
        let span_mask = file_mask | adj_mask;

        // White: ahead means higher rank indices (toward rank 8)
        let mut white_bits = 0u64;
        let mut r = rank + 1;
        while r < 8 {
            white_bits |= Bitboard::RANKS[r].inner();
            r += 1;
        }
        table[0][sq] = Bitboard::new(span_mask & white_bits);

        // Black: ahead means lower rank indices (toward rank 1)
        let mut black_bits = 0u64;
        // rank is usize, so we use a checked subtraction via a signed approach
        if rank > 0 {
            let mut r2 = 0usize;
            while r2 < rank {
                black_bits |= Bitboard::RANKS[r2].inner();
                r2 += 1;
            }
        }
        table[1][sq] = Bitboard::new(span_mask & black_bits);

        sq += 1;
    }

    table
}

// ---------------------------------------------------------------------------
// Evaluation constants
// ---------------------------------------------------------------------------

/// Passed pawn bonus by rank from that side's perspective.
///
/// Index by the pawn's rank (0-based from the pawn's own back rank).
/// Index 0 is impossible (pawns start on rank 2 at earliest), index 6 is
/// one step from promotion.
const PASSED_PAWN_BONUS: [Score; 7] = [
    S(0, 0),      // rank 1 — impossible for a pawn
    S(5, 10),     // rank 2 — starting rank
    S(10, 20),    // rank 3
    S(20, 40),    // rank 4
    S(40, 70),    // rank 5
    S(70, 120),   // rank 6
    S(100, 200),  // rank 7 — one step from promotion
];

/// Extra bonus when a passed pawn is directly supported by another friendly pawn.
const PASSED_PAWN_SUPPORTED_BONUS: Score = S(15, 25);

/// Penalty for an isolated pawn (no friendly pawns on adjacent files).
const ISOLATED_PAWN_PENALTY: Score = S(-10, -20);

/// Penalty per extra pawn on the same file (beyond the first).
const DOUBLED_PAWN_PENALTY: Score = S(-10, -15);

/// Penalty for a backward pawn.
const BACKWARD_PAWN_PENALTY: Score = S(-15, -10);

/// Bonus for a pawn that is directly supported by another friendly pawn
/// (connected pawns on adjacent files, same or +1 rank).
const CONNECTED_PAWN_BONUS: Score = S(5, 8);

// ---------------------------------------------------------------------------
// Public evaluation entry point
// ---------------------------------------------------------------------------

/// Evaluate pawn structure from White's perspective.
///
/// Returns a positive score when the pawn structure favours White.
pub fn evaluate_pawns(board: &Board) -> Score {
    let white_pawns = board.pieces(PieceKind::Pawn) & board.side(Color::White);
    let black_pawns = board.pieces(PieceKind::Pawn) & board.side(Color::Black);

    let white_score = evaluate_pawns_for_side(white_pawns, black_pawns, Color::White);
    let black_score = evaluate_pawns_for_side(black_pawns, white_pawns, Color::Black);

    white_score - black_score
}

// ---------------------------------------------------------------------------
// Per-side helper
// ---------------------------------------------------------------------------

/// Accumulate the pawn-structure score for one side.
///
/// All returned scores are from that side's own perspective (positive = good
/// for `color`). The caller is responsible for negating the Black score when
/// combining into a single White-relative total.
fn evaluate_pawns_for_side(
    friendly_pawns: Bitboard,
    enemy_pawns: Bitboard,
    color: Color,
) -> Score {
    let mut score = Score::ZERO;

    // ------------------------------------------------------------------
    // Doubled pawns: for each file, every pawn beyond the first is a penalty
    // ------------------------------------------------------------------
    for file in File::ALL {
        let count = (Bitboard::file_mask(file) & friendly_pawns).count();
        if count > 1 {
            score += DOUBLED_PAWN_PENALTY * (count - 1) as i16;
        }
    }

    // ------------------------------------------------------------------
    // Per-pawn evaluation: passed, isolated, backward
    // ------------------------------------------------------------------
    for sq in friendly_pawns {
        let file = sq.file();
        let file_idx = file.index();

        // Rank from this color's own perspective (0 = own back rank, 7 = promotion)
        let rank_idx = match color {
            Color::White => sq.rank().index(),
            Color::Black => 7 - sq.rank().index(),
        };

        // --- Passed pawn ---
        let passed = (PASSED_PAWN_MASK[color.index()][sq.index()] & enemy_pawns).is_empty();
        if passed {
            score += PASSED_PAWN_BONUS[rank_idx];

            // Supported: any friendly pawn that attacks `sq` from behind.
            // pawn_attacks(!color, sq) gives the squares a pawn of the
            // OPPOSITE color on `sq` would attack — which are exactly the
            // squares where a friendly pawn would need to be to attack `sq`.
            let supported = (pawn_attacks(!color, sq) & friendly_pawns).is_nonempty();
            if supported {
                score += PASSED_PAWN_SUPPORTED_BONUS;
            }
        }

        // --- Isolated pawn ---
        let adjacent_friendly = ADJACENT_FILES[file_idx] & friendly_pawns;
        let is_isolated = adjacent_friendly.is_empty();
        if is_isolated {
            score += ISOLATED_PAWN_PENALTY;
            // Skip backward check: isolated pawns are already penalized and
            // the backward logic requires a friendly pawn on an adjacent file.
            continue;
        }

        // --- Backward pawn ---
        // A pawn is backward when:
        //   (a) No friendly pawn on adjacent files is on the same rank or
        //       behind it (i.e., it cannot be supported on its advance).
        //   (b) The stop square (one step ahead) is attacked by an enemy pawn.
        //
        // For (a): compute the "rear span" — adjacent files restricted to
        // ranks at or behind the pawn. If that intersection is empty, the
        // pawn is backward in terms of pawn chain support.

        let rear_span = rear_span_mask(sq, color);
        let no_support_behind = (rear_span & friendly_pawns).is_empty();

        if no_support_behind {
            // (b) the stop square is attacked by an enemy pawn.
            //
            // To check if any enemy pawn attacks `stop_sq`, we use
            // `pawn_attacks(color, stop_sq)`. This gives the squares from
            // which a friendly pawn WOULD attack `stop_sq` — which are
            // exactly the squares where enemy pawns must stand to attack it.
            // For example: White pawn's stop_sq = e3, `pawn_attacks(White, e3)`
            // = {d4, f4}, the squares where Black pawns would cover e3.
            if let Some(stop_sq) = stop_square(sq, color) {
                let stop_attacked =
                    (pawn_attacks(color, stop_sq) & enemy_pawns).is_nonempty();
                if stop_attacked {
                    score += BACKWARD_PAWN_PENALTY;
                }
            }
        }

        // --- Connected pawn ---
        // A pawn is connected if a friendly pawn on an adjacent file attacks it
        // (i.e., is on the same rank or one rank behind and on an adjacent file).
        let supporters = pawn_attacks(!color, sq) & friendly_pawns;
        if supporters.is_nonempty() {
            score += CONNECTED_PAWN_BONUS;
        }
    }

    score
}

// ---------------------------------------------------------------------------
// Geometry helpers
// ---------------------------------------------------------------------------

/// Return the bitboard of squares on adjacent files that are on the same rank
/// or behind the pawn (the "rear span" on adjacent files).
///
/// Used to check whether any friendly pawn can support this pawn's advance.
fn rear_span_mask(sq: Square, color: Color) -> Bitboard {
    let rank_idx = sq.rank().index();
    let file_idx = sq.file().index();

    // Collect all ranks at or behind the pawn (inclusive of its own rank)
    let mut rank_bits = 0u64;
    match color {
        Color::White => {
            // "Behind" for White = smaller rank indices
            let mut r = 0usize;
            while r <= rank_idx {
                rank_bits |= Bitboard::RANKS[r].inner();
                r += 1;
            }
        }
        Color::Black => {
            // "Behind" for Black = larger rank indices
            let mut r = rank_idx;
            while r < 8 {
                rank_bits |= Bitboard::RANKS[r].inner();
                r += 1;
            }
        }
    }

    let rank_mask = Bitboard::new(rank_bits);
    ADJACENT_FILES[file_idx] & rank_mask
}

/// Return the stop square (one step forward) for a pawn of the given color,
/// or `None` if the pawn is already on the promotion rank (shouldn't happen
/// in a valid position).
fn stop_square(sq: Square, color: Color) -> Option<Square> {
    let idx = sq.index() as u8;
    match color {
        Color::White => Square::from_index(idx + 8),
        Color::Black => {
            if idx < 8 {
                None
            } else {
                Square::from_index(idx - 8)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use cesso_core::Board;

    use super::evaluate_pawns;
    use crate::eval::score::{Score, S};

    fn parse(fen: &str) -> Board {
        fen.parse::<Board>().unwrap()
    }

    /// Starting position is symmetric — pawn eval must be zero.
    #[test]
    fn starting_position_is_symmetric() {
        let board = parse("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
        let score = evaluate_pawns(&board);
        assert_eq!(score, Score::ZERO, "expected S(0,0) for starting position, got {score}");
    }

    /// A lone white pawn on e4 with no other pawns is both isolated and passed.
    ///
    /// With no enemy pawns at all, the PASSED_PAWN_MASK intersection is empty,
    /// so the pawn is passed. It is also isolated (no friendly pawns on d or f files).
    ///
    /// White e4: rank_idx = 3 (Rank4 index from White's back rank).
    ///   passed bonus → PASSED_PAWN_BONUS[3] = S(20, 40)
    ///   isolated penalty → ISOLATED_PAWN_PENALTY = S(-10, -20)
    /// Net white score: S(20,40) + S(-10,-20) = S(10, 20)
    /// Black score: 0 (no pawns)
    /// Result: S(10, 20)
    #[test]
    fn isolated_pawn_penalty() {
        let board = parse("4k3/8/8/8/4P3/8/8/4K3 w - - 0 1");
        let score = evaluate_pawns(&board);
        // Passed bonus (rank 3) + isolated penalty
        let expected = S(20, 40) + S(-10, -20);
        assert_eq!(score, expected, "expected passed+isolated score {expected}, got {score}");
    }

    /// Two white pawns on the e-file (e3 and e4) — doubled, isolated, and both passed.
    ///
    /// With no enemy pawns, both White pawns are passed. The e4 pawn's passed mask
    /// does NOT include e3 (only squares strictly ahead), so the e4 pawn is still
    /// considered passed. Both are also isolated.
    ///
    /// Doubled penalty: 1 extra pawn on e-file → S(-10, -15)
    ///
    /// e3 pawn (rank_idx=2 from White's POV):
    ///   passed bonus → PASSED_PAWN_BONUS[2] = S(10, 20)
    ///   isolated penalty → S(-10, -20)
    ///
    /// e4 pawn (rank_idx=3):
    ///   passed bonus → PASSED_PAWN_BONUS[3] = S(20, 40)
    ///   isolated penalty → S(-10, -20)
    ///
    /// White total: S(-10,-15) + S(10,20) + S(-10,-20) + S(20,40) + S(-10,-20) = S(0, 5)
    /// Black total: 0
    /// Result: S(0, 5)
    #[test]
    fn doubled_pawn_penalty() {
        let board = parse("4k3/8/8/8/4P3/4P3/8/4K3 w - - 0 1");
        let score = evaluate_pawns(&board);
        // Doubled + two pawns each isolated and passed
        let expected = S(-10, -15)                  // doubled penalty
            + S(10, 20) + S(-10, -20)               // e3: passed rank2 + isolated
            + S(20, 40) + S(-10, -20);              // e4: passed rank3 + isolated
        assert_eq!(score, expected, "expected doubled+isolated+passed score {expected}, got {score}");
    }

    /// A white pawn on e5 with no enemy pawns is passed and isolated.
    ///
    /// White e5: rank_idx = 4 (Rank5 index from White's back rank).
    ///   passed bonus → PASSED_PAWN_BONUS[4] = S(40, 70)
    ///   isolated penalty → ISOLATED_PAWN_PENALTY = S(-10, -20)
    /// Net: S(30, 50)
    #[test]
    fn passed_pawn_bonus() {
        let board = parse("4k3/8/8/4P3/8/8/8/4K3 w - - 0 1");
        let score = evaluate_pawns(&board);
        let expected = S(40, 70) + S(-10, -20);
        assert_eq!(score, expected, "expected passed+isolated score {expected}, got {score}");
    }

    /// Backward pawn position.
    ///
    /// Position: White K e1, White P e2, White P f4, Black K e8, Black P d4.
    ///
    /// e2 analysis (White):
    ///   - Not isolated: f4 is on adjacent f-file.
    ///   - Passed? PASSED_PAWN_MASK[White][e2] covers d3–d8, e3–e8, f3–f8.
    ///     Black d4 (rank4, d-file, rank_idx 3 >= 2) → in mask → NOT passed.
    ///   - Backward? rear_span = (d-file|f-file) & ranks 1–2. f4 is rank4, d-file has
    ///     nothing at ranks 1–2 → rear_span empty → no_support_behind.
    ///     Stop sq = e3. `pawn_attacks(White, e3)` = {d4, f4}.
    ///     d4 ∈ black pawns → stop is attacked → BACKWARD.
    ///     Adds BACKWARD_PAWN_PENALTY = S(-15, -10).
    ///
    /// f4 analysis (White, rank_idx=3):
    ///   - Not isolated: e2 is on adjacent e-file.
    ///   - Passed? PASSED_PAWN_MASK[White][f4] covers e5–e8, f5–f8, g5–g8.
    ///     Black d4 is on d-file, not in {e,f,g} → NOT blocked → f4 IS passed.
    ///     Bonus: PASSED_PAWN_BONUS[3] = S(20, 40).
    ///   - Supported? pawn_attacks(Black, f4) = {e3, g3}. No White pawn there → not supported.
    ///   - Backward? rear_span = (e-file|g-file) & ranks 1–3 contains e2 (rank2).
    ///     e2 ∈ White pawns → rear_span non-empty → NOT backward.
    ///
    /// White total: S(-15,-10) + S(20,40) = S(5, 30)
    ///
    /// d4 analysis (Black, rank_idx from Black's POV = 7-3 = 4):
    ///   - Not isolated would require a Black pawn on c or e file; there is none → ISOLATED.
    ///   - Passed? PASSED_PAWN_MASK[Black][d4] covers c1–c3, d1–d3, e1–e3.
    ///     White e2 (rank2, e-file, index 1 < 3) → in mask → NOT passed.
    ///   - Score: ISOLATED_PAWN_PENALTY = S(-10, -20). (continue, skip backward)
    ///
    /// Black total: S(-10, -20)
    ///
    /// Net = White - Black = S(5,30) - S(-10,-20) = S(15, 50)
    #[test]
    fn backward_pawn_penalty() {
        // White: K e1, P e2, P f4. Black: K e8, P d4.
        let board = parse("4k3/8/8/8/3p1P2/8/4P3/4K3 w - - 0 1");
        let score = evaluate_pawns(&board);

        let white_score = S(-15, -10) + S(20, 40); // e2 backward + f4 passed
        let black_score = S(-10, -20);              // d4 isolated
        let expected = white_score - black_score;
        assert_eq!(score, expected, "expected backward pawn score {expected}, got {score}");
    }
}
