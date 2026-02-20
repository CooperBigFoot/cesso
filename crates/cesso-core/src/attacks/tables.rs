//! Precomputed leaper attack tables and geometric ray tables.

use crate::bitboard::Bitboard;

const fn signum(x: i8) -> i8 {
    if x > 0 {
        1
    } else if x < 0 {
        -1
    } else {
        0
    }
}

const fn compute_knight_attacks() -> [Bitboard; 64] {
    let deltas: [(i8, i8); 8] = [
        (-2, -1), (-2, 1), (-1, -2), (-1, 2),
        (1, -2), (1, 2), (2, -1), (2, 1),
    ];

    let mut table = [Bitboard::EMPTY; 64];
    let mut sq = 0usize;
    while sq < 64 {
        let rank = (sq / 8) as i8;
        let file = (sq % 8) as i8;
        let mut bits = 0u64;
        let mut d = 0;
        while d < 8 {
            let r = rank + deltas[d].0;
            let f = file + deltas[d].1;
            if r >= 0 && r < 8 && f >= 0 && f < 8 {
                bits |= 1u64 << (r as usize * 8 + f as usize);
            }
            d += 1;
        }
        table[sq] = Bitboard::new(bits);
        sq += 1;
    }
    table
}

const fn compute_king_attacks() -> [Bitboard; 64] {
    let deltas: [(i8, i8); 8] = [
        (-1, -1), (-1, 0), (-1, 1),
        (0, -1),           (0, 1),
        (1, -1),  (1, 0),  (1, 1),
    ];

    let mut table = [Bitboard::EMPTY; 64];
    let mut sq = 0usize;
    while sq < 64 {
        let rank = (sq / 8) as i8;
        let file = (sq % 8) as i8;
        let mut bits = 0u64;
        let mut d = 0;
        while d < 8 {
            let r = rank + deltas[d].0;
            let f = file + deltas[d].1;
            if r >= 0 && r < 8 && f >= 0 && f < 8 {
                bits |= 1u64 << (r as usize * 8 + f as usize);
            }
            d += 1;
        }
        table[sq] = Bitboard::new(bits);
        sq += 1;
    }
    table
}

/// Compute pawn attack tables for both colors.
///
/// Index 0 = White (attacks NE/NW), index 1 = Black (attacks SE/SW).
/// Wrapping is prevented by masking: pawns on file A cannot attack to the
/// left (which would wrap to file H), and pawns on file H cannot attack right.
const fn compute_pawn_attacks() -> [[Bitboard; 64]; 2] {
    // FILE_A mask: 0x0101010101010101, FILE_H mask: 0x8080808080808080
    const FILE_A_BITS: u64 = 0x0101_0101_0101_0101;
    const FILE_H_BITS: u64 = 0x8080_8080_8080_8080;

    let mut table = [[Bitboard::EMPTY; 64]; 2];
    let mut sq = 0usize;
    while sq < 64 {
        let bit = 1u64 << sq;

        // White: attacks go north (rank+1). NW = shift left 7, NE = shift left 9.
        // NW attack: mask out FILE_A to prevent wrap from A-file to H-file.
        // NE attack: mask out FILE_H to prevent wrap from H-file to A-file.
        let white_nw = (bit << 7) & !FILE_H_BITS; // shift north-west
        let white_ne = (bit << 9) & !FILE_A_BITS; // shift north-east
        table[0][sq] = Bitboard::new(white_nw | white_ne);

        // Black: attacks go south (rank-1). SW = shift right 9, SE = shift right 7.
        // SE attack: mask out FILE_A to prevent wrap.
        // SW attack: mask out FILE_H to prevent wrap.
        let black_se = (bit >> 7) & !FILE_A_BITS; // shift south-east
        let black_sw = (bit >> 9) & !FILE_H_BITS; // shift south-west
        table[1][sq] = Bitboard::new(black_se | black_sw);

        sq += 1;
    }
    table
}

