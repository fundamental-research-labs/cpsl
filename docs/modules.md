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

The CLI's `MODULE_REGISTRY` in `cli/src/config.rs` lists a subset of modules for config validation and the `cpsl modules` command. However, it is **not** the single source of truth — it only covers ~10 modules while there are 27+.

The real sources of truth are:

- **`core/Cargo.toml` `[features]` section** — the complete list of available modules and their dependencies. The `all` feature flag enables every module.
- **`core/src/sandbox.rs` `register_*_globals` calls** — what actually gets loaded into the Luau runtime. Each call is gated by `#[cfg(feature = "mod-*")]`.
- **`MODULE_REGISTRY`** — used by the CLI for `cpsl modules` output, config validation (`to_cargo_features()`, `find_module(name)`), and mapping module names to Cargo feature strings.

Adding a new built-in module requires: adding a feature flag to `Cargo.toml`, implementing the `register_*_globals` function, adding the `#[cfg(feature)]`-gated call in `sandbox.rs`, and optionally adding a `ModuleManifest` entry to `MODULE_REGISTRY` for CLI support.

## Config Format (`cpsl.toml`)

Modules are declared in the `[modules]` section. Two forms are supported:

### Built-in modules (current)

```toml
[modules]
json = true
csv = true
yaml = false   # explicitly disabled
```

A boolean value enables or disables a built-in module. Omitted modules are not included.

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
    External { source: String },     // json = { source = "..." }
}
```

`ModuleEntry::is_enabled()` returns `true` for `Enabled(true)` and all `External` entries. `ModuleEntry::is_external()` distinguishes the two forms.

## Planned: External Module Workflow

The eventual workflow for external modules:

1. **Author** creates a crate that exports a `register_*_globals(lua)` function and a `ModuleManifest`
2. **User** references it in `cpsl.toml`: `my-mod = { source = "github.com/author/cpsl-mod-foo" }`
3. **`cpsl build`** fetches the source, adds it as a Cargo dependency, and compiles it alongside built-in modules
4. **The built binary** includes the external module just like a built-in one

### Open design questions

- **Security**: External modules get full access to the Luau VM. Should there be a permission model? Sandboxing within sandboxing?
- **Version pinning**: `source = "github.com/..."` needs a version/rev specifier. Likely `source = "github.com/foo@v1.2.3"` or a separate `version` field.
- **Registry**: Should there be a central module registry (like crates.io) or is Git-based resolution sufficient?
- **Binary distribution**: Can external modules ship pre-compiled `.a` files to avoid requiring a Rust toolchain? This intersects with the distribution research in Phase 11.

## Module Boundaries

Each module is self-contained:

- **Source**: One file per module (`json.rs`, `csv_mod.rs`, `yaml.rs`, etc.) in `core/src/`
- **Entry point**: `register_*_globals(lua)` — registers all globals for that module
- **Documentation**: `*_DOC` static — module-level help text
- **Dependencies**: Gated by `#[cfg(feature = "mod-*")]` — a module's deps are only compiled when the feature is enabled
- **No cross-module imports**: Modules don't import from each other. If `json` compiles without `csv`, it provably can't depend on it.

This isolation is what makes external modules feasible — the contract is simple and the boundaries are enforced at compile time.
