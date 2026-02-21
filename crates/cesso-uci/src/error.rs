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

    /// A `go` parameter is missing its required value.
    #[error("missing value for go parameter: {param}")]
    MissingGoValue {
        /// The parameter name (e.g., "wtime", "depth").
        param: String,
    },

    /// A `go` parameter value could not be parsed.
    #[error("invalid value for go parameter {param}: {value}")]
    InvalidGoValue {
        /// The parameter name.
        param: String,
        /// The value string that failed to parse.
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
