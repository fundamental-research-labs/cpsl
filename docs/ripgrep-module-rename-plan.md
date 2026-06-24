# Ripgrep Module Rename Plan

## Summary

Rename the existing `grep` provider module to `ripgrep` so provider names match
their implementations and line up with the existing `fff` provider name.

This is a naming cleanup for module/build configuration and Rust feature names.
It should not change the capsule-facing grep API, which remains `fs.grep(...)`
until the provider-config follow-up makes `grep` a provider-selected capability.

## Key Changes

- Rename the manifest/CLI module name from `grep` to `ripgrep`.
- Rename Cargo feature flags from `mod-grep` to `mod-ripgrep` across core, CLI,
  FFI, docs, and tests.
- Rename Rust symbols and docs where they describe the provider, for example
  provider structs/modules/docs should say `ripgrep` rather than generic
  `grep`.
- Keep the actual public runtime API unchanged for this step: enabling
  `ripgrep` still registers the same `fs.grep` implementation.
- Leave `fff` named `fff`.
- Update built-in manifests and examples to use `ripgrep = true` wherever they
  currently use `grep = true`.

## Implementation Notes

- Update feature declarations and forwarding in `core/Cargo.toml`,
  `cli/Cargo.toml`, and `ffi/Cargo.toml`.
- Update `cli/src/config.rs` so `MODULE_REGISTRY` exposes `ripgrep` instead of
  `grep`, with a description that it provides the regex-backed `fs.grep`
  provider.
- Update `#[cfg(feature = "mod-grep")]` gates to `mod-ripgrep`.
- Update test cfg attributes, feature-specific test commands, and docs tables.
- Do not add compatibility aliases for the old `grep` module name in this
  follow-up unless a release migration explicitly requires it.

## Test Plan

- Run the core and CLI tests with default features.
- Run a no-default-features build using `mod-fs,mod-ripgrep`.
- Verify sample manifests using `ripgrep = true` map to `mod-ripgrep`.
- Verify `grep = true` is rejected by config validation after the rename.
- Verify existing `fs.grep` runtime behavior is unchanged under the renamed
  provider.

## Assumptions

- This rename is allowed to be breaking for existing manifests.
- `ripgrep` is the provider name; `grep` is reserved for the later capability
  selected with `grep = { provider = "..." }`.
- `fs.tree` remains separate from the naming decision and should not drive the
  provider name.
