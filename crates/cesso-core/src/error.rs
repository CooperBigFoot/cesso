//! Error types for FEN parsing and board validation.

use std::fmt;

/// Errors that occur when parsing a FEN string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FenError {
    /// The FEN string does not have exactly 6 space-separated fields.
    WrongFieldCount {
        /// Number of fields found.
        found: usize,
    },
    /// The piece placement section does not have exactly 8 ranks.
    WrongRankCount {
        /// Number of ranks found.
        found: usize,
    },
    /// A rank in the piece placement describes more or fewer than 8 squares.
    BadRankLength {
        /// Zero-based rank index (0 = rank 8 in FEN, 7 = rank 1).
        rank_index: usize,
        /// Number of squares described.
        length: usize,
    },
    /// An unrecognized character appeared in the piece placement.
    InvalidPieceChar {
        /// The invalid character.
        character: char,
    },
    /// The active color field is not "w" or "b".
    InvalidColor {
        /// The invalid color string.
        found: String,
    },
    /// An unrecognized character appeared in the castling rights field.
    InvalidCastlingChar {
        /// The invalid character.
        character: char,
    },
    /// The en passant field is not "-" or a valid algebraic square.
    InvalidEnPassant {
        /// The invalid en passant string.
        found: String,
    },
    /// A move counter (halfmove clock or fullmove number) is not a valid number.
    InvalidMoveCounter {
        /// The field name ("halfmove clock" or "fullmove number").
        field: &'static str,
        /// The invalid string.
        found: String,
    },
    /// The parsed board fails structural validation.
    InvalidBoard {
        /// The underlying board validation error.
        source: BoardError,
    },
}

impl fmt::Display for FenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FenError::WrongFieldCount { found } => {
                write!(f, "expected 6 FEN fields, found {found}")
            }
            FenError::WrongRankCount { found } => {
                write!(f, "expected 8 ranks in piece placement, found {found}")
            }
            FenError::BadRankLength { rank_index, length } => {
                write!(
                    f,
                    "rank {rank_index} describes {length} squares, expected 8"
                )
            }
            FenError::InvalidPieceChar { character } => {
                write!(f, "invalid piece character: '{character}'")
            }
            FenError::InvalidColor { found } => {
                write!(f, "invalid active color: \"{found}\"")
            }
            FenError::InvalidCastlingChar { character } => {
                write!(f, "invalid castling character: '{character}'")
            }
            FenError::InvalidEnPassant { found } => {
                write!(f, "invalid en passant square: \"{found}\"")
            }
            FenError::InvalidMoveCounter { field, found } => {
                write!(f, "invalid {field}: \"{found}\"")
            }
            FenError::InvalidBoard { source } => {
                write!(f, "invalid board: {source}")
            }
        }
    }
}

impl std::error::Error for FenError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            FenError::InvalidBoard { source } => Some(source),
            _ => None,
        }
    }
}

impl From<BoardError> for FenError {
    fn from(source: BoardError) -> Self {
        FenError::InvalidBoard { source }
    }
}

/// Errors from structural validation of a [`Board`](crate::board::Board).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BoardError {
    /// A side does not have exactly one king.
    #[error("expected 1 king for {color}, found {count}")]
    InvalidKingCount {
        /// Which side has the wrong king count.
        color: &'static str,
        /// Number of kings found.
        count: u32,
    },
    /// Pawns occupy the first or eighth rank.
    #[error("pawns found on back rank")]
    PawnsOnBackRank,
    /// Two different piece kinds claim the same square.
    #[error("overlapping piece bitboards")]
    OverlappingPieces,
    /// The occupied bitboard does not equal the union of both sides.
    #[error("occupied bitboard is inconsistent with side bitboards")]
    InconsistentOccupied,
    /// The two side bitboards overlap.
    #[error("white and black side bitboards overlap")]
    InconsistentSides,
}

#[cfg(test)]
mod tests {
    use super::{BoardError, FenError};

    #[test]
    fn fen_error_display() {
        let err = FenError::WrongFieldCount { found: 4 };
        assert_eq!(format!("{err}"), "expected 6 FEN fields, found 4");
    }

    #[test]
    fn board_error_display() {
        let err = BoardError::PawnsOnBackRank;
        assert_eq!(format!("{err}"), "pawns found on back rank");
    }

    #[test]
    fn fen_error_from_board_error() {
        let board_err = BoardError::OverlappingPieces;
        let fen_err: FenError = board_err.into();
        assert!(matches!(fen_err, FenError::InvalidBoard { .. }));
    }
}
