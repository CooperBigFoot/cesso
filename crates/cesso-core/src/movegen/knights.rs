//! Knight move generation.

use crate::attacks::knight_attacks;
use crate::bitboard::Bitboard;
use crate::board::Board;
use crate::chess_move::Move;
use crate::piece_kind::PieceKind;
use crate::square::Square;

use super::MoveList;
use super::check::CheckType;

/// Generate legal knight moves.
pub(super) fn gen_knights<T: CheckType>(
    board: &Board,
    _king_sq: Square,
    pinned: Bitboard,
    check_mask: Bitboard,
    list: &mut MoveList,
) {
    let us = board.side_to_move();
    let friendly = board.side(us);
    let mut knights = board.pieces(PieceKind::Knight) & friendly;

    while let Some((src, rest)) = knights.pop_lsb() {
        knights = rest;
        // Pinned knights can NEVER move (L-shape can never stay on pin ray)
        if pinned.contains(src) {
            continue;
        }
        let mut targets = knight_attacks(src) & !friendly & check_mask;
        while let Some((dst, rest2)) = targets.pop_lsb() {
            targets = rest2;
            list.push(Move::new(src, dst));
        }
    }
}
