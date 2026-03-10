#!/usr/bin/env bash
# check_rfc_trace.sh — verify RFC anchor coverage in QUIC protocol source files.
#
# Counts:
#   - RFC-TRACE occurrences (canonical tagged format)
#   - RFC 9 occurrences (covers "RFC 9000", "RFC 9001", etc. — bare comment style)
#
# Exits 0 if the total combined anchor count across all three files is >= 30.
# Exits 1 otherwise.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

FILES=(
  "src/quic/quic_protocol.rs"
  "src/quic/quic_engine.rs"
  "src/quic/quic_server.rs"
)

TOTAL_ANCHORS=0

# Print header
printf "%-40s  %12s  %8s  %8s\n" "File" "RFC-TRACE" "RFC 9" "Combined"
printf "%-40s  %12s  %8s  %8s\n" "----------------------------------------" "------------" "--------" "--------"

for rel in "${FILES[@]}"; do
  f="$REPO_ROOT/$rel"
  if [[ ! -f "$f" ]]; then
    printf "%-40s  %12s  %8s  %8s\n" "$rel" "MISSING" "MISSING" "MISSING"
    continue
  fi

  rfc_trace=$(grep -c 'RFC-TRACE' "$f" || true)
  rfc_nine=$(grep -c 'RFC 9' "$f" || true)
  combined=$(( rfc_trace + rfc_nine ))
  TOTAL_ANCHORS=$(( TOTAL_ANCHORS + combined ))

  printf "%-40s  %12d  %8d  %8d\n" "$rel" "$rfc_trace" "$rfc_nine" "$combined"
done

printf "%-40s  %12s  %8s  %8s\n" "----------------------------------------" "------------" "--------" "--------"
printf "%-40s  %12s  %8s  %8d\n" "TOTAL" "" "" "$TOTAL_ANCHORS"
echo

THRESHOLD=30
if (( TOTAL_ANCHORS >= THRESHOLD )); then
  echo "PASS: total anchor count $TOTAL_ANCHORS >= $THRESHOLD"
  exit 0
else
  echo "FAIL: total anchor count $TOTAL_ANCHORS < $THRESHOLD"
  exit 1
fi
