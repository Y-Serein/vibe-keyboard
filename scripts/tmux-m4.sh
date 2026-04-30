#!/usr/bin/env bash
# M4: Daemon Core — 全功能测试 (Layout C)
# Usage: ./scripts/tmux-m4.sh

set -euo pipefail
SESSION="vk-m4"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"

tmux kill-session -t "$SESSION" 2>/dev/null || true

# Pane 0 (top-left): daemon
tmux new-session -s "$SESSION" -d -c "$ROOT"
tmux send-keys -t "$SESSION" "cargo run -p vk-daemon -- serve --headless" Enter

# Pane 1 (top-right): transport listen
tmux split-window -h -t "$SESSION" -c "$ROOT"
tmux send-keys -t "$SESSION" "sleep 2 && cargo run -p vk-daemon -- transport listen" Enter

# Pane 2 (bottom-left): mock inject
tmux split-window -v -t "$SESSION.0" -c "$ROOT"
tmux send-keys -t "$SESSION.2" "echo '=== Mock Inject Pane ===' && echo 'Try: cargo run -p vk-daemon -- session mock'" Enter

# Layout labels
tmux select-pane -t "$SESSION.0" -T "daemon --headless"
tmux select-pane -t "$SESSION.1" -T "transport listen"
tmux select-pane -t "$SESSION.2" -T "mock inject"

tmux attach -t "$SESSION"
