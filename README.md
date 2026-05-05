# CPSL (Capsule)

Composable sandboxes for agent software.

CPSL, short for Capsule, is an operating system for agents: a cross-platform runtime that packages tools, permissions, files, network access, and language adapters into explicit sandbox images. Instead of giving an agent a whole machine, CPSL gives it a capsule: small, inspectable, and limited to the capabilities selected for the job.

Agents can run Bash-style commands, a lightweight Python-compatible subset, or raw Luau inside the same underlying sandbox. Capabilities such as filesystem access, structured data, HTTP, documents, plotting, and numerical computing are composed from manifests instead of inherited from the host.

CPSL is built around an embedded language runtime, not a Linux VM or container. That means the sandbox model can run in places containers cannot, including browser and mobile application hosts. This repository currently ships the native Rust CLI and core crates; browser and mobile hosts build on the same sandbox contract.

## Why CPSL?

- 🧩 **Composable sandboxes** - build a capsule from the modules an agent needs, and leave everything else out.
- 🔒 **Policy by default** - filesystem mounts, HTTP domains, and module availability are explicit.
- ⚙️ **Agent-native execution** - use shell-style commands for quick actions, Python-style code for scripts, or Luau for direct control.
- **Portable by design** - the same sandbox contract can sit inside a CLI, server, desktop app, browser, or mobile app.

## Run Commands

The examples below use `./cpsl`. If you are working from source and do not have the binary yet, build it once with the steps in [Build the CLI](#build-the-cli).

Bash-style shell is the default mode, including in the interactive REPL:

```sh
./cpsl -- 'echo hello from CPSL'
./cpsl -i
```

Use `--python` or `--lua` when you want those modes:

```sh
./cpsl --python -- 'print("hello from python mode")'
./cpsl --lua -- 'print("hello from luau")'
```

The Python mode is intentionally not CPython. It does not support `pip install`, arbitrary native packages, or the full CPython standard library. It is a lightweight compatibility layer for common agent scripts.

## Compose a Sandbox

Sandbox image manifests live in `manifests/`. Build one into a named capsule, then run it:

```sh
./cpsl build -t json-tool -f manifests/json-only.toml
./cpsl run json-tool --lua -- 'print(json.encode({hello = "world"}))'
```

HTTP access is policy-gated. Build a sandbox with the HTTP module, then allow the domains it may reach at run time:

```sh
./cpsl build -t web-tool -f manifests/full.toml
./cpsl run web-tool --allow-domain httpbin.org --lua -- 'local r = http.get("https://httpbin.org/get"); print(r.status)'
```

Ready-to-use manifests:

- `manifests/json-only.toml` - filesystem and JSON
- `manifests/minimal.toml` - filesystem, JSON, and CSV
- `manifests/data-science.toml` - structured data, numerical computing, and plotting
- `manifests/full.toml` - broad CLI-registered module set with Python enabled
- `manifests/all.toml` - broad CLI-registered module set

List the modules accepted by sandbox manifests:

```sh
./cpsl modules
```

## Build the CLI

Requires Rust and Cargo.

```sh
./build-cli.sh
./cpsl --help
```

For direct Cargo builds:

```sh
cargo build --release -p cpsl-cli
cargo build -p cpsl-cli --no-default-features --features mod-json,mod-fs
```

## How Does It Work?

CPSL runs code inside an embedded [Luau](https://luau.org/) VM. Luau is a Lua 5.1-derived language improved by Roblox and released as [open source](https://luau.org/news/2021-11-03-luau-goes-open-source). CPSL chose Luau because it is fast, small, battle-tested at [Roblox scale](https://corp.roblox.com/engineering) across hundreds of millions of users, designed for embedding, and has first-class support for [sandboxed virtual machines](https://luau.org/sandbox/).

At startup, the Rust sandbox creates a virtual mount table, registers only the compiled-in CPSL modules, installs controlled `print`, `help`, and `require` functions, removes host-oriented globals such as `io`, `os`, `loadfile`, and `dofile`, then enables Luau sandbox mode.

Bash and Python do not execute through the host shell or CPython:

- **Bash-style mode** parses shell input with `conch-parser`, transpiles it to Luau, and runs it against `runtime/shrt.luau`.
- **Python mode** parses Python with `rustpython-parser`, transpiles it to Luau, and runs it against `runtime/pyrt.luau`.
- **Lua mode** executes Luau directly in the same sandbox.

CPSL modules are Rust capabilities exposed as Luau globals. A sandbox manifest chooses which modules are compiled into a capsule, while run-time flags and manifest policy decide which host resources, paths, and domains that capsule may touch.

## What CPSL Is Not

- It is not a Linux VM or OCI container. The isolation boundary is the Luau VM plus CPSL's Rust module APIs, mounts, and policy gates.
- It is not full Bash. Shell mode is a Bash-style compatibility layer for common agent commands.
- It is not full Python. Python mode is source-to-source compatibility for practical scripts.

## Repository Layout

- `cli/` - the `cpsl-cli` package and command-line entry point
- `core/` - the `cpsl-core` sandbox runtime and built-in modules
- `modules/` - native support crates used by CPSL modules
- `runtime/` - Luau runtimes for Python and shell compatibility
- `manifests/` - sandbox image manifests
- `docs/` - design and architecture notes
- `test/` - Python compatibility smoke tests

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for the local build, test, and contribution workflow.
