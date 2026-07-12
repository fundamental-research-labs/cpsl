# CPSL Apple Calendar EventKit V1 Plan

## Summary

Build `calendar` as the runtime module exposed by Apple-targeted CPSL capsules,
backed by EventKit through a new `modules/apple-calendar` crate. V1 is
deliberately small: full-access-only, events-only, explicit permission request,
no recurrence editing, no reminders, no attendees, and no implicit prompts.

CPSL capsules are platform-specific. An iOS or macOS capsule can expose this
Apple implementation as `calendar`; a future Android capsule can expose its own
`calendar` module backed by Android APIs. V1 should not force a universal
lowest-common-denominator Calendar API across operating systems.

This plan was reviewed by three subagents from the angles of CPSL module
architecture, EventKit platform design, and testing/build compatibility. They
converged on a platform-specific native crate plus thin CPSL facade
architecture.

## Key Changes

- Add `modules/apple-calendar` with plain Rust request/result types,
  `AppleCalendarGateway`, an injectable backend abstraction for tests,
  `AppleCalendarError`, and an EventKit implementation.
- Add `core/src/calendar.rs` as the thin Luau facade: docs, argument
  validation, table conversion, RFC3339 parsing/formatting through `chrono`,
  and calls into the Apple Calendar gateway.
- Add `mod-apple-calendar = ["dep:apple-calendar", "dep:chrono"]` to
  `core/Cargo.toml`.
- Do not add `calendar` to CLI `MODULE_REGISTRY` or manifest presets in V1.
  Target bundled/signed iOS and macOS host embeddings first; standalone CLI/TCC
  packaging can be a later pass.
- Register the runtime `calendar` global only when `mod-apple-calendar` is
  compiled and the capsule target is Apple, using a platform default gateway or
  an injected gateway from
  `SandboxBuilder::calendar_gateway(...)`.
- Do not support multiple Calendar modules in one capsule. The runtime module
  name is `calendar`; the package and feature names identify which platform
  implementation supplies it.

## Platform Model

`modules/apple-calendar` is intentionally Apple-specific. It should not try to
be a permanent abstraction layer for Android, Windows, or future providers.
Shared method names across platform Calendar modules are encouraged when the
semantics genuinely match, but source compatibility is not a V1 requirement.

Future platform variants can use the same runtime name in their own capsules:

- Apple capsules: `modules/apple-calendar`, feature `mod-apple-calendar`,
  runtime global `calendar`.
- Android capsules, later: likely `modules/android-calendar`, feature
  `mod-android-calendar`, runtime global `calendar`.

The agent-facing contract is the capsule's dynamic module help/capability
metadata, not a single static API promised by all platforms.

## Public API

```text
calendar.status() -> table
calendar.request_access("full") -> table
calendar.attach(event_id, paths) -> table
calendar.calendars() -> table
calendar.default_calendar() -> table
calendar.events(start_time, end_time, opts?) -> table
calendar.get(event_id) -> table
calendar.create(title, start_time, end_time, opts?) -> table
calendar.update(event_id, opts) -> table
calendar.delete(event_id) -> boolean
```

`calendar.request_access` accepts only `"full"` in V1. Any other access string
is a validation error.

Return only plain tables. Calendar shape:

```lua
{
  id = "...",
  title = "...",
  source = "...",
  color = "...",
  allows_modifications = true
}
```

Event shape:

```lua
{
  id = "...",
  calendar_id = "...",
  title = "...",
  start_time = "2026-07-01T15:00:00Z",
  end_time = "2026-07-01T16:00:00Z",
  all_day = false,
  location = "...",
  notes = "...",
  url = "..."
}
```

Options stay small and shell-friendly: `calendar_id`, `limit`, `notes`,
`location`, `url`, `all_day`, plus update fields `title`, `start_time`, and
`end_time`.

Example calls:

```lua
calendar.request_access("full")
calendar.attach(event_id, {"/home/herm/report.pdf"})
calendar.events("2026-07-01T00:00:00Z", "2026-07-08T00:00:00Z", {
  calendar_id = id,
  limit = 50
})
calendar.create("Dentist", start_time, end_time, { location = "Main St" })
calendar.update(event_id, { title = "Updated title" })
calendar.delete(event_id)
```

```python
calendar.events("2026-07-01T00:00:00Z", "2026-07-08T00:00:00Z",
                calendar_id=id, limit=50)
calendar.create("Dentist", start_time, end_time, location="Main St")
```

```sh
calendar events 2026-07-01T00:00:00Z 2026-07-08T00:00:00Z --calendar-id "$id" --limit 50
calendar create "Dentist" 2026-07-01T15:00:00Z 2026-07-01T16:00:00Z --location "Main St"
```

## Native Behavior

- Use `objc2-event-kit` 0.3.x with target-specific Apple dependencies; do not
  add a Swift or C shim.
- Use modern EventKit APIs only: iOS 17+ and macOS 14+
  `requestFullAccessToEvents`. Older Apple OS versions return `UnsupportedOs`.
- Keep `EKEventStore` private inside the Apple backend, isolated behind a
  serial worker or actor. Never move EventKit objects across CPSL/Lua
  boundaries.
- Core validates RFC3339 timestamps and passes typed request structs to native
  code; `modules/apple-calendar` bridges to and from `NSDate`.
- All operations require full access. Missing access returns a clear
  `calendar: full access required` style error.
- Recurring event mutations are rejected in V1. Fetching occurrences is allowed
  as normal EventKit query output.
- EventKit has no public native file-attachment API. `calendar.attach` is a
  Herm-managed association: it copies files to durable sandbox storage and
  records their virtual paths in a delimited notes block. Event results expose
  those paths as `attachments`; hosts may render them as openable files.
- EventKit change notifications and stale native objects are handled by
  returning snapshots only. V1 never exposes native handles.

## Test Plan

- Core mock-backed tests for every API: ordered Lua calls, shell/table-form
  calls, Python-transpiled call shape where relevant, bad args, missing access,
  denied/restricted/not-determined, and help output.
- `modules/apple-calendar` tests for gateway behavior, error mapping, and mock
  backend injection.
- Non-Apple CI should not be required to build or expose `mod-apple-calendar`
  as part of default capsule presets. If the feature is accidentally enabled on
  a non-Apple target, it should fail clearly at build/config time rather than
  pretending to provide Calendar access.
- Apple manual smoke test, ignored by default: request full access, create a
  unique event, list/find it, update it, and delete it.
- Host packaging docs must mention `NSCalendarsFullAccessUsageDescription`, the
  iOS 17/macOS 14 minimums, and the macOS sandbox Calendar entitlement where
  applicable.

## Assumptions

- V1 is full-access-only.
- V1 targets signed or bundled iOS/macOS host embeddings first, not standalone
  CLI Calendar access.
- V1 uses modern EventKit APIs only; no deprecated `requestAccess(to:)` fallback.
- Public runtime module name is `calendar` despite overlap with Python's
  standard library module name.
- Capsules expose at most one `calendar` module. Platform-specific package and
  feature names select the implementation at build time.
- Event identifiers are not treated as durable sync IDs. Callers should re-query
  by time range if an ID goes stale.

## References

- `docs/module-api-design.md` for CPSL cross-language module API rules.
- `docs/modules.md` for built-in module registration and feature-gating rules.
- Apple EventKit `EKEventStore`, full-access request, status, event query,
  save, and remove APIs.
- Apple Calendar usage description keys and Calendar entitlement docs.
- `objc2-event-kit` crate metadata and EventKit bindings.
