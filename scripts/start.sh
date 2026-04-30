#!/bin/bash
# Vibe Keyboard — one-click tmux start
# Usage: ./scripts/start.sh

set -e
cd "$(dirname "$0")/.."

# Build first
echo "Building..."
cargo build -p vk-daemon -p vk-simulator --release 2>&1 | tail -3

BIN_DIR="target/release"

# Kill any existing session
tmux kill-session -t vk 2>/dev/null || true
pkill -f "vk-daemon serve" 2>/dev/null || true
sleep 0.5

# Create tmux layout:
# ┌──────────────────┬──────────────────┐
# │  daemon serve    │  simulator CLI   │
# │  (logs)          │  (LCD + keys)    │
# ├──────────────────┤                  │
# │  command pane    │                  │
# │  (curl / test)   │                  │
# └──────────────────┴──────────────────┘

tmux new-session -d -s vk -x 200 -y 50

# Pane 0: daemon
tmux send-keys -t vk "RUST_LOG=info $BIN_DIR/vk-daemon serve --headless" Enter

# Pane 1: simulator (right side)
tmux split-window -h -t vk
sleep 2  # wait for daemon to start IPC listener
tmux send-keys -t vk.1 "$BIN_DIR/vk-simulator --cli" Enter

# Pane 2: command pane (bottom left)
tmux split-window -v -t vk.0
tmux send-keys -t vk.2 "# Command pane — paste curl commands here" Enter
tmux send-keys -t vk.2 "# Example:" Enter
tmux send-keys -t vk.2 "# curl -X POST http://localhost:3456/event -H 'Content-Type: application/json' -d '{\"type\":\"session_start\",\"session_id\":\"s1\",\"name\":\"Agent1\"}'" Enter

# Focus command pane
tmux select-pane -t vk.2

echo ""
echo "=== Vibe Keyboard started ==="
echo ""
echo "Attach:  tmux attach -t vk"
echo ""
echo "Layout:"
echo "  Top-left:     daemon (logs)"
echo "  Top-right:    simulator (LCD)"
echo "  Bottom-left:  command pane"
echo ""
echo "Quick test commands (paste in command pane):"
echo ""
echo '  curl -X POST http://localhost:3456/event -H "Content-Type: application/json" -d '"'"'{"type":"session_start","session_id":"s1","name":"Agent1"}'"'"
echo '  curl -X POST http://localhost:3456/event -H "Content-Type: application/json" -d '"'"'{"type":"permission_request","session_id":"s1","tool_name":"Write","tool_input":"main.rs"}'"'"
echo '  curl http://localhost:3456/sessions'
echo ""
echo "In simulator: Enter=Allow, Esc=Deny, q=Quit"
echo ""

tmux attach -t vk
