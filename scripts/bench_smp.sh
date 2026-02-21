#!/usr/bin/env bash
# Benchmark Lazy SMP scaling across thread counts.
#
# Runs depth-8 searches on 3 positions with 1, 2, 4, 8 threads.
# Extracts NPS from the last UCI `info` line per search.
#
# Usage: ./scripts/bench_smp.sh
#
# Prerequisites: cargo build --release

set -euo pipefail

BINARY="./target/release/cesso"

if [[ ! -f "$BINARY" ]]; then
    echo "Building release binary..."
    cargo build --release
fi

POSITIONS=(
    "startpos"
    "fen r1bqkbnr/pppppppp/2n5/8/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 1 2"
    "fen r1bqk2r/pppp1ppp/2n2n2/2b1p3/2B1P3/5N2/PPPP1PPP/RNBQK2R w KQkq - 4 4"
)

THREADS=(1 2 4 8)
DEPTH=8

printf "%-12s" "Position"
for t in "${THREADS[@]}"; do
    printf "%-15s" "${t}T NPS"
done
echo ""
printf '%0.s-' {1..72}
echo ""

for i in "${!POSITIONS[@]}"; do
    pos="${POSITIONS[$i]}"
    printf "%-12s" "pos_$((i+1))"

    for t in "${THREADS[@]}"; do
        # Send UCI commands via stdin, capture output
        output=$(printf "uci\nsetoption name Threads value %d\nsetoption name Hash value 64\nisready\nposition %s\ngo depth %d\nquit\n" "$t" "$pos" "$DEPTH" | timeout 60 "$BINARY" 2>/dev/null)

        # Extract NPS from the last info line
        nps=$(echo "$output" | grep "^info " | tail -1 | grep -oP 'nps \K[0-9]+' || echo "N/A")
        printf "%-15s" "$nps"
    done
    echo ""
done

echo ""
echo "Done."
