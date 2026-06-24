# Grep/Fff API Parity Plan

## Summary

Make `fs.grep` the shared capsule-facing API for both grep-like providers.
Do not add `grep.grep`, do not change shell `grep`, and do not add generic
module name/provider mapping in this cleanup.

`mod-grep` remains the default regex provider. `mod-fff` fills the same
`fs.grep` API only in fff-only capsules.

## Key Changes

- Keep existing `mod-grep` behavior unchanged: when enabled, it owns `fs.grep`.
- Add a `mod-fff` fallback registration only under:

  ```rust
  #[cfg(all(feature = "mod-fff", not(feature = "mod-grep")))]
  ```

- Keep `fff.grep` as an explicit fff-backed alias; do not make it the
  compatibility target.
- Make fff-backed `fs.grep` support the current common `fs.grep` inputs:
  `pattern`, `path`, `glob`, `max_count`, `files_only`.
- Keep the common `fs.grep` return shape: `file`, `line_number`, `line`,
  `match_text`. If `fff.grep` keeps extra `column`, callers should not rely on
  that through `fs.grep`.
- Split `FS_DOC` grep metadata so `fs.help()` shows grep docs for both
  `mod-grep` and fff-only builds, with regex vs literal wording by provider.

## Docs And Config

- Update CLI/module docs to describe `grep` and `fff` as alternative search
  providers for `fs.grep`.
- Keep current default/`all` feature sets unchanged; when both are compiled,
  `mod-grep` wins and `fff.grep` remains available explicitly.
- Document that pattern semantics differ by provider: `mod-grep` is regex,
  `mod-fff` is literal.
- Leave configurable name/provider mapping as a future extension, not part of
  this PR.

## Test Plan

- Existing `mod-grep` `fs.grep` tests stay green.
- Add fff-only integration tests with
  `--no-default-features --features mod-fs,mod-fff` covering:
  - `fs.grep` exists and returns matches.
  - single file and recursive directory search.
  - `glob`, `max_count`, `files_only`.
  - virtual paths and mount denial.
  - `fs.help()` includes `grep`.
  - `fff.grep` still works.
- Add/default-build test coverage confirming both features compiled together
  still use the existing regex `fs.grep`.

## Assumptions

- "Drop-in replacement" means the capsule API call site is stable as
  `fs.grep(...)`.
- Exact pattern semantics parity is out of scope for this cleanup; regex vs
  literal is documented.
- `fs.tree` is not part of the grep/fff parity work.
