//! Pawn move generation.

use crate::attacks::{line, pawn_attacks, rook_attacks};
use crate::bitboard::Bitboard;
use crate::board::Board;
use crate::chess_move::{Move, PromotionPiece};
use crate::color::Color;
use crate::piece_kind::PieceKind;
use crate::square::Square;

use super::MoveList;
use super::check::CheckType;

/// Generate legal pawn moves.
pub(super) fn gen_pawns<T: CheckType>(
    board: &Board,
    king_sq: Square,
    pinned: Bitboard,
    check_mask: Bitboard,
    list: &mut MoveList,
) {
    let us = board.side_to_move();
    let them = us.flip();
    let friendly = board.side(us);
    let enemy = board.side(them);
    let occupied = board.occupied();
    let empty = !occupied;
    let our_pawns = board.pieces(PieceKind::Pawn) & friendly;

    let (push_dir, promo_rank): (i8, Bitboard) = match us {
        Color::White => (8, Bitboard::RANK_8),
        Color::Black => (-8, Bitboard::RANK_1),
    };

    // --- Single pushes ---
    let single_push = if us == Color::White {
        (our_pawns << 8) & empty
    } else {
        (our_pawns >> 8) & empty
    };

    // Non-promotion single pushes
    let mut quiet_singles = single_push & !promo_rank & check_mask;
    while let Some((dst, rest)) = quiet_singles.pop_lsb() {
        quiet_singles = rest;
        let src = Square::from_index_unchecked((dst.index() as i8 - push_dir) as u8);
        if !pinned.contains(src) || line(king_sq, src).contains(dst) {
            list.push(Move::new(src, dst));
        }
    }

    // Promotion single pushes
    let mut promo_singles = single_push & promo_rank & check_mask;
    while let Some((dst, rest)) = promo_singles.pop_lsb() {
        promo_singles = rest;
        let src = Square::from_index_unchecked((dst.index() as i8 - push_dir) as u8);
        if !pinned.contains(src) || line(king_sq, src).contains(dst) {
            for promo in PromotionPiece::ALL {
                list.push(Move::new_promotion(src, dst, promo));
            }
        }
    }

    // --- Double pushes ---
    let intermediate = if us == Color::White {
        (our_pawns << 8) & empty
    } else {
        (our_pawns >> 8) & empty
    };
    let double_push = if us == Color::White {
        (intermediate << 8) & empty & Bitboard::RANK_4 & check_mask
    } else {
        (intermediate >> 8) & empty & Bitboard::RANK_5 & check_mask
    };

    let mut doubles = double_push;
    while let Some((dst, rest)) = doubles.pop_lsb() {
        doubles = rest;
        let src = Square::from_index_unchecked((dst.index() as i8 - push_dir * 2) as u8);
        if !pinned.contains(src) || line(king_sq, src).contains(dst) {
            list.push(Move::new(src, dst));
        }
    }

    // --- Captures ---
    let mut capturing_pawns = our_pawns;
    while let Some((src, rest)) = capturing_pawns.pop_lsb() {
        capturing_pawns = rest;
        let mut targets = pawn_attacks(us, src) & enemy & check_mask;
        while let Some((dst, rest2)) = targets.pop_lsb() {
            targets = rest2;
            // Pinned pawns can only capture along the pin ray
            if pinned.contains(src) && !line(king_sq, src).contains(dst) {
                continue;
            }
            if promo_rank.contains(dst) {
                for promo in PromotionPiece::ALL {
                    list.push(Move::new_promotion(src, dst, promo));
                }
            } else {
                list.push(Move::new(src, dst));
            }
        }
    }

    // --- En passant ---
    if let Some(ep_sq) = board.en_passant() {
        let mut ep_pawns = pawn_attacks(them, ep_sq) & our_pawns;
        while let Some((src, rest)) = ep_pawns.pop_lsb() {
            ep_pawns = rest;

            // The captured pawn's square
            let captured_sq = Square::from_index_unchecked(if us == Color::White {
                // Captured pawn is on rank below EP target square
                (ep_sq.index() as u8) - 8
            } else {
                // Captured pawn is on rank above EP target square
                (ep_sq.index() as u8) + 8
            });

            // In check: EP must resolve the check
            if T::IN_CHECK {
                let resolves = check_mask.contains(ep_sq) || check_mask.contains(captured_sq);
                if !resolves {
                    continue;
                }
            }

            // Check pin constraint
            if pinned.contains(src) && !line(king_sq, src).contains(ep_sq) {
                continue;
            }

            // Special EP legality: after removing both pawns, check if king is exposed
            // to a horizontal rook/queen attack. This catches the rare case where both
            // the capturing and captured pawn were blocking a slider on the same rank.
            let after_occ = (occupied ^ src.bitboard() ^ captured_sq.bitboard()) | ep_sq.bitboard();
            let their_rook_queen =
                (board.pieces(PieceKind::Rook) | board.pieces(PieceKind::Queen)) & board.side(them);
            if (rook_attacks(king_sq, after_occ) & their_rook_queen).is_nonempty() {
                continue;
            }

            list.push(Move::new_en_passant(src, ep_sq));
        }
    }
}
