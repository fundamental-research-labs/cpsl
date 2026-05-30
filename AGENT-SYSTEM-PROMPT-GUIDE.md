# CPSL Agent Integration: System Prompt and Tool Surface Guide

This guide is for teams exposing CPSL to an AI agent. CPSL should be presented
as the agent's local, Unix-like sandbox: a Luau VM with an explicit sandbox
toolset, mounted filesystem paths, shell and Python compatibility entry points,
and host-defined network policy.

The goal is to make the model use CPSL correctly. A good integration tells the
agent which execution interfaces are exposed, which sandbox tools are available
inside those interfaces, which sandbox paths are mounted, where outputs should
be written, and how to verify work.

Use **CPSL** in integration docs and implementation docs. In rendered
model-facing prompts and tool descriptions, prefer "local sandbox", "Luau
sandbox", or concrete tool names unless the product surface itself is named
CPSL. Do not expose legacy or internal library names in model-facing text.

## What CPSL Is

CPSL is a local sandbox runtime with a Luau VM and a scoped filesystem. It is
not Docker, not a Linux distribution, not a full host shell, and not CPython.

The core mental model for the agent:

- CPSL executes native Luau.
- Bash and Python entry points are compatibility language interfaces that are
  transpiled or adapted to Luau.
- The same sandbox toolset is reachable through every exposed language
  interface; the language interface changes syntax, not the underlying tools.
- Files are visible through mounted sandbox paths, not arbitrary host paths.
- Network access is mediated by the host integration and may require explicit
  allow/deny policy.
- Sandbox state can persist for the session, but user-facing outputs should be
  written to the mounted project or artifact paths documented by the host.

## Integration Model

The recommended architecture is:

1. The agent receives system prompt text and tool descriptions that explain the
   CPSL execution surface.
2. The model calls one of the CPSL-backed tools.
3. The host application receives the tool request and evaluates it in CPSL.
4. CPSL returns a simple command result, usually including stdout, stderr, and
   an exit code or equivalent error status.
5. The agent inspects the result, verifies generated outputs, and reports saved
   paths or errors.

The model-visible execution interfaces documented by this guide are:

- `local_sandbox_exec` for native Luau.
- `local_sandbox_exec_bash` for Bash-compatible shell input.
- `local_sandbox_exec_python` for Python-compatible input.

Use these exact names when this guide is the integration contract. Render only
the interfaces actually exposed by the agent. Do not include stale aliases or
unavailable language interfaces in the system prompt or tool descriptions.

## Prompt Architecture

Keep stable CPSL behavior separate from per-turn state.

1. **Host context**
   Describe the host application, the user's current task surface, and any
   host-specific constraints that are true for the whole session.

2. **CPSL execution-interface descriptions**
   Put the syntax expected by each CPSL-backed execution interface, sandbox
   tool discovery rules, Luau essentials, filesystem limitations, and execution
   limitations directly on the relevant tool descriptions.

3. **Filesystem and mount context**
   Describe the exact mounted sandbox paths, permissions, project or artifact
   output locations, attachments, and scratch locations.

4. **Per-message context**
   Add dynamic information for the current turn: active mounted resources,
   selected files, current project tree, generated artifacts, attachments, and
   any host-provided bridge details.

This split matters. Stable CPSL behavior belongs in reusable templates and tool
descriptions. Dynamic facts, such as the current active file path or artifact
list, belong in per-message context.

## Recommended Execution Surface

Document only the CPSL execution interfaces the target agent actually exposes.
All exposed interfaces reach the same mounted filesystem and sandbox toolset.
The interface only changes the input syntax.

| Agent tool | Interface | Model-facing meaning |
| --- | --- | --- |
| `local_sandbox_exec` | Luau | Execute native Luau in the sandbox. This is CPSL's priority interface. |
| `local_sandbox_exec_bash` | Bash-like shell input | Use shell syntax for the same sandbox. Compatibility interface, not a host shell. |
| `local_sandbox_exec_python` | Python-like input | Use Python syntax for the same sandbox. Compatibility interface, not CPython. |

Do not describe unavailable tools or unrelated host-application tools in a
CPSL guide unless that document is explicitly scoped to a particular host
application.

Luau is CPSL's native runtime language. Every CPSL integration should expose a
native Luau execution tool unless it has a temporary host-specific blocker that
is explicitly documented. Bash and Python compatibility layers are frontends
into the same sandbox; they do not replace native Luau as the priority
interface.

