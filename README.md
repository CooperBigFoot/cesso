# cesso

A chess engine written in pure Rust.

## Components

### Core (cesso-core) — DONE

| # | Component | Status | Notes |
|---|-----------|--------|-------|
| 1 | Board representation | DONE | Bitboard-based, LERF mapping, copy-make |
| 2 | Piece & move representation | DONE | Bit-packed `Piece` (u8), `Move` (u16) |
| 3 | Attack tables | DONE | Magic bitboards for sliders, const tables for leapers |
| 4 | Move generation (+ legality) | DONE | Fully legal movegen with pin/check analysis |
| 5 | FEN parsing & serialization | DONE | `FromStr` / `Display` on `Board` |
| 6 | Perft | DONE | Verified against reference positions through depth 5 |

### Evaluation (cesso-engine) — IN PROGRESS

| # | Component | Status | Notes |
|---|-----------|--------|-------|
| 7 | Score type | DONE | Packed mg/eg `Score(i32)`, tapered eval ready |
| 8 | Game phase detection | DONE | Material-based phase in `[0, 24]` |
| 9 | Piece-square tables | TODO | Tapered PSTs (mg + eg per piece per square) |
| 10 | Material evaluation | TODO | Basic piece values via `Score` |
| 11 | Pawn structure | TODO | Passed, isolated, doubled, backward pawns |
| 12 | Piece evaluation | TODO | Mobility, outposts, rook on open files |
| 13 | King safety | TODO | Pawn shield, king tropism |

### Search (cesso-engine) — TODO

| # | Component | Status | Notes |
|---|-----------|--------|-------|
| 14 | Negamax + alpha-beta | TODO | Core search framework |
| 15 | Iterative deepening | TODO | Required for time management and `stop` |
| 16 | Move ordering | TODO | MVV-LVA, killer moves, history heuristic |
| 17 | Quiescence search | TODO | Capture-only search to avoid horizon effect |
| 18 | Transposition table | TODO | Zobrist hashing, TT probing/storing |
| 19 | Time management | TODO | Clock-based search budgeting |

### UCI Protocol (cesso-uci) — TODO

| # | Component | Status | Notes |
|---|-----------|--------|-------|
| 20 | Command parser | TODO | Parse `uci`, `position`, `go`, `stop`, `quit` |
| 21 | Response formatter | TODO | Format `id`, `uciok`, `bestmove`, `info` |
| 22 | UCI loop | TODO | stdin/stdout I/O, threading for `stop` |
| 23 | UCI move parsing | TODO | Map UCI strings to legal `Move` values |
| 24 | Wire up main binary | TODO | `src/main.rs` → `cesso_uci::run()` |

### Lichess Deployment — TODO

| # | Component | Status | Notes |
|---|-----------|--------|-------|
| 25 | lichess-bot integration | TODO | Drop binary into lichess-bot, configure `config.yml` |
| 26 | Opening book | TODO | Polyglot `.bin` support (handled by lichess-bot) |
| 27 | Endgame tablebases | TODO | Syzygy via lichess-bot or engine-native |
