#!/usr/bin/env bash
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

cargo run -p ci-check -- docstring --min-doc-chars 20 --max-doc-chars 700 "${SOURCE_PATHS[@]}"
