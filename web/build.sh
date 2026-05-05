#!/usr/bin/env bash
set -euo pipefail

WEB_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(cd "$WEB_DIR/.." && pwd)"
PUBLIC_DIR="$WEB_DIR/public"
DIST_DIR="$WEB_DIR/dist"
WASM_DIR="$DIST_DIR/assets/wasm"

rm -rf "$DIST_DIR"
mkdir -p "$WASM_DIR"
cp -R "$PUBLIC_DIR"/. "$DIST_DIR"/
touch "$DIST_DIR/.nojekyll"

if [[ "${CPSL_SKIP_WASM:-0}" == "1" ]]; then
  echo "Built static site without CPSL WASM: $DIST_DIR"
  exit 0
fi

if ! command -v emcc >/dev/null 2>&1; then
  echo "error: emcc is required to build the CPSL browser runtime" >&2
  echo "hint: install Emscripten or run CPSL_SKIP_WASM=1 ./web/build.sh for static-only checks" >&2
  exit 1
fi

rustup target add wasm32-unknown-emscripten

export RUSTFLAGS="-C panic=abort -C link-arg=-O3 -C link-arg=-o -C link-arg=$WASM_DIR/cpsl.js -C link-arg=-sMODULARIZE=1 -C link-arg=-sEXPORT_ES6=1 -C link-arg=-sENVIRONMENT=worker -C link-arg=-sALLOW_MEMORY_GROWTH=1 -C link-arg=-sEXPORTED_FUNCTIONS=['_main','_cpsl_session_new','_cpsl_session_free','_cpsl_eval','_cpsl_string_free','_cpsl_last_error'] -C link-arg=-sEXPORTED_RUNTIME_METHODS=['ccall','cwrap','UTF8ToString']"
export CXXFLAGS_wasm32_unknown_emscripten="-fwasm-exceptions"

cargo build \
  --manifest-path "$REPO_DIR/Cargo.toml" \
  --release \
  -p cpsl-web \
  --target wasm32-unknown-emscripten

test -f "$WASM_DIR/cpsl.js"
test -f "$WASM_DIR/cpsl.wasm"

echo "Built CPSL web site: $DIST_DIR"
