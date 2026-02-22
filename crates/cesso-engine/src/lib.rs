//! Search and evaluation for cesso.

pub mod eval;
pub mod search;
pub mod time;
pub(crate) mod book;

pub use eval::evaluate;
pub use search::control::SearchControl;
pub use search::pool::ThreadPool;
pub use search::{SearchResult, Searcher};
pub use time::limits_from_go;
pub use search::draw::{DrawDecision, decide_draw};
