//! Move execution via copy-make.

use crate::attacks::{bishop_attacks, king_attacks, knight_attacks, pawn_attacks, rook_attacks};
use crate::bitboard::Bitboard;
use crate::board::Board;
use crate::castle_rights::CastleRights;
use crate::chess_move::{Move, MoveKind};
use crate::color::Color;
use crate::piece::Piece;
use crate::piece_kind::PieceKind;
use crate::square::Square;
use crate::zobrist;

/// Maps each square index to the castling rights that must be removed when
/// that square is the source or destination of any move.
const CASTLE_RIGHTS_REVOKE: [CastleRights; 64] = {
    let mut table = [CastleRights::NONE; 64];
    // E1 (index 4): White king moves — remove both white rights.
    table[Square::E1.index()] = CastleRights::WHITE_BOTH;
    // A1 (index 0): White queenside rook.
    table[Square::A1.index()] = CastleRights::WHITE_QUEEN;
    // H1 (index 7): White kingside rook.
    table[Square::H1.index()] = CastleRights::WHITE_KING;
    // E8 (index 60): Black king moves — remove both black rights.
    table[Square::E8.index()] = CastleRights::BLACK_BOTH;
    // A8 (index 56): Black queenside rook.
    table[Square::A8.index()] = CastleRights::BLACK_QUEEN;
    // H8 (index 63): Black kingside rook.
    table[Square::H8.index()] = CastleRights::BLACK_KING;
    table
};

impl Board {
    /// Return `true` if `sq` is attacked by any piece of `by_color`.
    ///
    /// Uses reverse-attack lookup: attack patterns are cast from the target
    /// square and intersected with the attacker's pieces of each type.
    pub fn is_square_attacked(&self, sq: Square, by_color: Color) -> bool {
        self.is_square_attacked_with_occ(sq, by_color, self.occupied())
    }

    /// Return `true` if `sq` is attacked by `by_color`, using a custom `occupied` bitboard.
    ///
    /// Useful for king-move legality checks where the king is temporarily
    /// removed from the occupied set.
    pub(crate) fn is_square_attacked_with_occ(
        &self,
        sq: Square,
        by_color: Color,
        occupied: Bitboard,
    ) -> bool {
        let them = self.side(by_color);

        // Knight attacks: non-sliding, occupancy-independent.
        if (knight_attacks(sq) & them & self.pieces(PieceKind::Knight)).is_nonempty() {
            return true;
        }

        // King attacks: non-sliding, occupancy-independent.
        if (king_attacks(sq) & them & self.pieces(PieceKind::King)).is_nonempty() {
            return true;
        }

        // Pawn attacks: a white pawn on X attacks Y iff pawn_attacks(Black, Y) contains X.
        // So to find pawns of `by_color` that attack `sq`, we cast pawn_attacks from `sq`
        // using the *opposite* color.
        let opp_color = by_color.flip();
        if (pawn_attacks(opp_color, sq) & them & self.pieces(PieceKind::Pawn)).is_nonempty() {
            return true;
        }

        // Rook / Queen (orthogonal sliders).
        let rook_queen = (self.pieces(PieceKind::Rook) | self.pieces(PieceKind::Queen)) & them;
        if (rook_attacks(sq, occupied) & rook_queen).is_nonempty() {
            return true;
        }

        // Bishop / Queen (diagonal sliders).
        let bishop_queen = (self.pieces(PieceKind::Bishop) | self.pieces(PieceKind::Queen)) & them;
        if (bishop_attacks(sq, occupied) & bishop_queen).is_nonempty() {
            return true;
        }

        false
    }

