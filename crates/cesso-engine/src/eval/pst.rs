//! Piece-square tables (PST) for all six piece types.
//!
//! All tables are defined from White's perspective in LERF order:
//! index 0 = A1, index 7 = H1, index 8 = A2, ..., index 63 = H8.
//! Use [`pst_value`] to look up the value for either color.

use cesso_core::{Color, PieceKind, Square};

use crate::eval::score::{Score, S};

// ---------------------------------------------------------------------------
// Individual piece-square tables
// ---------------------------------------------------------------------------

/// Pawn PST. Rank 1 and rank 8 entries are S(0,0) — pawns never sit there.
#[rustfmt::skip]
const PAWN_PST: [Score; 64] = [
    // Rank 1 (indices 0-7) — never used
    S(0,0),    S(0,0),    S(0,0),    S(0,0),    S(0,0),    S(0,0),    S(0,0),    S(0,0),
    // Rank 2 (indices 8-15)
    S(5,-5),   S(10,-5),  S(10,-10), S(-20,-10),S(-20,-10),S(10,-10), S(10,-5),  S(5,-5),
    // Rank 3 (indices 16-23)
    S(5,0),    S(-5,0),   S(-10,0),  S(0,5),    S(0,5),    S(-10,0),  S(-5,0),   S(5,0),
    // Rank 4 (indices 24-31)
    S(0,5),    S(0,5),    S(0,5),    S(20,20),  S(20,20),  S(0,5),    S(0,5),    S(0,5),
    // Rank 5 (indices 32-39)
    S(5,10),   S(5,10),   S(10,15),  S(25,25),  S(25,25),  S(10,15),  S(5,10),   S(5,10),
    // Rank 6 (indices 40-47)
    S(10,20),  S(10,20),  S(20,30),  S(30,30),  S(30,30),  S(20,30),  S(10,20),  S(10,20),
    // Rank 7 (indices 48-55)
    S(100,200),S(100,200),S(100,200),S(100,200),S(100,200),S(100,200),S(100,200),S(100,200),
    // Rank 8 (indices 56-63) — never used
    S(0,0),    S(0,0),    S(0,0),    S(0,0),    S(0,0),    S(0,0),    S(0,0),    S(0,0),
];

#[rustfmt::skip]
const KNIGHT_PST: [Score; 64] = [
    // Rank 1 (indices 0-7)
    S(-50,-50),S(-40,-40),S(-30,-30),S(-30,-30),S(-30,-30),S(-30,-30),S(-40,-40),S(-50,-50),
    // Rank 2 (indices 8-15)
    S(-40,-40),S(-20,-20),S(0,0),    S(5,5),    S(5,5),    S(0,0),    S(-20,-20),S(-40,-40),
    // Rank 3 (indices 16-23)
    S(-30,-30),S(5,0),    S(10,10),  S(15,15),  S(15,15),  S(10,10),  S(5,0),    S(-30,-30),
    // Rank 4 (indices 24-31)
    S(-30,-20),S(0,5),    S(15,15),  S(20,20),  S(20,20),  S(15,15),  S(0,5),    S(-30,-20),
    // Rank 5 (indices 32-39)
    S(-30,-20),S(5,5),    S(15,15),  S(20,20),  S(20,20),  S(15,15),  S(5,5),    S(-30,-20),
    // Rank 6 (indices 40-47)
    S(-30,-30),S(0,0),    S(10,10),  S(15,15),  S(15,15),  S(10,10),  S(0,0),    S(-30,-30),
    // Rank 7 (indices 48-55)
    S(-40,-40),S(-20,-20),S(0,0),    S(0,0),    S(0,0),    S(0,0),    S(-20,-20),S(-40,-40),
    // Rank 8 (indices 56-63)
    S(-50,-50),S(-40,-40),S(-30,-30),S(-30,-30),S(-30,-30),S(-30,-30),S(-40,-40),S(-50,-50),
];

