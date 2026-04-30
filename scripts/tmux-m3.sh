#!/usr/bin/env bash
# M3: Simulator CLI — 双进程联调 (Layout B)
# Usage: ./scripts/tmux-m3.sh

set -euo pipefail
SESSION="vk-m3"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"

tmux kill-session -t "$SESSION" 2>/dev/null || true

# Pane 0: daemon (headless)
tmux new-session -s "$SESSION" -d -c "$ROOT"
tmux send-keys -t "$SESSION" "cargo run -p vk-daemon -- serve --headless" Enter

# Pane 1: simulator (CLI mode)
tmux split-window -h -t "$SESSION" -c "$ROOT"
tmux send-keys -t "$SESSION" "sleep 2 && cargo run -p vk-simulator -- --cli" Enter

# Layout labels
tmux select-pane -t "$SESSION.0" -T "daemon --headless"
tmux select-pane -t "$SESSION.1" -T "simulator --cli"

tmux attach -t "$SESSION"
