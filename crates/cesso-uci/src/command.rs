//! UCI command parsing.

use cesso_core::{Board, Move};

use crate::error::UciError;

/// A parsed UCI command.
#[derive(Debug)]
pub enum Command {
    /// `uci` -- identify the engine.
    Uci,
    /// `isready` -- synchronization ping.
    IsReady,
    /// `ucinewgame` -- reset engine state.
    UciNewGame,
    /// `position` -- set up a board position with optional moves applied.
    Position(Board),
    /// `go depth N` -- search to a fixed depth.
    GoDepth {
        /// Maximum search depth in plies.
        depth: u8,
    },
    /// `stop` -- halt the current search.
    Stop,
    /// `quit` -- exit the engine.
    Quit,
    /// Unrecognized command (silently ignored per UCI spec).
    Unknown(String),
}

/// Parse a single line of UCI input into a [`Command`].
pub fn parse_command(line: &str) -> Result<Command, UciError> {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    if tokens.is_empty() {
        return Ok(Command::Unknown(String::new()));
    }

    match tokens[0] {
        "uci" => Ok(Command::Uci),
        "isready" => Ok(Command::IsReady),
        "ucinewgame" => Ok(Command::UciNewGame),
        "stop" => Ok(Command::Stop),
        "quit" => Ok(Command::Quit),
        "position" => parse_position(&tokens[1..]),
        "go" => parse_go(&tokens[1..]),
        _ => Ok(Command::Unknown(tokens[0].to_string())),
    }
}

/// Parse the `position` command arguments.
///
/// Supports:
/// - `position startpos [moves e2e4 d7d5 ...]`
/// - `position fen <fen-string> [moves e2e4 d7d5 ...]`
fn parse_position(tokens: &[&str]) -> Result<Command, UciError> {
    if tokens.is_empty() {
        return Err(UciError::MalformedPosition);
    }

    let (mut board, rest) = if tokens[0] == "startpos" {
        let rest = &tokens[1..];
        (Board::starting_position(), rest)
    } else if tokens[0] == "fen" {
        // FEN is 6 space-separated fields
        if tokens.len() < 7 {
            return Err(UciError::InvalidFen {
                fen: tokens[1..].join(" "),
            });
        }
        let fen = tokens[1..7].join(" ");
        let board: Board = fen.parse().map_err(|_| UciError::InvalidFen {
            fen: fen.clone(),
        })?;
        (board, &tokens[7..])
    } else {
        return Err(UciError::MalformedPosition);
    };

    // Apply moves if present: "moves e2e4 d7d5 ..."
    if !rest.is_empty() && rest[0] == "moves" {
        for uci_str in &rest[1..] {
            let mv = Move::from_uci(uci_str, &board).ok_or_else(|| UciError::InvalidMove {
                uci_move: uci_str.to_string(),
            })?;
            board = board.make_move(mv);
        }
    }

    Ok(Command::Position(board))
}

/// Parse the `go` command arguments.
///
/// Currently supports only `go depth N`. Defaults to depth 5
/// if no depth subcommand is provided.
fn parse_go(tokens: &[&str]) -> Result<Command, UciError> {
    let mut depth: u8 = 5;

    let mut i = 0;
    while i < tokens.len() {
        if tokens[i] == "depth" && i + 1 < tokens.len() {
            depth = tokens[i + 1]
                .parse()
                .map_err(|_| UciError::InvalidDepth {
                    value: tokens[i + 1].to_string(),
                })?;
            i += 2;
        } else {
            i += 1;
        }
    }

    Ok(Command::GoDepth { depth })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_uci() {
        assert!(matches!(parse_command("uci").unwrap(), Command::Uci));
    }

    #[test]
    fn parse_isready() {
        assert!(matches!(parse_command("isready").unwrap(), Command::IsReady));
    }

    #[test]
    fn parse_quit() {
        assert!(matches!(parse_command("quit").unwrap(), Command::Quit));
    }

    #[test]
    fn parse_ucinewgame() {
        assert!(matches!(
            parse_command("ucinewgame").unwrap(),
            Command::UciNewGame
        ));
    }

    #[test]
    fn parse_position_startpos() {
        let cmd = parse_command("position startpos").unwrap();
        assert!(matches!(cmd, Command::Position(_)));
    }

    #[test]
    fn parse_position_startpos_with_moves() {
        let cmd = parse_command("position startpos moves e2e4 e7e5").unwrap();
        assert!(matches!(cmd, Command::Position(_)));
    }

    #[test]
    fn parse_position_fen() {
        let cmd = parse_command(
            "position fen rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1",
        )
        .unwrap();
        assert!(matches!(cmd, Command::Position(_)));
    }

    #[test]
    fn parse_go_depth() {
        let cmd = parse_command("go depth 6").unwrap();
        match cmd {
            Command::GoDepth { depth } => assert_eq!(depth, 6),
            _ => panic!("expected GoDepth"),
        }
    }

    #[test]
    fn parse_go_default_depth() {
        let cmd = parse_command("go").unwrap();
        match cmd {
            Command::GoDepth { depth } => assert_eq!(depth, 5),
            _ => panic!("expected GoDepth with default depth"),
        }
    }

    #[test]
    fn parse_unknown_command() {
        let cmd = parse_command("foobar").unwrap();
        assert!(matches!(cmd, Command::Unknown(_)));
    }

    #[test]
    fn parse_empty_line() {
        let cmd = parse_command("").unwrap();
        assert!(matches!(cmd, Command::Unknown(_)));
    }

    #[test]
    fn parse_position_missing_keyword() {
        let result = parse_command("position");
        assert!(result.is_err());
    }

    #[test]
    fn parse_position_invalid_fen() {
        let result = parse_command("position fen invalid");
        assert!(result.is_err());
    }

    #[test]
    fn parse_stop() {
        assert!(matches!(parse_command("stop").unwrap(), Command::Stop));
    }
}
