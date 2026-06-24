# Grep Provider Config Plan

## Summary

Make `grep = { provider = "..." }` the only way to include grep functionality in
a CPSL capsule. The `grep` key becomes a capability selected by provider, not a
provider module name.

Supported capsule config:

```toml
[modules]
fs = true
grep = { provider = "fff" }
```

and:

```toml
[modules]
fs = true
grep = { provider = "ripgrep" }
```

No legacy boolean or provider-module inclusion behavior is required.

## Public Config Contract

- `grep = { provider = "fff" }` compiles the fff provider and exposes the common
  grep API as `fs.grep(...)`.
- `grep = { provider = "ripgrep" }` compiles the ripgrep provider and exposes
  the common grep API as `fs.grep(...)`.
- `grep = true` is invalid.
- Standalone provider entries such as `fff = true` or `ripgrep = true` are not
  valid ways to include grep functionality in a capsule.
- `fs = true` is required when `grep = { provider = ... }` is configured, since
  the public API is `fs.grep`.
- Unknown providers are rejected with a message listing `fff` and `ripgrep`.

## Key Changes

- Extend module config parsing to support a provider object for the `grep`
  capability.
- Keep normal boolean module entries for unrelated modules.
- Map `grep.provider = "fff"` to the `mod-fff` Cargo feature.
- Map `grep.provider = "ripgrep"` to the `mod-ripgrep` Cargo feature.
- Register `fs.grep` through the shared grep API layer from the parity plan, so
  both providers expose identical arguments, return shape, and error style.
- Remove `fff` and `ripgrep` from the user-facing built-in module registry as
  standalone capsule modules if their only purpose is providing grep.
- Update built-in manifests to use the explicit provider object.
- Update docs so `grep` is documented as a capability with providers, not as a
  concrete implementation.

## Validation Rules

- Accept exactly one provider for `grep`.
- Reject `grep = true`, `grep = false`, missing `provider`, and unknown provider
  values.
- Reject provider module booleans such as `fff = true` and `ripgrep = true`.
- Reject `grep = { provider = "..." }` unless `fs = true` is also present.
- Do not silently choose a default provider.

## Test Plan

- Config parsing accepts both supported provider forms.
- Config parsing rejects boolean `grep`, standalone provider booleans, missing
  provider, unknown provider, and missing `fs = true`.
- Feature translation returns the selected provider feature and does not include
  unselected grep providers.
- Manifest build tests cover one fff capsule and one ripgrep capsule.
- Runtime tests verify both provider configs expose `fs.grep` with the same
  input fields and return shape.

## Assumptions

- Breaking existing manifests is acceptable for this follow-up.
- Provider-specific globals such as `fff.grep` are not required for capsule
  inclusion once provider config is implemented.
- Superseded: `fs.grep(...)` now accepts `mode = "regex"` and `mode = "plain"`;
  `regex` is the default for both providers.
