#!/usr/bin/env bash
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

if ! cargo audit --version >/dev/null 2>&1; then
  echo "error: cargo-audit is not installed" >&2
  echo "install it with: cargo install cargo-audit --locked" >&2
  exit 127
fi

cargo audit