For integrations that expose native Luau, the model should treat that interface
as the default for sandbox execution, file/data inspection, and repetitive
scripting. Bash-compatible input is a compatibility surface, not the native or
default path.

Do not suggest `lua`, `luau`, `lua -e`, or `luau -e` shell commands unless the
host explicitly provides those commands. The native Luau entry point is the
agent tool, not a standalone executable inside the compatibility shell.

Do not write rendered prompt text like "if available" for Bash or Python. The
host should assemble the final prompt from the actual interface list:

- If only `local_sandbox_exec` is exposed, mention only Luau.
- If `local_sandbox_exec_bash` is exposed, describe it directly as available.
- If `local_sandbox_exec_python` is exposed, describe it directly as available.

Some legacy or incomplete host-specific integrations may temporarily expose only
`local_sandbox_exec_bash`. Treat that as a limitation to remove, not the
recommended design. In that case, do not tell the rendered model to call an
unavailable Luau tool, but document that CPSL is still natively Luau and keep
the "not a host shell" boundary explicit.

For Bash-only integrations, Bash-like syntax is only the entry format. The
model should still treat CPSL modules and `help`-listed shell builtins as the
sandbox capability surface.

Bash-compatible discovery:

- Run `help` to list available CPSL shell builtins and loaded modules.
- Run `<module> help`, for example `fs help`, before using a module.
- Use `which NAME` or `type NAME` only for checking one CPSL command or module.
- Do not use host-shell discovery idioms such as `compgen -c`, `command -v`,
  `type -a`, `which -a`, `ls /bin`, `man`, or package-manager queries to infer
  CPSL capabilities.

If the host has read/write modes, approval gates, or other permission states,
document the resulting CPSL read/write behavior in host-specific prompt text.
Do not imply mutation is possible unless the host actually mounted writable
paths and permits write operations for the current mode.

## Native Luau Tool Description

`local_sandbox_exec` is the most important CPSL tool. Its description should
make three facts impossible to miss:

- The runtime is native Luau, CPSL's priority interface language.
- The agent must use `help()` and each sandbox tool's help function before
  guessing APIs.
- CPSL is not a general host shell or package-managed Python environment.

Use this as the base pattern:

```markdown
Execute Luau code in the local sandbox.

IMPORTANT: Never guess sandbox tool signatures. Call `help()` to list available
sandbox tools. Before calling a sandbox tool for the first time, call its help
function, such as `fs.help()`, to see exact parameters, types, return values,
and examples.

Calling convention: For functions with 2+ parameters, prefer table-form calls
such as `fn({name = value})` when supported. Table-form calls are more readable
and avoid ordering errors.

Use this native Luau interface for new code. For repeated or multi-step
automation, keep reusable `.luau` source in a mounted project/script path and
pass that source through native Luau when running it. Use Bash or Python
compatibility interfaces only for existing snippets, user-requested syntax, or
simple compatibility tasks.
```

Include Luau essentials directly in the native tool description:

```markdown
Luau essentials:
- Declare variables with `local`; the sandbox blocks global writes.
- Indexing is 1-based: first element is `t[1]`, not `t[0]`.
- String concatenation is `..`, not `+`.
- Not-equal is `~=`, not `!=`.
- Only `nil` and `false` are falsy. `0`, `""`, and `{}` are truthy.
- Use `pcall(fn)` for error handling; there is no try/catch.
- Table length is `#t`.
- `string.find()` returns `nil` on no match.
```

## Bash Compatibility Tool Description

`local_sandbox_exec_bash` accepts Bash-compatible shell input, but it still runs
inside the same sandbox and reaches the same sandbox tools. It is not arbitrary
host command execution.

Use this as the base pattern:

```markdown
Execute Bash-compatible shell input in the local sandbox.

This is not a host shell. There is no system package manager, no arbitrary host
command access, no background services, and no access to files outside mounted
sandbox paths. Commands are evaluated by the CPSL shell compatibility layer.

Run `help` to list available CPSL shell builtins and loaded modules. Before
using a sandbox module, run `<module> help`, for example `fs help`. Do not use
host-shell discovery commands such as `compgen -c`, `command -v`, `ls /bin`,
`man`, or package-manager probes to infer available CPSL capabilities.

