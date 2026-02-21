//! UCI command parsing.

use std::time::Duration;

use cesso_core::{Board, Move};

use crate::error::UciError;

/// Parameters for the `go` command.
///
/// All fields are optional; a bare `go` uses defaults.
#[derive(Debug, Clone, Default)]
pub struct GoParams {
    /// White's remaining time.
    pub wtime: Option<Duration>,
    /// Black's remaining time.
    pub btime: Option<Duration>,
    /// White's increment per move.
    pub winc: Option<Duration>,
    /// Black's increment per move.
    pub binc: Option<Duration>,
    /// Moves until next time control.
    pub movestogo: Option<u32>,
    /// Search to this depth only.
    pub depth: Option<u8>,
    /// Search for exactly this duration.
    pub movetime: Option<Duration>,
    /// Search this many nodes only.
    pub nodes: Option<u64>,
    /// Search until `stop` (no time limit).
    pub infinite: bool,
    /// Search in pondering mode.
    pub ponder: bool,
}

/// A UCI option sent via `setoption`.
#[derive(Debug, Clone, PartialEq)]
pub enum UciOption {
    /// Hash table size in megabytes, clamped to [1, 65536].
    Hash(u32),
    /// Number of search threads, clamped to [1, 256].
    Threads(u16),
    /// Enable or disable pondering.
    Ponder(bool),
}

/// Board position with game history for repetition detection.
#[derive(Debug, Clone)]
pub struct PositionInfo {
    /// The current board position.
    pub board: Board,
    /// Zobrist hashes of all positions from game start, up to but NOT
    /// including the current position.
    pub history: Vec<u64>,
}

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
    Position(PositionInfo),
    /// `go` -- start searching with given parameters.
    Go(GoParams),
    /// `setoption` -- configure an engine option.
    SetOption(UciOption),
    /// `ponderhit` -- opponent played the expected move during pondering.
    PonderHit,
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
        "ponderhit" => Ok(Command::PonderHit),
        "position" => parse_position(&tokens[1..]),
        "go" => parse_go(&tokens[1..]),
        "setoption" => parse_setoption(&tokens[1..]),
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
    let mut history = Vec::new();
    if !rest.is_empty() && rest[0] == "moves" {
        for uci_str in &rest[1..] {
            history.push(board.hash());
            let mv = Move::from_uci(uci_str, &board).ok_or_else(|| UciError::InvalidMove {
                uci_move: uci_str.to_string(),
            })?;
            board = board.make_move(mv);
        }
    }

    Ok(Command::Position(PositionInfo { board, history }))
}

/// Parse the `go` command arguments.
///
/// Supports: wtime, btime, winc, binc, movestogo, depth, movetime,
/// nodes, infinite, ponder. Unknown tokens are silently skipped.
fn parse_go(tokens: &[&str]) -> Result<Command, UciError> {
    let mut params = GoParams::default();

    let mut i = 0;
    while i < tokens.len() {
        match tokens[i] {
            "wtime" => {
                params.wtime = Some(parse_millis(tokens.get(i + 1), "wtime")?);
                i += 2;
            }
            "btime" => {
                params.btime = Some(parse_millis(tokens.get(i + 1), "btime")?);
                i += 2;
            }
            "winc" => {
                params.winc = Some(parse_millis(tokens.get(i + 1), "winc")?);
                i += 2;
            }
            "binc" => {
                params.binc = Some(parse_millis(tokens.get(i + 1), "binc")?);
                i += 2;
            }
            "movestogo" => {
                params.movestogo = Some(parse_int(tokens.get(i + 1), "movestogo")?);
                i += 2;
            }
            "depth" => {
                params.depth = Some(parse_int(tokens.get(i + 1), "depth")?);
                i += 2;
            }
            "movetime" => {
                params.movetime = Some(parse_millis(tokens.get(i + 1), "movetime")?);
                i += 2;
            }
            "nodes" => {
                params.nodes = Some(parse_int(tokens.get(i + 1), "nodes")?);
                i += 2;
            }
            "infinite" => {
                params.infinite = true;
                i += 1;
            }
            "ponder" => {
                params.ponder = true;
                i += 1;
            }
            _ => {
                // Unknown token -- skip per UCI convention
                i += 1;
            }
        }
    }

    Ok(Command::Go(params))
}

