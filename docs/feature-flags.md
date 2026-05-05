# Cpsl Feature Flags & Module Selection

## Overview

Cpsl uses Cargo feature flags for compile-time module selection. Each module (json, csv, yaml, etc.) is an optional feature that can be independently enabled or disabled. This means:

- **Smaller binaries** — only ship the modules you need
- **Faster compiles** — unused modules and their heavy deps are skipped
- **Enforced boundaries** — if `json` compiles without `csv`, it provably doesn't depend on it

## Available Modules

| Module     | Feature flag     | Key dependencies                                              |
|------------|------------------|---------------------------------------------------------------|
| fs         | `mod-fs`         | rand                                                          |
| json       | `mod-json`       | serde_json                                                    |
| csv        | `mod-csv`        | csv                                                           |
| yaml       | `mod-yaml`       | yaml-rust2                                                    |
| xml        | `mod-xml`        | quick-xml                                                     |
| http       | `mod-http`       | native-http                                                   |
| compress   | `mod-compress`   | zip, tar, flate2, bzip2, xz2, sevenz-rust                    |
| doc        | `mod-doc`        | calamine, pdf-extract, rtf-parser, pulldown-cmark, native-webview-pdf |
| plot       | `mod-plot`       | plotters                                                      |
| numx       | `mod-numpy`      | ndarray, faer, rand, rand_distr                               |
| random     | `mod-random`     | (no extra deps — uses math.random)                            |
| fuzzy      | `mod-fuzzy`      | rapidfuzz                                                     |
| phone      | `mod-phone`      | phonenumber                                                   |
| email      | `mod-email`      | email_address                                                 |
| country    | `mod-country`    | isocountry, iso_currency, strum                               |
| datetime   | `mod-datetime`   | chrono                                                        |
| image      | `mod-image`      | image, imageproc, ab_glyph                                    |
| base64     | `mod-base64`     | (no extra deps — pure computation)                            |
| fin        | `mod-fin`        | (no extra deps — pure computation)                            |
| yfinance   | `mod-yfinance`   | mod-http, mod-json (composes http + json for Yahoo Finance)   |
| edgar      | `mod-edgar`      | mod-http, mod-json (composes http + json for SEC EDGAR)       |
| ocr        | `mod-ocr`        | ocrs, rten, image                                             |
| crypto     | `mod-crypto`     | sha2, md-5, hmac, aes-gcm, jsonwebtoken, uuid, hex           |
| regex      | `mod-regex`      | regex                                                         |
| html       | `mod-html`       | scraper                                                       |
| url        | `mod-url`        | url, percent-encoding                                         |
| qr         | `mod-qr`         | qrcode, png                                                   |

The `default` feature enables `all`, which includes every module.

## Using Feature Flags Directly (Cargo)

```bash
# Build with all modules (default)
cargo build -p cpsl-core

# Build with only json and fs
cargo build -p cpsl-core --no-default-features --features mod-json,mod-fs

# Build CLI with specific modules
cargo build -p cpsl-cli --no-default-features --features mod-json,mod-fs
```

Feature flags on `cpsl-cli` forward to `cpsl-core` automatically.

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

```bash
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

See `examples/` for ready-to-use configs:
- `json-only.toml` — Minimal: just JSON + filesystem
- `data-science.toml` — Structured data + numerical computing + plotting
- `full.toml` — All modules

## Downstream Consumers

Any Rust crate can depend on `cpsl-core` with selected features:

```toml
# In your Cargo.toml
[dependencies]
cpsl-core = { path = "path/to/cpsl-core", features = ["mod-json", "mod-fs"] }
```

The desktop app uses `features = ["all"]` to include everything.
