#!/usr/bin/env bash
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

cargo run -p ci-check -- function-length --max-function-lines 500 "${SOURCE_PATHS[@]}"