Use this for simple shell-style file, document, data, and automation tasks, or
when the user explicitly asks for shell syntax. For new scripted logic, prefer
`local_sandbox_exec` with native Luau and sandbox tool APIs.
```

Avoid teaching the model that shell commands are the primary interface for
structured work. Prefer native sandbox tools where they are available,
especially for parsing documents, processing structured data, making HTTP
requests, or generating charts.

## Python Compatibility Tool Description

`local_sandbox_exec_python` accepts Python-compatible input, but it is not
CPython.

Use this as the base pattern:

```markdown
Execute Python-compatible input in the local sandbox.

This is not CPython. There is no `pip install`, no arbitrary native packages,
and no full CPython standard library. Python-compatible input is adapted to run
through CPSL. Available imports and APIs come from the sandbox toolset and may
differ from Python packages with similar names.

Use this for existing Python snippets or when the user explicitly asks for
Python syntax. For new scripted logic, prefer `local_sandbox_exec` with native
Luau and inspect sandbox tool APIs with `help()` and each tool's help function.
```

Do not imply Python packages, Python virtual environments, or package-manager
installs are available unless the host has implemented and documented that
capability separately.

## Sandbox Toolset Catalog

CPSL sandboxes can be built with custom toolsets. List only sandbox tools that
are compiled into the target CPSL build and exposed by the host. Do not paste a
broad catalog into every integration unless it is generated from the actual
build or otherwise verified.

Keep the catalog minimal. The prompt should name tools and give a short purpose,
then direct the agent to inspect live help instead of relying on prose.

A useful native Luau sandbox tool prompt pattern is:

```markdown
Available sandbox tools are listed by `help()`. Before using a tool for the
first time, call that tool's help function, such as `fs.help()`, to inspect
exact functions, parameters, return values, and examples.
```

A useful Bash-compatible sandbox tool prompt pattern is:

```markdown
Available CPSL shell commands and modules are listed by `help`. Before using a
module for the first time, run `<module> help`, such as `fs help`, to inspect
exact functions, flags, return values, and examples.
```

If the host has verified sandbox tool metadata, include a concise catalog in the
tool description or filesystem prompt:

```markdown
Available sandbox tools:
- `fs` - filesystem operations.
- `json` - JSON parsing and encoding.
- `csv` - CSV parsing and writing.
- `http` - HTTP requests under host network policy.
- `doc` - document reading.
- `plot` - chart generation.

Call `help()` for the live list and `<tool>.help()` before using a tool. In a
Bash-compatible interface, run `help` for the live list and `<tool> help`
instead.
```

Keep credential, authenticated-service, and host-account capabilities out of
the baseline CPSL prompt. If a host adds credential-aware modules later,
document them as host-specific capabilities and update the tool list, module
catalog, and security guidance together.

## Filesystem And Mounts

The prompt must describe the exact mounted paths available in the target
integration. Do not mix path schemes. Pick the paths your host actually mounts.

Example mount layout:

```markdown
Directory layout:

- `/project/` - project folder or active resource mount.
- `/attachments/` - user-provided attachments, usually read-only.
- `/artifacts/` - generated outputs or persistent artifacts.
- `/tmp/` - scratch space, not guaranteed to persist.
```

If your host uses a different layout, document that layout instead. If the host
supports optional mounts such as external folders, user memory, or skill files,
list those only when they are actually mounted for the current agent session.

If your host has both folder mode and single-resource mode, state the
difference:

```markdown
Folder mode:
- `/project/` is the user's mounted project folder.
- Write final deliverables to `/project/` unless the task asks for an artifact.
- Use `/artifacts/` for generated outputs that should be surfaced separately.

Single-resource mode:
- `/project/` contains the active resource and any host-provided companion
  files.
- Write new final deliverables to `/artifacts/` unless the active resource is
  explicitly writable and the user asked to modify it.
```

For file operations, prefer semantic sandbox tools exposed by the target
toolset. For example, when the user asks to move, reorganize, or rename files,
use the filesystem rename operation instead of copy-then-delete. Use copy only
when the user explicitly asks for a copy.

## Host-App Runtime Boundary

Some agents expose both CPSL and a separate host-application runtime. That
runtime is outside the CPSL baseline. If your integration has one, document the
boundary explicitly in host-specific prompt text.

Use this generic pattern:

```markdown
CPSL is the local sandbox for mounted files, data processing, document parsing,
HTTP, charts, and script work.

The host-application runtime handles only the application APIs exposed by this
host.

