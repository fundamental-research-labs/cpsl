# Herm + CPSL Demo Plan

Status: execution plan for review before implementation.

Goal: demonstrate Herm running without containers by delegating office, file,
document, and data automation work to CPSL, a lightweight sandboxed Unix-like
runtime mounted on the current folder.

Herm has been prepared as a submodule at `herm/`. Work in that submodule should
happen on `aduermael/cpsl-integration`, created from `origin/main`.

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

## Consensus Decision

The first implementation should not start from the original fastest-path list.
Three review passes found the plan directionally right but under-specified in
places that would create avoidable coupling between CPSL FFI design, Herm
backend routing, Docker startup assumptions, and prompt wording.

Implementation should start only after the ABI, worker protocol, tool surface,
and network-policy behavior below are accepted.

The demo network path should use static allow/deny policy passed into CPSL.
Herm-owned host callbacks remain a future extension unless the FFI contract is
explicitly expanded before implementation. This keeps the first dynamic library
small and still demonstrates deny-by-default networking plus `--allow-domain`.

## Herm CLI Contract

For the demo, add one CPSL-specific flag:

```sh
herm --cpsl /absolute/path/to/libcpsl.dylib
herm --cpsl /absolute/path/to/libcpsl.so
herm --cpsl C:\path\to\cpsl.dll
```

Only direct dynamic library paths are supported in the first demo.

If the value passed to `--cpsl` is missing, is not a file, does not have the
platform library extension, fails to load, or does not expose the required CPSL
ABI, Herm should fail early with this user-facing message:

```text
You need to provide a CPSL sandbox library.
```

Detailed loader diagnostics may be written to debug logs, but the CLI-facing
message should stay simple for the demo path.

This intentionally avoids manifest detection, automatic builds, hub downloads,
or bundle resolution in the demo path.

Use backend-neutral network flags:

```sh
--allow-domain example.com
--deny-domain api.example.com
```

The flags should be repeatable. For the demo they only affect CPSL mode. Later
they can also apply to container networking when Herm has tighter container
network control.

## Backend Rule

`--cpsl` and containers are mutually exclusive.

When `--cpsl` is present:

- Herm does not check Docker.
- Herm does not start, pull, build, restart, or retry a container.
- Herm mounts the current host folder into CPSL as `/workdir`.
- Herm routes the bash tool into CPSL.
- Herm uses CPSL-specific system prompt and bash tool text.
- Herm disables container-specific setup, dev environment detection, package
  installation, auto-build behavior, and the `devenv` tool.
- Herm does not expose container file tools unless they are reimplemented
  against CPSL.
- Herm does not expose host `git` in the first demo, because the CPSL prompt
  says there is no host access.
- Herm must not fall back to host or container execution after a CPSL error,
  denial, unsupported command, timeout, or crash.

## CPSL FFI Contract

Add a dedicated native FFI crate in CPSL instead of turning `cpsl-core` into a
dynamic library.

Files:

- `ffi/Cargo.toml`
- `ffi/src/lib.rs`
- `ffi/include/cpsl.h`

The crate should build:

- macOS: `libcpsl.dylib`
- Linux: `libcpsl.so`
- Windows: `cpsl.dll`

The first FFI should stay small:

```c
#define CPSL_ABI_VERSION 1

typedef struct cpsl_session cpsl_session_t;

uint32_t cpsl_abi_version(void);
char *cpsl_backend_metadata_json(void);
cpsl_session_t *cpsl_session_new(const char *config_json);
void cpsl_session_free(cpsl_session_t *session);
char *cpsl_eval(cpsl_session_t *session, const char *request_json);
void cpsl_string_free(char *value);
const char *cpsl_last_error(void);
```

All returned `char *` values except `cpsl_last_error` are owned by the caller
and must be freed with `cpsl_string_free`. `cpsl_last_error` returns a borrowed
pointer that remains valid until the next CPSL FFI call on the same process.
FFI functions must catch panics and report failure through null returns plus
`cpsl_last_error`.

Backend metadata response:

```json
{
  "name": "cpsl",
  "abi_version": 1,
  "version": "0.1.0",
  "languages": ["bash"],
  "capabilities": {
    "mounts": true,
    "network_policy": true
  }
}
```