const fn compute_between() -> [[Bitboard; 64]; 64] {
    let mut table = [[Bitboard::EMPTY; 64]; 64];
    let mut s1 = 0usize;
    while s1 < 64 {
        let mut s2 = 0usize;
        while s2 < 64 {
            if s1 != s2 {
                let r1 = (s1 / 8) as i8;
                let f1 = (s1 % 8) as i8;
                let r2 = (s2 / 8) as i8;
                let f2 = (s2 % 8) as i8;
                let raw_dr = r2 - r1;
                let raw_df = f2 - f1;
                // Check alignment using raw deltas: same rank, same file, or true diagonal.
                let raw_dr_abs = if raw_dr < 0 { -raw_dr } else { raw_dr };
                let raw_df_abs = if raw_df < 0 { -raw_df } else { raw_df };
                let aligned =
                    raw_dr == 0 || raw_df == 0 || (raw_dr_abs == raw_df_abs);
                if aligned {
                    let dr = signum(raw_dr);
                    let df = signum(raw_df);
                    let mut bits = 0u64;
                    let mut r = r1 + dr;
                    let mut f = f1 + df;
                    // Walk from s1 toward s2, collecting squares strictly between them.
                    while (r != r2 || f != f2) && r >= 0 && r < 8 && f >= 0 && f < 8 {
                        bits |= 1u64 << (r as usize * 8 + f as usize);
                        r += dr;
                        f += df;
                    }
                    table[s1][s2] = Bitboard::new(bits);
                }
            }
            s2 += 1;
        }
        s1 += 1;
    }
    table
}

const fn compute_line() -> [[Bitboard; 64]; 64] {
    let mut table = [[Bitboard::EMPTY; 64]; 64];
    let mut s1 = 0usize;
    while s1 < 64 {
        let mut s2 = 0usize;
        while s2 < 64 {
            if s1 != s2 {
                let r1 = (s1 / 8) as i8;
                let f1 = (s1 % 8) as i8;
                let r2 = (s2 / 8) as i8;
                let f2 = (s2 % 8) as i8;
                let raw_dr = r2 - r1;
                let raw_df = f2 - f1;
                // Check alignment using raw deltas: same rank, same file, or true diagonal.
                let raw_dr_abs = if raw_dr < 0 { -raw_dr } else { raw_dr };
                let raw_df_abs = if raw_df < 0 { -raw_df } else { raw_df };
                let aligned =
                    raw_dr == 0 || raw_df == 0 || (raw_dr_abs == raw_df_abs);
                if aligned {
                    let dr = signum(raw_dr);
                    let df = signum(raw_df);
                    // Walk in both directions from s1 to cover the full line.
                    let mut bits = 0u64;

                    // Forward direction (toward s2 and beyond)
                    let mut r = r1;
                    let mut f = f1;
                    while r >= 0 && r < 8 && f >= 0 && f < 8 {
                        bits |= 1u64 << (r as usize * 8 + f as usize);
                        r += dr;
                        f += df;
                    }

                    // Backward direction (away from s2)
                    r = r1 - dr;
                    f = f1 - df;
                    while r >= 0 && r < 8 && f >= 0 && f < 8 {
                        bits |= 1u64 << (r as usize * 8 + f as usize);
                        r -= dr;
                        f -= df;
                    }

                    table[s1][s2] = Bitboard::new(bits);
                }
            }
            s2 += 1;
        }
        s1 += 1;
    }
    table
}

pub(crate) static KNIGHT_ATTACKS: [Bitboard; 64] = compute_knight_attacks();
pub(crate) static KING_ATTACKS: [Bitboard; 64] = compute_king_attacks();
pub(crate) static PAWN_ATTACKS: [[Bitboard; 64]; 2] = compute_pawn_attacks();
pub(crate) static BETWEEN: [[Bitboard; 64]; 64] = compute_between();
pub(crate) static LINE: [[Bitboard; 64]; 64] = compute_line();
