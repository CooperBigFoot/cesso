//! UCI protocol errors.

/// Errors that can occur during UCI protocol handling.
#[derive(Debug, thiserror::Error)]
pub enum UciError {
    /// The `position` command is missing `startpos` or `fen` keyword.
    #[error("malformed position command: missing startpos or fen keyword")]
    MalformedPosition,

    /// Failed to parse a FEN string.
    #[error("invalid FEN: {fen}")]
    InvalidFen {
        /// The FEN string that failed to parse.
        fen: String,
    },

    /// A move string in the `position` command could not be parsed.
    #[error("invalid move: {uci_move}")]
    InvalidMove {
        /// The UCI move string that failed to parse.
        uci_move: String,
    },

    /// The depth value in `go depth` could not be parsed.
    #[error("invalid depth: {value}")]
    InvalidDepth {
        /// The depth string that failed to parse.
        value: String,
    },

    /// An I/O error occurred while reading from stdin.
    #[error("I/O error: {source}")]
    Io {
        /// The underlying I/O error.
        #[from]
        source: std::io::Error,
    },
}