/// Parse the `setoption` command arguments.
///
/// Supports: `setoption name <name> [value <value>]` per UCI spec.
/// Option names are matched case-insensitively. Unknown option names
/// produce [`Command::Unknown`] (silently ignored per UCI spec).
///
/// # Errors
///
/// | Condition | Error |
/// |---|---|
/// | Missing `name` keyword | [`UciError::MalformedSetOption`] |
/// | Invalid or missing value | [`UciError::InvalidOptionValue`] |
fn parse_setoption(tokens: &[&str]) -> Result<Command, UciError> {
    // Require the "name" keyword as the first token
    if tokens.is_empty() || tokens[0] != "name" {
        return Err(UciError::MalformedSetOption);
    }

    // Collect tokens after "name" until we hit "value" keyword (or end)
    let rest = &tokens[1..];
    let value_pos = rest.iter().position(|&t| t == "value");

    let (name_tokens, value_token) = match value_pos {
        Some(pos) => (&rest[..pos], rest.get(pos + 1).copied()),
        None => (rest, None),
    };

    let name = name_tokens.join(" ").to_lowercase();

    match name.as_str() {
        "hash" => {
            let raw = value_token.ok_or_else(|| UciError::InvalidOptionValue {
                name: "Hash".to_string(),
                value: String::new(),
            })?;
            let parsed: u32 = raw.parse().map_err(|_| UciError::InvalidOptionValue {
                name: "Hash".to_string(),
                value: raw.to_string(),
            })?;
            let clamped = parsed.clamp(1, 65536);
            Ok(Command::SetOption(UciOption::Hash(clamped)))
        }
        "threads" => {
            let raw = value_token.ok_or_else(|| UciError::InvalidOptionValue {
                name: "Threads".to_string(),
                value: String::new(),
            })?;
            let parsed: u32 = raw.parse().map_err(|_| UciError::InvalidOptionValue {
                name: "Threads".to_string(),
                value: raw.to_string(),
            })?;
            let clamped = parsed.clamp(1, 256) as u16;
            Ok(Command::SetOption(UciOption::Threads(clamped)))
        }
        "ponder" => {
            let raw = value_token.ok_or_else(|| UciError::InvalidOptionValue {
                name: "Ponder".to_string(),
                value: String::new(),
            })?;
            let enabled = match raw {
                "true" => true,
                "false" => false,
                _ => {
                    return Err(UciError::InvalidOptionValue {
                        name: "Ponder".to_string(),
                        value: raw.to_string(),
                    });
                }
            };
            Ok(Command::SetOption(UciOption::Ponder(enabled)))
        }
        _ => Ok(Command::Unknown(name)),
    }
}

/// Parse a millisecond value from a token.
fn parse_millis(token: Option<&&str>, param: &str) -> Result<Duration, UciError> {
    let value = token.ok_or_else(|| UciError::MissingGoValue {
        param: param.to_string(),
    })?;
    let ms: u64 = value.parse().map_err(|_| UciError::InvalidGoValue {
        param: param.to_string(),
        value: value.to_string(),
    })?;
    Ok(Duration::from_millis(ms))
}

