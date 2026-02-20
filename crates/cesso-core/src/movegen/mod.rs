//! Legal move generation.

mod check;
mod king;
mod knights;
mod pawns;
mod pins;
mod sliders;

use crate::attacks::{between, bishop_attacks, king_attacks, knight_attacks, pawn_attacks, rook_attacks};
use crate::bitboard::Bitboard;
use crate::board::Board;
use crate::chess_move::Move;
use crate::color::Color;
use crate::piece_kind::PieceKind;
use crate::square::Square;

use self::check::{InCheck, NotInCheck};
use self::king::gen_king;
use self::knights::gen_knights;
use self::pawns::gen_pawns;
use self::pins::compute_checkers_and_pinned;
use self::sliders::gen_sliders;

/// Stack-allocated buffer for generated moves. Capacity 256 covers the theoretical max of 218.
pub struct MoveList {
    moves: [Move; 256],
    len: u16,
}

impl MoveList {
    /// Create an empty move list.
    pub fn new() -> MoveList {
        MoveList {
            moves: [Move::NULL; 256],
            len: 0,
        }
    }

    /// Push a move onto the list.
    #[inline]
    pub fn push(&mut self, mv: Move) {
        debug_assert!((self.len as usize) < 256);
        self.moves[self.len as usize] = mv;
        self.len += 1;
    }

    /// Return the number of moves in the list.
    #[inline]
    pub fn len(&self) -> usize {
        self.len as usize
    }

    /// Return `true` if the list is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Return a slice of the moves.
    #[inline]
    pub fn as_slice(&self) -> &[Move] {
        &self.moves[..self.len as usize]
    }
}

impl Default for MoveList {
    fn default() -> Self {
        Self::new()
    }
}

impl std::ops::Index<usize> for MoveList {
    type Output = Move;
    #[inline]
    fn index(&self, index: usize) -> &Move {
        &self.moves[index]
    }
}

impl<'a> IntoIterator for &'a MoveList {
    type Item = &'a Move;
    type IntoIter = std::slice::Iter<'a, Move>;
    fn into_iter(self) -> Self::IntoIter {
        self.as_slice().iter()
    }
}

/// Check if `sq` is attacked by `by_color`, using `occupied` for sliding piece rays.
fn is_attacked(board: &Board, sq: Square, by_color: Color, occupied: Bitboard) -> bool {
    let them = board.side(by_color);
    if (knight_attacks(sq) & them & board.pieces(PieceKind::Knight)).is_nonempty() {
        return true;
    }
    if (king_attacks(sq) & them & board.pieces(PieceKind::King)).is_nonempty() {
        return true;
    }
    if (pawn_attacks(by_color.flip(), sq) & them & board.pieces(PieceKind::Pawn)).is_nonempty() {
        return true;
    }
    if (rook_attacks(sq, occupied) & them & (board.pieces(PieceKind::Rook) | board.pieces(PieceKind::Queen)))
        .is_nonempty()
    {
        return true;
    }
    if (bishop_attacks(sq, occupied) & them & (board.pieces(PieceKind::Bishop) | board.pieces(PieceKind::Queen)))
        .is_nonempty()
    {
        return true;
    }
    false
}

