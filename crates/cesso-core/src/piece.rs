//! Colored chess piece, bit-packed into a single byte.

use std::fmt;

use crate::color::Color;
use crate::piece_kind::PieceKind;

/// A colored chess piece, bit-packed into a single byte.
///
/// Bit layout:
/// - bits 0-2: [`PieceKind`] (values 0-5)
/// - bit 3: [`Color`] (0 = White, 1 = Black)
///
/// Valid raw values are 0-5 (White pieces) and 8-13 (Black pieces).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Piece(u8);

impl Piece {
    /// All 12 valid pieces in order (White 0-5, Black 6-11 by index).
    pub const COUNT: usize = 12;

    /// White pawn. Raw value: 0.
    pub const WHITE_PAWN: Piece = Piece::new(PieceKind::Pawn, Color::White);
    /// White knight. Raw value: 1.
    pub const WHITE_KNIGHT: Piece = Piece::new(PieceKind::Knight, Color::White);
    /// White bishop. Raw value: 2.
    pub const WHITE_BISHOP: Piece = Piece::new(PieceKind::Bishop, Color::White);
    /// White rook. Raw value: 3.
    pub const WHITE_ROOK: Piece = Piece::new(PieceKind::Rook, Color::White);
    /// White queen. Raw value: 4.
    pub const WHITE_QUEEN: Piece = Piece::new(PieceKind::Queen, Color::White);
    /// White king. Raw value: 5.
    pub const WHITE_KING: Piece = Piece::new(PieceKind::King, Color::White);

    /// Black pawn. Raw value: 8.
    pub const BLACK_PAWN: Piece = Piece::new(PieceKind::Pawn, Color::Black);
    /// Black knight. Raw value: 9.
    pub const BLACK_KNIGHT: Piece = Piece::new(PieceKind::Knight, Color::Black);
    /// Black bishop. Raw value: 10.
    pub const BLACK_BISHOP: Piece = Piece::new(PieceKind::Bishop, Color::Black);
    /// Black rook. Raw value: 11.
    pub const BLACK_ROOK: Piece = Piece::new(PieceKind::Rook, Color::Black);
    /// Black queen. Raw value: 12.
    pub const BLACK_QUEEN: Piece = Piece::new(PieceKind::Queen, Color::Black);
    /// Black king. Raw value: 13.
    pub const BLACK_KING: Piece = Piece::new(PieceKind::King, Color::Black);

    /// All 12 pieces: White pieces (indices 0-5) followed by Black pieces (indices 6-11).
    pub const ALL: [Piece; 12] = [
        Self::WHITE_PAWN,
        Self::WHITE_KNIGHT,
        Self::WHITE_BISHOP,
        Self::WHITE_ROOK,
        Self::WHITE_QUEEN,
        Self::WHITE_KING,
        Self::BLACK_PAWN,
        Self::BLACK_KNIGHT,
        Self::BLACK_BISHOP,
        Self::BLACK_ROOK,
        Self::BLACK_QUEEN,
        Self::BLACK_KING,
    ];

    /// Create a piece from a kind and a color.
    #[inline]
    pub const fn new(kind: PieceKind, color: Color) -> Piece {
        Piece((color as u8) << 3 | (kind as u8))
    }

    /// Parse a FEN character into a piece.
    ///
    /// Uppercase letters produce White pieces; lowercase letters produce Black pieces.
    /// Returns `None` for characters that are not valid piece letters.
    #[inline]
    pub fn from_fen_char(c: char) -> Option<Piece> {
        let kind = PieceKind::from_fen_char(c)?;
        let color = if c.is_ascii_uppercase() {
            Color::White
        } else {
            Color::Black
        };
        Some(Piece::new(kind, color))
    }

    /// Return the piece kind (the lower 3 bits).
    #[inline]
    pub const fn kind(self) -> PieceKind {
        match self.0 & 0x07 {
            0 => PieceKind::Pawn,
            1 => PieceKind::Knight,
            2 => PieceKind::Bishop,
            3 => PieceKind::Rook,
            4 => PieceKind::Queen,
            _ => PieceKind::King,
        }
    }

    /// Return the color (bit 3: 0 = White, 1 = Black).
    #[inline]
    pub const fn color(self) -> Color {
        match self.0 >> 3 {
            0 => Color::White,
            _ => Color::Black,
        }
    }

    /// Return a contiguous index 0-11 for use in fixed-size arrays.
    ///
    /// White pieces occupy indices 0-5, Black pieces occupy indices 6-11.
    /// The kind index within each color group matches [`PieceKind::index`].
    #[inline]
    pub const fn index(self) -> usize {
        let color_bit = (self.0 >> 3) as usize;
        let kind_bits = (self.0 & 0x07) as usize;
        color_bit * 6 + kind_bits
    }

    /// Return the raw bit-packed byte (0-5 for White, 8-13 for Black).
    #[inline]
    pub const fn raw(self) -> u8 {
        self.0
    }

    /// Return the FEN character for this piece.
    ///
    /// Uppercase for White pieces, lowercase for Black pieces.
    #[inline]
    pub fn fen_char(self) -> char {
        let base = self.kind().fen_char();
        match self.color() {
            Color::White => base.to_ascii_uppercase(),
            Color::Black => base,
        }
    }
}

impl fmt::Display for Piece {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.fen_char())
    }
}

impl fmt::Debug for Piece {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let color_prefix = match self.color() {
            Color::White => 'W',
            Color::Black => 'B',
        };
        let kind_char = self.kind().fen_char().to_ascii_uppercase();
        write!(f, "{}{}", color_prefix, kind_char)
    }
}

