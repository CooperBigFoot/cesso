//! The chess board: piece placement, side to move, castling, en passant, and move counters.

use std::fmt;

use crate::bitboard::Bitboard;
use crate::castle_rights::CastleRights;
use crate::color::Color;
use crate::error::BoardError;
use crate::piece::Piece;
use crate::piece_kind::PieceKind;
use crate::square::Square;
use crate::zobrist;

/// Complete chess position state.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Board {
    /// Bitboard for each piece kind, indexed by [`PieceKind::index()`].
    pieces: [Bitboard; PieceKind::COUNT],
    /// Bitboard for each side, indexed by [`Color::index()`].
    sides: [Bitboard; Color::COUNT],
    /// Union of both sides — cached for performance.
    occupied: Bitboard,
    /// Which side moves next.
    side_to_move: Color,
    /// Current castling rights.
    castling: CastleRights,
    /// En passant target square, if any.
    en_passant: Option<Square>,
    /// Halfmove clock for the fifty-move rule.
    halfmove_clock: u16,
    /// Fullmove number (starts at 1, incremented after Black moves).
    fullmove_number: u16,
    /// Zobrist hash of the position.
    hash: u64,
}

impl Board {
    /// Return the standard starting position.
    pub fn starting_position() -> Board {
        // White pieces
        let white_pawns = Bitboard::RANK_2;
        let white_rooks = Bitboard::new(
            Square::A1.bitboard().inner() | Square::H1.bitboard().inner(),
        );
        let white_knights = Bitboard::new(
            Square::B1.bitboard().inner() | Square::G1.bitboard().inner(),
        );
        let white_bishops = Bitboard::new(
            Square::C1.bitboard().inner() | Square::F1.bitboard().inner(),
        );
        let white_queens = Square::D1.bitboard();
        let white_king = Square::E1.bitboard();

        // Black pieces
        let black_pawns = Bitboard::RANK_7;
        let black_rooks = Bitboard::new(
            Square::A8.bitboard().inner() | Square::H8.bitboard().inner(),
        );
        let black_knights = Bitboard::new(
            Square::B8.bitboard().inner() | Square::G8.bitboard().inner(),
        );
        let black_bishops = Bitboard::new(
            Square::C8.bitboard().inner() | Square::F8.bitboard().inner(),
        );
        let black_queens = Square::D8.bitboard();
        let black_king = Square::E8.bitboard();

        let pawns = white_pawns | black_pawns;
        let knights = white_knights | black_knights;
        let bishops = white_bishops | black_bishops;
        let rooks = white_rooks | black_rooks;
        let queens = white_queens | black_queens;
        let kings = white_king | black_king;

        let white = white_pawns | white_knights | white_bishops | white_rooks | white_queens | white_king;
        let black = black_pawns | black_knights | black_bishops | black_rooks | black_queens | black_king;
        let occupied = white | black;

        let mut board = Board {
            pieces: [pawns, knights, bishops, rooks, queens, kings],
            sides: [white, black],
            occupied,
            side_to_move: Color::White,
            castling: CastleRights::ALL,
            en_passant: None,
            halfmove_clock: 0,
            fullmove_number: 1,
            hash: 0,
        };
        board.hash = zobrist::hash_from_scratch(&board);
        board
    }

