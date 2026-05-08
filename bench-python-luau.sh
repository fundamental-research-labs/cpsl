#!/usr/bin/env bash
# CPSL Python-on-Luau benchmark runner.
#
# The local path requires Python 3 and a runnable CPSL CLI. If the CLI is
# missing, the script will build it with Cargo when available. If local
# prerequisites are missing, the script can retry inside Docker so users get a
# runnable path instead of a terse shell failure.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TEST_DIR="${CPSL_BENCH_TEST_DIR:-$SCRIPT_DIR/test}"
OUTPUT_DIR="${CPSL_BENCH_OUTPUT_DIR:-$TEST_DIR/output}"
REPORT="$OUTPUT_DIR/report.md"
CPSL="${CPSL_BIN:-$SCRIPT_DIR/cpsl}"
DOCKER_IMAGE="${CPSL_BENCH_DOCKER_IMAGE:-rust:1-bookworm}"
ALLOW_DOCKER=true
FORCE_DOCKER=false

SKIP_TESTS="fs_test.py fs_readwrite.py error_test.py"

usage() {
    cat <<EOF
Usage: ./bench-python-luau.sh [--docker] [--no-docker] [--output-dir DIR]

Runs every top-level test/*.py smoke test through both CPSL Python mode and
python3, compares stdout, records per-phase timing, and writes:

  $REPORT

Options:
  --docker          Run the benchmark in Docker even if local tools exist.
  --no-docker       Do not fall back to Docker. Useful inside CI containers.
  --output-dir DIR  Write benchmark artifacts to DIR.
  -h, --help        Show this help.

Local requirements:
  - Python 3.7+ as python3
  - a runnable ./cpsl binary, or Rust/Cargo so ./build-cli.sh can create one
  - standard shell tools: bash, basename, cat, date, diff, find, grep,
    head, mkdir, rm, sed, sort, uname

If those are not available, install the missing tools or run with Docker.
EOF
}

die() {
    echo "error: $*" >&2
    exit 1
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --docker)
            FORCE_DOCKER=true
            shift
            ;;
        --no-docker)
            ALLOW_DOCKER=false
            shift
            ;;
        --output-dir=*)
            OUTPUT_DIR="${1#--output-dir=}"
            REPORT="$OUTPUT_DIR/report.md"
            shift
            ;;
        --output-dir)
            shift
            if [[ $# -eq 0 ]]; then
                die "--output-dir requires a value"
            fi
            OUTPUT_DIR="$1"
            REPORT="$OUTPUT_DIR/report.md"
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            die "unknown argument: $1"
            ;;
    esac
done

case "$TEST_DIR" in
    /*) ;;
    *) TEST_DIR="$SCRIPT_DIR/$TEST_DIR" ;;
esac

case "$OUTPUT_DIR" in
    /*) ;;
    *)
        OUTPUT_DIR="$SCRIPT_DIR/$OUTPUT_DIR"
        REPORT="$OUTPUT_DIR/report.md"
        ;;
esac

print_local_requirements() {
    cat >&2 <<EOF

Local requirements:
  - Python 3.7+ as python3
  - a runnable ./cpsl binary, or Rust/Cargo so ./build-cli.sh can create one
  - standard shell tools: bash, basename, cat, date, diff, find, grep,
    head, mkdir, rm, sed, sort, uname

Docker fallback:
  - Docker CLI installed
  - Docker daemon running
  - network access the first time Docker pulls $DOCKER_IMAGE and Cargo crates
EOF
}

docker_output_dir() {
    case "$OUTPUT_DIR" in
        "$SCRIPT_DIR")
            printf "/work\n"
            ;;
        "$SCRIPT_DIR"/*)
            printf "/work/%s\n" "${OUTPUT_DIR#"$SCRIPT_DIR"/}"
            ;;
        *)
            printf "/work/test/output\n"
            ;;
    esac
}

run_in_docker() {
    local reason="$1"
    if [[ "$ALLOW_DOCKER" != "true" || "${CPSL_BENCH_IN_DOCKER:-0}" == "1" ]]; then
        echo "Cannot run benchmark locally: $reason" >&2
        print_local_requirements
        exit 1
    fi

    if ! command -v docker >/dev/null 2>&1; then
        echo "Cannot run benchmark locally: $reason" >&2
        echo "Docker fallback is not available because the docker CLI is not installed." >&2
        print_local_requirements
        exit 1
    fi

    if ! docker info >/dev/null 2>&1; then
        echo "Cannot run benchmark locally: $reason" >&2
        echo "Docker fallback is not available because the Docker daemon is not reachable." >&2
        print_local_requirements
        exit 1
    fi

    local out_dir
    out_dir="$(docker_output_dir)"
    if [[ "$out_dir" == "/work/test/output" && "$OUTPUT_DIR" != "$SCRIPT_DIR"/test/output ]]; then
        echo "warning: Docker fallback writes to test/output because the requested output dir is outside the repo mount." >&2
    fi

    echo "Local benchmark prerequisites are missing: $reason" >&2
    echo "Docker is available; running benchmark in $DOCKER_IMAGE." >&2

    if ! docker run --rm \
        -v "$SCRIPT_DIR:/work" \
        -w /work \
        -e CPSL_BENCH_IN_DOCKER=1 \
        -e CPSL_BENCH_OUTPUT_DIR="$out_dir" \
        "$DOCKER_IMAGE" \
        bash -lc 'set -euo pipefail
if ! command -v python3 >/dev/null 2>&1; then
    apt-get update
    apt-get install -y --no-install-recommends python3 ca-certificates
fi
cargo build --release -p cpsl-cli --manifest-path /work/Cargo.toml --target-dir /tmp/cpsl-bench-target
CPSL_BIN=/tmp/cpsl-bench-target/release/cpsl-cli ./bench-python-luau.sh --no-docker'; then
        cat >&2 <<EOF

Docker fallback failed.
Check that Docker can pull $DOCKER_IMAGE and that the container has network
access for Cargo dependencies. You can also run locally after installing:

  python3
  Rust/Cargo

Then build CPSL with:

  ./build-cli.sh
EOF
        exit 1
    fi

    exit 0
}

if [[ "$FORCE_DOCKER" == "true" ]]; then
    run_in_docker "forced with --docker"
fi

missing_tools=()
for tool in bash basename cat date diff find grep head mkdir rm sed sort uname; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        missing_tools+=("$tool")
    fi
done
if [[ ${#missing_tools[@]} -gt 0 ]]; then
    run_in_docker "missing shell tool(s): ${missing_tools[*]}"
fi

if ! command -v python3 >/dev/null 2>&1; then
    run_in_docker "python3 is not installed"
fi
PYTHON="$(command -v python3)"

if ! "$PYTHON" - <<'PY' >/dev/null 2>&1
import sys
import time
raise SystemExit(0 if sys.version_info >= (3, 7) and hasattr(time, "perf_counter_ns") else 1)
PY
then
    run_in_docker "python3 3.7+ with time.perf_counter_ns() is required"
fi

if [[ ! -d "$TEST_DIR" ]]; then
    die "test directory not found: $TEST_DIR"
fi

case "$OUTPUT_DIR" in
    ""|"/")
        die "refusing to use unsafe output directory: '$OUTPUT_DIR'"
        ;;
esac

ensure_cpsl() {
    if [[ -x "$CPSL" ]] && "$CPSL" --help >/dev/null 2>&1; then
        return 0
    fi

    if [[ -n "${CPSL_BIN:-}" ]]; then
        run_in_docker "CPSL_BIN is set but is not runnable: $CPSL_BIN"
    fi

    if [[ -x "$SCRIPT_DIR/build-cli.sh" ]] && command -v cargo >/dev/null 2>&1; then
        echo "cpsl binary is missing or not runnable; building it with ./build-cli.sh ..." >&2
        if "$SCRIPT_DIR/build-cli.sh"; then
            CPSL="$SCRIPT_DIR/cpsl"
        else
            run_in_docker "local ./build-cli.sh failed"
        fi
    else
        run_in_docker "no runnable ./cpsl binary and Rust/Cargo is not available to build one"
    fi

    if [[ ! -x "$CPSL" ]] || ! "$CPSL" --help >/dev/null 2>&1; then
        run_in_docker "the cpsl binary is still not runnable after build: $CPSL"
    fi
}

ensure_cpsl

rm -rf "$OUTPUT_DIR"
mkdir -p "$OUTPUT_DIR"

# Mount a repo-local scratch root into CPSL so the benchmark does not require
# writes to ~/.cpsl/ephemeral. This also keeps benchmark artifacts contained.
CPSL_ROOT="$OUTPUT_DIR/cpsl-root"
mkdir -p "$CPSL_ROOT"
CPSL_ARGS=(-v "$CPSL_ROOT:/")

should_skip() {
    local name="$1"
    local s
    for s in $SKIP_TESTS; do
        [[ "$name" == "$s" ]] && return 0
    done
    return 1
}

timed_run() {
    local out_path="$1"
    local err_path="$2"
    local ns_path="$3"
    shift 3
    "$PYTHON" - "$out_path" "$err_path" "$ns_path" "$@" <<'PY'
import subprocess
import sys
import time

out_path, err_path, ns_path, *cmd = sys.argv[1:]
with open(out_path, "wb") as out_file, open(err_path, "wb") as err_file:
    start = time.perf_counter_ns()
    proc = subprocess.run(cmd, stdout=out_file, stderr=err_file)
    end = time.perf_counter_ns()
with open(ns_path, "w", encoding="utf-8") as ns_file:
    ns_file.write(str(end - start))
sys.exit(proc.returncode)
PY
}

us_to_ms() {
    "$PYTHON" - "$1" <<'PY'
import sys
print(f"{float(sys.argv[1]) / 1000:.2f}")
PY
}

ns_to_ms() {
    "$PYTHON" - "$1" <<'PY'
import sys
print(f"{float(sys.argv[1]) / 1_000_000:.2f}")
PY
}

ratio() {
    "$PYTHON" - "$1" "$2" <<'PY'
import sys
num = float(sys.argv[1])
den = float(sys.argv[2])
print("n/a" if num <= 0 or den <= 0 else f"{num / den:.1f}")
PY
}

bench_val() {
    local key="$1"
    local file="$2"
    local line
    line="$(grep -m 1 "bench:$key=" "$file" 2>/dev/null || true)"
    if [[ -n "$line" ]]; then
        printf "%s\n" "${line##*bench:$key=}"
    else
        printf "0\n"
    fi
}

clean_output() {
    local input="$1"
    local output="$2"
    "$PYTHON" - "$input" "$output" <<'PY'
import re
import sys
from pathlib import Path

src = Path(sys.argv[1]).read_bytes()
src = re.sub(rb"\x1b\[[0-9;]*m", b"", src)
lines = [line for line in src.splitlines(keepends=True) if not line.startswith(b"bench:")]
Path(sys.argv[2]).write_bytes(b"".join(lines))
PY
}

append_file_block() {
    local title="$1"
    local path="$2"
    local fence="$3"
    if [[ -s "$path" ]]; then
        {
            echo ""
            echo "#### $title"
            echo "\`\`\`$fence"
            cat "$path"
            echo "\`\`\`"
        } >> "$REPORT"
    fi
}

NULL_OUT="$OUTPUT_DIR/.null.out"
NULL_ERR="$OUTPUT_DIR/.null.err"
TIMER_NS="$OUTPUT_DIR/.timer.ns"

echo "Measuring baselines..."
if ! timed_run "$NULL_OUT" "$NULL_ERR" "$TIMER_NS" "$CPSL" "${CPSL_ARGS[@]}" --python -- "pass"; then
    echo "CPSL failed during startup baseline:" >&2
    sed 's/^/  /' "$NULL_ERR" >&2 || true
    run_in_docker "CPSL failed during startup baseline"
fi
CPSL_STARTUP_NS="$(cat "$TIMER_NS")"
echo "  cpsl startup: $(ns_to_ms "$CPSL_STARTUP_NS")ms"

if ! timed_run "$NULL_OUT" "$NULL_ERR" "$TIMER_NS" "$PYTHON" -c "pass"; then
    echo "python3 failed during startup baseline:" >&2
    sed 's/^/  /' "$NULL_ERR" >&2 || true
    run_in_docker "python3 failed during startup baseline"
fi
PY_STARTUP_NS="$(cat "$TIMER_NS")"
echo "  python3 startup: $(ns_to_ms "$PY_STARTUP_NS")ms"
echo ""

TESTS=()
while IFS= read -r f; do
    name="$(basename "$f")"
    if should_skip "$name"; then
        continue
    fi
    TESTS+=("$f")
done < <(find "$TEST_DIR" -maxdepth 1 -type f -name '*.py' -print | sort)

TOTAL=${#TESTS[@]}
if [[ "$TOTAL" -eq 0 ]]; then
    die "no benchmark tests found in $TEST_DIR"
fi

PASS=0
FAIL=0
PY_RUNNER='import runpy, sys, time
t0 = time.perf_counter_ns()
runpy.run_path(sys.argv[1], run_name="__main__")
t1 = time.perf_counter_ns()
print(f"bench:py_exec_ns={t1-t0}", file=sys.stderr)'

cat > "$REPORT" <<'HEADER'
# CPSL Python-on-Luau Benchmark Report

## Results

| Test | Status | Match | Startup | Transpile | Luau exec | **CPSL total** | Py startup | Py exec | **Python total** | Exec speedup | Total speedup no startup |
|------|--------|-------|---------|-----------|-----------|----------------|------------|---------|------------------|--------------|--------------------------|
HEADER

echo "Running $TOTAL tests..."
echo ""

for test_file in "${TESTS[@]}"; do
    name="$(basename "$test_file" .py)"
    printf "  %s ... " "$name"

    cpsl_out="$OUTPUT_DIR/${name}.cpsl.txt"
    cpsl_err="$OUTPUT_DIR/${name}.cpsl.err"
    cpsl_ns="$OUTPUT_DIR/${name}.cpsl.ns"
    python_out="$OUTPUT_DIR/${name}.python.txt"
    python_err="$OUTPUT_DIR/${name}.python.err"
    python_ns="$OUTPUT_DIR/${name}.python.ns"
    diff_out="$OUTPUT_DIR/${name}.diff"

    if timed_run "$cpsl_out" "$cpsl_err" "$cpsl_ns" "$CPSL" "${CPSL_ARGS[@]}" --python --bench "$test_file"; then
        cpsl_ok=true
    else
        cpsl_ok=false
    fi
    cpsl_wall_ns="$(cat "$cpsl_ns")"

    startup_us="$(bench_val startup_us "$cpsl_err")"
    transpile_us="$(bench_val transpile_us "$cpsl_err")"
    exec_us="$(bench_val exec_us "$cpsl_err")"

    startup_ms="$(us_to_ms "$startup_us")"
    transpile_ms="$(us_to_ms "$transpile_us")"
    exec_ms="$(us_to_ms "$exec_us")"
    cpsl_total_ms="$(ns_to_ms "$cpsl_wall_ns")"

    if timed_run "$python_out" "$python_err" "$python_ns" "$PYTHON" -c "$PY_RUNNER" "$test_file"; then
        python_ok=true
    else
        python_ok=false
    fi
    python_wall_ns="$(cat "$python_ns")"
    python_total_ms="$(ns_to_ms "$python_wall_ns")"

    py_exec_ns="$(bench_val py_exec_ns "$python_err")"
    python_exec_ms="$(ns_to_ms "$py_exec_ns")"
    python_startup_ns=$((python_wall_ns - py_exec_ns))
    if (( python_startup_ns < 0 )); then
        python_startup_ns=0
    fi
    python_startup_ms="$(ns_to_ms "$python_startup_ns")"

    clean_output "$cpsl_out" "$OUTPUT_DIR/${name}.cpsl.clean"
    clean_output "$python_out" "$OUTPUT_DIR/${name}.python.clean"

    if diff -u "$OUTPUT_DIR/${name}.python.clean" "$OUTPUT_DIR/${name}.cpsl.clean" > "$diff_out" 2>&1; then
        outputs_match=true
    else
        outputs_match=false
    fi

    if ! $python_ok; then
        status_icon="PYFAIL"
        FAIL=$((FAIL + 1))
    elif $cpsl_ok && $outputs_match; then
        status_icon="pass"
        PASS=$((PASS + 1))
    elif $cpsl_ok && ! $outputs_match; then
        status_icon="diff"
        FAIL=$((FAIL + 1))
    else
        status_icon="FAIL"
        FAIL=$((FAIL + 1))
    fi

    exec_ns="$("$PYTHON" - "$exec_us" <<'PY'
import sys
print(int(float(sys.argv[1]) * 1000))
PY
)"
    exec_speedup="$(ratio "$py_exec_ns" "$exec_ns")"

    cpsl_work_ns="$("$PYTHON" - "$transpile_us" "$exec_us" <<'PY'
import sys
print(int((float(sys.argv[1]) + float(sys.argv[2])) * 1000))
PY
)"
    total_no_startup="$(ratio "$py_exec_ns" "$cpsl_work_ns")"

    match_str="yes"
    if ! $outputs_match; then
        match_str="NO"
    fi

    echo "$status_icon (startup=${startup_ms} transpile=${transpile_ms} exec=${exec_ms} total=${cpsl_total_ms}ms | py=${python_total_ms}ms)"

    echo "| \`$name\` | **$status_icon** | $match_str | $startup_ms | $transpile_ms | $exec_ms | **$cpsl_total_ms** | $python_startup_ms | $python_exec_ms | **$python_total_ms** | ${exec_speedup}x | ${total_no_startup}x |" >> "$REPORT"
done

cat >> "$REPORT" <<EOF

> All times are milliseconds. Lower is better. Speedup columns compare CPython time divided by CPSL time, so higher is better.

## Summary

- **Total:** $TOTAL
- **Pass:** $PASS
- **Fail:** $FAIL
- **Date:** $(date -u +"%Y-%m-%d %H:%M:%S UTC")
- **CPSL:** \`$("$CPSL" --help 2>&1 | head -1)\`
- **Python:** \`$("$PYTHON" - <<'PY'
import platform
print(f"{platform.python_implementation()} {platform.python_version()}")
PY
)\`
- **Platform:** \`$(uname -ms)\`
- **CPSL scratch root:** \`$CPSL_ROOT\`

## Phase Breakdown

| Phase | Description |
|-------|-------------|
| **Startup** | CPSL process launch, argument parsing, sandbox init, and Python runtime load |
| **Transpile** | Python source to Luau source |
| **Luau exec** | Luau VM execution of transpiled code |
| **Py startup** | Python process startup, estimated as wall time minus internal script time |
| **Py exec** | Python script time measured inside Python with \`time.perf_counter_ns()\` |

## Skipped Tests
$(for s in $SKIP_TESTS; do echo "- \`$s\` - requires volume mounts or intentionally tests errors"; done)
EOF

has_diagnostics=false
for test_file in "${TESTS[@]}"; do
    name="$(basename "$test_file" .py)"
    diff_file="$OUTPUT_DIR/${name}.diff"
    cpsl_err="$OUTPUT_DIR/${name}.cpsl.err"
    python_err="$OUTPUT_DIR/${name}.python.err"

    if [[ -s "$diff_file" ]] || [[ -s "$cpsl_err" && "$(grep -v '^bench:' "$cpsl_err" || true)" != "" ]] || [[ -s "$python_err" && "$(grep -v '^bench:' "$python_err" || true)" != "" ]]; then
        if ! $has_diagnostics; then
            echo "" >> "$REPORT"
            echo "## Diagnostics" >> "$REPORT"
            has_diagnostics=true
        fi
        echo "" >> "$REPORT"
        echo "### \`$name\`" >> "$REPORT"
        append_file_block "stdout diff" "$diff_file" "diff"
        if [[ -s "$cpsl_err" && "$(grep -v '^bench:' "$cpsl_err" || true)" != "" ]]; then
            grep -v '^bench:' "$cpsl_err" > "$OUTPUT_DIR/${name}.cpsl.err.clean" || true
            append_file_block "cpsl stderr" "$OUTPUT_DIR/${name}.cpsl.err.clean" "text"
        fi
        if [[ -s "$python_err" && "$(grep -v '^bench:' "$python_err" || true)" != "" ]]; then
            grep -v '^bench:' "$python_err" > "$OUTPUT_DIR/${name}.python.err.clean" || true
            append_file_block "python stderr" "$OUTPUT_DIR/${name}.python.err.clean" "text"
        fi
    fi
done

echo ""
echo "Report: $REPORT"
echo "Results: $PASS/$TOTAL passed"

[[ $FAIL -eq 0 ]]
