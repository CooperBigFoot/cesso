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

### Evaluation (cesso-engine) — DONE

| # | Component | Status | Notes |
|---|-----------|--------|-------|
| 7 | Score type | DONE | Packed mg/eg `Score(i32)`, tapered eval ready |
| 8 | Game phase detection | DONE | Material-based phase in `[0, 24]` |
| 9 | Piece-square tables | DONE | PeSTO-style tapered PSTs for all 6 piece types |
| 10 | Material evaluation | DONE | Piece values + bishop pair bonus |
| 11 | Pawn structure | DONE | Passed, isolated, doubled, backward pawns |
| 12 | Piece mobility | DONE | Safe-square counting for N/B/R/Q |
| 13 | King safety | DONE | Pawn shield (V1) |

### Search (cesso-engine) — IN PROGRESS

| # | Component | Status | Notes |
|---|-----------|--------|-------|
| 14 | Negamax + alpha-beta | DONE | Core search with beta cutoffs |
| 15 | Iterative deepening | DONE | Depth 1..N with per-iteration callback |
| 16 | Move ordering | DONE | MVV-LVA via `MovePicker` selection sort |
| 17 | Quiescence search | DONE | Captures + promotions, stand-pat, ply ceiling |
| 18 | Transposition table | TODO | Zobrist hashing, TT probing/storing |
| 19 | Time management | TODO | Clock-based search budgeting |

### UCI Protocol (cesso-uci) — DONE

| # | Component | Status | Notes |
|---|-----------|--------|-------|
| 20 | Command parser | DONE | `position`, `go depth`, `stop`, `quit`, etc. |
| 21 | Response formatter | DONE | `id`, `uciok`, `readyok`, `bestmove`, `info` lines |
| 22 | UCI loop | DONE | stdin line reader, dispatches to handlers |
| 23 | UCI move parsing | DONE | `Move::from_uci` with castling/EP/promotion disambiguation |
| 24 | Wire up main binary | DONE | `src/main.rs` → `UciEngine::new().run()` |

### Lichess Deployment — TODO

| # | Component | Status | Notes |
|---|-----------|--------|-------|
| 25 | lichess-bot integration | TODO | Drop binary into lichess-bot, configure `config.yml` |
| 26 | Opening book | TODO | Polyglot `.bin` support (handled by lichess-bot) |
| 27 | Endgame tablebases | TODO | Syzygy via lichess-bot or engine-native |
