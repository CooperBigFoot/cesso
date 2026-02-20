//! Pin and check detection.

use crate::attacks::{between, bishop_attacks, knight_attacks, pawn_attacks, rook_attacks};
use crate::bitboard::Bitboard;
use crate::board::Board;
use crate::piece_kind::PieceKind;

/// Compute the set of checking pieces and the set of pinned friendly pieces.
///
/// Returns `(checkers, pinned)` where:
/// - `checkers`: bitboard of enemy pieces giving check to our king
/// - `pinned`: bitboard of our pieces that are pinned to our king
pub(crate) fn compute_checkers_and_pinned(board: &Board) -> (Bitboard, Bitboard) {
    let us = board.side_to_move();
    let them = us.flip();
    let king_sq = board.king_square(us);
    let our_pieces = board.side(us);
    let their_pieces = board.side(them);
    let occupied = board.occupied();

    let mut checkers = Bitboard::EMPTY;
    let mut pinned = Bitboard::EMPTY;

    // Knight checks
    checkers |= knight_attacks(king_sq) & board.pieces(PieceKind::Knight) & their_pieces;

    // Pawn checks
    checkers |= pawn_attacks(us, king_sq) & board.pieces(PieceKind::Pawn) & their_pieces;

    // Diagonal slider checks/pins (bishops and queens)
    let diag_sliders =
        (board.pieces(PieceKind::Bishop) | board.pieces(PieceKind::Queen)) & their_pieces;
    // Candidates: enemy diagonal sliders visible from the king on an empty board
    let mut diag_candidates = bishop_attacks(king_sq, Bitboard::EMPTY) & diag_sliders;
    while let Some((attacker_sq, rest)) = diag_candidates.pop_lsb() {
        diag_candidates = rest;
        let between_bb = between(king_sq, attacker_sq);
        let blockers = between_bb & occupied;
        match blockers.count() {
            0 => {
                // Direct check — no pieces between king and attacker
                checkers |= attacker_sq.bitboard();
            }
            1 => {
                // Exactly one blocker — if it's ours, it's pinned
                if let Some(blocker_sq) = blockers.lsb()
                    && our_pieces.contains(blocker_sq)
                {
                    pinned |= blocker_sq.bitboard();
                }
            }
            _ => {} // 2+ blockers: no check or pin
        }
    }

    // Orthogonal slider checks/pins (rooks and queens)
    let orth_sliders =
        (board.pieces(PieceKind::Rook) | board.pieces(PieceKind::Queen)) & their_pieces;
    // Candidates: enemy orthogonal sliders visible from the king on an empty board
    let mut orth_candidates = rook_attacks(king_sq, Bitboard::EMPTY) & orth_sliders;
    while let Some((attacker_sq, rest)) = orth_candidates.pop_lsb() {
        orth_candidates = rest;
        let between_bb = between(king_sq, attacker_sq);
        let blockers = between_bb & occupied;
        match blockers.count() {
            0 => {
                checkers |= attacker_sq.bitboard();
            }
            1 => {
                if let Some(blocker_sq) = blockers.lsb()
                    && our_pieces.contains(blocker_sq)
                {
                    pinned |= blocker_sq.bitboard();
                }
            }
            _ => {}
        }
    }

    (checkers, pinned)
}
