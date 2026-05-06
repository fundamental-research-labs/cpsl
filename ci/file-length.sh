#!/usr/bin/env bash
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

cargo run -p ci-check -- file-length --max-file-lines 2000 "${SOURCE_PATHS[@]}"
