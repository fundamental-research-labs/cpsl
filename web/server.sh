#!/usr/bin/env bash
set -euo pipefail

WEB_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DIST_DIR="$WEB_DIR/dist"
HOST="${HOST:-127.0.0.1}"
PORT="${PORT:-8000}"

if ! command -v python3 >/dev/null 2>&1; then
  echo "error: python3 is required to serve the CPSL web site locally" >&2
  exit 1
fi

if [[ ! -f "$DIST_DIR/index.html" ]]; then
  echo "web/dist is missing; building a static preview first" >&2
  CPSL_SKIP_WASM="${CPSL_SKIP_WASM:-1}" "$WEB_DIR/build.sh"
fi

echo "Serving CPSL web site at http://$HOST:$PORT"
echo "Press Ctrl-C to stop."
cd "$DIST_DIR"
exec python3 -m http.server "$PORT" --bind "$HOST"
