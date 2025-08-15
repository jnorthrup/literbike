#!/data/data/com.termux/files/usr/bin/bash
# TERMUX connection script with protocol selection

TERMUX_HOST="${TERMUX_HOST:-192.168.1.100}"
TERMUX_PORT="${TERMUX_PORT:-8022}"
TERMUX_USER="${TERMUX_USER:-u0_a471}"
PROTOCOL="${1:-ssh}" # ssh, telnet, raw

case "$PROTOCOL" in
    "ssh")
        echo "🔗 Connecting via SSH to $TERMUX_USER@$TERMUX_HOST:$TERMUX_PORT"
        ssh -p "$TERMUX_PORT" "$TERMUX_USER@$TERMUX_HOST"
        ;;
    "telnet")
        echo "🔗 Connecting via Telnet to $TERMUX_HOST:$TERMUX_PORT"
        telnet "$TERMUX_HOST" "$TERMUX_PORT"
        ;;
    "raw")
        echo "🔗 Raw TCP connection to $TERMUX_HOST:$TERMUX_PORT"
        ./litebike raw-connect "$TERMUX_HOST:$TERMUX_PORT"
        ;;
    *)
        echo "Usage: $0 {ssh|telnet|raw}"
        echo "Environment: TERMUX_HOST, TERMUX_PORT, TERMUX_USER"
        exit 1
        ;;
esac