/// Parse an integer value from a token.
fn parse_int<T: std::str::FromStr>(token: Option<&&str>, param: &str) -> Result<T, UciError> {
    let value = token.ok_or_else(|| UciError::MissingGoValue {
        param: param.to_string(),
    })?;
    value.parse().map_err(|_| UciError::InvalidGoValue {
        param: param.to_string(),
        value: value.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

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
            Command::Go(params) => assert_eq!(params.depth, Some(6)),
            _ => panic!("expected Go"),
        }
    }

    #[test]
    fn parse_go_bare_defaults() {
        let cmd = parse_command("go").unwrap();
        match cmd {
            Command::Go(params) => {
                assert!(params.depth.is_none());
                assert!(params.wtime.is_none());
                assert!(!params.infinite);
                assert!(!params.ponder);
            }
            _ => panic!("expected Go"),
        }
    }

    #[test]
    fn parse_go_wtime_btime_winc_binc() {
        let cmd = parse_command("go wtime 300000 btime 300000 winc 2000 binc 2000").unwrap();
        match cmd {
            Command::Go(params) => {
                assert_eq!(params.wtime, Some(Duration::from_millis(300000)));
                assert_eq!(params.btime, Some(Duration::from_millis(300000)));
                assert_eq!(params.winc, Some(Duration::from_millis(2000)));
                assert_eq!(params.binc, Some(Duration::from_millis(2000)));
            }
            _ => panic!("expected Go"),
        }
    }

    #[test]
    fn parse_go_movetime() {
        let cmd = parse_command("go movetime 5000").unwrap();
        match cmd {
            Command::Go(params) => {
                assert_eq!(params.movetime, Some(Duration::from_millis(5000)));
            }
            _ => panic!("expected Go"),
        }
    }

    #[test]
    fn parse_go_infinite() {
        let cmd = parse_command("go infinite").unwrap();
        match cmd {
            Command::Go(params) => assert!(params.infinite),
            _ => panic!("expected Go"),
        }
    }

    #[test]
    fn parse_go_ponder_with_time() {
        let cmd = parse_command("go ponder wtime 300000 btime 300000").unwrap();
        match cmd {
            Command::Go(params) => {
                assert!(params.ponder);
                assert_eq!(params.wtime, Some(Duration::from_millis(300000)));
                assert_eq!(params.btime, Some(Duration::from_millis(300000)));
            }
            _ => panic!("expected Go"),
        }
    }

    #[test]
    fn parse_go_movestogo() {
        let cmd = parse_command("go wtime 60000 btime 60000 movestogo 20").unwrap();
        match cmd {
            Command::Go(params) => {
                assert_eq!(params.movestogo, Some(20));
            }
            _ => panic!("expected Go"),
        }
    }

    #[test]
    fn parse_go_nodes() {
        let cmd = parse_command("go nodes 1000000").unwrap();
        match cmd {
            Command::Go(params) => {
                assert_eq!(params.nodes, Some(1_000_000));
            }
            _ => panic!("expected Go"),
        }
    }

    #[test]
    fn parse_ponderhit() {
        assert!(matches!(
            parse_command("ponderhit").unwrap(),
            Command::PonderHit
        ));
    }

    #[test]
    fn parse_go_missing_wtime_value() {
        let result = parse_command("go wtime");
        assert!(result.is_err());
    }

    #[test]
    fn parse_go_invalid_depth_value() {
        let result = parse_command("go depth abc");
        assert!(result.is_err());
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

    #[test]
    fn parse_setoption_hash() {
        let cmd = parse_command("setoption name Hash value 64").unwrap();
        assert!(matches!(cmd, Command::SetOption(UciOption::Hash(64))));
    }

    #[test]
    fn parse_setoption_threads() {
        let cmd = parse_command("setoption name Threads value 4").unwrap();
        assert!(matches!(cmd, Command::SetOption(UciOption::Threads(4))));
    }

    #[test]
    fn parse_setoption_ponder_true() {
        let cmd = parse_command("setoption name Ponder value true").unwrap();
        assert!(matches!(cmd, Command::SetOption(UciOption::Ponder(true))));
    }

    #[test]
    fn parse_setoption_ponder_false() {
        let cmd = parse_command("setoption name Ponder value false").unwrap();
        assert!(matches!(cmd, Command::SetOption(UciOption::Ponder(false))));
    }

    #[test]
    fn parse_setoption_case_insensitive() {
        let cmd = parse_command("setoption name hash value 32").unwrap();
        assert!(matches!(cmd, Command::SetOption(UciOption::Hash(32))));
    }

    #[test]
    fn parse_setoption_hash_clamped_zero() {
        let cmd = parse_command("setoption name Hash value 0").unwrap();
        assert!(matches!(cmd, Command::SetOption(UciOption::Hash(1))));
    }

    #[test]
    fn parse_setoption_hash_clamped_max() {
        let cmd = parse_command("setoption name Hash value 99999").unwrap();
        assert!(matches!(cmd, Command::SetOption(UciOption::Hash(65536))));
    }

    #[test]
    fn parse_setoption_threads_clamped() {
        let cmd_zero = parse_command("setoption name Threads value 0").unwrap();
        assert!(matches!(cmd_zero, Command::SetOption(UciOption::Threads(1))));

        let cmd_over = parse_command("setoption name Threads value 999").unwrap();
        assert!(matches!(cmd_over, Command::SetOption(UciOption::Threads(256))));
    }

    #[test]
    fn parse_setoption_unknown_option() {
        let cmd = parse_command("setoption name FooBar value 42").unwrap();
        assert!(matches!(cmd, Command::Unknown(_)));
    }

    #[test]
    fn parse_setoption_missing_name() {
        let result = parse_command("setoption");
        assert!(result.is_err());
    }

    #[test]
    fn parse_setoption_invalid_hash_value() {
        let result = parse_command("setoption name Hash value abc");
        assert!(result.is_err());
    }

    #[test]
    fn parse_position_builds_history() {
        let cmd = parse_command("position startpos moves e2e4 e7e5").unwrap();
        match cmd {
            Command::Position(info) => {
                assert_eq!(info.history.len(), 2, "2 moves should produce 2 history entries");
            }
            _ => panic!("expected Position"),
        }
    }
}