    /// Apply a move and return the resulting board. Copy-make: `self` is not modified.
    ///
    /// # Errors
    ///
    /// If the source square is empty (invalid move), the board is returned unchanged.
    pub fn make_move(&self, mv: Move) -> Board {
        let mut b = *self;
        let us = b.side_to_move();
        let them = us.flip();
        let src = mv.source();
        let dst = mv.dest();

        // The piece on the source square must exist for a valid move.
        let moving_piece = match b.piece_on(src) {
            Some(kind) => kind,
            None => return b,
        };

        // XOR out old en passant file from hash (before clearing).
        if let Some(old_ep) = b.en_passant() {
            b.set_hash(b.hash() ^ zobrist::EN_PASSANT_FILE[old_ep.file().index()]);
        }

        // XOR out old castling rights from hash (before any modifications).
        b.set_hash(b.hash() ^ zobrist::CASTLING[b.castling().bits() as usize]);

        // Clear en passant target set by the previous move.
        b.set_en_passant(None);

        // Detect captures before we move any pieces. Castling moves the king
        // to the rook's square in some encodings, so exclude castling here.
        let is_capture = b.occupied().contains(dst) && !mv.is_castle();

        match mv.kind() {
            MoveKind::Normal => {
                // Remove the captured piece (if any) before placing ours.
                if is_capture && let Some(captured_kind) = b.piece_on(dst) {
                    b.toggle_piece(dst, captured_kind, them);
                    b.set_hash(b.hash() ^ zobrist::PIECE_SQUARE[Piece::new(captured_kind, them).index()][dst.index()]);
                }

                // Move our piece: XOR it off src and onto dst.
                b.toggle_piece(src, moving_piece, us);
                b.toggle_piece(dst, moving_piece, us);
                let piece_idx = Piece::new(moving_piece, us).index();
                b.set_hash(b.hash() ^ zobrist::PIECE_SQUARE[piece_idx][src.index()]);
                b.set_hash(b.hash() ^ zobrist::PIECE_SQUARE[piece_idx][dst.index()]);

                // Record en passant target square after a double pawn push.
                if moving_piece == PieceKind::Pawn {
                    let rank_diff = dst.index().abs_diff(src.index());
                    if rank_diff == 16 {
                        let ep_idx = if us == Color::White {
                            src.index() + 8
                        } else {
                            src.index() - 8
                        };
                        b.set_en_passant(Square::from_index(ep_idx as u8));
                    }
                }
            }

            MoveKind::Promotion => {
                // Remove the captured piece at the promotion square (if any).
                if is_capture && let Some(captured_kind) = b.piece_on(dst) {
                    b.toggle_piece(dst, captured_kind, them);
                    b.set_hash(b.hash() ^ zobrist::PIECE_SQUARE[Piece::new(captured_kind, them).index()][dst.index()]);
                }

                // Remove the promoting pawn from src.
                b.toggle_piece(src, PieceKind::Pawn, us);
                b.set_hash(b.hash() ^ zobrist::PIECE_SQUARE[Piece::new(PieceKind::Pawn, us).index()][src.index()]);

                // Place the promoted piece on dst.
                let promo_kind = mv.promotion_piece().to_piece_kind();
                b.toggle_piece(dst, promo_kind, us);
                b.set_hash(b.hash() ^ zobrist::PIECE_SQUARE[Piece::new(promo_kind, us).index()][dst.index()]);
            }

            MoveKind::EnPassant => {
                // Move our pawn to the en passant target square.
                b.toggle_piece(src, PieceKind::Pawn, us);
                b.toggle_piece(dst, PieceKind::Pawn, us);
                let pawn_idx = Piece::new(PieceKind::Pawn, us).index();
                b.set_hash(b.hash() ^ zobrist::PIECE_SQUARE[pawn_idx][src.index()]);
                b.set_hash(b.hash() ^ zobrist::PIECE_SQUARE[pawn_idx][dst.index()]);

                // Remove the captured pawn, which stands on the same rank as
                // `src` and the same file as `dst` — one rank behind `dst`.
                let captured_idx = if us == Color::White {
                    dst.index() - 8 // captured pawn is south of the EP square
                } else {
                    dst.index() + 8 // captured pawn is north of the EP square
                };
                if let Some(captured_sq) = Square::from_index(captured_idx as u8) {
                    b.toggle_piece(captured_sq, PieceKind::Pawn, them);
                    b.set_hash(b.hash() ^ zobrist::PIECE_SQUARE[Piece::new(PieceKind::Pawn, them).index()][captured_sq.index()]);
                }
            }

            MoveKind::Castling => {
                // Move the king.
                b.toggle_piece(src, PieceKind::King, us);
                b.toggle_piece(dst, PieceKind::King, us);
                let king_idx = Piece::new(PieceKind::King, us).index();
                b.set_hash(b.hash() ^ zobrist::PIECE_SQUARE[king_idx][src.index()]);
                b.set_hash(b.hash() ^ zobrist::PIECE_SQUARE[king_idx][dst.index()]);

                // Move the rook to its post-castling square.
                let (rook_src, rook_dst) = match dst.index() {
                    6 => (Square::H1, Square::F1),   // White kingside:  G1
                    2 => (Square::A1, Square::D1),   // White queenside: C1
                    62 => (Square::H8, Square::F8),  // Black kingside:  G8
                    58 => (Square::A8, Square::D8),  // Black queenside: C8
                    _ => return b,                   // should never occur for a valid move
                };
                b.toggle_piece(rook_src, PieceKind::Rook, us);
                b.toggle_piece(rook_dst, PieceKind::Rook, us);
                let rook_idx = Piece::new(PieceKind::Rook, us).index();
                b.set_hash(b.hash() ^ zobrist::PIECE_SQUARE[rook_idx][rook_src.index()]);
                b.set_hash(b.hash() ^ zobrist::PIECE_SQUARE[rook_idx][rook_dst.index()]);
            }
        }

        // Revoke castling rights affected by any piece touching a corner or king square.
        let new_castling = b
            .castling()
            .remove(CASTLE_RIGHTS_REVOKE[src.index()])
            .remove(CASTLE_RIGHTS_REVOKE[dst.index()]);
        b.set_castling(new_castling);

        // XOR in new castling rights.
        b.set_hash(b.hash() ^ zobrist::CASTLING[new_castling.bits() as usize]);

        // XOR in new en passant file (if set by a double pawn push).
        if let Some(ep_sq) = b.en_passant() {
            b.set_hash(b.hash() ^ zobrist::EN_PASSANT_FILE[ep_sq.file().index()]);
        }

        // Update the halfmove clock (reset on pawn moves and captures).
        if moving_piece == PieceKind::Pawn || is_capture || mv.kind() == MoveKind::EnPassant {
            b.set_halfmove_clock(0);
        } else {
            b.set_halfmove_clock(b.halfmove_clock() + 1);
        }

        // Switch the side to move.
        b.set_side_to_move(them);

        // XOR side-to-move key (always changes).
        b.set_hash(b.hash() ^ zobrist::SIDE_TO_MOVE);

        // Increment the fullmove counter after Black's move.
        if us == Color::Black {
            b.set_fullmove_number(b.fullmove_number() + 1);
        }

        b
    }
}

