#!/bin/bash
# Vibe Keyboard — start/stop script
# Usage: ./run.sh [stop|daemon|gui|both]

set -e
cd "$(dirname "$0")"

MODE="${1:-both}"

stop_all() {
    echo "Stopping..."
    pkill -f "vk-daemon serve" 2>/dev/null && echo "  killed vk-daemon" || true
    pkill -f "vk-desktop" 2>/dev/null && echo "  killed vk-desktop" || true
    pkill -f "tauri dev" 2>/dev/null && echo "  killed tauri dev" || true
    pkill -f "vk-simulator" 2>/dev/null && echo "  killed vk-simulator" || true
    # Kill processes on our ports
    lsof -ti:15173 | xargs kill -9 2>/dev/null && echo "  freed port 15173" || true
    lsof -ti:19280 | xargs kill -9 2>/dev/null && echo "  freed port 19280" || true
    # Unstick modifier keys
    osascript -e 'tell application "System Events" to key up 63' 2>/dev/null || true
    echo "Done."
}

case "$MODE" in
  stop)
    stop_all
    exit 0
    ;;
esac

# Always stop old processes first
stop_all
sleep 1

echo "Building..."
cargo build -p vk-daemon 2>&1 | grep -E "Compiling|Finished|error" || true

case "$MODE" in
  daemon)
    echo "Starting daemon..."
    cargo run -p vk-daemon -- serve
    ;;
  gui)
    echo "Starting daemon + GUI..."
    cargo run -p vk-daemon -- serve 2>/dev/null &
    DAEMON_PID=$!
    sleep 3
    cd desktop && npm run tauri dev
    kill $DAEMON_PID 2>/dev/null
    ;;
  both|*)
    echo "Starting daemon + GUI..."
    cargo run -p vk-daemon -- serve 2>/dev/null &
    DAEMON_PID=$!
    sleep 3
    cd desktop && npm run tauri dev 2>/dev/null &
    GUI_PID=$!
    sleep 8
    echo "Ready. Sessions: $(curl -s http://localhost:19280/sessions | python3 -c 'import sys,json; print(len(json.load(sys.stdin)))' 2>/dev/null || echo '?')"
    echo "Ctrl+C to stop all"

    # Trap Ctrl+C and EXIT to kill both daemon and GUI
    cleanup() {
        echo ""
        echo "Stopping all processes..."
        kill $GUI_PID 2>/dev/null || true
        kill $DAEMON_PID 2>/dev/null || true
        # Wait briefly for graceful shutdown, then force
        sleep 1
        kill -9 $DAEMON_PID 2>/dev/null || true
        kill -9 $GUI_PID 2>/dev/null || true
        stop_all
    }
    trap cleanup INT TERM EXIT

    wait
    ;;
esac
