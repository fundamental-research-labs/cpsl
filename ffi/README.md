# CPSL FFI

This crate exposes the CPSL C ABI for loading CPSL as a dynamic sandbox
library. It exports the frozen demo symbols from `include/cpsl.h`, validates the
initial session JSON shape, and evaluates bash requests inside a `/workdir`
mount backed by the configured host directory.

## Build Profiles

The default build is the minimal FFI profile. It keeps the dynamic library small
and enables only `fs`, `json`, `csv`, `http`, and `ripgrep` (`mod-ripgrep`):

```sh
cargo build -p cpsl-ffi --release
```

The embedded agent profile adds the cross-platform Rust-backed utility modules,
plus document/PDF support:

```sh
cargo build -p cpsl-ffi --release --no-default-features --features embedded-agent
```

To test every CPSL core feature compiled into the same library:

```sh
cargo build -p cpsl-ffi --release --features all
```

Both commands produce the same platform library path:

- Linux: `target/release/libcpsl.so`
- macOS: `target/release/libcpsl.dylib`
- Windows: `target/release/cpsl.dll`

Probe the release library with:

```sh
cargo test -p cpsl-ffi --test probe -- --ignored
```

See `../docs/feature-flags.md` for the full module list.
