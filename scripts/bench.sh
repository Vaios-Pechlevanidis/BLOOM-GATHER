#!/usr/bin/env bash
# Performance benchmarks.
#   1. Core rules (no window): placement, queries, harvesting, saves.
#   2. The real game (release build): a field of N flowers, a 12 s walk
#      through it, frame times measured. Uses a throwaway save folder.
# Usage: scripts/bench.sh [flower counts...]   (default 1000 10000 100000 500000)
#        BLOOM_BENCH_QUALITY=low|medium|high (default high)
set -euo pipefail
cd "$(dirname "$0")/.."
SIZES=("${@:-1000 10000 100000 500000}")
SIZES=(${SIZES[@]})
OUT="${TMPDIR:-/tmp}/bloom-bench"
rm -rf "$OUT"
mkdir -p "$OUT"
echo "== core (release)"
cargo run --release -q -p bloom_core --example bench "${SIZES[@]}"
echo "== game (release, quality ${BLOOM_BENCH_QUALITY:-high})"
cargo build --release -q
for n in "${SIZES[@]}"; do
  BLOOM_SAVE_DIR="$OUT/save-$n" BLOOM_BENCH="$n" ./target/release/bloom-and-gather \
    > "$OUT/game-$n.log" 2>&1 || { tail -20 "$OUT/game-$n.log"; exit 1; }
  grep -E "^BENCH" "$OUT/game-$n.log" || { tail -20 "$OUT/game-$n.log"; exit 1; }
done