    /// Construct a board from raw components. Used by FEN parsing.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_raw(
        pieces: [Bitboard; PieceKind::COUNT],
        sides: [Bitboard; Color::COUNT],
        occupied: Bitboard,
        side_to_move: Color,
        castling: CastleRights,
        en_passant: Option<Square>,
        halfmove_clock: u16,
        fullmove_number: u16,
        hash: u64,
    ) -> Board {
        Board {
            pieces,
            sides,
            occupied,
            side_to_move,
            castling,
            en_passant,
            halfmove_clock,
            fullmove_number,
            hash,
        }
    }

    /// Return the piece kind on the given square, if any.
    pub fn piece_on(&self, sq: Square) -> Option<PieceKind> {
        PieceKind::ALL
            .into_iter()
            .find(|&kind| self.pieces[kind.index()].contains(sq))
    }

    /// Return the color of the piece on the given square, if any.
    pub fn color_on(&self, sq: Square) -> Option<Color> {
        Color::ALL
            .into_iter()
            .find(|&color| self.sides[color.index()].contains(sq))
    }

    /// Return the bitboard for the given piece kind (both colors).
    #[inline]
    pub fn pieces(&self, kind: PieceKind) -> Bitboard {
        self.pieces[kind.index()]
    }

    /// Return the bitboard for the given side.
    #[inline]
    pub fn side(&self, color: Color) -> Bitboard {
        self.sides[color.index()]
    }

    /// Return the occupied squares bitboard.
    #[inline]
    pub fn occupied(&self) -> Bitboard {
        self.occupied
    }

    /// Return `true` if the given square is occupied.
    #[inline]
    pub fn is_occupied(&self, sq: Square) -> bool {
        self.occupied.contains(sq)
    }

    /// Return the square of the king for the given side.
    ///
    /// # Panics
    ///
    /// Panics if the board has no king for the given color (invalid board state).
    pub fn king_square(&self, color: Color) -> Square {
        let king_bb = self.pieces[PieceKind::King.index()] & self.sides[color.index()];
        king_bb
            .lsb()
            .expect("board must have a king for each side")
    }

    /// Return the side to move.
    #[inline]
    pub fn side_to_move(&self) -> Color {
        self.side_to_move
    }

    /// Return the current castling rights.
    #[inline]
    pub fn castling(&self) -> CastleRights {
        self.castling
    }

    /// Return the en passant target square, if any.
    #[inline]
    pub fn en_passant(&self) -> Option<Square> {
        self.en_passant
    }

    /// Return the halfmove clock.
    #[inline]
    pub fn halfmove_clock(&self) -> u16 {
        self.halfmove_clock
    }

    /// Return the fullmove number.
    #[inline]
    pub fn fullmove_number(&self) -> u16 {
        self.fullmove_number
    }

    /// Return the Zobrist hash of the position.
    #[inline]
    pub fn hash(&self) -> u64 {
        self.hash
    }

    /// Set the Zobrist hash.
    #[inline]
    pub(crate) fn set_hash(&mut self, hash: u64) {
        self.hash = hash;
    }

    /// Toggle a piece into/out of the board arrays via XOR.
    #[inline]
    #[allow(dead_code)]
    pub(crate) fn toggle_piece(&mut self, sq: Square, kind: PieceKind, color: Color) {
        let mask = sq.bitboard();
        self.pieces[kind.index()] = self.pieces[kind.index()] ^ mask;
        self.sides[color.index()] = self.sides[color.index()] ^ mask;
        self.occupied = self.sides[Color::White.index()] | self.sides[Color::Black.index()];
    }

    /// Return the colored piece on the given square, if any.
    pub fn colored_piece_on(&self, sq: Square) -> Option<Piece> {
        let kind = self.piece_on(sq)?;
        let color = self.color_on(sq)?;
        Some(Piece::new(kind, color))
    }

    /// Toggle a packed piece into/out of the board arrays via XOR.
    #[inline]
    #[allow(dead_code)]
    pub(crate) fn toggle_piece_packed(&mut self, sq: Square, piece: Piece) {
        self.toggle_piece(sq, piece.kind(), piece.color());
    }

    /// Set the en passant target square.
    #[inline]
    pub(crate) fn set_en_passant(&mut self, sq: Option<Square>) {
        self.en_passant = sq;
    }

    /// Set the castling rights.
    #[inline]
    pub(crate) fn set_castling(&mut self, rights: CastleRights) {
        self.castling = rights;
    }

    /// Set the halfmove clock.
    #[inline]
    pub(crate) fn set_halfmove_clock(&mut self, clock: u16) {
        self.halfmove_clock = clock;
    }

    /// Set the side to move.
    #[inline]
    pub(crate) fn set_side_to_move(&mut self, color: Color) {
        self.side_to_move = color;
    }

    /// Set the fullmove number.
    #[inline]
    pub(crate) fn set_fullmove_number(&mut self, number: u16) {
        self.fullmove_number = number;
    }

    /// Validate the structural integrity of the board.
    pub fn validate(&self) -> Result<(), BoardError> {
        // Check exactly one king per side
        for color in Color::ALL {
            let king_count = (self.pieces[PieceKind::King.index()] & self.sides[color.index()]).count();
            if king_count != 1 {
                let color_name = match color {
                    Color::White => "white",
                    Color::Black => "black",
                };
                return Err(BoardError::InvalidKingCount {
                    color: color_name,
                    count: king_count,
                });
            }
        }

        // Check no pawns on rank 1 or rank 8
        let back_ranks = Bitboard::RANK_1 | Bitboard::RANK_8;
        if (self.pieces[PieceKind::Pawn.index()] & back_ranks).is_nonempty() {
            return Err(BoardError::PawnsOnBackRank);
        }

        // Check no overlapping piece bitboards
        for i in 0..PieceKind::COUNT {
            for j in (i + 1)..PieceKind::COUNT {
                if (self.pieces[i] & self.pieces[j]).is_nonempty() {
                    return Err(BoardError::OverlappingPieces);
                }
            }
        }

        // Check sides don't overlap
        if (self.sides[Color::White.index()] & self.sides[Color::Black.index()]).is_nonempty() {
            return Err(BoardError::InconsistentSides);
        }

        // Check occupied == sides[0] | sides[1]
        let expected_occupied = self.sides[Color::White.index()] | self.sides[Color::Black.index()];
        if self.occupied != expected_occupied {
            return Err(BoardError::InconsistentOccupied);
        }

        Ok(())
    }

    /// Return a pretty-printable wrapper for this board.
    pub fn pretty(&self) -> PrettyBoard<'_> {
        PrettyBoard(self)
    }
}

