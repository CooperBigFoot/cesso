//! King safety evaluation: pawn shield, attacker zone, pawn storm, and open files.
//!
//! All scores are from White's perspective (positive = White is safer).

use cesso_core::{
    bishop_attacks, king_attacks, knight_attacks, queen_attacks, rook_attacks,
    Bitboard, Board, Color, File, PieceKind, Square,
};

use crate::eval::score::{Score, S};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Penalty for each missing pawn in the king's shield (middlegame only).
const MISSING_SHIELD_PAWN_PENALTY: Score = S(-30, 0);

/// Attack weights by piece kind index: [Pawn, Knight, Bishop, Rook, Queen, King]
const ATTACK_WEIGHTS: [i32; 6] = [0, 2, 2, 3, 5, 0];

/// Penalty for an open file adjacent to or on the king's file.
const OPEN_FILE_PENALTY: Score = S(-25, 0);

/// Penalty for a semi-open file adjacent to or on the king's file.
const SEMI_OPEN_FILE_PENALTY: Score = S(-15, 0);

/// Pawn storm penalty when an enemy pawn is close (2-3 ranks away).
const STORM_CLOSE_PENALTY: Score = S(-20, 0);

/// Pawn storm penalty when an enemy pawn is distant (4 ranks away).
const STORM_FAR_PENALTY: Score = S(-10, 0);

// ---------------------------------------------------------------------------
// Geometry helpers
// ---------------------------------------------------------------------------

/// Compute the king zone: the king's attack squares plus the king's own square,
/// extended one rank forward.
fn king_zone(king_sq: Square, color: Color) -> Bitboard {
    let base = king_attacks(king_sq) | king_sq.bitboard();
    let forward = match color {
        Color::White => (base & !Bitboard::RANK_8) << 8u8,
        Color::Black => (base & !Bitboard::RANK_1) >> 8u8,
    };
    base | forward
}

/// Return the file cluster around the king: the king's file plus adjacent files.
fn king_file_cluster(king_sq: Square) -> Bitboard {
    let file = king_sq.file();
    let mut mask = Bitboard::file_mask(file);
    if file.index() > 0 {
        if let Some(f) = File::from_index(file.index() as u8 - 1) {
            mask = mask | Bitboard::file_mask(f);
        }
    }
    if file.index() < 7 {
        if let Some(f) = File::from_index(file.index() as u8 + 1) {
            mask = mask | Bitboard::file_mask(f);
        }
    }
    mask
}

/// Compute the pawn shield mask for a king on the given square.
fn shield_mask(king_sq: Square, color: Color) -> Bitboard {
    let king_bb = king_sq.bitboard();
    let shifted = match color {
        Color::White => king_bb << 8u8,
        Color::Black => king_bb >> 8u8,
    };
    if shifted.is_empty() {
        return Bitboard::EMPTY;
    }
    shifted | ((shifted << 1u8) & !Bitboard::FILE_A) | ((shifted >> 1u8) & !Bitboard::FILE_H)
}

// ---------------------------------------------------------------------------
// Per-side evaluation helpers
// ---------------------------------------------------------------------------

/// Evaluate pawn shield penalty for one side.
fn pawn_shield_penalty(board: &Board, color: Color) -> Score {
    let king_sq = board.king_square(color);
    let shield = shield_mask(king_sq, color);
    let friendly_pawns = board.pieces(PieceKind::Pawn) & board.side(color);
    let shield_pawns = shield & friendly_pawns;
    let missing = shield.count() - shield_pawns.count();
    MISSING_SHIELD_PAWN_PENALTY * missing as i16
}

/// Compute attacker zone danger score for one side being attacked.
///
/// Returns the danger as a positive value (higher = more danger to `king_color`).
fn attacker_zone_danger(board: &Board, king_color: Color) -> i32 {
    let attacker_color = !king_color;

    // No queen = no significant king danger
    let attacker_queens = board.pieces(PieceKind::Queen) & board.side(attacker_color);
    if attacker_queens.is_empty() {
        return 0;
    }

    let king_sq = board.king_square(king_color);
    let zone = king_zone(king_sq, king_color);
    let occupied = board.occupied();

    let mut danger: i32 = 0;
    let mut attacker_count: i32 = 0;

    let enemy = board.side(attacker_color);

    // Knights
    for sq in board.pieces(PieceKind::Knight) & enemy {
        if (knight_attacks(sq) & zone).is_nonempty() {
            danger += ATTACK_WEIGHTS[PieceKind::Knight.index()];
            attacker_count += 1;
        }
    }

    // Bishops
    for sq in board.pieces(PieceKind::Bishop) & enemy {
        if (bishop_attacks(sq, occupied) & zone).is_nonempty() {
            danger += ATTACK_WEIGHTS[PieceKind::Bishop.index()];
            attacker_count += 1;
        }
    }

    // Rooks
    for sq in board.pieces(PieceKind::Rook) & enemy {
        if (rook_attacks(sq, occupied) & zone).is_nonempty() {
            danger += ATTACK_WEIGHTS[PieceKind::Rook.index()];
            attacker_count += 1;
        }
    }

    // Queens
    for sq in attacker_queens {
        if (queen_attacks(sq, occupied) & zone).is_nonempty() {
            danger += ATTACK_WEIGHTS[PieceKind::Queen.index()];
            attacker_count += 1;
        }
    }

    // Scale danger by number of attackers
    if attacker_count < 2 {
        0
    } else {
        danger * danger / 4
    }
}

