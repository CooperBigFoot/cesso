//! Zobrist hashing keys for position deduplication.

use crate::board::Board;
use crate::color::Color;
use crate::piece::Piece;

/// Zobrist key for each (piece, square) pair. Indexed by `[Piece::index()][Square::index()]`.
/// Piece::index() returns 0-11: White P,N,B,R,Q,K then Black P,N,B,R,Q,K.
pub(crate) static PIECE_SQUARE: [[u64; 64]; 12] = {
    let mut table = [[0u64; 64]; 12];
    let mut state = SEED;
    let mut piece = 0;
    while piece < 12 {
        let mut sq = 0;
        while sq < 64 {
            let (val, next) = xorshift64(state);
            table[piece][sq] = val;
            state = next;
            sq += 1;
        }
        piece += 1;
    }
    table
};

/// Zobrist key XORed when Black is the side to move.
pub(crate) static SIDE_TO_MOVE: u64 = {
    // State continues from PIECE_SQUARE generation
    let mut state = SEED;
    // Advance past all 12*64 = 768 piece-square keys
    let mut i = 0;
    while i < 768 {
        let (_, next) = xorshift64(state);
        state = next;
        i += 1;
    }
    let (val, _) = xorshift64(state);
    val
};

/// Zobrist keys for castling configurations. Indexed by `CastleRights::bits() as usize` (0..16).
pub(crate) static CASTLING: [u64; 16] = {
    let mut table = [0u64; 16];
    let mut state = SEED;
    // Advance past 768 + 1 = 769 previous keys
    let mut i = 0;
    while i < 769 {
        let (_, next) = xorshift64(state);
        state = next;
        i += 1;
    }
    let mut idx = 0;
    while idx < 16 {
        let (val, next) = xorshift64(state);
        table[idx] = val;
        state = next;
        idx += 1;
    }
    table
};

/// Zobrist keys for en passant file. Indexed by `File::index()` (0..8).
pub(crate) static EN_PASSANT_FILE: [u64; 8] = {
    let mut table = [0u64; 8];
    let mut state = SEED;
    // Advance past 769 + 16 = 785 previous keys
    let mut i = 0;
    while i < 785 {
        let (_, next) = xorshift64(state);
        state = next;
        i += 1;
    }
    let mut idx = 0;
    while idx < 8 {
        let (val, next) = xorshift64(state);
        table[idx] = val;
        state = next;
        idx += 1;
    }
    table
};

const SEED: u64 = 0x5a4f_4252_4953_5421; // "ZOBRIST!"

/// Xorshift64 PRNG. Returns (value, next_state).
const fn xorshift64(mut state: u64) -> (u64, u64) {
    state ^= state << 13;
    state ^= state >> 7;
    state ^= state << 17;
    (state, state)
}

/// Compute a Zobrist hash from scratch for the given board.
pub(crate) fn hash_from_scratch(board: &Board) -> u64 {
    let mut hash = 0u64;

    // Hash each piece on each square
    for piece in Piece::ALL {
        let kind = piece.kind();
        let color = piece.color();
        let mut bb = board.pieces(kind) & board.side(color);
        while let Some((sq, rest)) = bb.pop_lsb() {
            hash ^= PIECE_SQUARE[piece.index()][sq.index()];
            bb = rest;
        }
    }

    // Hash side to move
    if board.side_to_move() == Color::Black {
        hash ^= SIDE_TO_MOVE;
    }

    // Hash castling rights
    hash ^= CASTLING[board.castling().bits() as usize];

    // Hash en passant file (if any)
    if let Some(ep_sq) = board.en_passant() {
        hash ^= EN_PASSANT_FILE[ep_sq.file().index()];
    }

    hash
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::Board;

    #[test]
    fn starting_position_nonzero_hash() {
        let board = Board::starting_position();
        assert_ne!(hash_from_scratch(&board), 0);
    }

    #[test]
    fn starting_position_hash_matches_field() {
        let board = Board::starting_position();
        assert_eq!(board.hash(), hash_from_scratch(&board));
    }

    #[test]
    fn different_positions_different_hashes() {
        let starting = Board::starting_position();
        let sicilian: Board = "rnbqkbnr/pp1ppppp/8/2p5/4P3/8/PPPP1PPP/RNBQKBNR w KQkq c6 0 2"
            .parse()
            .unwrap();
        assert_ne!(starting.hash(), sicilian.hash());
    }

    #[test]
    fn fen_parsed_board_has_correct_hash() {
        let from_fen: Board = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
            .parse()
            .unwrap();
        assert_eq!(from_fen.hash(), hash_from_scratch(&from_fen));
    }

    #[test]
    fn all_keys_are_unique() {
        // Check that no two piece-square keys are the same
        let mut all_keys = Vec::new();
        for piece_keys in &PIECE_SQUARE {
            for &key in piece_keys {
                all_keys.push(key);
            }
        }
        all_keys.push(SIDE_TO_MOVE);
        for &key in &CASTLING {
            all_keys.push(key);
        }
        for &key in &EN_PASSANT_FILE {
            all_keys.push(key);
        }

        let count = all_keys.len();
        all_keys.sort();
        all_keys.dedup();
        assert_eq!(all_keys.len(), count, "some Zobrist keys collide");
    }
}
