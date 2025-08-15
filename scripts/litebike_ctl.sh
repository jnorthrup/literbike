#!/usr/bin/env bash
# litebike_ctl.sh - simple process control for scripts/litebike_sync.sh
# Usage: ./scripts/litebike_ctl.sh start|stop|status [--config path]

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SYNC_SCRIPT="$SCRIPT_DIR/litebike_sync.sh"
RUNDIR="$SCRIPT_DIR/.litebike"
PIDFILE="$RUNDIR/sync.pid"
LOGFILE="$RUNDIR/sync.log"
CONFIG=""

mkdir -p "$RUNDIR"

usage() {
  cat <<USAGE
Usage: $(basename "$0") start|stop|status [--config ./scripts/litebike_sync.conf]

Commands:
  start    Start the sync loop (writes PID file to $PIDFILE)
  stop     Stop the running sync loop
  status   Show running status and PID

Examples:
  $(basename "$0") start --config scripts/litebike_sync.conf
  $(basename "$0") stop
USAGE
}

parse_args() {
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --config)
        CONFIG="$2"; shift 2;;
      start|stop|status)
        CMD="$1"; shift;;
      -h|--help)
        usage; exit 0;;
      *)
        echo "Unknown arg: $1"; usage; exit 2;;
    esac
  done
}

start() {
  if [[ -f "$PIDFILE" ]]; then
    if kill -0 "$(cat "$PIDFILE")" 2>/dev/null; then
      echo "Sync already running with PID $(cat "$PIDFILE")"; exit 0
    else
      echo "Stale PID file found. Removing."; rm -f "$PIDFILE"
    fi
  fi
  echo "Starting litebike sync..."
  if [[ -n "$CONFIG" ]]; then
    nohup bash "$SYNC_SCRIPT" --config "$CONFIG" --loop > "$LOGFILE" 2>&1 &
  else
    nohup bash "$SYNC_SCRIPT" --loop > "$LOGFILE" 2>&1 &
  fi
  echo $! > "$PIDFILE"
  echo "Started (PID $(cat "$PIDFILE")). Logs: $LOGFILE"
}

stop() {
  if [[ ! -f "$PIDFILE" ]]; then
    echo "Not running (no PID file)."; exit 0
  fi
  pid=$(cat "$PIDFILE")
  echo "Stopping PID $pid"
  kill "$pid" 2>/dev/null || true
  # Wait up to 5s for shutdown
  for i in {1..5}; do
    if kill -0 "$pid" 2>/dev/null; then
      sleep 1
    else
      break
    fi
  done
  if kill -0 "$pid" 2>/dev/null; then
    echo "Force killing $pid"; kill -9 "$pid" 2>/dev/null || true
  fi
  rm -f "$PIDFILE"
  echo "Stopped."
}

status() {
  if [[ -f "$PIDFILE" ]]; then
    pid=$(cat "$PIDFILE")
    if kill -0 "$pid" 2>/dev/null; then
      echo "Running (PID $pid). Log: $LOGFILE"
      tail -n 10 "$LOGFILE" || true
    else
      echo "PID file exists but process $pid not running. Remove $PIDFILE if stale."
    fi
  else
    echo "Not running."
  fi
}

# main
CMD=""
parse_args "$@"
if [[ -z "$CMD" ]]; then
  usage; exit 2
fi
case "$CMD" in
  start) start;;
  stop) stop;;
  status) status;;
esac
