# data/

Chess data files for the cesso engine. This directory is **git-ignored** — contents must be downloaded locally.

## Directory Layout

```
data/
├── openings/          # Polyglot opening books (.bin)
└── tablebases/
    └── syzygy/        # Syzygy endgame tablebases (.rtbw, .rtbz)
```

## Opening Books (`openings/`)

14 Polyglot-format `.bin` files (~57 MB total). Each entry is a fixed 16-byte record:

| Offset | Size | Field |
|--------|------|-------|
| 0 | 8 bytes | Zobrist hash (u64, big-endian) |
| 8 | 2 bytes | Encoded move (u16, big-endian) |
| 10 | 2 bytes | Weight (u16, big-endian) |
| 12 | 4 bytes | Learn data (u32, unused by most engines) |

The move encoding packs `to_file`, `to_row`, `from_file`, `from_row`, and `promotion_piece` into 16 bits. See the [Polyglot book format spec](http://hgm.nubati.net/book_format.html) for bit layout details.

**Sources:** [SourceForge codekiddy-chess](https://sourceforge.net/projects/codekiddy-chess/files/Books/Polyglot%20books/), [GitHub free-opening-books](https://github.com/gmcheems-org/free-opening-books)

## Syzygy Tablebases (`tablebases/syzygy/`)

290 files (~974 MB total) covering all 3-4-5 piece endgame positions.

- `.rtbw` — **WDL** (Win/Draw/Loss) tables. Used during search to prune losing lines.
- `.rtbz` — **DTZ** (Distance To Zeroing move) tables. Used at the root to pick the fastest winning move.

File naming convention: `K[pieces]vK[pieces].rtb{w,z}` — e.g., `KRPvKR.rtbw` is King+Rook+Pawn vs King+Rook.

For probing these in Rust, consider [Fathom](https://github.com/jdart1/Fathom) (C library, FFI-friendly) or a pure Rust port.

**Source:** <http://tablebase.sesse.net/syzygy/3-4-5/>

## Downloading

Opening books and tablebases must be downloaded manually. See the commands used to populate this directory in the project history, or re-run:

```bash
# Syzygy tablebases
wget -e robots=off -c -r -np -nH --cut-dirs=2 \
  -P data/tablebases/syzygy -R "index.html*" -A "*.rtbw,*.rtbz" \
  http://tablebase.sesse.net/syzygy/3-4-5/

# Opening books (SourceForge — requires p7zip to extract)
curl -L -o data/openings/polyglot-collection.7z \
  "https://sourceforge.net/projects/codekiddy-chess/files/Books/Polyglot%20books/Update1/polyglot-collection.7z/download"
7z x data/openings/polyglot-collection.7z -o./data/openings/
```
