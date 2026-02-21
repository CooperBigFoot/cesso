//! Chess768 feature index mapping for NNUE evaluation.

use cesso_core::{Color, PieceKind, Square};

/// Compute the Chess768 feature index for a piece from a given perspective.
///
/// Layout (must match Bullet trainer):
/// - Own pieces:     `kind.index() * 64 + sq_index`  (offsets 0..383)
/// - Opponent pieces: `384 + kind.index() * 64 + sq_index`  (offsets 384..767)
///
/// For White perspective, `sq_index = sq.index()`.
/// For Black perspective, `sq_index = sq.index() ^ 56` (vertical flip).
#[inline]
pub fn feature_index(perspective: Color, piece_color: Color, kind: PieceKind, sq: Square) -> usize {
    let sq_index = match perspective {
        Color::White => sq.index(),
        Color::Black => sq.index() ^ 56,
    };

    let color_offset = if piece_color == perspective { 0 } else { 384 };

    color_offset + kind.index() * 64 + sq_index
}
