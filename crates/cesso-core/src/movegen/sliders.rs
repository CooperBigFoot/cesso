//! Sliding piece (bishop, rook, queen) move generation.

use crate::attacks::{bishop_attacks, line, rook_attacks};
use crate::bitboard::Bitboard;
use crate::board::Board;
use crate::chess_move::Move;
use crate::piece_kind::PieceKind;
use crate::square::Square;

use super::MoveList;
use super::check::CheckType;

/// Generate legal slider moves (bishops, rooks, queens).
pub(super) fn gen_sliders<T: CheckType>(
    board: &Board,
    king_sq: Square,
    pinned: Bitboard,
    check_mask: Bitboard,
    list: &mut MoveList,
) {
    let us = board.side_to_move();
    let friendly = board.side(us);
    let occupied = board.occupied();

    gen_slider_type(board, king_sq, pinned, check_mask, list, friendly, occupied, PieceKind::Bishop, bishop_attacks);
    gen_slider_type(board, king_sq, pinned, check_mask, list, friendly, occupied, PieceKind::Rook, rook_attacks);
    gen_slider_type(
        board,
        king_sq,
        pinned,
        check_mask,
        list,
        friendly,
        occupied,
        PieceKind::Queen,
        |sq, occ| rook_attacks(sq, occ) | bishop_attacks(sq, occ),
    );
}

#[allow(clippy::too_many_arguments)]
fn gen_slider_type(
    board: &Board,
    king_sq: Square,
    pinned: Bitboard,
    check_mask: Bitboard,
    list: &mut MoveList,
    friendly: Bitboard,
    occupied: Bitboard,
    kind: PieceKind,
    attacks_fn: impl Fn(Square, Bitboard) -> Bitboard,
) {
    let us = board.side_to_move();
    let mut pieces = board.pieces(kind) & board.side(us);

    while let Some((src, rest)) = pieces.pop_lsb() {
        pieces = rest;
        let mut targets = attacks_fn(src, occupied) & !friendly & check_mask;

        // Pinned sliders can only move along the pin ray
        if pinned.contains(src) {
            targets &= line(king_sq, src);
        }

        while let Some((dst, rest2)) = targets.pop_lsb() {
            targets = rest2;
            list.push(Move::new(src, dst));
        }
    }
}
