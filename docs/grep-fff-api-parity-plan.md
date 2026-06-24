# Grep/Fff API Parity Plan

## Summary

Make `fs.grep` the shared capsule-facing API for both grep-like providers.
Do not add `grep.grep`, do not change shell `grep`, and do not add generic
module name/provider mapping in this cleanup.

`mod-ripgrep` remains the default regex provider. `mod-fff` fills the same
`fs.grep` API only in fff-only capsules.

Superseded note: the later provider-config work makes `grep = { provider = "..." }`
the capsule-facing contract and removes provider-specific runtime globals such
as `fff.grep(...)` from capsule builds. The shared API is `fs.grep(...)`.

## Key Changes

- Keep existing `mod-ripgrep` behavior unchanged: when enabled, it owns `fs.grep`.
- Introduce a small shared grep API layer:
  - Typed request/result structs for the common API, such as `GrepRequest`,
    `GrepMatch`, and a files-only result shape.
  - A `GrepProvider` trait implemented by the ripgrep and fff providers.
  - One shared Lua registration helper for `fs.grep` so argument parsing,
    accepted option names, return table shape, and error style cannot drift
    between providers.
- Add a `mod-fff` fallback registration only under:

  ```rust
  #[cfg(all(feature = "mod-fff", not(feature = "mod-ripgrep")))]
  ```

- Keep the compatibility target as `fs.grep(...)`; later provider-config work
  removes provider-specific globals from capsule builds.
- Make fff-backed `fs.grep` support the current common `fs.grep` inputs:
  `pattern`, `path`, `mode`, `glob`, `max_count`, `files_only`.
- Keep the common `fs.grep` return shape: `file`, `line_number`, `line`,
  `match_text`.
- Split `FS_DOC` grep metadata so `fs.help()` shows grep docs for both
  `mod-ripgrep` and fff-only builds.

## Docs And Config

- Update CLI/module docs to describe `ripgrep` and `fff` as alternative search
  providers for `fs.grep`.
- Keep current default/`all` feature sets unchanged; when both are compiled,
  `mod-ripgrep` wins for `fs.grep(...)`.
- Superseded: `fs.grep(...)` now accepts `mode = "regex"` and `mode = "plain"`;
  `regex` is the default for both providers.
- Leave configurable name/provider mapping as a future extension, not part of
  this PR.

## Test Plan

- Existing `mod-ripgrep` `fs.grep` tests stay green.
- Add focused Rust-level tests around the shared API layer where useful, so
  both providers are exercised through the same request/result contract rather
  than separate Lua glue.
- Add fff-only integration tests with
  `--no-default-features --features mod-fs,mod-fff` covering:
  - `fs.grep` exists and returns matches.
  - single file and recursive directory search.
  - `glob`, `max_count`, `files_only`.
  - virtual paths and mount denial.
  - `fs.help()` includes `grep`.
  - Provider-specific globals remain out of the capsule-facing API.
- Add/default-build test coverage confirming both features compiled together
  still use the existing regex `fs.grep`.

## Assumptions

- "Drop-in replacement" means the capsule API call site is stable as
  `fs.grep(...)`.
- Superseded: exact regex/plain semantics are now part of the common
  `fs.grep(...)` contract.
- `fs.tree` is not part of the grep/fff parity work.
