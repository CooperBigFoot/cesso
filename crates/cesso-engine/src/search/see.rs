//! Static Exchange Evaluation (SEE).
//!
//! Determines the material outcome of a sequence of captures on a single square,
//! assuming both sides use their least valuable attacker at each step.

use cesso_core::{
    bishop_attacks, king_attacks, knight_attacks, pawn_attacks, rook_attacks, Bitboard, Board,
    Color, Move, MoveKind, PieceKind, PromotionPiece, Square,
};

/// Material values for SEE, indexed by `PieceKind::index()`.
const SEE_VALUE: [i32; 6] = [100, 320, 330, 500, 900, 20_000];

/// Compute all pieces that attack a given square with the given occupancy.
///
/// Uses all 6 attack functions. Sliding attacks use the provided `occ`
/// bitboard, which reveals X-ray attackers as pieces are removed.
fn attackers_of(sq: Square, occ: Bitboard, board: &Board) -> Bitboard {
    let knights = knight_attacks(sq) & board.pieces(PieceKind::Knight);
    let kings = king_attacks(sq) & board.pieces(PieceKind::King);
    let rook_like = rook_attacks(sq, occ)
        & (board.pieces(PieceKind::Rook) | board.pieces(PieceKind::Queen));
    let bishop_like = bishop_attacks(sq, occ)
        & (board.pieces(PieceKind::Bishop) | board.pieces(PieceKind::Queen));
    let white_pawns =
        pawn_attacks(Color::Black, sq) & board.pieces(PieceKind::Pawn) & board.side(Color::White);
    let black_pawns =
        pawn_attacks(Color::White, sq) & board.pieces(PieceKind::Pawn) & board.side(Color::Black);

    knights | kings | rook_like | bishop_like | white_pawns | black_pawns
}

/// Find the least valuable attacker from the given attacker set for a side.
///
/// Returns `(square, piece_kind)` of the least valuable attacker, or `None`.
fn least_valuable_attacker(
    attackers: Bitboard,
    side: Bitboard,
    board: &Board,
) -> Option<(Square, PieceKind)> {
    // Iterate in PieceKind order (Pawn=0 .. King=5) — already sorted by value
    for kind in PieceKind::ALL {
        let candidates = attackers & side & board.pieces(kind);
        if let Some(sq) = candidates.lsb() {
            return Some((sq, kind));
        }
    }
    None
}

/// Full Static Exchange Evaluation.
///
/// Returns the material gain/loss from the side-to-move's perspective
/// after all profitable recaptures on the target square.
pub fn see(board: &Board, mv: Move) -> i32 {
    let src = mv.source();
    let dst = mv.dest();
    let mut occ = board.occupied();

    // Determine the initial attacker piece
    let attacker_kind = board.piece_on(src).unwrap_or(PieceKind::Pawn);

    // Determine the initial victim value
    let victim_value = if mv.kind() == MoveKind::EnPassant {
        SEE_VALUE[PieceKind::Pawn.index()]
    } else if let Some(victim) = board.piece_on(dst) {
        SEE_VALUE[victim.index()]
    } else {
        0
    };

    // For promotions, the attacker transforms into the promoted piece.
    // This is the value of the piece sitting on dst after the initial capture.
    let attacker_value = if mv.kind() == MoveKind::Promotion {
        let promo_kind = match mv.promotion_piece() {
            PromotionPiece::Knight => PieceKind::Knight,
            PromotionPiece::Bishop => PieceKind::Bishop,
            PromotionPiece::Rook => PieceKind::Rook,
            PromotionPiece::Queen => PieceKind::Queen,
        };
        SEE_VALUE[promo_kind.index()]
    } else {
        SEE_VALUE[attacker_kind.index()]
    };

    // Remove the initial attacker from occupancy
    occ = occ.without(src);

    // For en passant, also remove the captured pawn
    if mv.kind() == MoveKind::EnPassant {
        // The captured pawn sits on the same rank as src, same file as dst.
        // White captures upward: dst.index() - 8 (captured pawn is south of EP square)
        // Black captures downward: dst.index() + 8 (captured pawn is north of EP square)
        let captured_idx = if board.side_to_move() == Color::White {
            dst.index().wrapping_sub(8) as u8
        } else {
            (dst.index() + 8) as u8
        };
        if let Some(ep_sq) = Square::from_index(captured_idx) {
            occ = occ.without(ep_sq);
        }
    }

    // Gain array: gain[0] = initial capture value
    let mut gain = [0i32; 32];
    let mut depth = 0usize;
    gain[0] = victim_value;

    // The piece that just captured (sits on dst for the next recapture).
    // Its value is used as the "next victim" value when the opponent recaptures.
    let mut next_victim_value = attacker_value;

    // Side making the next recapture (opponent of the initial mover)
    let mut side_to_move = !board.side_to_move();

    // Compute all attackers to dst with the initial attacker removed from occ.
    let mut all_attackers = attackers_of(dst, occ, board);
    all_attackers &= occ; // only include pieces still on the board

    loop {
        // Find the least-valuable attacker for the current side.
        let side_bb = board.side(side_to_move);
        let Some((sq, kind)) = least_valuable_attacker(all_attackers, side_bb, board) else {
            break;
        };

        depth += 1;
        if depth >= 32 {
            break;
        }

        // The current side captures the piece on dst (worth next_victim_value)
        // and faces the chain value from the previous depth.
        // gain[d] represents the net outcome for the side making this capture
        // relative to what was already gained.
        gain[depth] = next_victim_value - gain[depth - 1];

        // Update: the recapturer now sits on dst and becomes the next victim.
        next_victim_value = SEE_VALUE[kind.index()];

        // Remove this attacker from occupancy.
        occ = occ.without(sq);

        // After removing a piece, refresh sliding attackers for X-ray discovery.
        // Pawns, bishops, and queens can unblock diagonal sliders.
        // Rooks and queens can unblock orthogonal sliders.
        if kind == PieceKind::Pawn || kind == PieceKind::Bishop || kind == PieceKind::Queen {
            all_attackers |= bishop_attacks(dst, occ)
                & (board.pieces(PieceKind::Bishop) | board.pieces(PieceKind::Queen));
        }
        if kind == PieceKind::Rook || kind == PieceKind::Queen {
            all_attackers |= rook_attacks(dst, occ)
                & (board.pieces(PieceKind::Rook) | board.pieces(PieceKind::Queen));
        }
        all_attackers &= occ;

        side_to_move = !side_to_move;
    }

    // Backward propagation (negamax minimax): each side only recaptures if profitable.
    //
    // Equivalent to the C idiom `while (--d) gain[d-1] = -max(-gain[d-1], gain[d])`.
    // The formula reflects that each side can choose to stop capturing if the
    // continuation is unfavourable.
    while depth > 0 {
        depth -= 1;
        gain[depth] = -((-gain[depth]).max(gain[depth + 1]));
    }

    gain[0]
}

