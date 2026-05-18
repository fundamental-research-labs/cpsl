# Contributing to CPSL

Thanks for working on CPSL. Keep changes focused, update docs when behavior changes, and run the smallest useful test set before opening a PR.

## Source Policy

CPSL accepts human-written and agent-assisted contributions. The rule is provenance, not the tool used: every submitted change must be reviewable, understood by the contributor, and compatible with this repository's license.

These guardrails matter more for agentic contributions because agents can synthesize code from uncertain context or lose attribution. Treat generated code as code you authored: inspect it, understand it, and be able to explain it in review.

Do:

- Write original code or use sources that are clearly licensed for reuse.
- Attribute third-party code, fixtures, generated assets, and vendored dependencies in the PR.
- Keep generated files reproducible and document the command or tool that produced them.
- Prefer linking to external references over pasting substantial source text.

Do not:

- Paste code from private repositories, proprietary products, blogs, Q&A sites, or model output unless the license and attribution are clear and compatible.
- Add copied files without preserving required license notices.
- Ask an agent to recreate unavailable source from memory, screenshots, or non-public context.
- Submit changes you cannot build, test, or explain.

When in doubt, open an issue before the PR.

## Prerequisites

- Rust and Cargo for local builds
- Platform native build tools required by Rust crates on your OS
- Optional: PDFium for PDF-related tests, via `core/scripts/download-pdfium.sh`

Python and Docker are not required for normal CPSL development. They are only used by the optional Python-on-Luau benchmark script.

## Setup

Build the local CLI and verify it starts:

```sh
./build-cli.sh
./cpsl --help
```

## Running Capsules

Build a manifest into a named capsule and run code inside it:

```sh
./cpsl build -f manifests/json-only.toml
./cpsl ls
./cpsl run json-only --lua -- 'print(json.encode({hello = "world"}))'
```

Useful flags:

- `-v, --volume host:virtual[:ro]` mounts a host path.
- `--allow-domain example.com` allows HTTP for a domain when the HTTP module is present.
- `--deny-domain example.com` denies HTTP for a domain.

## Scratch-Mode Smoke Checks

These commands exercise the repo-local CLI directly in an ephemeral scratch sandbox. They do not require a manifest-built capsule:

```sh
./cpsl -- 'echo hello from CPSL'
./cpsl --python -- 'print("hello from python mode")'
./cpsl --lua -- 'print("hello from luau")'
```

## Tests and Checks

Run the Rust test suite:

```sh
cargo test
```

Some CLI integration tests are marked ignored because they build or mutate local capsule binaries:

```sh
cargo test -p cpsl-cli --test build_integration -- --ignored
cargo test -p cpsl-cli --test run_integration -- --ignored
cargo test -p cpsl-cli --test sandboxes_rm_integration -- --ignored
```

Check committed Bash/Python compatibility baselines:

```sh
core/tests/compat/generate_baselines.sh --check
```

Run source policy checks:

```sh
./ci/file-length.sh
./ci/function-length.sh
./ci/docstring.sh
```

The source policy job is intentionally strict about file length, function length, and module docstrings. It keeps reviews tractable, makes generated or pasted code easier to spot, and gives agentic contributions a narrow shape to work inside. Expect these checks to become stricter as the public contribution surface grows.

Run the optional Python compatibility benchmark:

```sh
./bench-python-luau.sh
```

This benchmark compares CPSL Python mode with local `python3`. It requires Python 3.7+ and can fall back to Docker when local benchmark prerequisites are missing.

## Where Changes Usually Go

- CLI behavior, manifest parsing, and capsule commands: `cli/`
- Runtime behavior, transpilers, module registration, and built-in modules: `core/`
- Native support crates used by modules: `modules/`
- Shell and Python compatibility runtimes: `runtime/`
- Example capsule manifests: `manifests/`
- Design notes and implementation references: `docs/`
- Browser demo and static site: `web/`
- CI policy scripts and support tooling: `ci/` and `tools/ci-check/`
- Python compatibility smoke scripts: `test/`

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
