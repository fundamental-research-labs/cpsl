# Self-Contained Distribution Research

**Status**: Research only — no implementation yet.

**Goal**: Users install `cpsl`, not Rust. `cpsl build` should work without requiring users to install a compiler toolchain.

## Current State

- `cpsl build` invokes `cargo build` directly, requiring a full Rust toolchain
- Rust toolchain size: **~1.2 GB** per platform (macOS arm64 measured)
  - `lib/`: 486 MB (includes std, core, alloc rlibs + LLVM codegen backend)
  - `bin/`: 60 MB (rustc, cargo, clippy, rustfmt, etc.)
  - `lib/rustlib/<target>/`: 254 MB (target-specific standard library)
- Full dependency tree: **486 crates**
- Binary sizes (release, unstripped):
  - Minimal (fs+json+csv): **6.5 MB**
  - All modules: **12 MB**
- C sys-crate dependencies: `mlua-sys` (Luau VM, compiled from C via `cc`), `bzip2-sys`, `lzma-sys`

## Approach 1: Bundle Rust Toolchain

Ship a pinned Rust toolchain inside the cpsl distribution (e.g., `~/.cpsl/toolchain/`). `cpsl build` uses this private toolchain instead of the system one.

### How it would work

1. `cpsl` distribution includes a pre-downloaded Rust toolchain for the host platform
2. `cpsl build` sets `RUSTUP_HOME` / `CARGO_HOME` (or directly invokes the bundled `rustc`/`cargo`) to use the private toolchain
3. First build also needs the cpsl source tree — either bundled or fetched from a registry
4. Subsequent builds reuse cached compilation artifacts from `~/.cpsl/target/`

### Tradeoffs

| Aspect | Assessment |
|--------|-----------|
| **Distribution size** | ~600-800 MB compressed per platform (toolchain + cpsl source + cargo registry cache). Possibly trim to ~400 MB by stripping clippy, rustfmt, rustdoc, and unused components. Still very large. |
| **Platform matrix** | Need separate bundles for: macOS arm64, macOS x86_64, Linux x86_64, Linux arm64, Windows x86_64. 5 platforms × 600 MB = 3 GB of hosting. |
| **First build time** | Still a full `cargo build` — 2-5 minutes on a modern machine for all modules. Cached rebuilds are fast (~10s for single module change). |
| **Correctness** | Highest. Identical to current behavior — Rust's monomorphization, LTO, and optimization all work normally. |
| **Update mechanism** | Toolchain version is pinned to the cpsl release. Updating cpsl may require re-downloading the toolchain bundle. Could use delta updates. |
| **C compiler requirement** | `mlua-sys`, `bzip2-sys`, and `lzma-sys` compile C code via the `cc` crate. On macOS this needs Xcode CLT (`xcrun --find cc`). On Linux, `gcc` or `clang`. This is an implicit dependency users must have. Could vendor pre-compiled C objects to eliminate this, but it's platform-specific. |
| **Complexity** | Low — mostly packaging and path management. The compilation model doesn't change at all. |

### Verdict

**Viable but heavy.** The distribution size is the main problem. Acceptable for developer tools (Xcode is 12 GB, Android Studio is 1 GB+), but not great for a tool that's supposed to feel lightweight. The C compiler dependency is a secondary annoyance.

---

## Approach 2: Pre-Compiled Module Libraries

Ship platform-specific `.a` (static libraries) for every module. `cpsl build` becomes a linker step — pick the requested module libs and link them into a binary. No Rust compiler needed at all.

### How it would work

1. CI builds each module as a separate static library (`.a` / `.lib`) for each target platform
2. Distribution ships these pre-compiled archives: `libmod_fs.a`, `libmod_json.a`, `libmod_csv.a`, etc. plus `libcpsl_core.a` (the runtime)
3. `cpsl build` selects the requested module `.a` files and invokes `cc` (the system linker) to combine them into a single binary

### Blockers

**Rust's monomorphization makes this extremely difficult.** Rust doesn't produce relocatable object files with stable ABIs. Each crate is compiled with knowledge of its generic instantiations, inline functions, and LTO opportunities. You can't just `ar` together two separately-compiled Rust `.a` files and expect them to link — they share global state, allocator symbols, std library instances, and panic runtime.

Specifically:
- **Generic instantiations**: If `mod-json` and `mod-csv` both use `Vec<u8>`, Rust may inline different versions in each. Linking them together causes duplicate symbol errors or undefined behavior.
- **std library**: Each `.a` expects to link against the same std. The std rlib itself is compiled with specific flags (codegen units, optimization level, target features). Mixing `.a` files compiled with different std versions or flags is undefined.
- **LTO and codegen**: Rust's release builds use thin LTO by default. Pre-compiled `.a` files can't participate in cross-crate LTO. This means larger and slower binaries.
- **`mlua-sys` global state**: The Luau VM has C global state. If it's included in multiple `.a` files, linking produces duplicate symbols. If it's a separate `.a`, all modules must agree on the exact version and compilation flags.
- **`cc` invocation complexity**: Even if you got the `.a` files right, the linker invocation is non-trivial. You need the right system libraries (`-lSystem`, `-framework CoreFoundation` on macOS, `-lpthread -ldl -lm` on Linux), the right search paths, and the right ABI compatibility.

