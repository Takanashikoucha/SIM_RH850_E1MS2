#!/usr/bin/env bash
# RH850/E1M-S2 simulator — one-command launcher (Linux / macOS).
#
#   ./start.sh          # serve the prebuilt app with Python 3 (no Node.js needed)
#   ./start.sh 9000     # custom port
#   ./start.sh --dev    # development mode (Vite hot reload; requires Node.js)
#
set -euo pipefail
cd "$(cd "$(dirname "$0")" && pwd)"

PORT="${1:-8080}"
DEV=0
case "${1:-}" in
  --dev) DEV=1; PORT=5174 ;;
esac

dist="web/dist"

if [ "$DEV" = "1" ]; then
  # ---- Development mode: Vite + hot reload (requires Node.js + npm) ----
  if ! command -v npm >/dev/null 2>&1; then
    echo "Error: --dev mode requires Node.js/npm." >&2
    echo "Install from https://nodejs.org/, or run without --dev for the prebuilt app." >&2
    exit 1
  fi
  if [ ! -d web/node_modules ]; then
    echo "==> Installing frontend dependencies (first run)..."
    (cd web && npm install)
  fi
  echo "==> Development server at http://127.0.0.1:${PORT} (Ctrl+C to stop)"
  exec npm run dev --prefix web -- --port "$PORT"
fi

# ---- Static mode: prebuilt app via Python 3 ----
if [ ! -d "$dist" ]; then
  echo "Error: $dist/ not found (prebuilt app missing)." >&2
  echo "Build it with:  npm install --prefix web && npm run build --prefix web" >&2
  echo "Or use dev mode: ./start.sh --dev" >&2
  exit 1
fi

PY=python3
command -v python3 >/dev/null 2>&1 || PY=python
if ! command -v "$PY" >/dev/null 2>&1; then
  echo "Error: Python 3 not found. Install it, or use dev mode: ./start.sh --dev" >&2
  exit 1
fi

exec "$PY" serve.py "$PORT"
