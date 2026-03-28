#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

if [[ ! -f .env ]]; then
  echo "Missing .env in repo root."
  exit 1
fi

if ! command -v http >/dev/null 2>&1; then
  echo "httpie is not installed or not on PATH."
  exit 1
fi

set -a
source .env
set +a

PORT="${PORT:-3000}"
NGROK_API_HOST="127.0.0.1:4040"

ngrok http "$PORT" >/tmp/ngrok.log 2>&1 &
NGROK_PID=$!

cleanup() {
  if kill -0 "$NGROK_PID" >/dev/null 2>&1; then
    kill "$NGROK_PID" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT INT TERM

URL=""
for _ in {1..30}; do
  RESPONSE="$(http --check-status --body GET "http://${NGROK_API_HOST}/api/tunnels" 2>/dev/null || true)"
  URL="$(
    printf '%s' "$RESPONSE" | python3 -c '
import json, sys
try:
    data = json.load(sys.stdin)
except Exception:
    sys.exit(0)
for tunnel in data.get("tunnels", []):
    public_url = tunnel.get("public_url", "")
    if public_url.startswith("https://"):
        print(public_url)
        break
'
  )"
  if [[ -n "$URL" ]]; then
    break
  fi
  sleep 0.5
done

if [[ -z "$URL" ]]; then
  echo "Failed to read ngrok public URL from ${NGROK_API_HOST}."
  exit 1
fi

export WEBHOOK_BASE_URL="$URL"

echo "Ngrok URL: $WEBHOOK_BASE_URL"
echo "Starting cargo run"
echo

cargo run
