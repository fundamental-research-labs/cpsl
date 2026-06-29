# Module Architecture

## Module Manifest

Every built-in module is described by a `ModuleManifest`:

```rust
pub struct ModuleManifest {
    pub name: &'static str,         // "json"
    pub description: &'static str,  // "JSON encoding and decoding"
    pub cargo_feature: &'static str, // "mod-json"
}
```

The CLI's `MODULE_REGISTRY` in `cli/src/config.rs` lists boolean modules for
config validation and the `cpsl modules` command. Provider-backed capabilities
such as `grep` are validated separately because one public capability can map to
more than one Cargo feature.

The real sources of truth are:

- **`core/Cargo.toml` `[features]` section** — the complete list of available modules and their dependencies. The `all` feature flag enables the default cross-platform module set; platform-hosted modules can be opt-in only.
- **`core/src/sandbox.rs` `register_*_globals` calls** — what actually gets loaded into the Luau runtime. Each call is gated by `#[cfg(feature = "mod-*")]`.
- **`MODULE_REGISTRY`** — used by the CLI for boolean module output, config validation (`to_cargo_features()`, `find_module(name)`), and mapping module names to Cargo feature strings.
- **Provider capability validation** — special-case config such as `grep = { provider = "ripgrep" }`, which maps the public `grep` capability to an internal provider feature.

Adding a new built-in module requires: adding a feature flag to `Cargo.toml`, implementing the `register_*_globals` function, adding the `#[cfg(feature)]`-gated call in `sandbox.rs`, and optionally adding a `ModuleManifest` entry to `MODULE_REGISTRY` for CLI support.

Apple Calendar is an opt-in platform module, not a CLI manifest module in V1.
Its feature is `mod-apple-calendar`, its native crate is
`modules/apple-calendar`, and the runtime global is `calendar` only for
Apple-targeted host embeddings. It is not listed in `MODULE_REGISTRY`, is not
included in manifest presets, and is not part of `all` because enabling it on
non-Apple targets fails at build time. Host applications can inject
`SandboxBuilder::calendar_gateway(...)`; otherwise Apple builds use the
platform EventKit gateway.

## Config Format (`cpsl.toml`)

Modules are declared in the `[modules]` section. Boolean built-in modules and
provider-backed capabilities use different forms.

### Built-in modules (current)

```toml
[modules]
json = true
csv = true
yaml = false   # explicitly disabled
```

A boolean value enables or disables a built-in module. Omitted modules are not included.

### Grep provider capability

`grep` is not a standalone provider module. It is a public capability exposed as
`fs.grep(...)`, so `fs = true` is required and the provider must be selected
explicitly:

```toml
[modules]
fs = true
grep = { provider = "ripgrep" }
```

```toml
[modules]
fs = true
grep = { provider = "fff" }
```

`fs.grep(...)` supports `mode = "regex"` and `mode = "plain"` with `regex` as
the default for both providers.

`grep = true`, `grep = false`, missing providers, unknown providers, and
standalone `ripgrep = true` or `fff = true` entries are invalid in capsule
manifests.

### External modules (future, forward-compatible schema)

```toml
[modules]
json = true
custom-parser = { source = "github.com/someone/cpsl-mod-custom-parser" }
```

The `{ source = "..." }` form is parsed today but **rejected at validation** with:

```
external modules not yet supported — use built-in modules (module 'custom-parser' has source = "...")
```

This ensures the config schema is forward-compatible. When external module support ships, existing configs with `source` fields will start working without format changes.

### Internal representation

Both forms deserialize into `ModuleEntry`:

```rust
enum ModuleEntry {
    Enabled(bool),                   // json = true
    Config(ModuleConfig),            // grep = { provider = "ripgrep" }
}
```

`ModuleEntry::is_enabled()` returns `true` for `Enabled(true)` and table config
entries. `ModuleEntry::is_external()` distinguishes future `source` entries from
built-in provider config.

## Planned: External Module Workflow

Tracked by roadmap issue [#18](https://github.com/fundamental-research-labs/cpsl/issues/18). This work is source-level module authoring and resolution, not CPSL Hub artifact distribution. CPSL Hub may later link to compatible module metadata, but community modules must be able to live in their own repositories without being hosted by Hub.

The eventual workflow for external modules:

1. **Author** creates a crate that exports a `register_*_globals(lua)` function and a `ModuleManifest`
2. **User** references it in `cpsl.toml`: `my-mod = { source = "github.com/author/cpsl-mod-foo" }`
3. **`cpsl build`** fetches the source, adds it as a Cargo dependency, and compiles it alongside built-in modules
4. **The built binary** includes the external module just like a built-in one

### Open design questions

- **Security**: External modules get full access to the Luau VM. Should there be a permission model? Sandboxing within sandboxing?
- **Version pinning**: `source = "github.com/..."` needs a version/rev specifier. Likely `source = "github.com/foo@v1.2.3"` or a separate `version` field.
- **Registry**: Should there be a central module registry (like crates.io) or is Git-based resolution sufficient?
- **Binary distribution**: Can external modules ship pre-compiled `.a` files to avoid requiring a Rust toolchain? This intersects with the self-contained distribution research in `docs/distribution.md` and any future build-service issue.

## Module Boundaries

Each module is self-contained:

- **Source**: One file per module (`json.rs`, `csv_mod.rs`, `yaml.rs`, etc.) in `core/src/`
- **Entry point**: `register_*_globals(lua)` — registers all globals for that module
- **Documentation**: `*_DOC` static — module-level help text
- **Dependencies**: Gated by `#[cfg(feature = "mod-*")]` — a module's deps are only compiled when the feature is enabled
- **No cross-module imports**: Modules don't import from each other. If `json` compiles without `csv`, it provably can't depend on it.

This isolation is what makes external modules feasible — the contract is simple and the boundaries are enforced at compile time.
