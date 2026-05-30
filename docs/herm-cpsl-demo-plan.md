# Herm + CPSL Demo Plan

Status: Phase 9 CPSL Demo Polish Follow-Ups complete.

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
#include <stdint.h>

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

The ABI exports these exact unmangled C symbols with the platform C calling
convention. `cpsl_session_t` is opaque, and no Rust types cross the boundary.

All strings crossing the boundary are NUL-terminated UTF-8. Embedded NUL bytes
are unsupported. Input strings are borrowed for the duration of the call only;
CPSL must copy anything retained after return. All returned `char *` values
except `cpsl_last_error` are owned by the caller and must be freed with
`cpsl_string_free`.

`cpsl_session_new(NULL)`, `cpsl_eval(NULL, ...)`, and
`cpsl_eval(..., NULL)` return `NULL` and set `cpsl_last_error`.
`cpsl_string_free(NULL)` and `cpsl_session_free(NULL)` are no-ops. Passing any
other non-CPSL pointer to CPSL free functions is undefined behavior.

`cpsl_last_error` returns a borrowed NUL-terminated UTF-8 pointer. An empty
string means no current error. The pointer remains valid until the next
non-`cpsl_last_error` FFI call on the same process. The first demo treats a
session as single-thread owned by its worker process.

No panic or Rust unwind may cross the C boundary. `cpsl_abi_version`,
`cpsl_session_free`, `cpsl_string_free`, and `cpsl_last_error` must be
infallible from the caller's perspective. A `NULL` return from
`cpsl_backend_metadata_json`, `cpsl_session_new`, or `cpsl_eval` is an
FFI/contract failure and must be paired with `cpsl_last_error`. A JSON eval
response with `ok=false` is a CPSL evaluation result, not an ABI failure.

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

For the first demo, the config supports exactly one writable mount: the current
host folder mounted at virtual path `/workdir`. The host path must be absolute
and canonical. The virtual path must be absolute. `mode` must be `"rw"`.
`language` must be `"bash"`. `initial_cwd` must be `/workdir` or a descendant
of `/workdir`; the demo starts at exactly `/workdir`.

Malformed JSON, missing required fields, unsupported field values, relative
host or virtual paths, unsupported mount modes, and an `initial_cwd` outside the
mounted virtual tree make `cpsl_session_new` return `NULL` with
`cpsl_last_error`.

All eval-time path resolution is confined to mounted virtual paths. Relative
paths, absolute paths, `..` traversal, symlinks, and `cd` changes must not
escape the mounted workspace or reveal host paths outside the configured mount.

Network policy is static for the life of the session and is fixed during
`cpsl_session_new`. The first demo supports only `"mode": "policy"` with
domain allow/deny lists. There is no host callback, credential forwarding, or
runtime policy mutation path. With an empty allow list, outbound network access
is denied by default. Deny entries always win over allow entries. Domain entries
are lowercase DNS hostnames with no wildcard syntax; matching is exact host or
subdomain suffix, so `example.com` matches `example.com` and `api.example.com`
but not `badexample.com`.

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

Eval responses always use the same top-level fields. On successful evaluation
or shell completion, `ok` is `true`, `exit_code` is the shell exit code, and
`error` is `null`. A shell command that runs and exits nonzero still returns
`ok=true` with a nonzero `exit_code`, so Herm can format it like its current
bash tool results.

`ok=false` means the request could not be evaluated by CPSL itself. For
`ok=false`, `exit_code` is `null`, `error` is an object with stable `code` and
human-readable `message` fields, `warnings` is an array of strings, and `cwd`
is the last known virtual cwd or `/workdir` if no better value is available.
`stdout` and `stderr` are strings and may contain partial output.

Example eval failure:

```json
{
  "ok": false,
  "stdout": "",
  "stderr": "",
  "exit_code": null,
  "error": {
    "code": "sandbox_denied",
    "message": "Network access is denied by policy"
  },
  "warnings": [],
  "cwd": "/workdir"
}
```

Stable demo error codes are `invalid_request`, `unsupported_language`,
`sandbox_denied`, `timeout`, and `runtime_error`. Malformed `request_json`,
missing required fields, or wrong JSON field types make `cpsl_eval` return
`NULL` with `cpsl_last_error`; valid requests that CPSL cannot evaluate return
structured JSON with `ok=false`.

The dynamic library should embed CPSL shell runtime assets with `include_str!`
so Herm does not need to locate CPSL runtime files. The initial shell cwd must
be `/workdir`; this can be implemented by initializing the shell runtime and
then setting the shell cwd during session creation.

Herm validates a CPSL library by resolving every required symbol by exact name,
requiring `cpsl_abi_version() == CPSL_ABI_VERSION`, parsing metadata JSON, and
requiring `name == "cpsl"`, `abi_version == 1`, `languages` containing
`"bash"`, and the demo capabilities `mounts` and `network_policy` set to
`true`. The loader uses the direct library path only, creates one session for
the Herm process, frees returned strings with `cpsl_string_free`, frees the
session before unload, and makes no calls after unload.

