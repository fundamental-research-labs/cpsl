#!/usr/bin/env bash
set -euo pipefail

WEB_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(cd "$WEB_DIR/.." && pwd)"
PUBLIC_DIR="$WEB_DIR/public"
DIST_DIR="$WEB_DIR/dist"
WASM_DIR="$DIST_DIR/assets/wasm"
EMSDK_ENV="${EMSDK_ENV:-$REPO_DIR/emsdk/emsdk_env.sh}"
WEB_SANDBOX_MANIFEST="${CPSL_WEB_MANIFEST:-$PUBLIC_DIR/cpsl-web.toml}"

web_manifest_features() {
  local manifest="$1"

  awk '
    /^[[:space:]]*\[/ {
      section = $0
      gsub(/[[:space:]]/, "", section)
      next
    }
    section == "[modules]" && /^[[:space:]]*[A-Za-z0-9_-]+[[:space:]]*=[[:space:]]*true[[:space:]]*$/ {
      name = $1
      gsub(/[[:space:]]/, "", name)
      print name
    }
  ' "$manifest" | while read -r module; do
    case "$module" in
      fs) echo "mod-fs" ;;
      json) echo "mod-json" ;;
      http) echo "mod-http" ;;
      base64) echo "mod-base64" ;;
      random) echo "mod-random" ;;
      fin) echo "mod-fin" ;;
      regex) echo "mod-regex" ;;
      url) echo "mod-url" ;;
      *)
        echo "error: web sandbox manifest enables unsupported module '$module'" >&2
        exit 1
        ;;
    esac
  done | paste -sd, -
}

if [[ ! -f "$WEB_SANDBOX_MANIFEST" ]]; then
  echo "error: missing web sandbox manifest: $WEB_SANDBOX_MANIFEST" >&2
  exit 1
fi

if [[ -d "$DIST_DIR" ]]; then
  find "$DIST_DIR" -mindepth 1 -maxdepth 1 -exec rm -rf {} +
else
  mkdir -p "$DIST_DIR"
fi
mkdir -p "$WASM_DIR"
cp -R "$PUBLIC_DIR"/. "$DIST_DIR"/
touch "$DIST_DIR/.nojekyll"

apply_build_id() {
  local build_id="$1"
  build_id="$(printf '%s' "$build_id" | tr -cd 'A-Za-z0-9._-')"
  if [[ -z "$build_id" ]]; then
    build_id="local"
  fi

  perl -0pi -e "s/__CPSL_BUILD_ID__/$build_id/g" \
    "$DIST_DIR/main.js" \
    "$DIST_DIR/cpsl.worker.js"
  perl -0pi -e "s#src=\"\\./main\\.js\"#src=\"./main.js?v=$build_id\"#g" \
    "$DIST_DIR/index.html"
}

if [[ "${CPSL_SKIP_WASM:-0}" == "1" ]]; then
  apply_build_id "${CPSL_BUILD_ID:-static}"
  echo "Built static site without CPSL WASM: $DIST_DIR"
  exit 0
fi

if ! command -v emcc >/dev/null 2>&1; then
  if [[ -f "$EMSDK_ENV" ]]; then
    export EMSDK_QUIET="${EMSDK_QUIET:-1}"
    # shellcheck source=/dev/null
    source "$EMSDK_ENV" >/dev/null
  fi
fi

if ! command -v emcc >/dev/null 2>&1; then
  echo "error: emcc is required to build the CPSL browser runtime" >&2
  echo "hint: install Emscripten with:" >&2
  echo "  git clone https://github.com/emscripten-core/emsdk.git" >&2
  echo "  ./emsdk/emsdk install 5.0.7" >&2
  echo "  ./emsdk/emsdk activate 5.0.7" >&2
  echo "then rerun ./web/build.sh, or run CPSL_SKIP_WASM=1 ./web/build.sh for static-only checks" >&2
  exit 1
fi

rustup target add wasm32-unknown-emscripten

WEB_FEATURES="$(web_manifest_features "$WEB_SANDBOX_MANIFEST")"
export CPSL_WEB_MANIFEST="$WEB_SANDBOX_MANIFEST"

export RUSTFLAGS="-C panic=abort -C link-arg=-O3 -C link-arg=-o -C link-arg=$WASM_DIR/cpsl.js -C link-arg=-sMODULARIZE=1 -C link-arg=-sEXPORT_ES6=1 -C link-arg=-sENVIRONMENT=worker -C link-arg=-sALLOW_MEMORY_GROWTH=1 -C link-arg=-sEXPORTED_FUNCTIONS=['_main','_cpsl_session_new','_cpsl_session_free','_cpsl_eval','_cpsl_string_free','_cpsl_last_error'] -C link-arg=-sEXPORTED_RUNTIME_METHODS=['ccall','cwrap','UTF8ToString']"
export CXXFLAGS_wasm32_unknown_emscripten="-fwasm-exceptions"

cargo build \
  --manifest-path "$REPO_DIR/Cargo.toml" \
  --release \
  -p cpsl-web \
  --no-default-features \
  --features "$WEB_FEATURES" \
  --target wasm32-unknown-emscripten

test -f "$WASM_DIR/cpsl.js"
test -f "$WASM_DIR/cpsl.wasm"

if [[ -n "${CPSL_BUILD_ID:-}" ]]; then
  BUILD_ID="$CPSL_BUILD_ID"
elif command -v shasum >/dev/null 2>&1; then
  BUILD_ID="$(shasum -a 256 "$WASM_DIR/cpsl.wasm" | cut -d ' ' -f 1 | cut -c 1-16)"
else
  BUILD_ID="$(cksum "$WASM_DIR/cpsl.wasm" | cut -d ' ' -f 1)"
fi
apply_build_id "$BUILD_ID"

echo "Built CPSL web site: $DIST_DIR"