impl fmt::Debug for Board {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Board(\"{}\")", self)
    }
}

/// Wrapper for pretty-printing a board as an 8x8 grid.
pub struct PrettyBoard<'a>(&'a Board);

impl fmt::Display for PrettyBoard<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let board = self.0;
        for rank_idx in (0u8..8).rev() {
            write!(f, "{}  ", rank_idx + 1)?;
            for file_idx in 0u8..8 {
                let sq = Square::from_index(rank_idx * 8 + file_idx).unwrap();
                let c = match (board.piece_on(sq), board.color_on(sq)) {
                    (Some(kind), Some(Color::White)) => kind.fen_char().to_ascii_uppercase(),
                    (Some(kind), Some(Color::Black)) => kind.fen_char(),
                    _ => '.',
                };
                if file_idx < 7 {
                    write!(f, "{c} ")?;
                } else {
                    write!(f, "{c}")?;
                }
            }
            writeln!(f)?;
        }
        write!(f, "   a b c d e f g h")
    }
}

#[cfg(test)]
mod tests {
    use super::Board;
    use crate::color::Color;
    use crate::piece::Piece;
    use crate::piece_kind::PieceKind;
    use crate::square::Square;

    #[test]
    fn starting_position_validates() {
        let board = Board::starting_position();
        board.validate().unwrap();
    }

    #[test]
    fn starting_position_piece_on() {
        let board = Board::starting_position();
        assert_eq!(board.piece_on(Square::E1), Some(PieceKind::King));
        assert_eq!(board.piece_on(Square::D1), Some(PieceKind::Queen));
        assert_eq!(board.piece_on(Square::A1), Some(PieceKind::Rook));
        assert_eq!(board.piece_on(Square::B1), Some(PieceKind::Knight));
        assert_eq!(board.piece_on(Square::C1), Some(PieceKind::Bishop));
        assert_eq!(board.piece_on(Square::E2), Some(PieceKind::Pawn));
        assert_eq!(board.piece_on(Square::E4), None);
    }

    #[test]
    fn starting_position_color_on() {
        let board = Board::starting_position();
        assert_eq!(board.color_on(Square::E1), Some(Color::White));
        assert_eq!(board.color_on(Square::E8), Some(Color::Black));
        assert_eq!(board.color_on(Square::E4), None);
    }

    #[test]
    fn king_square() {
        let board = Board::starting_position();
        assert_eq!(board.king_square(Color::White), Square::E1);
        assert_eq!(board.king_square(Color::Black), Square::E8);
    }

    #[test]
    fn occupied_count() {
        let board = Board::starting_position();
        assert_eq!(board.occupied().count(), 32);
    }

    #[test]
    fn toggle_piece() {
        let mut board = Board::starting_position();
        board.toggle_piece(Square::E2, PieceKind::Pawn, Color::White);
        assert!(!board.is_occupied(Square::E2));
        assert_eq!(board.occupied().count(), 31);

        board.toggle_piece(Square::E4, PieceKind::Pawn, Color::White);
        assert!(board.is_occupied(Square::E4));
        assert_eq!(board.piece_on(Square::E4), Some(PieceKind::Pawn));
        assert_eq!(board.color_on(Square::E4), Some(Color::White));
    }

    #[test]
    fn pretty_print() {
        let board = Board::starting_position();
        let output = format!("{}", board.pretty());
        assert!(output.contains("r n b q k b n r"));
        assert!(output.contains("R N B Q K B N R"));
        assert!(output.contains("a b c d e f g h"));
    }

    #[test]
    fn colored_piece_on_starting() {
        let board = Board::starting_position();
        assert_eq!(board.colored_piece_on(Square::E1), Some(Piece::WHITE_KING));
        assert_eq!(board.colored_piece_on(Square::E8), Some(Piece::BLACK_KING));
        assert_eq!(board.colored_piece_on(Square::D1), Some(Piece::WHITE_QUEEN));
        assert_eq!(board.colored_piece_on(Square::E4), None);
    }
}