#[cfg(test)]
mod tests {
    use crate::board::Board;
    use crate::castle_rights::CastleRights;
    use crate::chess_move::{Move, PromotionPiece};
    use crate::color::Color;
    use crate::piece_kind::PieceKind;
    use crate::square::Square;

    fn starting() -> Board {
        Board::starting_position()
    }

    #[test]
    fn normal_pawn_push_e2e4() {
        let board = starting();
        let mv = Move::new(Square::E2, Square::E4);
        let after = board.make_move(mv);

        assert_eq!(after.piece_on(Square::E4), Some(PieceKind::Pawn));
        assert_eq!(after.color_on(Square::E4), Some(Color::White));
        assert_eq!(after.piece_on(Square::E2), None);
        assert_eq!(after.en_passant(), Some(Square::E3));
        assert_eq!(after.side_to_move(), Color::Black);
    }

    #[test]
    fn capture_resets_clock() {
        // 1.e4 d5 2.exd5
        let b0 = starting();
        let b1 = b0.make_move(Move::new(Square::E2, Square::E4));
        let b2 = b1.make_move(Move::new(Square::D7, Square::D5));
        let b3 = b2.make_move(Move::new(Square::E4, Square::D5));

        assert_eq!(b3.piece_on(Square::D5), Some(PieceKind::Pawn));
        assert_eq!(b3.color_on(Square::D5), Some(Color::White));
        assert_eq!(b3.piece_on(Square::E4), None);
        assert_eq!(b3.halfmove_clock(), 0);
    }

