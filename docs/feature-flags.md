# CPSL Feature Flags & Module Selection

## Overview

CPSL uses Cargo feature flags for compile-time module selection. Each module
(`json`, `csv`, `yaml`, etc.) is an optional feature that can be independently
enabled or disabled. This means:

- **Smaller binaries** — only ship the modules you need
- **Faster compiles** — unused modules and their heavy deps are skipped
- **Enforced boundaries** — if `json` compiles without `csv`, it provably doesn't depend on it

## Available Core Features

`core/Cargo.toml` is the source of truth for the complete feature list. The
`default` feature enables `all`, which includes every core module and the
PDFium document backend.

| Capability | Feature flag | Key dependencies |
|------------|--------------|------------------|
| `fs` | `mod-fs` | rand |
| `json` | `mod-json` | serde_json |
| `csv` | `mod-csv` | csv |
| `yaml` | `mod-yaml` | yaml-rust2 |
| `xml` | `mod-xml` | quick-xml |
| `http` | `mod-http` | native-http |
| `compress` | `mod-compress` | zip, tar, flate2, bzip2, xz2, sevenz-rust |
| `doc` | `mod-doc` | calamine, rtf-parser, pulldown-cmark, native-webview-pdf, sha2, hex |
| `plot` | `mod-plot` | plotly |
| `numx` | `mod-numpy` | ndarray, faer, rand, rand_distr |
| `random` | `mod-random` | no extra deps |
| `fuzzy` | `mod-fuzzy` | rapidfuzz |
| `phone` | `mod-phone` | phonenumber |
| `email` | `mod-email` | email_address |
| `country` | `mod-country` | isocountry, iso_currency, strum |
| `datetime` | `mod-datetime` | chrono |
| `image` | `mod-image` | image, imageproc, ab_glyph |
| `base64` | `mod-base64` | no extra deps |
| `fin` | `mod-fin` | no extra deps |
| `yfinance` | `mod-yfinance` | composes `mod-http` and `mod-json` |
| `edgar` | `mod-edgar` | composes `mod-http` and `mod-json` |
| `crypto` | `mod-crypto` | sha2, md-5, hmac, aes-gcm, jsonwebtoken, uuid, hex |
| `regex` | `mod-regex` | regex |
| `html` | `mod-html` | scraper |
| `url` | `mod-url` | url, percent-encoding |
| `qr` | `mod-qr` | qrcode, png |
| `grep` | `mod-grep` | grep-regex, grep-searcher, grep-matcher, ignore, globset |
| `doc` PDF engine | `pdfium-render` | pdfium-render; also enables `mod-doc` |

## Using Feature Flags Directly (Cargo)

```sh
# Build with all modules (default)
cargo build -p cpsl-core

# Build with only json and fs
cargo build -p cpsl-core --no-default-features --features mod-json,mod-fs

# Build the CLI with specific modules
cargo build -p cpsl-cli --no-default-features --features mod-json,mod-fs
```

Feature flags on `cpsl-cli`, `cpsl-ffi`, and other downstream crates forward to
`cpsl-core`.

## Herm CPSL Library Builds

Herm's `--cpsl` demo path loads the native dynamic library from `cpsl-ffi`. It
does not use `cpsl build` or a manifest.

| Profile | Command | Compiled CPSL modules |
|---------|---------|-----------------------|
| Herm demo minimum | `cargo build -p cpsl-ffi --release` | `fs`, `json`, `csv`, `http` |
| All core features | `cargo build -p cpsl-ffi --release --features all` | every `cpsl-core/all` feature listed above |

The output library path is platform-specific:

- Linux: `target/release/libcpsl.so`
- macOS: `target/release/libcpsl.dylib`
- Windows: `target/release/cpsl.dll`

From the CPSL repo root, the complete minimum-profile flow is:

```sh
cargo build -p cpsl-ffi --release
CPSL_LIB="$(pwd)/target/release/libcpsl.so"

cd herm
go build -o herm ./cmd/herm
./herm --cpsl "$CPSL_LIB"
```

To run the same Herm build against an all-features CPSL library, change only the
first build command:

```sh
cargo build -p cpsl-ffi --release --features all
```

Enabling all CPSL features expands the modules available inside
`local_sandbox_exec` and `local_sandbox_exec_bash`. It does not enable Herm's
container-mode tools such as `devenv`, host `git`, or package installation.

The `all` profile pulls native document/PDF dependencies. On Linux that can
require GTK/WebKit development packages, and PDF-related tests may also need
PDFium.

## Using `cpsl.toml` (Recommended)

Instead of passing feature flags manually, write a `cpsl.toml`:

```toml
[sandbox]
name = "my-tool"

[modules]
fs = true
json = true
csv = true

[python]
enabled = true

[http]
allowed_domains = ["api.example.com"]
denied_domains = []
```

Then build and run:

```sh
cpsl build -t my-tool              # reads ./cpsl.toml, compiles binary
cpsl run my-tool -i                # interactive REPL
cpsl run my-tool script.luau       # execute a script
cpsl run my-tool -- 'print(1+1)'   # inline code
```

### Config Sections

**`[sandbox]`** — Required. `name` is a human-readable label.

**`[modules]`** — Required. Each key is a module name (see table above). Set to `true` to include, omit or set `false` to exclude. Unknown module names cause a validation error.

**`[python]`** — Optional. Set `enabled = true` to include Python-to-Luau transpiler support.

**`[mounts]`** — Optional. `volumes` is a list of mount specs (`host:virtual[:ro]`).

**`[http]`** — Optional. `allowed_domains` and `denied_domains` configure the HTTP gateway. Never put credentials here — inject them at runtime via host application hooks.

### Example Configs

See `manifests/` for ready-to-use sandbox image manifests:
- `minimal.toml` — Filesystem, JSON, and CSV
- `json-only.toml` — Just JSON + filesystem
- `data-science.toml` — Structured data + numerical computing + plotting
- `full.toml` — Broad CLI-registered module set with Python enabled
- `all.toml` — Broad CLI-registered module set

`cpsl build` currently accepts the module registry exposed by the CLI:
`fs`, `json`, `csv`, `yaml`, `xml`, `http`, `compress`, `doc`, `plot`, and
`numx`. Use direct Cargo feature builds for core modules that are not yet
manifest-exposed.

## Downstream Consumers

Any Rust crate can depend on `cpsl-core` with selected features:

```toml
# In your Cargo.toml
[dependencies]
cpsl-core = { path = "path/to/cpsl/core", features = ["mod-json", "mod-fs"] }
```

The desktop app uses `features = ["all"]` to include everything.
