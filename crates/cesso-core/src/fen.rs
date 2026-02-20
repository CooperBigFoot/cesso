//! FEN string parsing and serialization for [`Board`].

use std::str::FromStr;
use std::fmt;

use crate::bitboard::Bitboard;
use crate::board::Board;
use crate::castle_rights::CastleRights;
use crate::color::Color;
use crate::error::FenError;
use crate::file::File;
use crate::piece_kind::PieceKind;
use crate::rank::Rank;
use crate::square::Square;

/// The FEN string for the standard starting position.
pub const STARTING_FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

impl FromStr for Board {
    type Err = FenError;

    fn from_str(fen: &str) -> Result<Board, FenError> {
        let fields: Vec<&str> = fen.split_whitespace().collect();
        if fields.len() != 6 {
            return Err(FenError::WrongFieldCount {
                found: fields.len(),
            });
        }

        // Parse piece placement
        let ranks: Vec<&str> = fields[0].split('/').collect();
        if ranks.len() != 8 {
            return Err(FenError::WrongRankCount {
                found: ranks.len(),
            });
        }

        let mut pieces = [Bitboard::EMPTY; PieceKind::COUNT];
        let mut sides = [Bitboard::EMPTY; Color::COUNT];

        for (rank_index, rank_str) in ranks.iter().enumerate() {
            // FEN ranks go from 8 to 1 (top to bottom)
            let rank = Rank::from_index(7 - rank_index as u8).unwrap();
            let mut file_index: u8 = 0;

            for c in rank_str.chars() {
                if let Some(digit) = c.to_digit(10) {
                    if !(1..=8).contains(&digit) {
                        return Err(FenError::InvalidPieceChar { character: c });
                    }
                    file_index += digit as u8;
                } else {
                    let kind = PieceKind::from_fen_char(c).ok_or(FenError::InvalidPieceChar {
                        character: c,
                    })?;
                    let color = if c.is_ascii_uppercase() {
                        Color::White
                    } else {
                        Color::Black
                    };

                    if file_index >= 8 {
                        return Err(FenError::BadRankLength {
                            rank_index,
                            length: file_index as usize + 1,
                        });
                    }

                    let file = File::from_index(file_index).unwrap();
                    let sq = Square::new(rank, file);
                    let bb = sq.bitboard();

                    pieces[kind.index()] = pieces[kind.index()] | bb;
                    sides[color.index()] = sides[color.index()] | bb;
                    file_index += 1;
                }
            }

            if file_index != 8 {
                return Err(FenError::BadRankLength {
                    rank_index,
                    length: file_index as usize,
                });
            }
        }

        let occupied = sides[Color::White.index()] | sides[Color::Black.index()];

        // Parse active color
        let side_to_move = match fields[1] {
            "w" => Color::White,
            "b" => Color::Black,
            other => {
                return Err(FenError::InvalidColor {
                    found: other.to_string(),
                })
            }
        };

        // Parse castling rights
        let castling = CastleRights::from_fen(fields[2])?;

        // Parse en passant
        let en_passant = if fields[3] == "-" {
            None
        } else {
            Some(
                Square::from_algebraic(fields[3]).ok_or_else(|| FenError::InvalidEnPassant {
                    found: fields[3].to_string(),
                })?,
            )
        };

        // Parse halfmove clock
        let halfmove_clock = fields[4].parse::<u16>().map_err(|_| FenError::InvalidMoveCounter {
            field: "halfmove clock",
            found: fields[4].to_string(),
        })?;

        // Parse fullmove number
        let fullmove_number =
            fields[5]
                .parse::<u16>()
                .map_err(|_| FenError::InvalidMoveCounter {
                    field: "fullmove number",
                    found: fields[5].to_string(),
                })?;

        let board = Board::from_raw(
            pieces,
            sides,
            occupied,
            side_to_move,
            castling,
            en_passant,
            halfmove_clock,
            fullmove_number,
        );

        board.validate()?;
        Ok(board)
    }
}

impl fmt::Display for Board {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Piece placement
        for rank_idx in (0u8..8).rev() {
            let rank = Rank::from_index(rank_idx).unwrap();
            let mut empty_count = 0u8;

            for file_idx in 0u8..8 {
                let file = File::from_index(file_idx).unwrap();
                let sq = Square::new(rank, file);

                match (self.piece_on(sq), self.color_on(sq)) {
                    (Some(kind), Some(color)) => {
                        if empty_count > 0 {
                            write!(f, "{empty_count}")?;
                            empty_count = 0;
                        }
                        let c = match color {
                            Color::White => kind.fen_char().to_ascii_uppercase(),
                            Color::Black => kind.fen_char(),
                        };
                        write!(f, "{c}")?;
                    }
                    _ => {
                        empty_count += 1;
                    }
                }
            }

            if empty_count > 0 {
                write!(f, "{empty_count}")?;
            }

            if rank_idx > 0 {
                write!(f, "/")?;
            }
        }

        // Side to move
        write!(f, " {}", self.side_to_move())?;

        // Castling
        write!(f, " {}", self.castling())?;

        // En passant
        match self.en_passant() {
            Some(sq) => write!(f, " {sq}")?,
            None => write!(f, " -")?,
        }

        // Move counters
        write!(f, " {} {}", self.halfmove_clock(), self.fullmove_number())
    }
}

#[cfg(test)]
mod tests {
    use super::STARTING_FEN;
    use crate::board::Board;

    fn roundtrip(fen: &str) {
        let board: Board = fen.parse().unwrap();
        let output = format!("{board}");
        assert_eq!(output, fen, "FEN roundtrip failed");
        // Parse again to verify
        let board2: Board = output.parse().unwrap();
        assert_eq!(board, board2);
    }

    #[test]
    fn roundtrip_starting() {
        roundtrip(STARTING_FEN);
    }

    #[test]
    fn roundtrip_sicilian() {
        roundtrip("rnbqkbnr/pp1ppppp/8/2p5/4P3/8/PPPP1PPP/RNBQKBNR w KQkq c6 0 2");
    }

    #[test]
    fn roundtrip_kiwipete() {
        roundtrip(
            "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
        );
    }

    #[test]
    fn roundtrip_endgame() {
        roundtrip("8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1");
    }

    #[test]
    fn roundtrip_black_to_move() {
        roundtrip("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1");
    }

    #[test]
    fn starting_position_matches_fen() {
        let from_constructor = Board::starting_position();
        let from_fen: Board = STARTING_FEN.parse().unwrap();
        assert_eq!(from_constructor, from_fen);
    }

    #[test]
    fn error_wrong_field_count() {
        let result = "e4 e5".parse::<Board>();
        assert!(result.is_err());
    }

    #[test]
    fn error_invalid_piece_char() {
        let result =
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPXPPP/RNBQKBNR w KQkq - 0 1".parse::<Board>();
        assert!(result.is_err());
    }

    #[test]
    fn error_bad_rank_length() {
        let result =
            "rnbqkbnr/ppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".parse::<Board>();
        assert!(result.is_err());
    }

    #[test]
    fn error_invalid_color() {
        let result =
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR x KQkq - 0 1".parse::<Board>();
        assert!(result.is_err());
    }

    #[test]
    fn error_invalid_castling() {
        let result =
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w XQkq - 0 1".parse::<Board>();
        assert!(result.is_err());
    }

    #[test]
    fn error_invalid_en_passant() {
        let result =
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq z9 0 1".parse::<Board>();
        assert!(result.is_err());
    }

    #[test]
    fn error_invalid_move_counter() {
        let result =
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - abc 1".parse::<Board>();
        assert!(result.is_err());
    }
}
