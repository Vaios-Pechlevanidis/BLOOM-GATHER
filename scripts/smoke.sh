#!/usr/bin/env bash
# Real-game smoke test in two runs:
#   1. fresh save: walk, pick flowers with E, open the basket, save
#   2. restart:    Continue, verify picked flowers stayed picked
# Uses a private save folder, never the player's real save.
set -euo pipefail
cd "$(dirname "$0")/.."
OUT="${1:-${TMPDIR:-/tmp}/bloom-smoke}"
rm -rf "$OUT"
mkdir -p "$OUT"
cargo build
echo "== run 1: play and save"
BLOOM_SMOKE_TEST="$OUT" ./target/debug/bloom-and-gather > "$OUT/run1.log" 2>&1 || { tail -40 "$OUT/run1.log"; exit 1; }
grep -E "SMOKE (ok|FAIL|PASSED|WARN)" "$OUT/run1.log" | sed -E 's/.*SMOKE/SMOKE/'
echo "== run 2: restart and verify persistence"
BLOOM_SMOKE_TEST="$OUT" BLOOM_SMOKE_RESUME=1 ./target/debug/bloom-and-gather > "$OUT/run2.log" 2>&1 || { tail -40 "$OUT/run2.log"; exit 1; }
grep -E "SMOKE (ok|FAIL|PASSED|WARN)" "$OUT/run2.log" | sed -E 's/.*SMOKE/SMOKE/'
echo "Screenshots in $OUT"
