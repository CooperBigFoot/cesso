//! Magic bitboard tables for sliding piece attack generation.

use std::sync::OnceLock;

use crate::bitboard::Bitboard;

use super::magic_data::{BISHOP_RAW, ROOK_RAW};

/// A single entry in the magic lookup table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MagicEntry {
    pub(crate) magic: u64,
    pub(crate) mask: Bitboard,
    pub(crate) shift: u8,
    pub(crate) offset: u32,
}

// ---------------------------------------------------------------------------
// On-the-fly attack generators (used for table population and cross-validation)
// ---------------------------------------------------------------------------

/// Compute rook attacks from square `sq` with the given `occupied` bitboard,
/// walking rays until a blocker is hit (blocker square is included).
pub(crate) const fn rook_attacks_on_the_fly(sq: usize, occupied: u64) -> u64 {
    let rank = (sq / 8) as i8;
    let file = (sq % 8) as i8;
    let mut attacks = 0u64;

    // North
    let mut r = rank + 1;
    while r < 8 {
        let bit = 1u64 << (r as usize * 8 + file as usize);
        attacks |= bit;
        if occupied & bit != 0 {
            break;
        }
        r += 1;
    }
    // South
    r = rank - 1;
    while r >= 0 {
        let bit = 1u64 << (r as usize * 8 + file as usize);
        attacks |= bit;
        if occupied & bit != 0 {
            break;
        }
        r -= 1;
    }
    // East
    let mut f = file + 1;
    while f < 8 {
        let bit = 1u64 << (rank as usize * 8 + f as usize);
        attacks |= bit;
        if occupied & bit != 0 {
            break;
        }
        f += 1;
    }
    // West
    f = file - 1;
    while f >= 0 {
        let bit = 1u64 << (rank as usize * 8 + f as usize);
        attacks |= bit;
        if occupied & bit != 0 {
            break;
        }
        f -= 1;
    }

    attacks
}

/// Compute bishop attacks from square `sq` with the given `occupied` bitboard,
/// walking diagonals until a blocker is hit (blocker square is included).
pub(crate) const fn bishop_attacks_on_the_fly(sq: usize, occupied: u64) -> u64 {
    let rank = (sq / 8) as i8;
    let file = (sq % 8) as i8;
    let mut attacks = 0u64;

    // NE
    let mut r = rank + 1;
    let mut f = file + 1;
    while r < 8 && f < 8 {
        let bit = 1u64 << (r as usize * 8 + f as usize);
        attacks |= bit;
        if occupied & bit != 0 {
            break;
        }
        r += 1;
        f += 1;
    }
    // NW
    r = rank + 1;
    f = file - 1;
    while r < 8 && f >= 0 {
        let bit = 1u64 << (r as usize * 8 + f as usize);
        attacks |= bit;
        if occupied & bit != 0 {
            break;
        }
        r += 1;
        f -= 1;
    }
    // SE
    r = rank - 1;
    f = file + 1;
    while r >= 0 && f < 8 {
        let bit = 1u64 << (r as usize * 8 + f as usize);
        attacks |= bit;
        if occupied & bit != 0 {
            break;
        }
        r -= 1;
        f += 1;
    }
    // SW
    r = rank - 1;
    f = file - 1;
    while r >= 0 && f >= 0 {
        let bit = 1u64 << (r as usize * 8 + f as usize);
        attacks |= bit;
        if occupied & bit != 0 {
            break;
        }
        r -= 1;
        f -= 1;
    }

    attacks
}

// ---------------------------------------------------------------------------
// Magic index computation
// ---------------------------------------------------------------------------

#[inline(always)]
fn magic_index(entry: &MagicEntry, occupied: Bitboard) -> usize {
    let relevant = (occupied & entry.mask).inner();
    let hash = relevant.wrapping_mul(entry.magic);
    (hash >> entry.shift) as usize
}

// ---------------------------------------------------------------------------
// Lazy-initialized sliding attack tables
// ---------------------------------------------------------------------------

struct SlidingTables {
    rook_entries: [MagicEntry; 64],
    bishop_entries: [MagicEntry; 64],
    rook_attacks: Vec<Bitboard>,
    bishop_attacks: Vec<Bitboard>,
}

static SLIDING_TABLES: OnceLock<SlidingTables> = OnceLock::new();

fn build_entries_and_size(
    raw: &[super::magic_data::RawMagic; 64],
) -> ([MagicEntry; 64], usize) {
    let dummy = MagicEntry { magic: 0, mask: Bitboard::EMPTY, shift: 0, offset: 0 };
    let mut entries = [dummy; 64];
    let mut offset: u32 = 0;
    let mut sq = 0usize;
    while sq < 64 {
        entries[sq] = MagicEntry {
            magic: raw[sq].magic,
            mask: Bitboard::new(raw[sq].mask),
            shift: raw[sq].shift,
            offset,
        };
        let table_size = 1u32 << (64 - raw[sq].shift);
        offset = offset.checked_add(table_size).expect("offset overflow building magic tables");
        sq += 1;
    }
    (entries, offset as usize)
}

fn populate_attacks(
    entries: &[MagicEntry; 64],
    table: &mut [Bitboard],
    on_the_fly: fn(usize, u64) -> u64,
) {
    for (sq, entry) in entries.iter().enumerate() {
        let mask = entry.mask.inner();
        // Carry-rippler trick: enumerate all subsets of mask
        let mut subset: u64 = 0;
        loop {
            let attacks = Bitboard::new(on_the_fly(sq, subset));
            let idx = entry.offset as usize + magic_index(entry, Bitboard::new(subset));
            table[idx] = attacks;
            // Advance to next subset (carry-rippler)
            subset = subset.wrapping_sub(mask) & mask;
            if subset == 0 {
                break;
            }
        }
    }
}

fn tables() -> &'static SlidingTables {
    SLIDING_TABLES.get_or_init(|| {
        let (rook_entries, rook_size) = build_entries_and_size(&ROOK_RAW);
        let (bishop_entries, bishop_size) = build_entries_and_size(&BISHOP_RAW);

        let mut rook_attacks = vec![Bitboard::EMPTY; rook_size];
        let mut bishop_attacks = vec![Bitboard::EMPTY; bishop_size];

        populate_attacks(&rook_entries, &mut rook_attacks, rook_attacks_on_the_fly);
        populate_attacks(&bishop_entries, &mut bishop_attacks, bishop_attacks_on_the_fly);

        SlidingTables {
            rook_entries,
            bishop_entries,
            rook_attacks,
            bishop_attacks,
        }
    })
}

// ---------------------------------------------------------------------------
// Public lookup functions
// ---------------------------------------------------------------------------

/// Look up rook attacks from square `sq` given `occupied` squares.
#[inline]
pub(crate) fn rook_attacks_lookup(sq: usize, occupied: Bitboard) -> Bitboard {
    let t = tables();
    let entry = &t.rook_entries[sq];
    let idx = entry.offset as usize + magic_index(entry, occupied);
    t.rook_attacks[idx]
}

/// Look up bishop attacks from square `sq` given `occupied` squares.
#[inline]
pub(crate) fn bishop_attacks_lookup(sq: usize, occupied: Bitboard) -> Bitboard {
    let t = tables();
    let entry = &t.bishop_entries[sq];
    let idx = entry.offset as usize + magic_index(entry, occupied);
    t.bishop_attacks[idx]
}
