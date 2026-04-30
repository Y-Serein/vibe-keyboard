#!/usr/bin/env bash
# Dev: 日常开发环境 — daemon + simulator + log
# Usage: ./scripts/tmux-dev.sh

set -euo pipefail
SESSION="vk-dev"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"

tmux kill-session -t "$SESSION" 2>/dev/null || true

# Pane 0 (top-left): daemon
tmux new-session -s "$SESSION" -d -c "$ROOT"
tmux send-keys -t "$SESSION" "cargo run -p vk-daemon -- serve --headless" Enter

# Pane 1 (top-right): simulator
tmux split-window -h -t "$SESSION" -c "$ROOT"
tmux send-keys -t "$SESSION" "sleep 2 && cargo run -p vk-simulator -- --cli" Enter

# Pane 2 (bottom): shell for ad-hoc commands
tmux split-window -v -t "$SESSION.0" -c "$ROOT" -p 30
tmux send-keys -t "$SESSION.2" "echo '=== Dev Shell — run any command here ==='" Enter

# Layout labels
tmux select-pane -t "$SESSION.0" -T "daemon"
tmux select-pane -t "$SESSION.1" -T "simulator"
tmux select-pane -t "$SESSION.2" -T "shell"

# Focus on simulator pane
tmux select-pane -t "$SESSION.1"

tmux attach -t "$SESSION"
