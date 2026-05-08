# Contributing to CPSL

Thanks for working on CPSL. Keep changes focused, update docs when behavior changes, and run the smallest useful test set before opening a PR.

## Prerequisites

- Rust and Cargo for local builds
- `python3` 3.7+ for `./bench-python-luau.sh`
- Optional: Docker, used by `./bench-python-luau.sh` as a fallback when local benchmark prerequisites are missing
- Platform native build tools required by Rust crates on your OS
- Optional: PDFium for PDF-related tests, via `core/scripts/download-pdfium.sh`

## Setup

Build the local CLI and verify it starts:

```sh
./build-cli.sh
./cpsl --help
```

Run a quick smoke command:

```sh
./cpsl -- 'echo hello from CPSL'
./cpsl --python -- 'print("hello from python mode")'
./cpsl --lua -- 'print("hello from luau")'
```

## Running Sandboxes

Build a manifest into a named sandbox:

```sh
./cpsl build -t json-tool -f manifests/json-only.toml
./cpsl run json-tool --lua -- 'print(json.encode({hello = "world"}))'
```

Useful flags:

- `-v, --volume host:virtual[:ro]` mounts a host path.
- `--allow-domain example.com` allows HTTP for a domain when the HTTP module is present.
- `--deny-domain example.com` denies HTTP for a domain.

## Tests

Run the Rust test suite:

```sh
cargo test
```

Run the Python compatibility smoke suite:

```sh
./bench-python-luau.sh
```

Check committed Bash/Python compatibility baselines:

```sh
core/tests/compat/generate_baselines.sh --check
```

Some CLI integration tests are marked ignored because they build or mutate local sandbox images:

```sh
cargo test -p cpsl-cli --test build_integration -- --ignored
cargo test -p cpsl-cli --test run_integration -- --ignored
cargo test -p cpsl-cli --test sandboxes_rm_integration -- --ignored
```

## Project Layout

- `cli/` - command-line interface, manifest parsing, sandbox build/run commands
- `core/` - Luau sandbox runtime, transpilers, module registration, built-in modules
- `modules/` - native support crates
- `runtime/` - Luau runtimes for shell and Python compatibility
- `manifests/` - example sandbox manifests
- `docs/` - design notes and implementation references
- `test/` - Python compatibility scripts used by `./bench-python-luau.sh`

## Module Changes

When adding or changing a module, update the relevant pieces together:

- Cargo feature flags in `core/Cargo.toml` and, when exposed to manifests, `cli/Cargo.toml`
- module registration in `core/src/sandbox.rs`
- CLI manifest registry in `cli/src/config.rs`
- examples in `manifests/` when useful
- tests under `core/tests/` or `cli/tests/`
- README/docs when user-visible behavior changes

## Pull Request Checklist

- Build passes.
- Relevant tests pass, or skipped tests are explained.
- README/docs are updated for user-visible changes.
- Generated outputs such as `target/`, `cpsl`, and `test/output/` are not committed.
