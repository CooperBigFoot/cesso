//! Core chess types: board representation, move generation, and game rules.

mod bitboard;
mod board;
mod castle_rights;
mod color;
mod error;
mod fen;
mod file;
mod piece;
mod piece_kind;
mod rank;
mod square;

pub use bitboard::Bitboard;
pub use board::{Board, PrettyBoard};
pub use castle_rights::{CastleRights, CastleSide};
pub use color::Color;
pub use error::{BoardError, FenError};
pub use fen::STARTING_FEN;
pub use file::File;
pub use piece::Piece;
pub use piece_kind::PieceKind;
pub use rank::Rank;
pub use square::Square;