Session config:

```json
{
  "mounts": [
    {"host": "<current host folder>", "virtual": "/workdir", "mode": "rw"}
  ],
  "initial_cwd": "/workdir",
  "language": "bash",
  "http": {
    "mode": "policy",
    "allow_domains": [],
    "deny_domains": []
  }
}
```

Eval request:

```json
{
  "language": "bash",
  "input": "pwd",
  "timeout_ms": 120000
}
```

Eval response:

```json
{
  "ok": true,
  "stdout": "/workdir\n",
  "stderr": "",
  "exit_code": 0,
  "error": null,
  "warnings": [],
  "cwd": "/workdir"
}
```

`ok=false` means the request could not be evaluated by CPSL itself. A shell
command that runs and exits nonzero should return `ok=true` with a nonzero
`exit_code`, so Herm can format it like its current bash tool results.

The dynamic library should embed CPSL shell runtime assets with `include_str!`
so Herm does not need to locate CPSL runtime files. The initial shell cwd must
be `/workdir`; this can be implemented by initializing the shell runtime and
then setting the shell cwd during session creation.

## Herm Integration Shape

Herm should keep its normal binary small. The CPSL backend is loaded only when
`--cpsl` is present.

Preferred implementation:

1. Herm starts a small worker process for CPSL mode.
2. The worker loads the CPSL dynamic library with `dlopen` or `LoadLibraryW`.
3. Herm communicates with the worker over a JSONL stdin/stdout protocol.
4. Herm can terminate the worker on timeout or crash without taking down the
   main Herm process.

Worker startup inputs:

- CPSL library path
- current host folder
- allow-domain list
- deny-domain list

Worker protocol:

```json
{"id":1,"op":"eval","language":"bash","input":"ls -la","timeout_ms":120000}
{"id":1,"ok":true,"stdout":"...","stderr":"","exit_code":0,"error":null,"warnings":[],"cwd":"/workdir"}
```

If the worker cannot load or validate the library, Herm should surface only:

```text
You need to provide a CPSL sandbox library.
```

The worker may include richer details in stderr for debug mode.

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

Herm must also replace tool descriptions that currently say "dev container" or
"Docker" when CPSL mode is active. The first demo should expose CPSL `bash`
and the sub-agent tool only if sub-agents inherit the same CPSL-safe tool set.
Provider-side web search can remain available when the selected model supports
it, because it is not host shell or container execution.

The CPSL library should eventually expose compiled module metadata so Herm can
include exact available capabilities in the prompt. That is not required for
the first demo.

## Execution Phases

Phase checklist:

- [ ] Phase 0: Planning And Repo Setup
- [ ] Phase 1: Contract Freeze
- [ ] Phase 2: CPSL FFI Skeleton
- [ ] Phase 3: CPSL Bash Session Eval
- [ ] Phase 4: Herm CLI And Backend Mode
- [ ] Phase 5: Herm CPSL Worker
- [ ] Phase 6: Herm Tool Routing And Prompt Pruning
- [ ] Phase 7: Network Policy
- [ ] Phase 8: End-To-End Demo Smoke

### Phase 0: Planning And Repo Setup

Owner: CPSL superproject.

Commit contents:

- [ ] expanded execution plan
- [ ] Herm submodule pointer at `herm/`
- [ ] Herm submodule branch prepared as `aduermael/cpsl-integration`

Acceptance:

- [ ] reviewers can inspect CPSL and Herm side by side
- [ ] no implementation code has been changed yet

### Phase 1: Contract Freeze

Owner: CPSL docs, with Herm implementation assumptions captured.

Commit: `docs: specify CPSL FFI contract for Herm`.

Acceptance:

- [ ] C ABI signatures are frozen for the demo
- [ ] session config JSON and eval request/response JSON are frozen
- [ ] string ownership and panic/error behavior are documented
- [ ] timeout behavior is documented as worker-enforced
- [ ] network policy is documented as static allow/deny for the demo

### Phase 2: CPSL FFI Skeleton

Owner: CPSL branch `aduermael/lib-build`.

Commit: `ffi: add cpsl-ffi cdylib crate`.

Acceptance:

- [ ] `ffi` is a workspace member
- [ ] `cargo build -p cpsl-ffi --release` produces the platform dynamic library
- [ ] `cpsl_abi_version`, metadata, string allocation/free, and last-error calls
  work from a tiny loader/probe test
- [ ] no Herm code is required to validate the library skeleton

### Phase 3: CPSL Bash Session Eval

Owner: CPSL branch `aduermael/lib-build`.

Commit: `ffi: add bash eval sessions with workdir mounts`.

Acceptance:

- [ ] session config mounts a host temp directory as `/workdir`
- [ ] initial cwd is `/workdir`
- [ ] `pwd`, `ls`, `cat`, `grep`, `echo > file`, JSON, CSV, and Markdown file
  workflows work through `cpsl_eval`
- [ ] unsupported development commands return clear CPSL feedback
- [ ] nonzero shell exits return `ok=true` and nonzero `exit_code`
- [ ] no command can escape mounted paths

### Phase 4: Herm CLI And Backend Mode

Owner: Herm submodule branch `aduermael/cpsl-integration`.

Commit: `cli: add cpsl backend flags`.

Acceptance:

- [ ] `--cpsl`, `--allow-domain`, and `--deny-domain` parse correctly
- [ ] invalid CPSL library values fail with exactly
  `You need to provide a CPSL sandbox library.`
- [ ] CPSL mode does not call Docker check, image pull, image build, container
  start, container retry, or `devenv`
- [ ] non-CPSL behavior remains unchanged

### Phase 5: Herm CPSL Worker

Owner: Herm submodule branch `aduermael/cpsl-integration`.

Commit: `cpsl: add worker process and protocol`.

Acceptance:

- [ ] worker loads the CPSL library by direct path
- [ ] worker validates ABI version and metadata
- [ ] worker creates one CPSL session for the Herm process
- [ ] worker handles JSONL eval requests and returns structured eval responses
- [ ] Herm kills the worker on timeout or crash and does not fall back to Docker or
  host execution

### Phase 6: Herm Tool Routing And Prompt Pruning

Owner: Herm submodule branch `aduermael/cpsl-integration`.

Commit: `cpsl: route bash through CPSL`.

Acceptance:

- [ ] Herm's bash tool calls use the CPSL worker in CPSL mode
- [ ] CPSL mode prompt does not claim Docker or a container exists
- [ ] bash tool description says commands run in CPSL at `/workdir`
- [ ] `devenv`, container file tools, `/shell`, and host `git` are unavailable in
  the first CPSL demo
- [ ] sub-agents, if enabled, receive the same CPSL-safe tool set and prompt

### Phase 7: Network Policy

Owner: CPSL and Herm.

Commits:

- [ ] CPSL: `ffi: accept network policy in session config`
- [ ] Herm: `cpsl: pass network policy to worker`

Acceptance:

- [ ] network access is denied by default
- [ ] repeated `--allow-domain` values are passed to CPSL
- [ ] repeated `--deny-domain` values are passed to CPSL
- [ ] explicit deny wins over allow
- [ ] no credential or host callback path is required for the first demo

### Phase 8: End-To-End Demo Smoke

Owner: CPSL superproject and Herm submodule.

Commits:

- [ ] Herm: `test: add cpsl smoke path`
- [ ] CPSL superproject: `chore: pin Herm CPSL integration submodule`

Acceptance:

- [ ] with Docker unavailable, `herm --cpsl /abs/path/to/libcpsl.so -p ...`
  starts and completes
- [ ] Herm bash execution runs inside CPSL
- [ ] the current folder is visible as `/workdir`
- [ ] a task can inspect files and create or edit Markdown, JSON, CSV, or report
  files in `/workdir`
- [ ] unsupported development commands produce clear CPSL feedback
- [ ] network access is denied by default and can be allowed with `--allow-domain`
- [ ] no manifest build, hub download, or automatic CPSL library resolution is
  required

## Future Distribution

Do not block the demo on this.

Later, CPSL Hub can expose verified CPSL images and checksums so Herm can verify
that a selected CPSL image is legitimate. Office-focused Herm distributions may
also bundle a verified CPSL image and make CPSL the default backend for that
edition, while coding-focused Herm distributions keep containers as the default.