#[rustfmt::skip]
const BISHOP_PST: [Score; 64] = [
    // Rank 1 (indices 0-7)
    S(-20,-20),S(-10,-10),S(-10,-10),S(-10,-10),S(-10,-10),S(-10,-10),S(-10,-10),S(-20,-20),
    // Rank 2 (indices 8-15)
    S(-10,-10),S(5,0),    S(0,0),    S(0,0),    S(0,0),    S(0,0),    S(5,0),    S(-10,-10),
    // Rank 3 (indices 16-23)
    S(-10,-10),S(5,5),    S(5,5),    S(5,5),    S(5,5),    S(5,5),    S(5,5),    S(-10,-10),
    // Rank 4 (indices 24-31)
    S(-10,-5), S(5,0),    S(5,5),    S(10,10),  S(10,10),  S(5,5),    S(5,0),    S(-10,-5),
    // Rank 5 (indices 32-39)
    S(-10,-5), S(0,0),    S(5,10),   S(10,10),  S(10,10),  S(5,10),   S(0,0),    S(-10,-5),
    // Rank 6 (indices 40-47)
    S(-10,-5), S(10,5),   S(0,0),    S(5,5),    S(5,5),    S(0,0),    S(10,5),   S(-10,-5),
    // Rank 7 (indices 48-55)
    S(-10,-10),S(0,0),    S(0,0),    S(0,0),    S(0,0),    S(0,0),    S(0,0),    S(-10,-10),
    // Rank 8 (indices 56-63)
    S(-20,-20),S(-10,-10),S(-10,-10),S(-10,-10),S(-10,-10),S(-10,-10),S(-10,-10),S(-20,-20),
];

#[rustfmt::skip]
const ROOK_PST: [Score; 64] = [
    // Rank 1 (indices 0-7)
    S(0,0),   S(0,0),   S(0,5),   S(5,5),   S(5,5),   S(0,5),   S(0,0),   S(0,0),
    // Rank 2 (indices 8-15)
    S(-5,0),  S(0,5),   S(0,5),   S(0,5),   S(0,5),   S(0,5),   S(0,5),   S(-5,0),
    // Rank 3 (indices 16-23)
    S(-5,0),  S(0,0),   S(0,0),   S(0,0),   S(0,0),   S(0,0),   S(0,0),   S(-5,0),
    // Rank 4 (indices 24-31)
    S(-5,0),  S(0,0),   S(0,0),   S(0,0),   S(0,0),   S(0,0),   S(0,0),   S(-5,0),
    // Rank 5 (indices 32-39)
    S(-5,0),  S(0,0),   S(0,0),   S(0,0),   S(0,0),   S(0,0),   S(0,0),   S(-5,0),
    // Rank 6 (indices 40-47)
    S(-5,0),  S(0,0),   S(0,0),   S(0,0),   S(0,0),   S(0,0),   S(0,0),   S(-5,0),
    // Rank 7 (indices 48-55)
    S(5,10),  S(10,10), S(10,10), S(10,10), S(10,10), S(10,10), S(10,10), S(5,10),
    // Rank 8 (indices 56-63)
    S(0,5),   S(0,5),   S(0,5),   S(0,5),   S(0,5),   S(0,5),   S(0,5),   S(0,5),
];

#[rustfmt::skip]
const QUEEN_PST: [Score; 64] = [
    // Rank 1 (indices 0-7)
    S(-20,-20),S(-10,-10),S(-10,-10),S(-5,-5), S(-5,-5), S(-10,-10),S(-10,-10),S(-20,-20),
    // Rank 2 (indices 8-15)
    S(-10,-10),S(0,0),    S(5,0),    S(0,0),   S(0,0),   S(5,0),    S(0,0),    S(-10,-10),
    // Rank 3 (indices 16-23)
    S(-10,-5), S(5,5),    S(5,5),    S(5,5),   S(5,5),   S(5,5),    S(5,5),    S(-10,-5),
    // Rank 4 (indices 24-31)
    S(0,0),    S(0,5),    S(5,5),    S(5,10),  S(5,10),  S(5,5),    S(0,5),    S(0,0),
    // Rank 5 (indices 32-39)
    S(-5,0),   S(0,5),    S(5,5),    S(5,10),  S(5,10),  S(5,5),    S(0,5),    S(-5,0),
    // Rank 6 (indices 40-47)
    S(-10,-5), S(0,5),    S(5,5),    S(5,5),   S(5,5),   S(5,5),    S(0,5),    S(-10,-5),
    // Rank 7 (indices 48-55)
    S(-10,-10),S(0,0),    S(0,0),    S(0,0),   S(0,0),   S(0,0),    S(0,0),    S(-10,-10),
    // Rank 8 (indices 56-63)
    S(-20,-20),S(-10,-10),S(-10,-10),S(-5,0),  S(-5,0),  S(-10,-10),S(-10,-10),S(-20,-20),
];

