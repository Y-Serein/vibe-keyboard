#!/bin/bash
# Vibe Keyboard — inject demo data into running daemon
# Usage: ./scripts/demo.sh
# Requires: daemon already running (./scripts/start.sh)

set -e
BASE="http://localhost:3456"

echo "=== Injecting demo data ==="

# Check daemon is running
if ! curl -s "$BASE/health" > /dev/null 2>&1; then
    echo "ERROR: daemon not running. Start it with: ./scripts/start.sh"
    exit 1
fi

echo "1. Creating 3 sessions..."
curl -s -X POST "$BASE/event" -H "Content-Type: application/json" \
  -d '{"type":"session_start","session_id":"demo-1","name":"RustAgent"}'
curl -s -X POST "$BASE/event" -H "Content-Type: application/json" \
  -d '{"type":"session_start","session_id":"demo-2","name":"FrontEnd"}'
curl -s -X POST "$BASE/event" -H "Content-Type: application/json" \
  -d '{"type":"session_start","session_id":"demo-3","name":"DevOps"}'

echo "2. Changing statuses..."
curl -s -X POST "$BASE/event" -H "Content-Type: application/json" \
  -d '{"type":"status","session_id":"demo-1","status":"thinking"}'
curl -s -X POST "$BASE/event" -H "Content-Type: application/json" \
  -d '{"type":"tool_use","session_id":"demo-2"}'

echo "3. Injecting permission request..."
curl -s -X POST "$BASE/event" -H "Content-Type: application/json" \
  -d '{"type":"permission_request","session_id":"demo-3","tool_name":"Bash","tool_input":"cargo build --release"}'

echo ""
echo "=== Done! ==="
echo "Sessions:"
curl -s "$BASE/sessions" | python3 -m json.tool 2>/dev/null || curl -s "$BASE/sessions"
echo ""
echo "Now go to the simulator and press Enter to Allow or Esc to Deny the permission."
