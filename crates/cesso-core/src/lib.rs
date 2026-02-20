//! Core chess types: board representation, move generation, and game rules.

mod attacks;
mod bitboard;
mod board;
mod castle_rights;
mod chess_move;
mod color;
mod error;
mod fen;
mod file;
mod make_move;
mod movegen;
mod perft;
mod piece;
mod piece_kind;
mod rank;
mod square;

pub use bitboard::Bitboard;
pub use board::{Board, PrettyBoard};
pub use castle_rights::{CastleRights, CastleSide};
pub use chess_move::{Move, MoveKind, PromotionPiece};
pub use color::Color;
pub use error::{BoardError, FenError};
pub use fen::STARTING_FEN;
pub use file::File;
pub use piece::Piece;
pub use piece_kind::PieceKind;
pub use rank::Rank;
pub use attacks::{
    between, bishop_attacks, king_attacks, knight_attacks, line, pawn_attacks, queen_attacks,
    rook_attacks,
};
pub use movegen::{generate_legal_moves, MoveList};
pub use perft::{divide, perft};
pub use square::Square;
