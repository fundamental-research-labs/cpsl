# Herm + CPSL Demo Plan

Status: planning document for the first public demo.

Goal: demonstrate Herm running without containers by delegating office, file,
document, and data automation work to CPSL, a lightweight sandboxed Unix-like
runtime mounted on the current folder.

## Demo Positioning

CPSL mode is an alternative to containers, not an extra layer inside a
container. When Herm is started with CPSL, no container is launched.

CPSL is best positioned for office-style work:

- managing and transforming files
- reading and writing structured data
- creating reports and documents
- lightweight automation over a mounted workspace

CPSL is not a full coding or development environment. The prompt and tool
descriptions must make it clear that agents should not assume package
installation, native compilers, Node, npm, pip, apt, brew, long-running
services, daemons, Docker, or host shell access.

## Herm CLI Contract

For the demo, add one CPSL-specific flag:

```sh
herm --cpsl /absolute/path/to/libcpsl.dylib
herm --cpsl /absolute/path/to/libcpsl.so
herm --cpsl C:\path\to\cpsl.dll
```

Only direct dynamic library paths are supported in the first demo.

If the value passed to `--cpsl` is not a platform CPSL sandbox library, Herm
should fail early with a simple message:

```text
You need to provide a CPSL sandbox library.
```

This intentionally avoids manifest detection, automatic builds, hub downloads,
or bundle resolution in the demo path.

Use backend-neutral network flags:

```sh
--allow-domain example.com
--deny-domain api.example.com
```

For the demo these flags only affect CPSL mode. Later they can also apply to
container networking when Herm has tighter container network control.

## Backend Rule

`--cpsl` and containers are mutually exclusive.

When `--cpsl` is present:

- Herm does not start a container.
- Herm mounts the current host folder into CPSL as `/workdir`.
- Herm routes the bash tool into CPSL.
- Herm uses a CPSL-specific system prompt.
- Herm disables container-specific setup, dev environment detection, package
  installation, and auto-build behavior.
- Herm must not fall back to host or container execution after a CPSL error,
  denial, or unsupported command.

## CPSL Library Shape

Add a dedicated native FFI crate in CPSL instead of turning `cpsl-core` into a
dynamic library.

Likely files:

- `ffi/Cargo.toml`
- `ffi/src/lib.rs`
- `ffi/include/cpsl.h`

The crate should build:

- macOS: `libcpsl.dylib`
- Linux: `libcpsl.so`
- Windows: `cpsl.dll`

The first FFI should stay small:

- ABI version query
- backend metadata query
- session create/free
- eval request
- string free
- last error

The dynamic library should embed CPSL shell runtime assets with `include_str!`
so Herm does not need to locate CPSL runtime files.

## Herm Integration Shape

Herm should keep its normal binary small. The CPSL backend is loaded only when
`--cpsl` is present.

Preferred implementation:

1. Herm starts a small worker process for CPSL mode.
2. The worker loads the CPSL dynamic library with `dlopen` or `LoadLibraryW`.
3. Herm communicates with the worker over a local protocol.
4. Herm can terminate the worker on timeout or crash without taking down the
   main Herm process.

The worker configures CPSL with:

```json
{
  "mounts": [
    {"host": "<current host folder>", "virtual": "/workdir", "mode": "rw"}
  ],
  "initial_cwd": "/workdir",
  "language": "bash",
  "http": "host_callback"
}
```

HTTP should route through Herm policy callbacks so Herm owns allow/deny
decisions and future credential policy.

## Prompt Contract

When CPSL mode is active, Herm should add a prompt section like:

```text
You are running commands inside CPSL, a lightweight sandboxed Unix-like runtime.
No container is running. The current folder is mounted at /workdir.

CPSL is suited for office, file, document, data, and automation tasks. It is
not a full development environment. Do not assume apt, brew, pip, npm, Node,
C/C++ compilers, system package installs, background services, daemons, Docker,
or host shell access.

Use the available CPSL commands and modules. Network access is policy-gated by
allow/deny domain rules. If a command is unavailable, adapt within CPSL instead
of trying to bypass the sandbox.
```

The CPSL library should eventually expose compiled module metadata so Herm can
include the exact available capabilities in the prompt.

## Fastest Demo Phases

1. Add `cpsl-ffi` dynamic library target with shell eval and `/workdir` mount
   support.
2. Add Herm `--cpsl` flag that accepts only a direct library path.
3. Make non-library `--cpsl` values fail with "You need to provide a CPSL
   sandbox library."
4. Run CPSL mode without containers.
5. Route Herm bash tool calls into CPSL.
6. Add backend-neutral `--allow-domain` and `--deny-domain`, wired only to CPSL
   for now.
7. Add CPSL-specific prompt text.
8. Record a demo showing Herm editing and creating files in `/workdir` without
   Docker.

## Future Distribution

Do not block the demo on this.

Later, CPSL Hub can expose verified CPSL images and checksums so Herm can verify
that a selected CPSL image is legitimate. Office-focused Herm distributions may
also bundle a verified CPSL image and make CPSL the default backend for that
edition, while coding-focused Herm distributions keep containers as the default.

## Demo Success Criteria

- `herm --cpsl ./libcpsl.dylib` starts without a container.
- Herm bash execution runs inside CPSL.
- The current folder is visible as `/workdir`.
- A task can inspect files and create or edit Markdown, JSON, CSV, or report
  files in `/workdir`.
- Unsupported development commands produce clear CPSL feedback.
- Network access is denied by default and can be allowed with `--allow-domain`.
- No manifest build, hub download, or automatic CPSL library resolution is
  required for the first demo.