#[cfg(test)]
mod tests {
    use super::Piece;
    use crate::color::Color;
    use crate::piece_kind::PieceKind;

    #[test]
    fn new_roundtrip() {
        for color in Color::ALL {
            for kind in PieceKind::ALL {
                let piece = Piece::new(kind, color);
                assert_eq!(piece.kind(), kind, "kind mismatch for {color:?} {kind:?}");
                assert_eq!(piece.color(), color, "color mismatch for {color:?} {kind:?}");
            }
        }
    }

    #[test]
    fn raw_values() {
        assert_eq!(Piece::WHITE_PAWN.raw(), 0);
        assert_eq!(Piece::WHITE_KNIGHT.raw(), 1);
        assert_eq!(Piece::WHITE_BISHOP.raw(), 2);
        assert_eq!(Piece::WHITE_ROOK.raw(), 3);
        assert_eq!(Piece::WHITE_QUEEN.raw(), 4);
        assert_eq!(Piece::WHITE_KING.raw(), 5);
        assert_eq!(Piece::BLACK_PAWN.raw(), 8);
        assert_eq!(Piece::BLACK_KNIGHT.raw(), 9);
        assert_eq!(Piece::BLACK_BISHOP.raw(), 10);
        assert_eq!(Piece::BLACK_ROOK.raw(), 11);
        assert_eq!(Piece::BLACK_QUEEN.raw(), 12);
        assert_eq!(Piece::BLACK_KING.raw(), 13);
    }

    #[test]
    fn index_contiguity() {
        let mut seen = [false; 12];
        for piece in Piece::ALL {
            let idx = piece.index();
            assert!(idx < 12, "index {idx} out of range for {piece:?}");
            assert!(!seen[idx], "duplicate index {idx} for {piece:?}");
            seen[idx] = true;
        }
        assert!(seen.iter().all(|&v| v), "not all indices 0-11 were covered");
    }

    #[test]
    fn fen_char_roundtrip() {
        for piece in Piece::ALL {
            let c = piece.fen_char();
            assert_eq!(
                Piece::from_fen_char(c),
                Some(piece),
                "roundtrip failed for {piece:?} (char '{c}')"
            );
        }
    }

    #[test]
    fn from_fen_char_case_sensitivity() {
        // Uppercase → White
        assert_eq!(Piece::from_fen_char('P'), Some(Piece::WHITE_PAWN));
        assert_eq!(Piece::from_fen_char('N'), Some(Piece::WHITE_KNIGHT));
        assert_eq!(Piece::from_fen_char('B'), Some(Piece::WHITE_BISHOP));
        assert_eq!(Piece::from_fen_char('R'), Some(Piece::WHITE_ROOK));
        assert_eq!(Piece::from_fen_char('Q'), Some(Piece::WHITE_QUEEN));
        assert_eq!(Piece::from_fen_char('K'), Some(Piece::WHITE_KING));

        // Lowercase → Black
        assert_eq!(Piece::from_fen_char('p'), Some(Piece::BLACK_PAWN));
        assert_eq!(Piece::from_fen_char('n'), Some(Piece::BLACK_KNIGHT));
        assert_eq!(Piece::from_fen_char('b'), Some(Piece::BLACK_BISHOP));
        assert_eq!(Piece::from_fen_char('r'), Some(Piece::BLACK_ROOK));
        assert_eq!(Piece::from_fen_char('q'), Some(Piece::BLACK_QUEEN));
        assert_eq!(Piece::from_fen_char('k'), Some(Piece::BLACK_KING));

        // Invalid chars → None
        assert_eq!(Piece::from_fen_char('x'), None);
        assert_eq!(Piece::from_fen_char('1'), None);
        assert_eq!(Piece::from_fen_char(' '), None);
        assert_eq!(Piece::from_fen_char('Z'), None);
    }

    #[test]
    fn display_format() {
        assert_eq!(format!("{}", Piece::WHITE_PAWN), "P");
        assert_eq!(format!("{}", Piece::WHITE_KING), "K");
        assert_eq!(format!("{}", Piece::BLACK_PAWN), "p");
        assert_eq!(format!("{}", Piece::BLACK_KING), "k");
        assert_eq!(format!("{}", Piece::WHITE_KNIGHT), "N");
        assert_eq!(format!("{}", Piece::BLACK_QUEEN), "q");
    }

    #[test]
    fn debug_format() {
        assert_eq!(format!("{:?}", Piece::WHITE_PAWN), "WP");
        assert_eq!(format!("{:?}", Piece::WHITE_KNIGHT), "WN");
        assert_eq!(format!("{:?}", Piece::WHITE_BISHOP), "WB");
        assert_eq!(format!("{:?}", Piece::WHITE_ROOK), "WR");
        assert_eq!(format!("{:?}", Piece::WHITE_QUEEN), "WQ");
        assert_eq!(format!("{:?}", Piece::WHITE_KING), "WK");
        assert_eq!(format!("{:?}", Piece::BLACK_PAWN), "BP");
        assert_eq!(format!("{:?}", Piece::BLACK_KNIGHT), "BN");
        assert_eq!(format!("{:?}", Piece::BLACK_BISHOP), "BB");
        assert_eq!(format!("{:?}", Piece::BLACK_ROOK), "BR");
        assert_eq!(format!("{:?}", Piece::BLACK_QUEEN), "BQ");
        assert_eq!(format!("{:?}", Piece::BLACK_KING), "BK");
    }

    #[test]
    fn count_and_all() {
        assert_eq!(Piece::COUNT, 12);
        assert_eq!(Piece::ALL.len(), Piece::COUNT);
    }
}
