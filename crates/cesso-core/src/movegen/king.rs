//! King move and castling generation.

use crate::attacks::king_attacks;
use crate::board::Board;
use crate::castle_rights::CastleRights;
use crate::chess_move::Move;
use crate::color::Color;
use crate::square::Square;

use super::MoveList;
use super::is_attacked;

/// Generate legal king moves (normal moves + castling).
pub(super) fn gen_king(board: &Board, king_sq: Square, list: &mut MoveList) {
    let us = board.side_to_move();
    let them = us.flip();
    let friendly = board.side(us);
    // Remove king from occupied so sliding pieces "see through" the king when
    // checking destination safety (prevents the king from blocking its own retreat).
    let occupied_no_king = board.occupied() ^ king_sq.bitboard();

    // Normal king moves
    let mut targets = king_attacks(king_sq) & !friendly;
    while let Some((dst, rest)) = targets.pop_lsb() {
        targets = rest;
        if !is_attacked(board, dst, them, occupied_no_king) {
            list.push(Move::new(king_sq, dst));
        }
    }

    // Castling — only when not currently in check
    if is_attacked(board, king_sq, them, board.occupied()) {
        return;
    }

    let castling = board.castling();
    let occupied = board.occupied();

    match us {
        Color::White => {
            // Kingside: E1→G1, F1 and G1 must be empty and not attacked
            if castling.contains(CastleRights::WHITE_KING) {
                let path_clear =
                    !occupied.contains(Square::F1) && !occupied.contains(Square::G1);
                if path_clear
                    && !is_attacked(board, Square::F1, them, occupied)
                    && !is_attacked(board, Square::G1, them, occupied)
                {
                    list.push(Move::new_castle(Square::E1, Square::G1));
                }
            }
            // Queenside: E1→C1, B1/C1/D1 must be empty, C1 and D1 not attacked
            if castling.contains(CastleRights::WHITE_QUEEN) {
                let path_clear = !occupied.contains(Square::B1)
                    && !occupied.contains(Square::C1)
                    && !occupied.contains(Square::D1);
                if path_clear
                    && !is_attacked(board, Square::C1, them, occupied)
                    && !is_attacked(board, Square::D1, them, occupied)
                {
                    list.push(Move::new_castle(Square::E1, Square::C1));
                }
            }
        }
        Color::Black => {
            // Kingside: E8→G8, F8 and G8 must be empty and not attacked
            if castling.contains(CastleRights::BLACK_KING) {
                let path_clear =
                    !occupied.contains(Square::F8) && !occupied.contains(Square::G8);
                if path_clear
                    && !is_attacked(board, Square::F8, them, occupied)
                    && !is_attacked(board, Square::G8, them, occupied)
                {
                    list.push(Move::new_castle(Square::E8, Square::G8));
                }
            }
            // Queenside: E8→C8, B8/C8/D8 must be empty, C8 and D8 not attacked
            if castling.contains(CastleRights::BLACK_QUEEN) {
                let path_clear = !occupied.contains(Square::B8)
                    && !occupied.contains(Square::C8)
                    && !occupied.contains(Square::D8);
                if path_clear
                    && !is_attacked(board, Square::C8, them, occupied)
                    && !is_attacked(board, Square::D8, them, occupied)
                {
                    list.push(Move::new_castle(Square::E8, Square::C8));
                }
            }
        }
    }
}
