#!/usr/bin/env bash
#
# Generate or verify .expected baseline files for compat tests.
#
# Usage:
#   ./generate_baselines.sh          # Regenerate all .expected files
#   ./generate_baselines.sh --check  # Verify existing .expected files match (for CI)
#
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CHECK_MODE=false
ERRORS=0
UPDATED=0
SKIPPED=0

if [[ "${1:-}" == "--check" ]]; then
    CHECK_MODE=true
fi

# ── Python baselines ─────────────────────────────────────────────

echo "=== Python baselines ==="

while IFS= read -r -d '' pyfile; do
    expected="${pyfile}.expected"
    name="${pyfile#"$SCRIPT_DIR"/}"

    # Check for SKIP marker
    if head -1 "$pyfile" | grep -q '^# SKIP:'; then
        SKIPPED=$((SKIPPED + 1))
        continue
    fi

    actual=$(python3 "$pyfile" 2>&1) || {
        echo "  ERROR: $name exited non-zero"
        ERRORS=$((ERRORS + 1))
        continue
    }

    if $CHECK_MODE; then
        if [[ ! -f "$expected" ]]; then
            echo "  MISSING: $name — no .expected file"
            ERRORS=$((ERRORS + 1))
            continue
        fi
        existing=$(cat "$expected")
        if [[ "$actual" != "$existing" ]]; then
            echo "  STALE: $name — .expected file is out of date"
            echo "    expected: $(head -3 <<< "$existing")"
            echo "    actual:   $(head -3 <<< "$actual")"
            ERRORS=$((ERRORS + 1))
        fi
    else
        printf '%s\n' "$actual" > "$expected"
        UPDATED=$((UPDATED + 1))
    fi
done < <(find "$SCRIPT_DIR/python" -name '*.py' -print0 | sort -z)

# ── Bash baselines ───────────────────────────────────────────────

echo "=== Bash baselines ==="

while IFS= read -r -d '' shfile; do
    expected="${shfile}.expected"
    name="${shfile#"$SCRIPT_DIR"/}"

    # Check for SKIP marker
    if head -1 "$shfile" | grep -q '^# SKIP:'; then
        SKIPPED=$((SKIPPED + 1))
        continue
    fi

    actual=$(bash "$shfile" 2>&1) || {
        echo "  ERROR: $name exited non-zero"
        ERRORS=$((ERRORS + 1))
        continue
    }

    if $CHECK_MODE; then
        if [[ ! -f "$expected" ]]; then
            echo "  MISSING: $name — no .expected file"
            ERRORS=$((ERRORS + 1))
            continue
        fi
        existing=$(cat "$expected")
        if [[ "$actual" != "$existing" ]]; then
            echo "  STALE: $name — .expected file is out of date"
            echo "    expected: $(head -3 <<< "$existing")"
            echo "    actual:   $(head -3 <<< "$actual")"
            ERRORS=$((ERRORS + 1))
        fi
    else
        printf '%s\n' "$actual" > "$expected"
        UPDATED=$((UPDATED + 1))
    fi
done < <(find "$SCRIPT_DIR/bash" -name '*.sh' -print0 | sort -z)

# ── Summary ──────────────────────────────────────────────────────

echo ""
if $CHECK_MODE; then
    echo "Check complete. Errors: $ERRORS, Skipped: $SKIPPED"
    if [[ $ERRORS -gt 0 ]]; then
        echo "FAIL: $ERRORS baseline(s) are stale or missing. Run ./generate_baselines.sh to update."
        exit 1
    fi
    echo "OK: All baselines are up to date."
else
    echo "Updated $UPDATED baseline(s). Skipped: $SKIPPED. Errors: $ERRORS"
    if [[ $ERRORS -gt 0 ]]; then
        echo "WARNING: $ERRORS script(s) errored — check the output above."
        exit 1
    fi
fi