### Could it be made to work?

Theoretically, if all modules were compiled in a single Cargo invocation with `crate-type = ["staticlib"]` and the feature flags, Rust produces a single `.a` with all symbols resolved. But that's just a regular `cargo build` — you need the compiler.

The only way pre-compiled libraries could work is if modules were **C libraries with a C ABI**, not Rust crates. This would mean rewriting the module interface to use `extern "C"` functions, losing all Rust type safety at the boundary. Enormous effort for marginal benefit.

### Verdict

**Not feasible.** Rust's compilation model fundamentally doesn't support mix-and-match static linking of separately-compiled crates. This approach would require either abandoning Rust's compilation model or rewriting all modules as C libraries. The effort vastly exceeds the benefit.

---

## Approach 3: Pre-Built Module Matrix

Ship pre-built binaries for common module combinations. `cpsl build` downloads/copies the closest match.

### How it would work

1. CI pre-builds common module combinations:
   - `fs` (bare minimum)
   - `fs+json` (data transform)
   - `fs+json+csv` (data pipeline)
   - `fs+json+csv+yaml+xml` (config/data processing)
   - `fs+json+csv+yaml+xml+http` (API integration)
   - `fs+json+csv+yaml+xml+http+compress+doc` (full data — no heavy numerical)
   - `all` (everything)
2. `cpsl build` reads the config, finds the smallest pre-built binary that contains all requested modules, and copies/downloads it
3. Custom combinations that don't match any pre-built set fall back to... what? This is the problem.

### Tradeoffs

| Aspect | Assessment |
|--------|-----------|
| **Distribution size** | 7 combos × 5 platforms × 6-12 MB = **210-420 MB** of pre-built binaries. Reasonable if stored as a download registry, large if bundled. |
| **Coverage** | The 7 combos above cover maybe 80-90% of use cases. But any custom combo (e.g., `json+numx` without fs) requires a fallback. |
| **Fallback problem** | Without a compiler, custom combos can't be built. Options: (a) error and tell user to install Rust, (b) use the closest superset binary (wastes modules but works), (c) download from a build service. |
| **Build time** | Near-zero for users — just a file copy. CI build time increases linearly with combinations. |
| **Binary correctness** | Perfect — each binary is a real `cargo build` output. |
| **Staleness** | Every cpsl release must rebuild all combos for all platforms. CI matrix: 7 × 5 = 35 builds per release. |

### Superset strategy

The "closest superset" approach deserves attention. If a user wants `json+csv` but we only pre-built `fs+json+csv`, we give them that. The extra `fs` module costs ~0 bytes (it has no dependencies, just uses std::fs). For heavier modules, the waste is real — giving someone `numx` when they didn't ask for it adds ~5 MB of ndarray/faer.

A practical matrix focused on "tiers" rather than arbitrary combos:

| Tier | Modules | Size (est.) |
|------|---------|-------------|
| `core` | fs, json | ~6.5 MB |
| `data` | fs, json, csv, yaml, xml | ~7 MB |
| `web` | fs, json, csv, yaml, xml, http | ~7.5 MB |
| `full-lite` | fs, json, csv, yaml, xml, http, compress, doc | ~10 MB |
| `full` | all | ~12 MB |

5 tiers × 5 platforms = 25 builds, ~200 MB total.

### Verdict

**Viable and pragmatic.** This is the lowest-friction approach for users. The "closest superset" strategy handles 95%+ of cases without needing a compiler. The remaining 5% (people who want exactly `numx+json` and nothing else) can install Rust. Main downside: the CI matrix and the philosophical impurity of shipping unused modules.

---

## Approach 4: WASM Modules

Each module compiles to a WASM blob. `cpsl build` bundles selected WASM blobs into a single runtime. No native compilation needed.

### How it would work

1. Each module is compiled to a WASM component: `mod_json.wasm`, `mod_csv.wasm`, etc.
2. The cpsl runtime (Luau VM + module loader) is also compiled to WASM, or remains native and loads WASM modules via a WASM runtime (wasmtime, wasmer)
3. `cpsl build` concatenates the selected `.wasm` blobs into a bundle
4. At runtime, the host loads the WASM bundle and executes Luau code with the available modules

### Blockers

**mlua/Luau is fundamentally native.** The Luau VM is a C codebase that mlua wraps via FFI. The key issues:

1. **Luau compiles to WASM... in theory.** Luau is C, so it can target `wasm32-wasi`. But `mlua` uses Rust FFI (`extern "C"`) to call into Luau, which means the Rust code and the C code must target the same platform. Both must be compiled to WASM, and the FFI boundary must work in WASM.

2. **mlua's `send` feature.** We use `mlua` with the `send` feature, which requires `Send + Sync` bounds. WASM is single-threaded — this may or may not cause issues depending on how mlua implements the feature.

3. **File I/O.** The `fs` module uses `std::fs` which maps to WASI filesystem capabilities in WASM. This works but requires a WASI runtime. The sandbox semantics change: instead of OS-level file access mediated by the mount table, you get WASI-level virtual filesystem. The security model is different.

4. **HTTP.** The `native-http` crate uses OS-native HTTP (Foundation/URLSession on macOS). This doesn't exist in WASM. Would need a WASM-compatible HTTP client or WASI-HTTP proposal support.

5. **native-webview-pdf.** Uses macOS WebKit for PDF rendering. Completely impossible in WASM.

6. **Performance.** WASM execution is 1.5-3x slower than native for compute-heavy workloads. For Luau (which is already interpreted), the overhead is Luau interpretation inside WASM VM inside host — two layers of interpretation. The `numx` module with `ndarray`/`faer` linear algebra would be dramatically slower.

7. **Module isolation.** The interesting part: WASM components have natural isolation boundaries. Each module could be a separate WASM component with defined imports/exports. This is actually cleaner than Cargo features for module isolation. But the WASM component model is still maturing.

### Could parts of it work?

A hybrid approach is conceivable: the Luau VM and core runtime are native, but individual modules are WASM components that the native runtime loads. This gives module-level isolation and dynamic loading without compiling the entire stack to WASM.

However, the module ↔ Luau VM interface would need to cross a native-WASM boundary for every function call. A `json.encode(data)` call would: native Luau → FFI to native runtime → WASM boundary → json module in WASM → WASM boundary → back to native. The overhead per call would be significant, and data must be serialized/deserialized across the boundary.

### Verdict

**Not feasible in the near term.** Too many platform-native dependencies (HTTP, PDF, filesystem semantics) are incompatible with WASM. The performance overhead for numerical modules is unacceptable. The WASM component model isn't mature enough for production use. The hybrid approach is architecturally interesting but the per-call overhead makes it impractical for a scripting sandbox where modules are called thousands of times per script.

---

## Recommendation

**Short term (next release): Approach 3 — Pre-Built Module Matrix.**

This gives 95% of users a zero-install experience with minimal engineering effort. The implementation is:

1. CI pipeline builds 5 tier binaries (core, data, web, full-lite, full) for each platform
2. Binaries are hosted as GitHub release assets or an S3 bucket
3. `cpsl build` computes the requested module set, finds the smallest superset tier, downloads the binary
4. The binary is saved to `~/.cpsl/bin/<name>` exactly as today
5. If no tier covers the exact set, warn the user and suggest the closest superset

The UX change is minimal: `cpsl build` goes from "runs cargo" to "downloads a pre-built binary" for most users. Power users who want exact module sets can still install Rust and use `--compile` (or equivalent) to get a custom build.

**Medium term: Approach 1 (bundled toolchain) as the fallback for custom builds.**

For users who need exact module combinations not covered by the pre-built matrix, offer a `cpsl build --compile` mode that downloads a minimal Rust toolchain on first use (lazily, not in the distribution). This covers the remaining 5% of use cases.

The minimal toolchain can be stripped to essentials: `rustc`, `cargo`, target libs. No clippy, no rustfmt, no docs. Estimated ~300-400 MB download, acceptable as a one-time lazy install.

**Long term: Neither.**

Once external modules exist (Phase 10's forward-compatible schema), the module system becomes a registry problem, not a compilation problem. The right long-term answer is likely a build service: `cpsl build` sends the module manifest to a cloud service that compiles and returns the binary. This is how many platforms handle plugin compilation (e.g., Cloudflare Workers). But this is a fundamentally different architecture that depends on having the external module system first.

---

## Summary Table

| Approach | Feasible? | User friction | Distribution size | Build time | Module flexibility |
|----------|-----------|--------------|-------------------|------------|-------------------|
| **1. Bundle toolchain** | Yes | Low (needs C compiler) | ~600 MB | 2-5 min first build | Any combination |
| **2. Pre-compiled .a** | **No** | — | — | — | — |
| **3. Pre-built matrix** | **Yes** | Near-zero | ~200 MB (hosted) | Instant (download) | Tier-based (95% coverage) |
| **4. WASM modules** | **No** (near term) | — | — | — | — |

Recommended path: **3 now, 1 as fallback, build service long term.**