These runtimes do not share globals or implicit state. Exchange data only
through mounted files or the documented host bridge API. Use exact sandbox
paths when crossing the boundary.
```

Do not include host-runtime function names, bridge APIs, or application object
models in a generic CPSL guide. Put those details in the host application's own
agent integration documentation.

## Per-Message Context

Per-message context should be dynamic and concrete. Include only facts known
for the current turn.

For an active mounted resource:

```markdown
### Active Resource
The user sent this message while viewing: **<filename>** (<resource type>)
Sandbox path: `<absolute CPSL path>`
Permissions: `<read-only | read-write>`
```

For a project or folder:

```markdown
### Project
Project folder: **<project name>**
Sandbox mount: `/project/`

<directory tree>
```

For generated files:

```markdown
### Artifacts
- `/artifacts/report.html`
- `/artifacts/cleaned-data.csv`
```

For attachments:

```markdown
### Attachments
- `/attachments/invoice.pdf`
- `/attachments/source.csv`
```

The important part is exact paths. The model should not have to infer where a
file is mounted, whether it is writable, or where final outputs belong.

## System Prompt Skeleton

Use the following as a compact system-prompt skeleton. Adjust paths,
permissions, network policy, available execution interfaces, and sandbox
toolset catalog to your host. The skeleton deliberately avoids product names;
use the tool names and behavior directly in model-facing text.

```markdown
Use `local_sandbox_exec` to run native Luau in the local sandbox. Prefer it for
new file, data, document, HTTP, chart, and script work.

Call `help()` to list available sandbox tools. Before using a sandbox tool for
the first time, call its help function, such as `fs.help()`, to read exact
signatures and examples. Do not guess APIs from Python, Node, shell, or browser
conventions.

Use mounted sandbox paths exactly as provided in the context. Write final
deliverables to the documented project or artifact location. Use temporary
paths only for intermediate work.

Network access is not ambient. Use only the network capabilities exposed by the
host and follow the documented domain allow/deny policy.

For repetitive sandbox work, consider writing reusable Luau source to a mounted
path and passing that source to the native Luau tool later instead of
regenerating the full script each time.

