#!/bin/bash
# litebike client -> host sync & test script
# Auto-detect TERMUX_HOST, sync repo, and run remote test

# Auto-detect TERMUX_HOST
TERMUX_HOST=$(route get default 2>/dev/null | grep gateway | awk '{print $2}')
if [ -z "$TERMUX_HOST" ]; then
	TERMUX_HOST=$(ip route get 8.8.8.8 2>/dev/null | grep via | awk '{print $3}')
fi
if [ -z "$TERMUX_HOST" ]; then
	TERMUX_HOST="192.168.1.1"
fi

echo "Detected TERMUX_HOST: $TERMUX_HOST"

# Usage help
if [[ "$1" == "-h" || "$1" == "--help" ]]; then
	echo "Usage: $0 [remote-path] [remote-cmd]"
	echo "Syncs current repo to TERMUX_HOST and runs remote command."
	echo "remote-path: Path on host to sync to (default: ~/litebike-sync)"
	echo "remote-cmd: Command to run on host after sync (default: ls -l)"
	exit 0
fi

# Arguments
REMOTE_PATH="${1:-~/litebike-sync}"
REMOTE_CMD="${2:-ls -l}"

# Sync repo using rsync over SSH (TERMUX default port 8022)
echo "Syncing repo to $TERMUX_HOST:$REMOTE_PATH ..."
rsync -az --delete --exclude 'target/' --exclude '.git/' -e 'ssh -p 8022' ./ "$TERMUX_HOST:$REMOTE_PATH"
if [ $? -ne 0 ]; then
	echo "[ERROR] rsync failed. Check SSH connectivity and permissions."
	exit 1
fi

# Run remote command
echo "Running remote command: $REMOTE_CMD"
ssh -p 8022 "$TERMUX_HOST" "cd $REMOTE_PATH && $REMOTE_CMD"
if [ $? -ne 0 ]; then
	echo "[ERROR] Remote command failed."
	exit 2
fi

echo "[SUCCESS] Sync and remote test complete."
