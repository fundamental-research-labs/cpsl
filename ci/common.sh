#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

SOURCE_PATHS=(
  "cli/src"
  "core/src"
  "modules/native-http/src"
  "modules/native-webview-pdf/src"
  "tools/ci-check/src"
)

cd "$ROOT_DIR"