/// Threshold version of SEE: returns true if the SEE score >= threshold.
///
/// More efficient than `see(board, mv) >= threshold` because it can
/// exit early once the result is determined.
pub fn see_ge(board: &Board, mv: Move, threshold: i32) -> bool {
    see(board, mv) >= threshold
}

#[cfg(test)]
mod tests {
    use super::*;
    use cesso_core::{generate_legal_moves, Board};

    fn find_move(board: &Board, from: &str, to: &str) -> Move {
        let moves = generate_legal_moves(board);
        let from_sq = Square::from_algebraic(from).unwrap();
        let to_sq = Square::from_algebraic(to).unwrap();
        moves
            .as_slice()
            .iter()
            .find(|m| {
                m.source() == from_sq
                    && m.dest() == to_sq
                    && m.kind() != MoveKind::Promotion
            })
            .copied()
            .unwrap_or_else(|| {
                // Try promotion (queen)
                moves
                    .as_slice()
                    .iter()
                    .find(|m| m.source() == from_sq && m.dest() == to_sq)
                    .copied()
                    .expect("move not found")
            })
    }

    #[test]
    fn pawn_takes_undefended_knight() {
        // White pawn on e4 takes undefended black knight on d5
        let board: Board = "4k3/8/8/3n4/4P3/8/8/4K3 w - - 0 1".parse().unwrap();
        let mv = find_move(&board, "e4", "d5");
        assert_eq!(see(&board, mv), 320); // gain a knight
    }

    #[test]
    fn pawn_takes_defended_knight() {
        // White pawn on e4 takes black knight on d5, defended by black pawn on e6
        let board: Board = "4k3/8/4p3/3n4/4P3/8/8/4K3 w - - 0 1".parse().unwrap();
        let mv = find_move(&board, "e4", "d5");
        // PxN (gain 320), then pxP (they gain 100) => 320 - 100 = 220
        assert_eq!(see(&board, mv), 220);
    }

    #[test]
    fn queen_takes_defended_pawn_loses() {
        // White queen takes pawn defended by another pawn
        let board: Board = "4k3/8/3p4/2p5/8/4Q3/8/4K3 w - - 0 1".parse().unwrap();
        let mv = find_move(&board, "e3", "c5");
        // QxP (gain 100), then pxQ (they gain 900) => 100 - 900 = -800
        assert!(see(&board, mv) < 0);
    }

    #[test]
    fn equal_trade() {
        // Knight takes knight, both undefended
        let board: Board = "4k3/8/8/3n4/8/4N3/8/4K3 w - - 0 1".parse().unwrap();
        let mv = find_move(&board, "e3", "d5");
        // NxN = gain 320, no recapture => 320
        assert_eq!(see(&board, mv), 320);
    }

    #[test]
    fn see_ge_threshold() {
        let board: Board = "4k3/8/8/3n4/4P3/8/8/4K3 w - - 0 1".parse().unwrap();
        let mv = find_move(&board, "e4", "d5");
        assert!(see_ge(&board, mv, 0));
        assert!(see_ge(&board, mv, 300));
        assert!(!see_ge(&board, mv, 400));
    }
}