    #[test]
    fn en_passant_capture() {
        // 1.e4 a6 2.e5 d5 3.exd6 e.p.
        let b = starting()
            .make_move(Move::new(Square::E2, Square::E4)) // 1.e4
            .make_move(Move::new(Square::A7, Square::A6)) // 1...a6
            .make_move(Move::new(Square::E4, Square::E5)) // 2.e5
            .make_move(Move::new(Square::D7, Square::D5)); // 2...d5

        assert_eq!(b.en_passant(), Some(Square::D6));

        let after = b.make_move(Move::new_en_passant(Square::E5, Square::D6));
        assert_eq!(after.piece_on(Square::D6), Some(PieceKind::Pawn));
        assert_eq!(after.color_on(Square::D6), Some(Color::White));
        assert_eq!(after.piece_on(Square::D5), None); // captured pawn removed
        assert_eq!(after.piece_on(Square::E5), None); // moved from here
    }

    #[test]
    fn promotion() {
        let board: Board = "4k3/4P3/8/8/8/8/8/4K3 w - - 0 1".parse().unwrap();
        let mv = Move::new_promotion(Square::E7, Square::E8, PromotionPiece::Queen);
        let after = board.make_move(mv);

        assert_eq!(after.piece_on(Square::E8), Some(PieceKind::Queen));
        assert_eq!(after.color_on(Square::E8), Some(Color::White));
        assert_eq!(after.piece_on(Square::E7), None);
    }

    #[test]
    fn capture_promotion() {
        // White pawn on e7, black rook on d8.
        let board: Board = "3rk3/4P3/8/8/8/8/8/4K3 w - - 0 1".parse().unwrap();
        let mv = Move::new_promotion(Square::E7, Square::D8, PromotionPiece::Queen);
        let after = board.make_move(mv);

        assert_eq!(after.piece_on(Square::D8), Some(PieceKind::Queen));
        assert_eq!(after.color_on(Square::D8), Some(Color::White));
        assert_eq!(after.piece_on(Square::E7), None);
    }

    #[test]
    fn kingside_castling_white() {
        let board: Board = "r3k2r/pppppppp/8/8/8/8/PPPPPPPP/R3K2R w KQkq - 0 1"
            .parse()
            .unwrap();
        let mv = Move::new_castle(Square::E1, Square::G1);
        let after = board.make_move(mv);

        assert_eq!(after.piece_on(Square::G1), Some(PieceKind::King));
        assert_eq!(after.color_on(Square::G1), Some(Color::White));
        assert_eq!(after.piece_on(Square::F1), Some(PieceKind::Rook));
        assert_eq!(after.color_on(Square::F1), Some(Color::White));
        assert_eq!(after.piece_on(Square::E1), None);
        assert_eq!(after.piece_on(Square::H1), None);
        // White rights removed, black rights preserved.
        assert!(!after.castling().contains(CastleRights::WHITE_KING));
        assert!(!after.castling().contains(CastleRights::WHITE_QUEEN));
        assert!(after.castling().contains(CastleRights::BLACK_KING));
        assert!(after.castling().contains(CastleRights::BLACK_QUEEN));
    }