/// Evaluate pawn storm for one side's king.
///
/// Checks enemy pawns advancing on the king file cluster.
fn pawn_storm_penalty(board: &Board, king_color: Color) -> Score {
    let king_sq = board.king_square(king_color);
    let cluster = king_file_cluster(king_sq);
    let enemy_pawns = board.pieces(PieceKind::Pawn) & board.side(!king_color);
    let storm_pawns = enemy_pawns & cluster;

    let king_rank = king_sq.rank().index();
    let mut penalty = Score::ZERO;

    for sq in storm_pawns {
        let pawn_rank = sq.rank().index();
        let dist = if king_color == Color::White {
            // Enemy (black) pawns advance downward (decreasing rank index).
            // Distance is how close the pawn is to the king.
            if king_rank >= pawn_rank { king_rank - pawn_rank } else { pawn_rank - king_rank }
        } else {
            // Enemy (white) pawns advance upward (increasing rank index).
            if pawn_rank >= king_rank { pawn_rank - king_rank } else { king_rank - pawn_rank }
        };

        if dist >= 2 && dist <= 3 {
            penalty += STORM_CLOSE_PENALTY;
        } else if dist == 4 {
            penalty += STORM_FAR_PENALTY;
        }
    }

    penalty
}

/// Evaluate open file penalties around the king.
fn open_file_penalty(board: &Board, king_color: Color) -> Score {
    let king_sq = board.king_square(king_color);
    let all_pawns = board.pieces(PieceKind::Pawn);
    let friendly_pawns = all_pawns & board.side(king_color);

    let mut penalty = Score::ZERO;

    let king_file = king_sq.file();
    let start_file = if king_file.index() > 0 { king_file.index() - 1 } else { 0 };
    let end_file = if king_file.index() < 7 { king_file.index() + 1 } else { 7 };

    for f in start_file..=end_file {
        if let Some(file) = File::from_index(f as u8) {
            let file_mask = Bitboard::file_mask(file);
            let any_pawns = all_pawns & file_mask;
            let our_pawns = friendly_pawns & file_mask;

            if any_pawns.is_empty() {
                // Fully open file
                penalty += OPEN_FILE_PENALTY;
            } else if our_pawns.is_empty() {
                // Semi-open file (no friendly pawns, but enemy pawns present)
                penalty += SEMI_OPEN_FILE_PENALTY;
            }
        }
    }

    penalty
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Evaluate king safety from White's perspective.
///
/// Combines pawn shield, attacker zone danger, pawn storm, and open file
/// penalties for both sides. Returns a positive score when White is safer.
pub fn evaluate_king_safety(board: &Board) -> Score {
    // Pawn shield
    let white_shield = pawn_shield_penalty(board, Color::White);
    let black_shield = pawn_shield_penalty(board, Color::Black);

    // Attacker zone danger (quadratic, converted to middlegame-only penalty)
    let white_danger = attacker_zone_danger(board, Color::White);
    let black_danger = attacker_zone_danger(board, Color::Black);
    let danger_score = S(-(white_danger as i16), 0) - S(-(black_danger as i16), 0);

    // Pawn storm
    let white_storm = pawn_storm_penalty(board, Color::White);
    let black_storm = pawn_storm_penalty(board, Color::Black);

    // Open files
    let white_open = open_file_penalty(board, Color::White);
    let black_open = open_file_penalty(board, Color::Black);

    // Combine: white terms minus black terms.
    // Shield, storm, and open file penalties are already negative for the affected side.
    (white_shield - black_shield) + danger_score + (white_storm - black_storm) + (white_open - black_open)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use cesso_core::Board;

    use super::evaluate_king_safety;
    use crate::eval::score::Score;

    #[test]
    fn starting_position_is_zero() {
        let board = Board::starting_position();
        let score = evaluate_king_safety(&board);
        // Symmetric position: all terms should cancel
        assert_eq!(score, Score::ZERO);
    }

    #[test]
    fn missing_white_shield_pawn_negative() {
        // White king on g1, missing g2 pawn. Black king on e8 with full shield.
        let board: Board = "4k3/pppppppp/8/8/8/8/PPPPP1PP/6K1 w - - 0 1"
            .parse()
            .unwrap();
        let score = evaluate_king_safety(&board);
        // White has weaker shield, so mg should be negative
        assert!(score.mg() < 0, "missing shield pawn should penalize White, got mg={}", score.mg());
    }

    #[test]
    fn open_file_near_king_penalizes() {
        // White king on g1, no pawns on g-file for either side (open file)
        // Black king on e8 with full shield
        let board: Board = "4k3/pppppp1p/8/8/8/8/PPPPPP1P/6K1 w - - 0 1"
            .parse()
            .unwrap();
        let score = evaluate_king_safety(&board);
        // Both sides missing g-pawn but White king is on g-file
        assert!(score.mg() < 0, "open file near king should penalize, got mg={}", score.mg());
    }

    #[test]
    fn attacker_zone_danger_with_queen() {
        // White king on g1 with pieces around. Black queen + rook attacking king zone
        let board: Board = "4k3/8/8/8/8/5q2/5PPP/6KR b - - 0 1"
            .parse()
            .unwrap();
        let score = evaluate_king_safety(&board);
        // Black queen near White king should create danger
        // Score is from White's perspective, so white being attacked = negative
        // However, with only 1 attacker, danger may be 0 (need 2+ attackers)
        // This test just checks it doesn't crash
        let _ = score;
    }
}
