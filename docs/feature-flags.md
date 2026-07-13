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
`default` feature enables `all`, which includes the cross-platform core module
set and the PDFium document backend. Platform-hosted native modules such as
Apple Calendar are opt-in and are not included in `all`.

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
| `grep` provider `ripgrep` | `mod-ripgrep` | grep-regex, grep-searcher, grep-matcher, ignore, globset |
| `grep` provider `fff` | `mod-fff` | fff-grep, grep-regex, grep-matcher, ignore, memchr, globset |
| `doc` PDF engine | `pdfium-render` | pdfium-render; also enables `mod-doc` |
| Apple `calendar` | `mod-apple-calendar` | apple-calendar, chrono, EventKit |

`ripgrep` and `fff` are internal providers for the capsule-facing
`fs.grep(...)` API. In `cpsl.toml`, select exactly one with
`grep = { provider = "ripgrep" }` or `grep = { provider = "fff" }` and include
`fs = true`. `fs.grep(...)` accepts `mode = "regex"` and `mode = "plain"`;
`regex` is the default for both providers. Capsule manifests expose search
through `fs.grep(...)`; the provider names are configuration details, not
standalone runtime modules.

`mod-apple-calendar` is Apple-only and intentionally excluded from `all`, CLI
module manifests, and default capsule presets. It is for signed or bundled
macOS/iOS host embeddings that can satisfy EventKit privacy requirements. The
runtime global is `calendar`; host applications may inject a gateway with
`SandboxBuilder::calendar_gateway(...)`, otherwise Apple builds use the platform
EventKit gateway. Enabling this feature on non-Apple targets fails at build time
with a clear error instead of registering a stub module.

## Using Feature Flags Directly (Cargo)

```sh
# Build with all modules (default)
cargo build -p cpsl-core

# Build with only json and fs
cargo build -p cpsl-core --no-default-features --features mod-json,mod-fs

# Build an Apple-hosted calendar capsule/library on macOS or iOS targets
cargo build -p cpsl-core --no-default-features --features mod-apple-calendar

# Build the CLI with specific modules
cargo build -p cpsl-cli --no-default-features --features mod-json,mod-fs
```

Feature flags on `cpsl-cli`, `cpsl-ffi`, and other downstream crates forward to
`cpsl-core`.

## CPSL FFI Library Builds

The `cpsl-ffi` crate builds a native dynamic library that downstream
applications can load through the C ABI. It does not use `cpsl build` or a
manifest.

| Profile | Command | Compiled CPSL modules |
|---------|---------|-----------------------|
| Minimal FFI | `cargo build -p cpsl-ffi --release` | `fs`, `json`, `csv`, `http`, `ripgrep` |
| Embedded agent | `cargo build -p cpsl-ffi --release --no-default-features --features embedded-agent` | minimal profile plus `yaml`, `xml`, `doc` + PDFium, `plot`, `numpy`, `random`, `fuzzy`, `phone`, `email`, `country`, `datetime`, `image`, `base64`, `fin`, `regex`, `html`, `url`, `qr` |
| All core features | `cargo build -p cpsl-ffi --release --features all` | every `cpsl-core/all` feature listed above |

The output library path is platform-specific:

- Linux: `target/release/libcpsl.so`
- macOS: `target/release/libcpsl.dylib`
- Windows: `target/release/cpsl.dll`

Downstream consumers choose the modules they compile into their library build.

The `all` profile pulls native document/PDF dependencies. On Linux that can
require GTK/WebKit development packages, and PDF-related tests may also need
PDFium.

Hosts can enable AI-backed `doc.read` through
`SandboxBuilder::vision_callback(...)` or the C ABI
`cpsl_session_new_with_host_callbacks_v3`. CPSL keeps provider credentials out
of the sandbox: it passes borrowed image inputs and the extraction prompt to the
host callback. With PDFium enabled, a PDF is rendered to page PNGs and all
pages are supplied together for one multimodal request. `doc.read` defaults to
vision for images and PDFs when this callback exists, while other formats
default to local structural parsing; `opts.mode` overrides that choice per
read.

Apple Calendar host packaging requirements:

- Minimum OS: iOS 17 or macOS 14, because V1 uses
  `requestFullAccessToEvents`.
- Info.plist must include `NSCalendarsFullAccessUsageDescription`.
- Sandboxed macOS hosts need the Calendar entitlement
  `com.apple.security.personal-information.calendars`.
- V1 requests full access only and supports events only; reminders, attendees,
  recurrence editing, and implicit permission prompts are not supported.

## Using `cpsl.toml` (Recommended)

Instead of passing feature flags manually, write a `cpsl.toml`:

```toml
[sandbox]
name = "my-tool"

[modules]
fs = true
grep = { provider = "ripgrep" }
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

**`[modules]`** — Required. Boolean modules are set to `true` to include, omitted or set to `false` to exclude. The `grep` capability must use `grep = { provider = "ripgrep" }` or `grep = { provider = "fff" }` and requires `fs = true`. Unknown module names and standalone provider entries such as `ripgrep = true` or `fff = true` cause validation errors.

**`[python]`** — Optional. Set `enabled = true` to include Python-to-Luau transpiler support.

**`[mounts]`** — Optional. `volumes` is a list of mount specs (`host:virtual[:ro]`).

**`[http]`** — Optional. `allowed_domains` and `denied_domains` configure the HTTP gateway. Never put credentials here — inject them at runtime via host application hooks.

### Example Configs

See `manifests/` for ready-to-use sandbox image manifests:
- `minimal.toml` — Filesystem, grep search, JSON, and CSV
- `json-only.toml` — Just JSON + filesystem
- `data-science.toml` — Structured data + numerical computing + plotting
- `full.toml` — Broad CLI-registered module set with Python enabled
- `all.toml` — Broad CLI-registered module set

`cpsl build` currently accepts the module registry exposed by the CLI:
`fs`, `json`, `csv`, `yaml`, `xml`, `http`, `compress`, `doc`, `plot`, `numx`,
and `grep = { provider = "ripgrep" }` or `grep = { provider = "fff" }`. Use
direct Cargo feature builds for core modules that are not yet manifest-exposed.

## Downstream Consumers

Any Rust crate can depend on `cpsl-core` with selected features:

```toml
# In your Cargo.toml
[dependencies]
cpsl-core = { path = "path/to/cpsl/core", features = ["mod-json", "mod-fs"] }
```

The desktop app uses `features = ["all"]` to include everything.
