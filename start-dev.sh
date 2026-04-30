#!/bin/bash
# Vibe Keyboard - WSL2 dev launcher
# Starts: vk-daemon + vite dev + vk-desktop (Tauri)
# Tuned for: no node_modules/.bin (--no-bin-links), DrvFS workspace (CARGO_TARGET_DIR=/tmp).
# Usage:    bash start-dev.sh

set -u
cd "$(dirname "$0")"
ROOT="$(pwd)"
LOG_DIR="/tmp/vk-logs"
mkdir -p "$LOG_DIR"

DAEMON_PID=""
VITE_PID=""

cleanup() {
    echo ""
    echo "==> Stopping background processes..."
    [ -n "$VITE_PID" ]   && kill "$VITE_PID"   2>/dev/null || true
    [ -n "$DAEMON_PID" ] && kill "$DAEMON_PID" 2>/dev/null || true
    sleep 1
    [ -n "$VITE_PID" ]   && kill -9 "$VITE_PID"   2>/dev/null || true
    [ -n "$DAEMON_PID" ] && kill -9 "$DAEMON_PID" 2>/dev/null || true
    pkill -f "vk-daemon serve"          2>/dev/null || true
    pkill -f "node.*vite/bin/vite.js"   2>/dev/null || true
    rm -f /tmp/vk-daemon.sock
    echo "    done."
}
trap cleanup INT TERM EXIT

# Pre-clean stale procs/sockets from a previous crashed run.
pkill -f "vk-daemon serve"          2>/dev/null || true
pkill -f "node.*vite/bin/vite.js"   2>/dev/null || true
rm -f /tmp/vk-daemon.sock

echo "==> 1/3 vk-daemon  (log: $LOG_DIR/daemon.log)"
cargo run -p vk-daemon -- serve --headless >"$LOG_DIR/daemon.log" 2>&1 &
DAEMON_PID=$!

printf "    waiting for http://127.0.0.1:19280 "
for i in $(seq 1 60); do
    if curl --noproxy '*' -s -o /dev/null http://127.0.0.1:19280/health; then
        echo " ok"
        break
    fi
    if ! kill -0 "$DAEMON_PID" 2>/dev/null; then
        echo ""
        echo "!! daemon died early. Last log lines:"
        tail -n 30 "$LOG_DIR/daemon.log"
        exit 1
    fi
    printf "."
    sleep 1
done

echo "==> 2/3 vite dev   (log: $LOG_DIR/vite.log)"
( cd desktop && node node_modules/vite/bin/vite.js >"$LOG_DIR/vite.log" 2>&1 ) &
VITE_PID=$!

printf "    waiting for http://localhost:15173 "
for i in $(seq 1 60); do
    if curl -s -o /dev/null http://localhost:15173/; then
        echo " ok"
        break
    fi
    if ! kill -0 "$VITE_PID" 2>/dev/null; then
        echo ""
        echo "!! vite died early. Last log lines:"
        tail -n 30 "$LOG_DIR/vite.log"
        exit 1
    fi
    printf "."
    sleep 1
done

echo "==> 3/3 vk-desktop (foreground — close window or Ctrl+C to stop everything)"
echo ""
CARGO_TARGET_DIR=/tmp/vk-target cargo run -p vk-desktop
