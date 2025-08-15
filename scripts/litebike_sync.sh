#!/usr/bin/env bash
# litebike_sync.sh: Host/client-aware automation for SSH git updates, file watching/polling, and result collection.
#
# Features:
# - Multiple hosts (comma-separated)
# - Config/env-driven (sourceable .conf with variables)
# - Remote OS-aware watcher with fallbacks (fswatch/inotifywait) or polling loop
# - Structured results per host and timestamp
# - Optional continuous loop
#
# Quickstart examples:
#   ./scripts/litebike_sync.sh --host user@myserver --path /home/user/litebike --results ./results
#   ./scripts/litebike_sync.sh --config ./scripts/litebike_sync.conf --loop --interval 5
#
# Config file keys (bash-style):
#   LITEBIKE_HOSTS="user@h1,user@h2"   LITEBIKE_REMOTE_PATH="/path/to/repo"   LITEBIKE_BRANCH="master"
#   LITEBIKE_RESULTS_DIR="./results"    LITEBIKE_SSH_PORT=22                   LITEBIKE_WATCH=true

set -euo pipefail

SCRIPT_NAME=$(basename "$0")

# Defaults
LITEBIKE_HOSTS=${LITEBIKE_HOSTS:-}
LITEBIKE_REMOTE_PATH=${LITEBIKE_REMOTE_PATH:-}
LITEBIKE_BRANCH=${LITEBIKE_BRANCH:-master}
LITEBIKE_RESULTS_DIR=${LITEBIKE_RESULTS_DIR:-"./results"}
LITEBIKE_SSH_PORT=${LITEBIKE_SSH_PORT:-22}
LITEBIKE_WATCH=${LITEBIKE_WATCH:-false}
LITEBIKE_LOOP=${LITEBIKE_LOOP:-false}
LITEBIKE_INTERVAL=${LITEBIKE_INTERVAL:-5}

CONFIG_FILE=""

usage() {
  cat <<USAGE
Usage: $SCRIPT_NAME [--host user@h[,user@h2]] --path /remote/repo [options]

Options:
  --host H[,H2]        Comma-separated SSH hosts (e.g., user@host,host2) [env: LITEBIKE_HOSTS]
  --path PATH          Remote repository path [env: LITEBIKE_REMOTE_PATH]
  --branch BRANCH      Git branch to track (default: $LITEBIKE_BRANCH)
  --results DIR        Local results dir (default: $LITEBIKE_RESULTS_DIR)
  --port N             SSH port (default: $LITEBIKE_SSH_PORT)
  --watch              Use remote watchers (fswatch/inotifywait); fallback to polling
  --loop               Run continuously (polling or watch). Without --loop runs once.
  --interval N         Poll interval seconds (default: $LITEBIKE_INTERVAL)
  --config FILE        Source bash-style config to set vars above
  -h|--help            Show this help

Examples:
  $SCRIPT_NAME --host user@server --path /home/user/litebike --results ./results
  LITEBIKE_HOSTS=user@server LITEBIKE_REMOTE_PATH=/srv/litebike $SCRIPT_NAME --loop --interval 10
  $SCRIPT_NAME --config ./scripts/litebike_sync.conf --watch --loop
USAGE
}

parse_args() {
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --host)
        LITEBIKE_HOSTS="$2"; shift 2;;
      --path)
        LITEBIKE_REMOTE_PATH="$2"; shift 2;;
      --branch)
        LITEBIKE_BRANCH="$2"; shift 2;;
      --results)
        LITEBIKE_RESULTS_DIR="$2"; shift 2;;
      --port)
        LITEBIKE_SSH_PORT="$2"; shift 2;;
      --watch)
        LITEBIKE_WATCH=true; shift;;
      --loop)
        LITEBIKE_LOOP=true; shift;;
      --interval)
        LITEBIKE_INTERVAL="$2"; shift 2;;
      --config)
        CONFIG_FILE="$2"; shift 2;;
      -h|--help)
        usage; exit 0;;
      *)
        echo "Unknown argument: $1" >&2; usage; exit 2;;
    esac
  done
}

load_config() {
  if [[ -n "$CONFIG_FILE" ]]; then
    if [[ -f "$CONFIG_FILE" ]]; then
      # shellcheck disable=SC1090
      source "$CONFIG_FILE"
    else
      echo "Config file not found: $CONFIG_FILE" >&2; exit 2
    fi
  fi
}

ensure_prereqs() {
  mkdir -p "$LITEBIKE_RESULTS_DIR"
}

remote() { # $1 host, rest: cmd
  local host="$1"; shift
  ssh -p "$LITEBIKE_SSH_PORT" -o BatchMode=yes -o StrictHostKeyChecking=no "$host" "$@"
}

remote_has() { # $1 host, $2 cmd
  local host="$1"; shift
  remote "$host" "command -v $1 >/dev/null 2>&1" && return 0 || return 1
}