    #[test]
    fn queenside_castling_white() {
        let board: Board = "r3k2r/pppppppp/8/8/8/8/PPPPPPPP/R3K2R w KQkq - 0 1"
            .parse()
            .unwrap();
        let mv = Move::new_castle(Square::E1, Square::C1);
        let after = board.make_move(mv);

        assert_eq!(after.piece_on(Square::C1), Some(PieceKind::King));
        assert_eq!(after.piece_on(Square::D1), Some(PieceKind::Rook));
        assert_eq!(after.piece_on(Square::E1), None);
        assert_eq!(after.piece_on(Square::A1), None);
    }

    #[test]
    fn rook_move_revokes_castling() {
        let board: Board = "r3k2r/pppppppp/8/8/8/8/PPPPPPPP/R3K2R w KQkq - 0 1"
            .parse()
            .unwrap();
        let mv = Move::new(Square::H1, Square::G1);
        let after = board.make_move(mv);

        assert!(!after.castling().contains(CastleRights::WHITE_KING));
        assert!(after.castling().contains(CastleRights::WHITE_QUEEN));
    }

    #[test]
    fn halfmove_clock_increments_on_quiet() {
        // Nf3 is a quiet non-pawn move.
        let board = starting();
        let mv = Move::new(Square::G1, Square::F3);
        let after = board.make_move(mv);
        assert_eq!(after.halfmove_clock(), 1);
    }

    #[test]
    fn fullmove_increments_after_black() {
        let b0 = starting();
        assert_eq!(b0.fullmove_number(), 1);
        let b1 = b0.make_move(Move::new(Square::E2, Square::E4));
        assert_eq!(b1.fullmove_number(), 1); // unchanged after White
        let b2 = b1.make_move(Move::new(Square::E7, Square::E5));
        assert_eq!(b2.fullmove_number(), 2); // incremented after Black
    }

    #[test]
    fn is_square_attacked_starting() {
        let board = starting();
        // e2 is defended by White pieces (king, queen, bishop).
        assert!(board.is_square_attacked(Square::E2, Color::White));
        // e4 is not attacked by anyone in the starting position.
        assert!(!board.is_square_attacked(Square::E4, Color::White));
        assert!(!board.is_square_attacked(Square::E4, Color::Black));
    }

    #[test]
    fn is_square_attacked_knight() {
        let board = starting();
        // f3 attacked by the white knight on g1.
        assert!(board.is_square_attacked(Square::F3, Color::White));
        // f6 attacked by the black knight on g8.
        assert!(board.is_square_attacked(Square::F6, Color::Black));
    }

    // --- Incremental Zobrist hash tests ---

    #[test]
    fn incremental_hash_normal_move() {
        let board = starting();
        let after = board.make_move(Move::new(Square::E2, Square::E4));
        assert_eq!(after.hash(), crate::zobrist::hash_from_scratch(&after));
    }

    #[test]
    fn incremental_hash_capture() {
        // 1.e4 d5 2.exd5
        let b = starting()
            .make_move(Move::new(Square::E2, Square::E4))
            .make_move(Move::new(Square::D7, Square::D5));
        let after = b.make_move(Move::new(Square::E4, Square::D5));
        assert_eq!(after.hash(), crate::zobrist::hash_from_scratch(&after));
    }

    #[test]
    fn incremental_hash_double_pawn_push() {
        let board = starting();
        let after = board.make_move(Move::new(Square::E2, Square::E4));
        assert_eq!(after.hash(), crate::zobrist::hash_from_scratch(&after));
        assert!(after.en_passant().is_some());
    }

    #[test]
    fn incremental_hash_en_passant() {
        let b = starting()
            .make_move(Move::new(Square::E2, Square::E4))
            .make_move(Move::new(Square::A7, Square::A6))
            .make_move(Move::new(Square::E4, Square::E5))
            .make_move(Move::new(Square::D7, Square::D5));
        // Verify each intermediate board
        assert_eq!(b.hash(), crate::zobrist::hash_from_scratch(&b));
        let after = b.make_move(Move::new_en_passant(Square::E5, Square::D6));
        assert_eq!(after.hash(), crate::zobrist::hash_from_scratch(&after));
    }

