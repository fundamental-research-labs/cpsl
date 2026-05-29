# CPSL FFI

This crate exposes the CPSL C ABI for loading CPSL as a dynamic sandbox
library. It exports the frozen demo symbols from `include/cpsl.h`, validates the
initial session JSON shape, and evaluates bash requests inside a `/workdir`
mount backed by the configured host directory.

Build and probe the release library with:

```sh
cargo build -p cpsl-ffi --release
cargo test -p cpsl-ffi --test probe -- --ignored
```
