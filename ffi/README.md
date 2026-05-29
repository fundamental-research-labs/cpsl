# CPSL FFI

This crate is the Phase 2 C ABI skeleton for loading CPSL as a dynamic sandbox
library. It exports the frozen demo symbols from `include/cpsl.h`, validates the
initial session JSON shape, and returns contract-shaped metadata and eval
responses.

It does not run the CPSL bash runtime yet. Valid `cpsl_eval` requests currently
return `ok=false` with `runtime_error`; real bash session evaluation starts in
Phase 3.

Build and probe the release library with:

```sh
cargo build -p cpsl-ffi --release
cargo test -p cpsl-ffi --test probe -- --ignored
```