    #[test]
    fn incremental_hash_kingside_castling() {
        let board: Board = "r3k2r/pppppppp/8/8/8/8/PPPPPPPP/R3K2R w KQkq - 0 1"
            .parse()
            .unwrap();
        let after = board.make_move(Move::new_castle(Square::E1, Square::G1));
        assert_eq!(after.hash(), crate::zobrist::hash_from_scratch(&after));
    }

    #[test]
    fn incremental_hash_queenside_castling() {
        let board: Board = "r3k2r/pppppppp/8/8/8/8/PPPPPPPP/R3K2R w KQkq - 0 1"
            .parse()
            .unwrap();
        let after = board.make_move(Move::new_castle(Square::E1, Square::C1));
        assert_eq!(after.hash(), crate::zobrist::hash_from_scratch(&after));
    }

    #[test]
    fn incremental_hash_black_castling() {
        let board: Board = "r3k2r/pppppppp/8/8/8/8/PPPPPPPP/R3K2R b KQkq - 0 1"
            .parse()
            .unwrap();
        let after = board.make_move(Move::new_castle(Square::E8, Square::G8));
        assert_eq!(after.hash(), crate::zobrist::hash_from_scratch(&after));
        let after2 = board.make_move(Move::new_castle(Square::E8, Square::C8));
        assert_eq!(after2.hash(), crate::zobrist::hash_from_scratch(&after2));
    }

    #[test]
    fn incremental_hash_promotion() {
        let board: Board = "4k3/4P3/8/8/8/8/8/4K3 w - - 0 1".parse().unwrap();
        for promo in crate::chess_move::PromotionPiece::ALL {
            let after = board.make_move(Move::new_promotion(Square::E7, Square::E8, promo));
            assert_eq!(
                after.hash(),
                crate::zobrist::hash_from_scratch(&after),
                "hash mismatch for promotion to {:?}",
                promo
            );
        }
    }

    #[test]
    fn incremental_hash_capture_promotion() {
        let board: Board = "3rk3/4P3/8/8/8/8/8/4K3 w - - 0 1".parse().unwrap();
        let after = board.make_move(Move::new_promotion(
            Square::E7,
            Square::D8,
            PromotionPiece::Queen,
        ));
        assert_eq!(after.hash(), crate::zobrist::hash_from_scratch(&after));
    }

    #[test]
    fn transposition_same_hash() {
        // 1.Nf3 Nf6 2.Nc3 Nc6 vs 1.Nc3 Nc6 2.Nf3 Nf6 — same position!
        let path_a = starting()
            .make_move(Move::new(Square::G1, Square::F3)) // 1.Nf3
            .make_move(Move::new(Square::G8, Square::F6)) // 1...Nf6
            .make_move(Move::new(Square::B1, Square::C3)) // 2.Nc3
            .make_move(Move::new(Square::B8, Square::C6)); // 2...Nc6

        let path_b = starting()
            .make_move(Move::new(Square::B1, Square::C3)) // 1.Nc3
            .make_move(Move::new(Square::B8, Square::C6)) // 1...Nc6
            .make_move(Move::new(Square::G1, Square::F3)) // 2.Nf3
            .make_move(Move::new(Square::G8, Square::F6)); // 2...Nf6

        assert_eq!(path_a.hash(), path_b.hash(), "transposed positions should have equal hashes");
    }

    #[test]
    fn incremental_hash_many_moves_sequence() {
        // Play a longer sequence and verify hash after each move
        let moves = [
            Move::new(Square::E2, Square::E4),
            Move::new(Square::E7, Square::E5),
            Move::new(Square::G1, Square::F3),
            Move::new(Square::B8, Square::C6),
            Move::new(Square::F1, Square::B5),
            Move::new(Square::A7, Square::A6),
        ];

        let mut board = starting();
        for mv in &moves {
            board = board.make_move(*mv);
            assert_eq!(
                board.hash(),
                crate::zobrist::hash_from_scratch(&board),
                "hash mismatch after move {}",
                mv
            );
        }
    }
}
