#!/usr/bin/env bash
# Cpsl Python transpiler test runner
# Runs each .py test file via both cpsl --python and python3,
# compares output, captures per-phase timing, and generates a markdown report.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
CPSL="$SCRIPT_DIR/cpsl"
TEST_DIR="$SCRIPT_DIR/test"
OUTPUT_DIR="$SCRIPT_DIR/test/output"
REPORT="$OUTPUT_DIR/report.md"

# Ensure binary exists
if [[ ! -x "$CPSL" ]]; then
    echo "error: cpsl binary not found at $CPSL"
    echo "  run: cargo build --release && ditto target/release/cpsl-cli cpsl"
    echo "  (use 'ditto', not 'cp': macOS SIGKILLs cp'd ad-hoc-signed binaries)"
    exit 1
fi

# Flush and recreate output dir
rm -rf "$OUTPUT_DIR"
mkdir -p "$OUTPUT_DIR"

# Tests to skip (require volume mounts, expected to error, etc.)
SKIP_TESTS="fs_test.py fs_readwrite.py error_test.py"

should_skip() {
    local name="$1"
    for s in $SKIP_TESTS; do
        [[ "$name" == "$s" ]] && return 0
    done
    return 1
}

# Nanosecond timer
now_ns() {
    perl -MTime::HiRes=time -e 'printf "%d\n", time * 1e9'
}

us_to_ms() {
    echo "scale=2; $1 / 1000" | bc
}

ns_to_ms() {
    echo "scale=2; $1 / 1000000" | bc
}

# ── Warmup (prime macOS dyld/filesystem cache) ─────────────────
echo "Measuring baselines..."
"$CPSL" -- 'print("ok")' >/dev/null 2>&1
python3 -c "pass" 2>/dev/null

t0=$(now_ns); "$CPSL" -- 'print("ok")' >/dev/null 2>&1; t1=$(now_ns)
CPSL_STARTUP_NS=$((t1 - t0))
echo "  cpsl startup: $(ns_to_ms $CPSL_STARTUP_NS)ms"

t0=$(now_ns); python3 -c "pass" 2>/dev/null; t1=$(now_ns)
PY_STARTUP_NS=$((t1 - t0))
echo "  python3 startup: $(ns_to_ms $PY_STARTUP_NS)ms"
echo ""

