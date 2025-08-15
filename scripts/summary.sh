#!/usr/bin/env bash
# summary.sh - produce a compact summary of latest results in results/ directory
# Usage: ./scripts/summary.sh [results_dir]

set -euo pipefail
RESULTS_DIR="${1:-./results}"

if [[ ! -d "$RESULTS_DIR" ]]; then
  echo "No results directory at $RESULTS_DIR"
  exit 1
fi

printf "%-30s %-24s %-12s %-8s %s\n" "HOST" "TIMESTAMP/COMMIT" "BUILD" "TEST" "PATH"
printf '%0.s-' {1..100}; echo

for hostdir in "$RESULTS_DIR"/*; do
  [[ -d "$hostdir" ]] || continue
  host=$(basename "$hostdir")
  latest=$(ls -1t "$hostdir" 2>/dev/null | head -n1)
  [[ -n "$latest" ]] || continue
  entry="$hostdir/$latest"
  build_status="missing"
  test_status="missing"
  if [[ -f "$entry/build.log" ]]; then
    if grep -q "error\|failed\|FAILED" "$entry/build.log"; then
      build_status="fail"
    else
      build_status="ok"
    fi
  fi
  if [[ -f "$entry/test.log" ]]; then
    if grep -q "FAILED\|fail" "$entry/test.log"; then
      test_status="fail"
    else
      test_status="ok"
    fi
  fi
  commit="unknown"
  if [[ -f "$entry/commit.txt" ]]; then
    commit=$(cat "$entry/commit.txt" | tr -d "\n")
  fi
  printf "%-30s %-24s %-12s %-8s %s\n" "$host" "$latest/$commit" "$build_status" "$test_status" "$entry"
done
