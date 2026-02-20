//! Hardcoded magic numbers for sliding piece attack generation.

/// Raw magic entry: magic multiplier, occupancy mask, and right-shift amount.
pub(super) struct RawMagic {
    pub(super) magic: u64,
    pub(super) mask: u64,
    pub(super) shift: u8,
}

// Allow this since we need a Copy-able default for array init in const context.
impl Copy for RawMagic {}
impl Clone for RawMagic {
    fn clone(&self) -> Self {
        *self
    }
}

/// Compute the rook occupancy mask for square index `sq`.
///
/// Rays extend in 4 cardinal directions, excluding the edge squares at the far
/// end of each ray (since blockers on the edge have no further effect). The
/// square itself is also excluded.
const fn rook_mask(sq: usize) -> u64 {
    let rank = (sq / 8) as i8;
    let file = (sq % 8) as i8;
    let mut mask = 0u64;

    // North: rank+1 to rank 6 (not rank 7)
    let mut r = rank + 1;
    while r <= 6 {
        mask |= 1u64 << (r as usize * 8 + file as usize);
        r += 1;
    }
    // South: rank-1 to rank 1 (not rank 0)
    r = rank - 1;
    while r >= 1 {
        mask |= 1u64 << (r as usize * 8 + file as usize);
        r -= 1;
    }
    // East: file+1 to file 6 (not file 7)
    let mut f = file + 1;
    while f <= 6 {
        mask |= 1u64 << (rank as usize * 8 + f as usize);
        f += 1;
    }
    // West: file-1 to file 1 (not file 0)
    f = file - 1;
    while f >= 1 {
        mask |= 1u64 << (rank as usize * 8 + f as usize);
        f -= 1;
    }

    mask
}

/// Compute the bishop occupancy mask for square index `sq`.
///
/// Rays extend in 4 diagonal directions, excluding the edge squares at the far
/// end of each diagonal. The square itself is also excluded.
const fn bishop_mask(sq: usize) -> u64 {
    let rank = (sq / 8) as i8;
    let file = (sq % 8) as i8;
    let mut mask = 0u64;

    // NE
    let mut r = rank + 1;
    let mut f = file + 1;
    while r <= 6 && f <= 6 {
        mask |= 1u64 << (r as usize * 8 + f as usize);
        r += 1;
        f += 1;
    }
    // NW
    r = rank + 1;
    f = file - 1;
    while r <= 6 && f >= 1 {
        mask |= 1u64 << (r as usize * 8 + f as usize);
        r += 1;
        f -= 1;
    }
    // SE
    r = rank - 1;
    f = file + 1;
    while r >= 1 && f <= 6 {
        mask |= 1u64 << (r as usize * 8 + f as usize);
        r -= 1;
        f += 1;
    }
    // SW
    r = rank - 1;
    f = file - 1;
    while r >= 1 && f >= 1 {
        mask |= 1u64 << (r as usize * 8 + f as usize);
        r -= 1;
        f -= 1;
    }

    mask
}

const fn make_rook_raw() -> [RawMagic; 64] {
    // Verified rook magic numbers (CPW "Best Magics so far" / Pradyumna Kannan).
    // These produce no destructive collisions for any square.
    #[rustfmt::skip]
    let magics: [u64; 64] = [
        0x0a8002c000108020, 0x06c00049b0002001, 0x0100200010090040, 0x2480041000800801,
        0x0280028004000800, 0x0900410008040022, 0x0280020001001080, 0x2880002041000080,
        0xa000800080400034, 0x0004808020004000, 0x2290802004801000, 0x0411000d00100020,
        0x0402800800040080, 0x000b000401004208, 0x2409000100040200, 0x0001002100004082,
        0x0022878001e24000, 0x1090810021004010, 0x0801030040200012, 0x0500808008001000,
        0x0a08018014000880, 0x8000808004000200, 0x0201008080010200, 0x0801020000441091,
        0x0000800080204005, 0x1040200040100048, 0x0000120200402082, 0x0d14880480100080,
        0x0012040280080080, 0x0100040080020080, 0x9020010080800200, 0x0813241200148449,
        0x0491604001800080, 0x0100401000402001, 0x4820010021001040, 0x0400402202000812,
        0x0209009005000802, 0x0810800601800400, 0x4301083214000150, 0x204026458e001401,
        0x0040204000808000, 0x8001008040010020, 0x8410820820420010, 0x1003001000090020,
        0x0804040008008080, 0x0012000810020004, 0x1000100200040208, 0x430000a044020001,
        0x0280009023410300, 0x00e0100040002240, 0x0000200100401700, 0x2244100408008080,
        0x0008000400801980, 0x0002000810040200, 0x8010100228810400, 0x2000009044210200,
        0x4080008040102101, 0x0040002080411d01, 0x2005524060000901, 0x0502001008400422,
        0x489a000810200402, 0x0001004400080a13, 0x4000011008020084, 0x0026002114058042,
    ];

    let mut result = [RawMagic { magic: 0, mask: 0, shift: 0 }; 64];
    let mut i = 0;
    while i < 64 {
        let mask = rook_mask(i);
        let shift = 64 - mask.count_ones() as u8;
        result[i] = RawMagic { magic: magics[i], mask, shift };
        i += 1;
    }
    result
}

const fn make_bishop_raw() -> [RawMagic; 64] {
    #[rustfmt::skip]
    let magics: [u64; 64] = [
        0x0002020202020200, 0x0002020202020000, 0x0004010202000000, 0x0004040080000000,
        0x0001104000000000, 0x0000821040000000, 0x0000410410400000, 0x0000104104104000,
        0x0000040404040400, 0x0000020202020200, 0x0000040102020000, 0x0000040400800000,
        0x0000011040000000, 0x0000008210400000, 0x0000004104104000, 0x0000002082082000,
        0x0004000808080800, 0x0002000404040400, 0x0001000202020200, 0x0000800802004000,
        0x0000800400A00000, 0x0000200100884000, 0x0000400082082000, 0x0000200041041000,
        0x0002080010101000, 0x0001040008080800, 0x0000208004010400, 0x0000404004010200,
        0x0000840000802000, 0x0000404002011000, 0x0000808001041000, 0x0000404000820800,
        0x0001041000202000, 0x0000820800101000, 0x0000104400080800, 0x0000020080080080,
        0x0000404040040100, 0x0000808100020100, 0x0001010100020800, 0x0000808080010400,
        0x0000820820004000, 0x0000410410002000, 0x0000082088001000, 0x0000002011000800,
        0x0000080100400400, 0x0001010101000200, 0x0002020202000400, 0x0001010101000200,
        0x0000410410400000, 0x0000208208200000, 0x0000002084100000, 0x0000000020880000,
        0x0000001002020000, 0x0000040408020000, 0x0004040404040000, 0x0002020202020000,
        0x0000104104104000, 0x0000002082082000, 0x0000000020841000, 0x0000000000208800,
        0x0000000010020200, 0x0000000404080200, 0x0000040404040400, 0x0002020202020200,
    ];

    let mut result = [RawMagic { magic: 0, mask: 0, shift: 0 }; 64];
    let mut i = 0;
    while i < 64 {
        let mask = bishop_mask(i);
        let shift = 64 - mask.count_ones() as u8;
        result[i] = RawMagic { magic: magics[i], mask, shift };
        i += 1;
    }
    result
}

pub(super) const ROOK_RAW: [RawMagic; 64] = make_rook_raw();
pub(super) const BISHOP_RAW: [RawMagic; 64] = make_bishop_raw();