remote_os() { # $1 host -> prints linux/macos/other
  local host="$1"
  local uname_out
  uname_out=$(remote "$host" "uname -s" 2>/dev/null || echo unknown)
  case "$uname_out" in
    Darwin) echo macos;;
    Linux) echo linux;;
    *) echo other;;
  esac
}

git_update() { # $1 host
  local host="$1"
  remote "$host" "git -C '$LITEBIKE_REMOTE_PATH' fetch --all --prune && \
                    git -C '$LITEBIKE_REMOTE_PATH' checkout '$LITEBIKE_BRANCH' && \
                    git -C '$LITEBIKE_REMOTE_PATH' reset --hard origin/'$LITEBIKE_BRANCH'"
}

git_head() { # $1 host -> prints commit
  local host="$1"
  remote "$host" "git -C '$LITEBIKE_REMOTE_PATH' rev-parse HEAD" 2>/dev/null || echo "unknown"
}

build_and_test() { # $1 host -> emits log files remotely
  local host="$1"
  local ts commit
  ts=$(date -u +%Y%m%dT%H%M%SZ)
  commit=$(git_head "$host")
  local rdir="$LITEBIKE_REMOTE_PATH/.litebike_logs/$ts-$commit"
  remote "$host" "mkdir -p '$rdir' && cd '$LITEBIKE_REMOTE_PATH' && \
                    (cargo build --locked --all-targets > '$rdir/build.log' 2>&1 || true) && \
                    (cargo test --all -- --nocapture > '$rdir/test.log' 2>&1 || true) && \
                    echo '$commit' > '$rdir/commit.txt'"
  echo "$rdir"
}

sync_results() { # $1 host, $2 remote_results_dir
  local host="$1"; local rdir="$2"
  local ts_base
  ts_base=$(basename "$rdir")
  local ldir="$LITEBIKE_RESULTS_DIR/$host/$ts_base"
  mkdir -p "$ldir"
  scp -P "$LITEBIKE_SSH_PORT" -o StrictHostKeyChecking=no \
    "$host:$rdir/build.log" "$host:$rdir/test.log" "$host:$rdir/commit.txt" "$ldir/" >/dev/null 2>&1 || true
  echo "$ldir"
}

watch_once() { # $1 host -> return when change detected
  local host="$1"
  local os
  os=$(remote_os "$host")
  if [[ "$LITEBIKE_WATCH" == true ]]; then
    if [[ "$os" == macos ]] && remote_has "$host" fswatch; then
      remote "$host" "fswatch -1 '$LITEBIKE_REMOTE_PATH' >/dev/null"
      return 0
    elif [[ "$os" == linux ]] && remote_has "$host" inotifywait; then
      remote "$host" "inotifywait -q -r -e modify,move,create,delete '$LITEBIKE_REMOTE_PATH' -t $((LITEBIKE_INTERVAL*60))" || true
      return 0
    fi
  fi
  # Fallback: polling on HEAD
  local before after
  before=$(git_head "$host")
  sleep "$LITEBIKE_INTERVAL"
  after=$(git_head "$host")
  if [[ "$before" != "$after" ]]; then return 0; else return 1; fi
}

run_once_for_host() { # $1 host
  local host="$1"
  echo "[$host] Updating repo (branch=$LITEBIKE_BRANCH)"
  git_update "$host" || echo "[$host] git update failed (continuing)"
  echo "[$host] Building & testing"
  local rdir
  rdir=$(build_and_test "$host")
  echo "[$host] Syncing results"
  local ldir
  ldir=$(sync_results "$host" "$rdir")
  echo "[$host] Results: $ldir"
}

run_loop_for_host() { # $1 host
  local host="$1"
  echo "[$host] Starting loop (interval=${LITEBIKE_INTERVAL}s, watch=$LITEBIKE_WATCH)"
  local last_head=""
  while true; do
    git_update "$host" || true
    local head
    head=$(git_head "$host")
    if [[ "$head" != "$last_head" ]]; then
      run_once_for_host "$host"
      last_head="$head"
    fi
    watch_once "$host" || sleep "$LITEBIKE_INTERVAL"
  done
}

main() {
  parse_args "$@"
  load_config
  if [[ -z "${LITEBIKE_HOSTS}" || -z "${LITEBIKE_REMOTE_PATH}" ]]; then
    echo "Error: host and path are required." >&2
    usage; exit 2
  fi
  ensure_prereqs
  IFS=',' read -r -a HOST_ARR <<<"$LITEBIKE_HOSTS"
  for host in "${HOST_ARR[@]}"; do
    if [[ "$LITEBIKE_LOOP" == true ]]; then
      # Run loops sequentially in background to allow multiple hosts
      run_loop_for_host "$host" &
    else
      run_once_for_host "$host"
    fi
  done
  if [[ "$LITEBIKE_LOOP" == true ]]; then
    wait
  fi
}

main "$@"