/// King PST. Middlegame values reward castled corners; endgame values reward centralization.
#[rustfmt::skip]
const KING_PST: [Score; 64] = [
    // Rank 1 (indices 0-7)
    S(20,-20), S(30,-10), S(10,0),   S(0,0),   S(0,0),   S(10,0),   S(30,-10), S(20,-20),
    // Rank 2 (indices 8-15)
    S(20,-5),  S(20,0),   S(0,5),    S(0,5),   S(0,5),   S(0,5),    S(20,0),   S(20,-5),
    // Rank 3 (indices 16-23)
    S(-10,5),  S(-20,10), S(-20,10), S(-20,10),S(-20,10),S(-20,10), S(-20,10), S(-10,5),
    // Rank 4 (indices 24-31)
    S(-20,0),  S(-30,10), S(-30,10), S(-40,10),S(-40,10),S(-30,10), S(-30,10), S(-20,0),
    // Rank 5 (indices 32-39)
    S(-30,-10),S(-40,0),  S(-40,0),  S(-50,10),S(-50,10),S(-40,0),  S(-40,0),  S(-30,-10),
    // Rank 6 (indices 40-47)
    S(-30,-20),S(-40,-10),S(-40,-10),S(-50,-10),S(-50,-10),S(-40,-10),S(-40,-10),S(-30,-20),
    // Rank 7 (indices 48-55)
    S(-30,-30),S(-40,-20),S(-40,-20),S(-50,-20),S(-50,-20),S(-40,-20),S(-40,-20),S(-30,-30),
    // Rank 8 (indices 56-63)
    S(-30,-50),S(-40,-30),S(-40,-30),S(-50,-30),S(-50,-30),S(-40,-30),S(-40,-30),S(-30,-50),
];

// ---------------------------------------------------------------------------
// Master table
// ---------------------------------------------------------------------------

/// Piece-square table values indexed `[piece_kind][square]`.
///
/// Defined from White's perspective in LERF order (A1 = index 0).
/// Use [`pst_value`] rather than indexing this directly, so that color
/// mirroring is handled correctly.
pub static PST: [[Score; 64]; PieceKind::COUNT] = [
    PAWN_PST,
    KNIGHT_PST,
    BISHOP_PST,
    ROOK_PST,
    QUEEN_PST,
    KING_PST,
];

// ---------------------------------------------------------------------------
// Lookup helper
// ---------------------------------------------------------------------------

/// Look up the PST bonus for a piece of the given kind and color on `sq`.
///
/// For Black pieces the square is mirrored vertically (`sq ^ 56`) so that the
/// tables, which are defined from White's perspective, apply symmetrically.
#[inline]
pub fn pst_value(kind: PieceKind, color: Color, sq: Square) -> Score {
    let idx = match color {
        Color::White => sq.index(),
        Color::Black => sq.index() ^ 56,
    };
    PST[kind.index()][idx]
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use cesso_core::{Color, PieceKind, Square};

    use super::pst_value;

    /// E4 for White is rank 4 (index 3 from rank 1), file E (index 4).
    /// LERF index = 3*8 + 4 = 28.
    #[test]
    fn pawn_white_e4() {
        let score = pst_value(PieceKind::Pawn, Color::White, Square::E4);
        assert_eq!(score.mg(), 20);
        assert_eq!(score.eg(), 20);
    }

    /// E5 for Black mirrors to rank 4 for White: index 36 ^ 56 = 28 (= E4).
    #[test]
    fn pawn_black_e5_mirrors_white_e4() {
        let white_e4 = pst_value(PieceKind::Pawn, Color::White, Square::E4);
        let black_e5 = pst_value(PieceKind::Pawn, Color::Black, Square::E5);
        assert_eq!(white_e4, black_e5);
    }

    /// Knight table is symmetric: A1 and H1 should have the same value.
    #[test]
    fn knight_a1_h1_symmetric() {
        let a1 = pst_value(PieceKind::Knight, Color::White, Square::A1);
        let h1 = pst_value(PieceKind::Knight, Color::White, Square::H1);
        assert_eq!(a1, h1);
    }

    /// Bishop table is symmetric: A1 and H1 should have the same value.
    #[test]
    fn bishop_a1_h1_symmetric() {
        let a1 = pst_value(PieceKind::Bishop, Color::White, Square::A1);
        let h1 = pst_value(PieceKind::Bishop, Color::White, Square::H1);
        assert_eq!(a1, h1);
    }

    /// Black mirroring: pst_value for Black on rank 1 should equal White on rank 8.
    #[test]
    fn black_rank1_mirrors_white_rank8() {
        // A1 for Black: index 0 ^ 56 = 56 = A8 for White.
        let black_a1 = pst_value(PieceKind::King, Color::Black, Square::A1);
        let white_a8 = pst_value(PieceKind::King, Color::White, Square::A8);
        assert_eq!(black_a1, white_a8);
    }
}
