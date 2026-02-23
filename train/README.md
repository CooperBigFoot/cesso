# cesso-train

NNUE trainer for cesso, built on [Bullet](https://github.com/jw1912/bullet).

## Network Architecture

```
(768 -> 1024)x2 -> 1x8
```

| Layer | Shape | Activation | Quantization |
|-------|-------|------------|--------------|
| L0 (feature) | 768 -> 1024, dual perspective | SCReLU (clamp [0,255], square) | QA = 255 (i16) |
| L1 (output) | 2048 -> 8 (one per bucket) | Linear | QB = 64 (i16, transposed) |

- **Input features**: Chess768 — 2 perspectives x 6 piece types x 64 squares
- **Output buckets**: 8, selected by `MaterialCount` (piece_count mapped to bucket via `(count - 2) / 4`)
- **Eval scale**: 400 (maps network output to centipawns)

### Binary layout (`nets/cesso-nnue-320.bin`, ~1.6 MB)

| Offset | Size | Contents |
|--------|------|----------|
| 0 | 1,572,864 B | `feature_weights` [768 x 1024 x i16] |
| 1,572,864 | 21,504 B | `feature_bias` [1024 x i16] (64-byte aligned) |
| 1,594,368 | 10,752 B | `output_weights` [8 x 2 x 1024 x i16] (transposed) |
| 1,605,120 | 2,624 B | `output_bias` [8 x i16] (padded) |

### Forward pass (inference)

```
screlu(x) = clamp(x, 0, QA)^2

output[bucket] = sum_i(screlu(white_acc[i]) * out_w[bucket][i])
               + sum_i(screlu(black_acc[i]) * out_w[bucket][1024+i])

eval = (output / QA + out_bias[bucket]) * SCALE / (QA * QB)
```

Perspective is flipped so `white_acc` = side-to-move accumulator.

## Training

### Hyperparameters

| Parameter | Value |
|-----------|-------|
| Optimiser | AdamW |
| Superbatches | 320 |
| Batch size | 16,384 |
| Batches/superbatch | 6,104 |
| LR schedule | Cosine decay 0.001 -> 0.000003 |
| WDL schedule | Linear 0.0 -> 0.5 |
| Save rate | Every 20 superbatches |
| Loss | Sigmoid squared error |

### Data

Training data: Stockfish `test80` binpacks (Jan-Jun 2024), `.min-v2.v6` format.

#### Position filter

Positions are filtered to keep only quiet, non-trivial training samples:

- `ply >= 16` (skip opening book noise)
- Side to move not in check
- `|score| <= 10000` cp
- Recorded move is a normal quiet move (no captures, promotions, or specials)

### Running

Requires CUDA. Place binpack files in `data/`, then:

```bash
cargo run --release
```

Checkpoints are saved to `checkpoints/` every 20 superbatches.

## Search Features (v0.1.44)

The engine using this network implements the following search techniques:

### Core

- **Negamax PVS** with aspiration windows (delta=50, 4x expansion)
- **Iterative deepening** with stability-based soft time scaling
- **Lazy SMP** with depth-offset divergence (odd helpers skip depth 1)
- **Lockless TT** (16 MB default, atomic XOR torn-write detection, 5-bit generation)

### Pruning

| Technique | Conditions | Details |
|-----------|-----------|---------|
| Razoring | depth <= 3, non-PV | margins [300, 550, 900] cp |
| Reverse Futility | depth 1-3, non-PV | margins [200, 450, 700] cp |
| Forward Futility | depth 1-3, move_count > 0 | margins [200, 450, 700] cp |
| Null Move | depth >= 3, has material, eval >= beta | R = 2 (depth<6) or 3; verification at depth > 12 |
| ProbCut | depth >= 7, non-PV | margin = beta + 344 cp |
| Late Move | depth 1-4 | thresholds [4, 7, 12, 19] (halved if not improving) |
| SEE (quiet) | depth <= 5 | prune if SEE < -59*depth |
| SEE (tactical) | depth <= 5 | prune if SEE < -27*depth^2 |
| History | depth <= 5, quiet | prune if hist < -2711*depth |

### Extensions

| Type | Condition | Effect |
|------|-----------|--------|
| Check | in check, ply < 127 | +1 |
| Singular | depth >= 8, TT move | +1 |
| Double | singular_score < beta - 23, cumulative < 16 | +2 |
| Negative (not singular) | tt_score >= beta | -3 |
| Negative (cutnode) | cutnode after SE fails | -2 |
| IIR | PV/cutnode, depth > 4, no TT move | -2 |

### LMR (Late Move Reductions)

Base reduction: `floor(0.76 + ln(move_index) * ln(depth) / 2.32)` in 1024ths.

Adjustments (1024ths): base offset -372, PV -1062, cutnode +1303, TT-PV -975, killer -932, history -hist/8.

### Move Ordering

TT move (100k) > queen promotions (30k) > good captures via MVV-LVA + SEE (10k+) > killers (9k) > quiets (history + cont_history) > bad captures (-50k+).

### Time Management

Dynamic allocation based on remaining time, increment, and game phase. Stability tracker scales soft limit: score drops extend (up to 2.5x), stable best moves reduce (down to 0.3x). Hard limit checked every 2048 nodes.