## Herm Integration Shape

Herm should keep its normal binary small. The CPSL backend is loaded only when
`--cpsl` is present.

Preferred implementation:

1. Herm starts a small worker process for CPSL mode.
2. The worker loads the CPSL dynamic library with `dlopen` or `LoadLibraryW`.
3. Herm communicates with the worker over a JSONL stdin/stdout protocol.
4. The worker boundary owns per-request timeout enforcement. The worker returns
   a timeout response when it can regain control; Herm keeps an outer watchdog
   so it can terminate a crashed or wedged worker without taking down the main
   process.

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

The worker protocol is newline-delimited JSON over stdin/stdout. Worker stdout
must contain only protocol messages. Worker stderr is diagnostic-only and is not
parsed as protocol. The first demo supports only `op: "eval"` and one in-flight
request at a time. Each response must repeat the request `id`.

Worker responses use the same result shape as `cpsl_eval`, with the `id` field
added. On worker or CPSL failure:

```json
{"id":1,"ok":false,"stdout":"","stderr":"","exit_code":null,"error":{"code":"timeout","message":"Command timed out after 120000 ms"},"warnings":[],"cwd":"/workdir"}
```

The worker enforces `timeout_ms` for each eval. The eval request still includes
`timeout_ms` so CPSL can cooperate with cancellation and produce consistent
diagnostics, but Herm must not depend on in-library timeout handling as the only
guard. Herm may kill and replace a crashed or wedged worker, but timeout
handling is not a fallback path.

After a bad request, CPSL denial, unsupported operation, unsupported language,
timeout, worker crash, malformed response, or EOF, Herm surfaces the
CPSL/worker error and does not retry through host execution, Docker, or direct
in-process CPSL execution.

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

- [x] Phase 0: Planning And Repo Setup
- [x] Phase 1: Contract Freeze
- [x] Phase 2: CPSL FFI Skeleton
- [x] Phase 3: CPSL Bash Session Eval
- [x] Phase 4: Herm CLI And Backend Mode
- [x] Phase 5: Herm CPSL Worker
- [x] Phase 6: Herm Tool Routing And Prompt Pruning
- [x] Phase 7: Network Policy
- [x] Phase 8: End-To-End Demo Smoke
- [x] Phase 9: CPSL Demo Polish Follow-Ups

### Phase 0: Planning And Repo Setup

Owner: CPSL superproject.

Commit contents:

- [x] expanded execution plan
- [x] Herm submodule pointer at `herm/`
- [x] Herm submodule branch prepared as `aduermael/cpsl-integration`

Acceptance:

- [x] reviewers can inspect CPSL and Herm side by side
- [x] no implementation code has been changed yet

### Phase 1: Contract Freeze

Owner: CPSL docs, with Herm implementation assumptions captured.

Commit: `docs: specify CPSL FFI contract for Herm`.

Acceptance:

- [x] C ABI signatures are frozen for the demo
- [x] session config JSON and eval request/response JSON are frozen
- [x] string ownership and panic/error behavior are documented
- [x] timeout behavior is documented as worker-enforced
- [x] network policy is documented as static allow/deny for the demo
- [x] mount confinement and path escape behavior are documented
- [x] worker JSONL protocol and no-fallback behavior are documented

### Phase 2: CPSL FFI Skeleton

Owner: CPSL branch `aduermael/lib-build`.

Commit: `ffi: add cpsl-ffi cdylib crate`.

Acceptance:

- [x] `ffi` is a workspace member
- [x] `cargo build -p cpsl-ffi --release` produces the platform dynamic library
- [x] `cpsl_abi_version`, metadata, string allocation/free, and last-error calls
  work from a tiny loader/probe test
- [x] no Herm code is required to validate the library skeleton

### Phase 3: CPSL Bash Session Eval

Owner: CPSL branch `aduermael/lib-build`.

Commit: `ffi: add bash eval sessions with workdir mounts`.

Acceptance:

- [x] session config mounts a host temp directory as `/workdir`
- [x] initial cwd is `/workdir`
- [x] `pwd`, `ls`, `cat`, `grep`, `echo > file`, JSON, CSV, and Markdown file
  workflows work through `cpsl_eval`
- [x] unsupported development commands return clear CPSL feedback
- [x] nonzero shell exits return `ok=true` and nonzero `exit_code`
- [x] no command can escape mounted paths

### Phase 4: Herm CLI And Backend Mode

Owner: Herm submodule branch `aduermael/cpsl-integration`.

Commit: `cli: add cpsl backend flags`.

Acceptance:

- [x] `--cpsl`, `--allow-domain`, and `--deny-domain` parse correctly
- [x] invalid CPSL library values fail with exactly
  `You need to provide a CPSL sandbox library.`
- [x] CPSL mode does not call Docker check, image pull, image build, container
  start, container retry, or `devenv`
