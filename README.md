# CPSL 💊

Safe, UNIX-like mini-OS capsules for agents, designed to run across Linux, macOS, Windows, web browsers, iOS, and Android.

Package tools, files, and permissions into a capsule. Build it. Run it.

Run capsules through Bash-compatible commands, Python-style scripts, or Luau code.

Try a WASM CPSL capsule in the browser at [cpsl.io](https://cpsl.io/).

The CPSL CLI builds capsules from TOML manifests:

```text
cpsl build -> cpsl ls -> cpsl run
```

CPSL is not Docker, a Linux distribution, a container image, or CPython. It is **UNIX-like enough** for agents, with explicit modules, files, mounts, and network rules.

## Early and Hackable 🛠️

CPSL is new as an open-source project. It is already used in some [Fundamental Research Labs](https://fundamentalresearchlabs.com) products, but the public project is young: install targets, module boundaries, SDK builds, and demos are actively evolving.

This is a good time to join and contribute. See [CONTRIBUTING.md](CONTRIBUTING.md) to get started.

## Quick Start ⚡️

Requires Rust and Cargo for now. Installers are coming soon.

```sh
# Download the source tree.
git clone https://github.com/fundamental-research-labs/cpsl

# Enter the checkout.
cd cpsl

# Build the repo-local CPSL CLI at ./cpsl.
./build-cli.sh

# Build an included capsule that only enables filesystem and JSON modules.
./cpsl build -f manifests/json-only.toml

# List capsules built on this machine.
./cpsl ls

# Run a Bash-compatible command inside the json-only capsule.
./cpsl run json-only -- 'json decode "{\"hello\":\"world\"}"'
```

For interactive sessions inside the same capsule, choose the language interface you want:

```sh
# Bash-compatible shell.
./cpsl run json-only -i

# Python-style shell.
./cpsl run json-only --python -i

# Lua/Luau shell.
./cpsl run json-only --lua -i
```

`./build-cli.sh` builds the CLI. `./cpsl build` builds a named capsule from a TOML manifest. `./cpsl run NAME` runs code inside that built capsule. No capsule is built by default.

### Scratch Mode

For quick experiments, the repo-local CLI can also run an ephemeral scratch sandbox directly. This is not a manifest-backed capsule, does not appear in `./cpsl ls`, and uses the modules compiled into `./cpsl`.

```sh
./cpsl -- 'echo hello from CPSL'
./cpsl -i
./cpsl --python -- 'print("hello from python mode")'
./cpsl --lua -- 'print("hello from luau")'
```

The default mode is Bash-compatible, including for `./cpsl run NAME`. `--lua` executes Luau directly. `--python` transpiles Python syntax to Luau; it does not invoke CPython or require Python to be installed.

### Custom Capsule 📝

A capsule starts as TOML. Give it a name, enable only the modules it needs, and allow specific network domains:

```toml
[sandbox]
name = "browser-agent"

[modules]
fs = true
json = true
http = true

[http]
allowed_domains = ["httpbin.org"]
```

Save that as `browser-agent.toml`, then build and run it:

```sh
./cpsl build -f browser-agent.toml
./cpsl run browser-agent --lua -- 'print(json.encode({status = "ready"}))'
```

Included manifest examples:

- `manifests/json-only.toml` - filesystem and JSON
- `manifests/minimal.toml` - filesystem, JSON, and CSV
- `manifests/data-science.toml` - structured data, numerical computing, and plotting
- `manifests/full.toml` - broad CLI-registered module set
- `manifests/all.toml` - broad CLI-registered module set

List the built-in modules accepted by manifests:

```sh
./cpsl modules
```

## How Does It Work? ⚙️

CPSL runs a Luau VM and exposes selected Rust-backed modules inside each capsule.

### Luau VM

[Luau](https://github.com/luau-lang/luau) is a small, fast, embeddable programming language based on Lua with a gradual type system. It was built and open-sourced by [Roblox](https://luau.org/news/2022-11-04-luau-origins-and-evolution/) and is battle-tested by millions of users.

Luau is a good fit for CPSL because it is designed for [sandboxed VMs](https://luau.org/sandbox/). CPSL adds its own mount table, module registry, HTTP policy, and host-resource gates around that VM.

### Composable Modules

Enable only the modules a capsule needs: filesystem, networking, JSON, compression, or custom modules. If you only need JSON and HTTP for one domain, keep the manifest that small.

### Communication

Agents and humans can interact with CPSL using Bash, Python, or Lua/Luau. A Luau runtime runs under the hood; Bash and Python are transpiled.

### Python-on-Luau

Python mode is intentionally not CPython. It does not support `pip install`, arbitrary native packages, or the full CPython standard library. It is a lightweight compatibility layer for practical scripts.

<details>
<summary>Python-on-Luau benchmark notes</summary>

These local comparison runs use `./bench-python-luau.sh`, which is optional and requires `python3`. Python is not required to build CPSL or use CPSL's Python mode.

| Test | CPSL total ms | CPython total ms |
|------|---------------|------------------|
| `comprehensive` | 16.87 | 24.73 |
| `control_flow` | 14.59 | 21.52 |
| `dict_ops` | 15.74 | 22.03 |
| `fibonacci` | 14.70 | 24.45 |
| `functional` | 15.60 | 22.12 |
| `hello` | 15.74 | 22.05 |
| `imports` | 16.17 | 23.12 |
| `list_ops` | 16.84 | 22.06 |
| `math_heavy` | 22.87 | 25.09 |
| `patterns` | 17.90 | 22.41 |
| `sorting` | 28.18 | 23.75 |
| `string_ops` | 18.63 | 23.19 |

```sh
./bench-python-luau.sh
```

</details>

## Roadmap 🚙

| Area | Next milestone | Tracking |
|------|----------------|----------|
| SDK targets | Build manifest-aware SDKs for iOS, macOS, Windows, Android, and Linux, starting with generated C headers. SDK artifacts should be built on demand by `cpsl` from manifest features, not published as version-tag artifacts. | [#9](https://github.com/fundamental-research-labs/cpsl/issues/9) |
| Detached sessions | Add `cpsl run -d` and `cpsl --exec` entry points for long-lived CPSL sessions while leaving the implementation architecture open. | [#10](https://github.com/fundamental-research-labs/cpsl/issues/10) |
| CLI release artifacts | Publish `cpsl` CLI binaries for macOS, Windows, and Linux when tagging a version. | [#11](https://github.com/fundamental-research-labs/cpsl/issues/11) |
| Capsule module contracts | Define the external capsule-module contract, including module metadata, source pinning, compatibility checks, and build boundaries so community modules can live in separate repositories. Distinct from CPSL Hub: this is the source/build contract; Hub is artifact distribution and discovery. | [#18](https://github.com/fundamental-research-labs/cpsl/issues/18) |
| CPSL Hub | Design the push and pull workflow for pre-built capsules, including metadata, compatibility checks, and provenance. | [#12](https://github.com/fundamental-research-labs/cpsl/issues/12) |
| Agent sandbox demo | Build the first demo with Herm, a container-aware and easily customizable agent. Herm runs a bounded workflow inside a CPSL capsule with explicit files, modules, network rules, and inspectable output artifacts. | [#13](https://github.com/fundamental-research-labs/cpsl/issues/13), [draft PR #17](https://github.com/fundamental-research-labs/cpsl/pull/17) |
