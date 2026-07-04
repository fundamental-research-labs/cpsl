#!/usr/bin/env bash
# Download the correct PDFium binary for the current platform.
#
# Usage: ./download-pdfium.sh [--version VERSION] [--target TARGET] [--output DIR]
#
# Defaults:
#   VERSION  = 7734  (chromium build number)
#   OUTPUT   = ../libs/pdfium  (relative to this script)
#
# Cache: ~/.cache/pdfium/{version}/{target}/
# Source: https://github.com/bblanchon/pdfium-binaries

set -euo pipefail

PDFIUM_VERSION="${PDFIUM_VERSION:-7734}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
DEFAULT_OUTPUT="$SCRIPT_DIR/../libs/pdfium"

# Parse arguments
OUTPUT=""
TARGET="${PDFIUM_TARGET:-}"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --version) PDFIUM_VERSION="$2"; shift 2 ;;
    --target)  TARGET="$2"; shift 2 ;;
    --output)  OUTPUT="$2"; shift 2 ;;
    *)         echo "Unknown arg: $1" >&2; exit 1 ;;
  esac
done
OUTPUT="${OUTPUT:-$DEFAULT_OUTPUT}"

# Detect platform
detect_target() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"

  case "$os" in
    Darwin)
      case "$arch" in
        arm64)  echo "mac-arm64" ;;
        x86_64) echo "mac-x64" ;;
        *)      echo "Unsupported macOS arch: $arch" >&2; exit 1 ;;
      esac
      ;;
    Linux)
      case "$arch" in
        aarch64) echo "linux-arm64" ;;
        x86_64)  echo "linux-x64" ;;
        armv7l)  echo "linux-arm" ;;
        *)       echo "Unsupported Linux arch: $arch" >&2; exit 1 ;;
      esac
      ;;
    MINGW*|MSYS*|CYGWIN*|Windows_NT)
      case "$arch" in
        x86_64|AMD64) echo "win-x64" ;;
        aarch64)      echo "win-arm64" ;;
        *)            echo "Unsupported Windows arch: $arch" >&2; exit 1 ;;
      esac
      ;;
    *) echo "Unsupported OS: $os" >&2; exit 1 ;;
  esac
}

TARGET="${TARGET:-$(detect_target)}"
ASSET="pdfium-${TARGET}.tgz"
CACHE_DIR="$HOME/.cache/pdfium/${PDFIUM_VERSION}/${TARGET}"
CACHE_FILE="$CACHE_DIR/$ASSET"

# Library filename per platform
case "$TARGET" in
  ios-*|mac-*) LIB_NAME="libpdfium.dylib" ;;
  linux-*)     LIB_NAME="libpdfium.so" ;;
  win-*)       LIB_NAME="pdfium.dll" ;;
  *)           echo "Unsupported PDFium target: $TARGET" >&2; exit 1 ;;
esac

# Check if already at output location
if [[ -f "$OUTPUT/lib/$LIB_NAME" ]]; then
  echo "PDFium already present: $OUTPUT/lib/$LIB_NAME"
  exit 0
fi

# Download to cache if not cached
if [[ ! -f "$CACHE_FILE" ]]; then
  URL="https://github.com/bblanchon/pdfium-binaries/releases/download/chromium%2F${PDFIUM_VERSION}/${ASSET}"
  echo "Downloading PDFium ${PDFIUM_VERSION} for ${TARGET}..."
  echo "  URL: $URL"
  mkdir -p "$CACHE_DIR"
  curl -fSL --progress-bar "$URL" -o "$CACHE_FILE.tmp"
  mv "$CACHE_FILE.tmp" "$CACHE_FILE"
  echo "  Cached: $CACHE_FILE"
else
  echo "Using cached: $CACHE_FILE"
fi

# Extract to output directory
mkdir -p "$OUTPUT"
echo "Extracting to $OUTPUT..."
tar xzf "$CACHE_FILE" -C "$OUTPUT"

echo "PDFium ${PDFIUM_VERSION} ready: $OUTPUT/lib/$LIB_NAME"
echo ""
echo "Set PDFIUM_DYNAMIC_LIB_PATH=$OUTPUT/lib/$LIB_NAME for pdfium-render"
