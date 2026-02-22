# cesso

A chess engine written in pure Rust.

## Features

- **Bitboard-based board representation** with LERF mapping and magic bitboards for sliding piece attacks
- **Fully legal move generation** with pin/check analysis, no pseudo-legal filtering needed
- **Principal Variation Search (PVS)** with iterative deepening and aspiration windows
- **Dual evaluation backends** — hand-crafted evaluation (HCE) or NNUE, selectable at compile time
- **Lazy SMP** multi-threaded search with a shared lockless transposition table
- **Dynamic time management** with stability-based scaling and easy-move detection
- **Full UCI protocol** support — compatible with any UCI-compliant GUI

## Architecture

The engine is organized as a Cargo workspace with three crates:

```
cesso
├── crates/
│   ├── cesso-core       Board, moves, attack tables, move generation
│   ├── cesso-engine      Search, evaluation (HCE + NNUE), time management
│   └── cesso-uci         UCI protocol parsing and engine loop
├── nets/                 NNUE network weights
├── train/                NNUE training script (Bullet)
└── src/main.rs           Binary entry point
```

### Board representation

The board uses 64-bit bitboards in a LERF (Little-Endian Rank-File) layout. Piece placements are stored as six kind-bitboards and two color-bitboards, with a cached occupancy bitboard. Moves are bit-packed into 16 bits (source, destination, promotion piece, move kind). Board updates follow the copy-make pattern — `make_move` returns a new `Board`, leaving the original untouched.

Sliding piece attacks (bishop, rook, queen) are resolved in O(1) through magic bitboard lookups. Leaper attacks (knight, king, pawn) use precomputed tables. Move generation produces only legal moves by computing check masks and pin rays upfront, so no generated move ever needs a separate legality test.

### Search

The search is a negamax alpha-beta with PVS refinement, wrapped in iterative deepening. Aspiration windows narrow the search bounds starting at depth 5, widening by 4x on fail-high/fail-low.

| Technique | Description |
|---|---|
| Null move pruning | Skip a turn to get a lower bound (R=2 or 3) |
| Late move reductions | Search late quiet moves at reduced depth |
| Late move pruning | Skip late quiet moves entirely at shallow depths |
| Futility pruning | Prune quiet moves when static eval + margin < alpha |
| Reverse futility pruning | Return static eval early when it exceeds beta by a margin |
| Check extensions | Extend search by 1 ply when in check |
| Quiescence search | Resolve captures and promotions past the horizon |
| Static exchange evaluation | Classify captures as winning/losing for ordering and pruning |

Move ordering follows: TT move, good captures (MVV-LVA + SEE), killer moves, history-scored quiets, bad captures. The transposition table is a lockless atomic structure with XOR integrity checks, generation-based replacement, and mate score adjustment.

Lazy SMP distributes search across multiple threads sharing a single transposition table — each thread runs the full search independently, and the shared TT provides implicit communication.

### Evaluation

Two evaluation backends are available behind feature flags (`--features hce` or `--features nnue`):

**Hand-crafted evaluation (HCE)** — the default — uses tapered midgame/endgame scoring across:
- Material balance with bishop pair and piece-count adjustments
- Piece-square tables
- Pawn structure (passed, isolated, doubled, backward, connected pawns)
- Piece mobility (safe squares for knights, bishops, rooks, queens)
- King safety (pawn shield, attacker zone danger, pawn storms, open files)
- Rook bonuses (open/semi-open files, 7th rank)
- Knight and bishop outposts

Game phase (0-24) is derived from non-pawn material and drives the taper between middlegame and endgame scores.

**NNUE** — a (768 &rarr; 128)x2 &rarr; 1 network with SCReLU activation, trained with [Bullet](https://github.com/jw1912/bullet) on Stockfish data. The 768 input features encode piece placements from both perspectives (Chess768 scheme). The accumulator supports incremental updates for fast inference during search.

### Time management

Time allocation is phase-aware: the engine estimates moves remaining from the game phase and distributes time accordingly. Soft and hard limits are computed separately — the soft limit controls when to stop between iterations, while the hard limit aborts mid-search.

Between iterations, a stability tracker adjusts the soft limit based on best-move consistency and score changes. If the best move has been stable for 5+ iterations with no score drop, the engine plays quickly (easy-move detection). Score drops widen the time budget. If only one legal move exists, the engine returns immediately.

## Building

```bash
# Default build (HCE evaluation)
cargo build --release

# Build with NNUE evaluation
cargo build --release --no-default-features --features nnue
```

## Usage

The engine communicates over the UCI protocol via stdin/stdout. Point any UCI-compatible chess GUI (Arena, CuteChess, etc.) at the binary.

```bash
./target/release/cesso
```

### UCI options

| Option | Type | Default | Range | Description |
|---|---|---|---|---|
| Hash | spin | 16 | 1 - 65536 | Transposition table size in MB |
| Threads | spin | 1 | 1 - 256 | Number of search threads |
| Ponder | check | false | — | Enable pondering |