- [x] non-CPSL behavior remains unchanged

### Phase 5: Herm CPSL Worker

Owner: Herm submodule branch `aduermael/cpsl-integration`.

Commit: `cpsl: add worker process and protocol`.

Acceptance:

- [x] worker loads the CPSL library by direct path
- [x] worker validates ABI version and metadata
- [x] worker creates one CPSL session for the Herm process
- [x] worker handles JSONL eval requests and returns structured eval responses
- [x] Herm kills the worker on timeout or crash and does not fall back to Docker or
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

- [x] CPSL: `ffi: accept network policy in session config`
- [x] Herm: `cpsl: pass network policy to worker`

Acceptance:

- [x] network access is denied by default
- [x] repeated `--allow-domain` values are passed to CPSL
- [x] repeated `--deny-domain` values are passed to CPSL
- [x] explicit deny wins over allow
- [x] no credential or host callback path is required for the first demo

### Phase 8: End-To-End Demo Smoke

Owner: CPSL superproject and Herm submodule.

Commits:

- [x] Herm: `test: add cpsl smoke path`
- [x] CPSL superproject: `chore: pin Herm CPSL integration submodule`

Acceptance:

- [x] with Docker unavailable, `herm --cpsl /abs/path/to/libcpsl.so -p ...`
  starts and completes
- [x] Herm bash execution runs inside CPSL
- [x] the current folder is visible as `/workdir`
- [x] a task can inspect files and create or edit Markdown, JSON, CSV, or report
  files in `/workdir`
- [x] unsupported development commands produce clear CPSL feedback
- [x] network access is denied by default and can be allowed with `--allow-domain`
- [x] no manifest build, hub download, or automatic CPSL library resolution is
  required

### Phase 9: CPSL Demo Polish Follow-Ups

Owner: Herm submodule, using `AGENT-SYSTEM-PROMPT-GUIDE.md` as the CPSL prompt
source of truth.

Do not block Phase 8 on this. These are post-smoke improvements observed during
manual demo testing.

Commit:

- [x] Herm: `cpsl: polish CPSL demo mode`

Acceptance:

- [x] CPSL mode never displays container-oriented status text such as
  `vdev (container: 0.4)`.
- [x] CPSL mode displays a backend-appropriate status label that makes clear the
  session is running with CPSL, not a Docker/container backend.
- [x] CPSL system prompt wording is revised against
  `AGENT-SYSTEM-PROMPT-GUIDE.md` before wider demo use.
- [x] `AGENT-SYSTEM-PROMPT-GUIDE.md` is updated during Herm integration if
  implementation reveals unclear guidance, missing sections, or prompt wording
  gaps.
- [x] Herm prompt assembly is refactored around backend prompt profiles instead
  of scattered `IsCPSL` branches in shared templates.
- [x] Prompt templates are organized by backend directory:
  - `herm/prompts/common/` for behavior that is true regardless of backend
  - `herm/prompts/container/` for Docker/container-specific runtime guidance
  - `herm/prompts/cpsl/` for local sandbox guidance derived from
    `AGENT-SYSTEM-PROMPT-GUIDE.md`
- [x] Final main-agent and sub-agent system prompts are assembled from the
  selected backend profile plus the available tool surface, with the backend
  selected once in code instead of repeatedly inside shared templates.
- [x] CPSL-rendered prompts and tool descriptions mention only the CPSL-safe
  execution surface exposed by Herm, omit Docker/container/devenv claims, and
  preserve the no-host-fallback rule.
- [x] Existing CPSL tool-description override precedent
  (`herm/prompts/tools_cpsl/`) is either retained or folded into the same
  backend-profile assembly mechanism.
- [x] `/shell` is available in CPSL mode and routes commands to the same CPSL
  sandbox mounted at `/workdir`.
- [x] Explore `/shell --lua` or `/shell --luau` for direct Luau interaction if
  the CPSL worker/FFI contract is expanded to support Luau eval safely.
- [x] Any CPSL shell mode must preserve the no-fallback rule: failures must not
  escape to host shell or container execution.

Update: the worker/FFI contract now exposes native Luau eval alongside Bash
compatibility, so Herm can prefer Luau for agent execution and support
`/shell --lua` / `/shell --luau` for direct line-oriented Luau interaction.
Follow-up: Herm's default model-facing tool surface exposes
`local_sandbox_exec` for native Luau and `local_sandbox_exec_bash` for explicit
Bash-compatible input, with native Luau listed first. `/shell` opens Luau by
default, while Bash remains supported through `/shell --bash` and internal
worker calls. The rendered agent prompt presents the backend as a local sandbox
with a Luau interface rather than naming CPSL.

## Future Distribution

Do not block the demo on this.

Later, CPSL Hub can expose verified CPSL images and checksums so Herm can verify
that a selected CPSL image is legitimate. Office-focused Herm distributions may
also bundle a verified CPSL image and make CPSL the default backend for that
edition, while coding-focused Herm distributions keep containers as the default.