# ── Collect test files ───────────────────────────────────────────
TESTS=()
for f in "$TEST_DIR"/*.py; do
    name="$(basename "$f")"
    if should_skip "$name"; then continue; fi
    TESTS+=("$f")
done

TOTAL=${#TESTS[@]}
PASS=0
FAIL=0

# ── Report header ───────────────────────────────────────────────
cat > "$REPORT" <<'HEADER'
# Cpsl Python Transpiler — Test Report

## Results

| Test | Status | Match | Startup | Transpile | Luau exec | **Cpsl total** | Py startup | Py exec | **Python total** | Exec speedup | Total speedup (no startup) |
|------|--------|-------|---------|-----------|-----------|-----------------|------------|---------|-----------------|-------------|---------------------------|
HEADER

echo "Running $TOTAL tests..."
echo ""

for test_file in "${TESTS[@]}"; do
    name="$(basename "$test_file" .py)"
    echo -n "  $name ... "

    cpsl_out="$OUTPUT_DIR/${name}.cpsl.txt"
    cpsl_err="$OUTPUT_DIR/${name}.cpsl.err"
    python_out="$OUTPUT_DIR/${name}.python.txt"
    diff_out="$OUTPUT_DIR/${name}.diff"

    # ── Run via cpsl --python --bench ──────────────────────────
    t_cpsl_start=$(now_ns)
    if "$CPSL" --python --bench "$test_file" > "$cpsl_out" 2> "$cpsl_err"; then
        cpsl_ok=true
    else
        cpsl_ok=false
    fi
    t_cpsl_end=$(now_ns)
    cpsl_wall_ns=$((t_cpsl_end - t_cpsl_start))

    # Parse bench lines from stderr (macOS-compatible)
    bench_val() { grep "bench:$1=" "$cpsl_err" 2>/dev/null | sed "s/.*bench:$1=//" | head -1; }
    startup_us=$(bench_val startup_us); startup_us=${startup_us:-0}
    transpile_us=$(bench_val transpile_us); transpile_us=${transpile_us:-0}
    exec_us=$(bench_val exec_us); exec_us=${exec_us:-0}

    startup_ms=$(us_to_ms "$startup_us")
    transpile_ms=$(us_to_ms "$transpile_us")
    exec_ms=$(us_to_ms "$exec_us")
    cpsl_total_ms=$(ns_to_ms "$cpsl_wall_ns")

    # ── Run via python3 with internal timing ────────────────────
    python_err="$OUTPUT_DIR/${name}.python.err"
    t_python_start=$(now_ns)
    if python3 -c "
import time, runpy, sys
t0 = time.perf_counter_ns()
runpy.run_path(sys.argv[1], run_name='__main__')
t1 = time.perf_counter_ns()
print(f'bench:py_exec_ns={t1-t0}', file=sys.stderr)
" "$test_file" > "$python_out" 2> "$python_err"; then
        python_ok=true
    else
        python_ok=false
    fi
    t_python_end=$(now_ns)
    python_wall_ns=$((t_python_end - t_python_start))
    python_total_ms=$(ns_to_ms "$python_wall_ns")

    # Parse python exec time from stderr
    py_exec_ns=$(grep "bench:py_exec_ns=" "$python_err" 2>/dev/null | sed 's/.*bench:py_exec_ns=//' | head -1)
    py_exec_ns=${py_exec_ns:-0}
    python_exec_ms=$(ns_to_ms "$py_exec_ns")
    python_startup_ns=$((python_wall_ns - py_exec_ns))
    if (( python_startup_ns < 0 )); then python_startup_ns=0; fi
    python_startup_ms=$(ns_to_ms "$python_startup_ns")

    # ── Compare outputs ──────────────────────────────────────────
    # Strip ANSI codes and bench: lines for comparison
    sed 's/\x1b\[[0-9;]*m//g' "$cpsl_out" > "$OUTPUT_DIR/${name}.cpsl.clean"
    sed 's/\x1b\[[0-9;]*m//g' "$python_out" > "$OUTPUT_DIR/${name}.python.clean"

    if diff -u "$OUTPUT_DIR/${name}.python.clean" "$OUTPUT_DIR/${name}.cpsl.clean" > "$diff_out" 2>&1; then
        outputs_match=true
    else
        outputs_match=false
    fi

    # ── Determine status ─────────────────────────────────────────
    if $cpsl_ok && $outputs_match; then
        status_icon="pass"
        PASS=$((PASS + 1))
    elif $cpsl_ok && ! $outputs_match; then
        status_icon="diff"
        FAIL=$((FAIL + 1))
    else
        status_icon="FAIL"
        FAIL=$((FAIL + 1))
    fi

    # ── Exec speedup (Py exec vs Luau exec only) ──────────────────
    exec_us_int=${exec_us%.*}; exec_us_int=${exec_us_int:-0}
    exec_ns=$((exec_us_int * 1000))
    if [[ "$exec_ns" != "0" && "$py_exec_ns" != "0" ]]; then
        exec_speedup=$(echo "scale=1; $py_exec_ns / $exec_ns" | bc 2>/dev/null || echo "?")
    else
        exec_speedup="—"
    fi

    # ── Total speedup excluding startup (transpile+exec vs py_exec) ──
    transpile_us_int=${transpile_us%.*}; transpile_us_int=${transpile_us_int:-0}
    cpsl_work_ns=$(( (transpile_us_int + exec_us_int) * 1000 ))
    if [[ "$cpsl_work_ns" != "0" && "$py_exec_ns" != "0" ]]; then
        total_no_startup=$(echo "scale=1; $py_exec_ns / $cpsl_work_ns" | bc 2>/dev/null || echo "?")
    else
        total_no_startup="—"
    fi

    match_str="yes"
    if ! $outputs_match; then match_str="**NO**"; fi

    echo "$status_icon (startup=${startup_ms} transpile=${transpile_ms} exec=${exec_ms} total=${cpsl_total_ms}ms | py=${python_total_ms}ms)"

    # ── Write report row ─────────────────────────────────────────
    echo "| \`$name\` | **$status_icon** | $match_str | $startup_ms | $transpile_ms | $exec_ms | **$cpsl_total_ms** | $python_startup_ms | $python_exec_ms | **$python_total_ms** | ${exec_speedup}x | ${total_no_startup}x |" >> "$REPORT"
done

echo ""

# ── Summary ──────────────────────────────────────────────────────
cat >> "$REPORT" <<EOF

> All times in milliseconds (ms). Lower is better.

## Summary

- **Total:** $TOTAL
- **Pass:** $PASS
- **Fail:** $FAIL
- **Date:** $(date -u +"%Y-%m-%d %H:%M:%S UTC")
- **Cpsl:** \`$("$CPSL" --help 2>&1 | head -1)\`
- **Python:** \`$(python3 --version 2>&1)\`
- **Platform:** \`$(uname -ms)\`

### Phase breakdown

| Phase | Description |
|-------|-------------|
| **Startup** | Process launch + arg parse + sandbox init + pyrt.luau load |
| **Transpile** | Python source → Luau source (Rust, single-pass) |
| **Luau exec** | Luau VM execution of transpiled code |
| **Py startup** | Python interpreter startup (wall time minus internal exec; baseline via \`python3 -c "pass"\`: $(ns_to_ms $PY_STARTUP_NS)ms) |
| **Py exec** | Pure script execution (measured from inside Python via \`time.perf_counter_ns()\`) |

### Skipped tests
$(for s in $SKIP_TESTS; do echo "- \`$s\` — requires volume mounts or tests error handling"; done)
EOF

# ── Add diff details for failures ────────────────────────────────
has_diffs=false
for test_file in "${TESTS[@]}"; do
    name="$(basename "$test_file" .py)"
    diff_file="$OUTPUT_DIR/${name}.diff"
    if [[ -s "$diff_file" ]]; then
        if ! $has_diffs; then
            echo "" >> "$REPORT"
            echo "## Output Diffs" >> "$REPORT"
            has_diffs=true
        fi
        echo "" >> "$REPORT"
        echo "### \`$name\`" >> "$REPORT"
        echo '```diff' >> "$REPORT"
        cat "$diff_file" >> "$REPORT"
        echo '```' >> "$REPORT"
    fi
done

echo "Report: $REPORT"
echo "Results: $PASS/$TOTAL passed"

# Exit with failure if any test failed
[[ $FAIL -eq 0 ]]