/// Generate all legal moves for the current position.
pub fn generate_legal_moves(board: &Board) -> MoveList {
    let mut list = MoveList::new();
    let us = board.side_to_move();
    let king_sq = board.king_square(us);
    let (checkers, pinned) = compute_checkers_and_pinned(board);

    match checkers.count() {
        0 => {
            // Not in check: all piece moves are candidate-legal; check_mask = full board
            let check_mask = Bitboard::FULL;
            gen_pawns::<NotInCheck>(board, king_sq, pinned, check_mask, &mut list);
            gen_knights::<NotInCheck>(board, king_sq, pinned, check_mask, &mut list);
            gen_sliders::<NotInCheck>(board, king_sq, pinned, check_mask, &mut list);
            gen_king(board, king_sq, &mut list);
        }
        1 => {
            // Single check: non-king pieces must either capture the checker or
            // block the check ray.
            // SAFETY: count() == 1 means exactly one bit is set, so lsb() is Some.
            let checker_sq = checkers.lsb().expect("checkers has exactly 1 bit set");
            // check_mask = squares between king and checker (blocking) + checker itself
            let check_mask = between(king_sq, checker_sq) | checkers;
            gen_pawns::<InCheck>(board, king_sq, pinned, check_mask, &mut list);
            gen_knights::<InCheck>(board, king_sq, pinned, check_mask, &mut list);
            gen_sliders::<InCheck>(board, king_sq, pinned, check_mask, &mut list);
            gen_king(board, king_sq, &mut list);
        }
        _ => {
            // Double (or more) check: only king moves can resolve it
            gen_king(board, king_sq, &mut list);
        }
    }

    list
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::Board;
    use crate::piece_kind::PieceKind;
    use crate::square::Square;

    #[test]
    fn starting_position_20_moves() {
        let board = Board::starting_position();
        let moves = generate_legal_moves(&board);
        assert_eq!(
            moves.len(),
            20,
            "starting position should have 20 legal moves, got {}",
            moves.len()
        );
    }

    #[test]
    fn pinned_knight_zero_moves() {
        // King on e1, knight on e2, rook on e8 — knight is pinned along the e-file
        let board: Board = "4r2k/8/8/8/8/8/4N3/4K3 w - - 0 1".parse().unwrap();
        let moves = generate_legal_moves(&board);
        let knight_moves: Vec<_> = moves
            .as_slice()
            .iter()
            .filter(|m| m.source() == Square::E2)
            .collect();
        assert_eq!(knight_moves.len(), 0, "pinned knight should have 0 moves");
    }

    #[test]
    fn double_check_king_only() {
        // King e1, black knight f3 + black rook e8 — double check
        let board: Board = "4r1k1/8/8/8/8/5n2/8/4K3 w - - 0 1".parse().unwrap();
        let moves = generate_legal_moves(&board);
        // All moves should be king moves
        for mv in moves.as_slice() {
            assert_eq!(
                board.piece_on(mv.source()),
                Some(PieceKind::King),
                "in double check, only king moves should be legal, but got move from {:?}",
                mv.source()
            );
        }
    }

    #[test]
    fn castling_not_through_check() {
        // Bishop on a6 attacks f1 (a6→b5→c4→d3→e2→f1), preventing kingside castling
        let board: Board = "4k3/8/b7/8/8/8/8/R3K2R w KQ - 0 1".parse().unwrap();
        let moves = generate_legal_moves(&board);
        let castle_moves: Vec<_> = moves.as_slice().iter().filter(|m| m.is_castle()).collect();
        for mv in &castle_moves {
            assert_ne!(
                mv.dest(),
                Square::G1,
                "should not castle kingside through attacked f1"
            );
        }
    }

    #[test]
    fn en_passant_legal() {
        // White pawn e5, black pawn d5 just moved, EP square d6
        let board: Board = "4k3/8/8/3pP3/8/8/8/4K3 w - d6 0 1".parse().unwrap();
        let moves = generate_legal_moves(&board);
        let ep_moves: Vec<_> = moves.as_slice().iter().filter(|m| m.is_en_passant()).collect();
        assert_eq!(ep_moves.len(), 1, "should have 1 en passant move");
    }

    #[test]
    fn en_passant_discovered_check_illegal() {
        // White king a5, white pawn b5, black pawn c5 (just double-pushed),
        // black rook h5. EP capture bxc6 would expose king to rook on h5.
        let board: Board = "4k3/8/8/KPp4r/8/8/8/8 w - c6 0 1".parse().unwrap();
        let moves = generate_legal_moves(&board);
        let ep_moves: Vec<_> = moves.as_slice().iter().filter(|m| m.is_en_passant()).collect();
        assert_eq!(ep_moves.len(), 0, "EP should be illegal due to discovered check");
    }

    #[test]
    fn promotion_generates_4_moves() {
        // White pawn on a7 about to promote
        let board: Board = "4k3/P7/8/8/8/8/8/4K3 w - - 0 1".parse().unwrap();
        let moves = generate_legal_moves(&board);
        let promo_moves: Vec<_> = moves.as_slice().iter().filter(|m| m.is_promotion()).collect();
        assert_eq!(promo_moves.len(), 4, "promotion should generate 4 moves (Q/R/B/N)");
    }
}