After meaningful work, verify the result by re-reading generated files,
checking parsed data, or running a small validation snippet. Report any
unverified assumptions.
```

If `local_sandbox_exec_bash` is exposed, add direct text like:

```markdown
Use `local_sandbox_exec_bash` for Bash-like shell input in the same local
sandbox. It is not a host shell and does not support system package
installation or arbitrary host commands.
```

If `local_sandbox_exec_python` is exposed, add direct text like:

```markdown
Use `local_sandbox_exec_python` for Python-like input in the same local sandbox.
It is not CPython and does not support `pip install`, arbitrary native
packages, or the full CPython standard library.
```

Do not include those optional interface paragraphs in rendered prompts unless
the corresponding tool is actually exposed.

## Reusable Sandbox Scripts

For repetitive tasks, teach the agent that it can write reusable Luau source to
mounted paths and pass that source to the native Luau tool later. This is often
more efficient than repeatedly generating a full Luau program.

Use this guidance only when it fits the host application and the mounted
filesystem is writable:

```markdown
For repetitive sandbox work, write reusable Luau source under the project,
artifact, or scratch path documented by the host, then pass that source to the
native Luau tool later. Prefer this when the same parsing, conversion,
reporting, or validation logic will run multiple times.
```

The host prompt should still tell the agent where scripts may be written and
which paths persist across turns or sessions.

## Tool Description Checklist

Every CPSL-backed execution-interface description should answer:

- Which syntax does this interface accept?
- Does it execute native Luau or transpile/adapt compatibility syntax to Luau?
- Does it reach the same sandbox toolset as the other exposed interfaces?
- What filesystem paths can it access?
- Where should final outputs be written?
- Are external packages available? The usual answer should be no.
- How does the agent discover sandbox tool APIs? Use root `help()` and each
  tool's help function.
- How are stdout, stderr, and failures returned?
- What network policy applies?
- What should the agent use this interface for?
- What should the agent not use this interface for?

For `local_sandbox_exec`, the "when to use" answer should be broad: use it for
new local scripting, file inspection, data transformation, document parsing,
chart generation, output validation, and reusable Luau scripts.

For `local_sandbox_exec_bash` and `local_sandbox_exec_python`, the "when to
use" answer should be narrower: use them for compatibility or user-requested
syntax, not as the default for new scripted work.

## Recommended Agent Workflow

Teach the model this sandbox-specific loop:

1. Read the current context and identify the exact mounted paths.
2. Choose `local_sandbox_exec` for new code unless Bash or Python syntax is
   specifically useful.
3. Call root `help()` when it needs the available sandbox tool list.
4. Before using a sandbox tool for the first time, call its help function.
5. Use sandbox tool APIs for structured filesystem, data, document, HTTP, and
   chart work where the target toolset exposes them.
6. For repetitive tasks, write a reusable Luau script to a documented writable
   path when that is more efficient than regenerating full scripts.
7. Write intermediate files to scratch or artifact paths.
8. Write final deliverables to the documented user-visible project or artifact
   path.
9. Re-read or validate generated files.
10. Report saved paths and any unverified assumptions.

## Network And External Services

Do not present network access as unrestricted. The system prompt should describe
the host policy:

```markdown
HTTP access is controlled by host-defined allow/deny policy. If a task needs a
domain that is not allowed, request or explain the specific domain according to
the host application's approval flow. Do not assume credentials or
authenticated-service helpers are available.
```

If the target agent has no credential integration, say so by omission: do not
list authenticated-service modules, credential request flows, or examples with
token placeholders.

## Portable Static Outputs

Sandbox-mounted paths are the paths visible inside the sandbox and may differ
from host filesystem paths used by previews, exports, or the user's file
manager. For portable static output such as generated HTML, keep referenced
assets beside the entry file or in relative subdirectories, and use relative
paths like `./chart.svg` or `./assets/style.css`. Avoid absolute sandbox or host
paths unless the host integration explicitly requires them.

## Verification Checklist

Before shipping a prompt/tool integration, verify:

- The rendered prompt uses local sandbox, Luau sandbox, or concrete tool names
  in model-facing text, not internal library names.
- The rendered prompt mentions only the execution interfaces actually exposed
  by the agent.
- The native Luau interface is exposed for CPSL integrations unless a documented
  temporary blocker exists.
- The native Luau interface is clearly preferred for new code and repeated
  automation.
- The native Luau interface is the default for sandbox execution and
  inspection, not only for prompts that explicitly mention Lua.
- Bash and Python, if exposed, are described as compatibility language
  interfaces into the same sandbox, not separate toolsets.
- The prompt does not describe Bash as a host shell or Python as CPython.
- Root `help()` and per-tool help functions are prominent for native Luau.
- Bash-compatible prompts explicitly say `help` and `<module> help` are the
  sandbox discovery path.
- Bash-compatible prompts prohibit host shell enumeration such as `compgen -c`.
- The sandbox tool catalog contains only tools available in the target build.
- Custom sandbox toolsets are reflected accurately.
- Credential-specific modules and flows are absent unless implemented by the
  host.
- The path scheme matches the actual mount table.
- File/resource mode and folder mode, if both exist, have distinct output
  guidance.
- Any host-application runtime boundary is documented generically or in a
  host-specific guide.
- Reusable Luau script guidance is present when the host exposes a writable
  persistent path.
- Example snippets run in the interface and syntax they are attached to.
- Tool failures surface stdout, stderr, exit code, and relevant paths or
  equivalent structured error details.
- Generated artifacts are verified before completion is reported.
- Failures, denials, unsupported commands, or timeouts do not suggest falling
  back to host shell, container, or other out-of-sandbox execution.

## Misleading Patterns To Avoid

Avoid:

- Calling CPSL by an internal library name in external docs or prompts.
- Describing CPSL as a full host shell.
- Treating Bash, Python, and Luau as different sandbox toolsets.
- Making Bash the default for file inspection or structured work.
- Calling `lua`, `luau`, `lua -e`, or `luau -e` through a compatibility shell
  when Luau is exposed as an agent tool.
- Using host Bash discovery idioms such as `compgen -c`, `command -v`,
  `type -a`, `which -a`, `ls /bin`, `man`, or package-manager probes to infer
  CPSL capabilities.
- Presenting Python as CPython or as a package-managed runtime.
- Mentioning package-manager installs, system packages, or arbitrary host
  commands as supported capabilities.
- Listing sandbox tools that are not compiled into the target build.
- Including credential or authenticated-service flows that the target host
  cannot call.
- Suggesting shared memory or hidden globals between CPSL and a host
  application runtime.
- Omitting verification steps after generated artifacts.

The best CPSL prompt makes the model consistent: expose and use native Luau
first, discover the live sandbox toolset with `help()` or Bash-compatible
`help` as appropriate, inspect tool-specific help before calling APIs, respect
mounted paths, reuse Luau scripts when efficient, adapt within the sandbox, and
verify outputs before reporting completion.
