#!/usr/bin/env bash
# M5: E2E Integration — 全链路测试 (Layout C, 4 panes)
# Usage: ./scripts/tmux-m5.sh

set -euo pipefail
SESSION="vk-m5"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"

tmux kill-session -t "$SESSION" 2>/dev/null || true

# Pane 0 (top-left): daemon
tmux new-session -s "$SESSION" -d -c "$ROOT"
tmux send-keys -t "$SESSION" "cargo run -p vk-daemon -- serve --headless" Enter

# Pane 1 (top-right): simulator
tmux split-window -h -t "$SESSION" -c "$ROOT"
tmux send-keys -t "$SESSION" "sleep 2 && cargo run -p vk-simulator -- --cli" Enter

# Pane 2 (bottom-left): mock inject
tmux split-window -v -t "$SESSION.0" -c "$ROOT"
tmux send-keys -t "$SESSION.2" "echo '=== Mock Inject Pane ===' && echo 'Try: cargo run -p vk-daemon -- session mock'" Enter

# Pane 3 (bottom-right): transport listen
tmux split-window -v -t "$SESSION.1" -c "$ROOT"
tmux send-keys -t "$SESSION.3" "sleep 2 && cargo run -p vk-daemon -- transport listen" Enter

# Layout labels
tmux select-pane -t "$SESSION.0" -T "daemon --headless"
tmux select-pane -t "$SESSION.1" -T "simulator --cli"
tmux select-pane -t "$SESSION.2" -T "mock inject"
tmux select-pane -t "$SESSION.3" -T "transport listen"

tmux attach -t "$SESSION"
