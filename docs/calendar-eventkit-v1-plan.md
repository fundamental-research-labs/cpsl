# CPSL Calendar EventKit V1 Plan

## Summary

Build `calendar` as a CPSL built-in module for iOS and macOS Calendar events,
backed by EventKit through a new `modules/native-calendar` crate. V1 is
deliberately small: full-access-only, events-only, explicit permission request,
no recurrence editing, no reminders, no attendees, and no implicit prompts.

This plan was reviewed by three subagents from the angles of CPSL module
architecture, EventKit platform design, and testing/build compatibility. They
converged on the native crate plus thin CPSL facade architecture.

## Key Changes

- Add `modules/native-calendar` with plain Rust request/result types,
  `CalendarGateway`, an injectable backend abstraction for tests,
  `CalendarError`, an Apple EventKit backend, and an unsupported backend for
  non-Apple builds.
- Add `core/src/calendar.rs` as the thin Luau facade: docs, argument
  validation, table conversion, RFC3339 parsing/formatting through `chrono`,
  and calls into `CalendarGateway`.
- Add `mod-calendar = ["dep:native-calendar", "dep:chrono"]` to
  `core/Cargo.toml`; include it in `core/all` only once the non-Apple
  unsupported backend compiles cleanly.
- Do not add `calendar` to CLI `MODULE_REGISTRY` or manifest presets in V1.
  Target bundled/signed iOS and macOS host embeddings first; standalone CLI/TCC
  packaging can be a later pass.
- Register `calendar` only when `mod-calendar` is compiled, using a platform
  default gateway or an injected gateway from
  `SandboxBuilder::calendar_gateway(...)`.

## Public API

```text
calendar.status() -> table
calendar.request_access("full") -> table
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
  code; native code bridges to and from `NSDate`.
- All operations require full access. Missing access returns a clear
  `calendar: full access required` style error.
- Recurring event mutations are rejected in V1. Fetching occurrences is allowed
  as normal EventKit query output.
- EventKit change notifications and stale native objects are handled by
  returning snapshots only. V1 never exposes native handles.

## Test Plan

- Core mock-backed tests for every API: ordered Lua calls, shell/table-form
  calls, Python-transpiled call shape where relevant, bad args, missing access,
  denied/restricted/not-determined, unsupported platform, and help output.
- Native crate tests for gateway behavior, unsupported backend, error mapping,
  and mock backend injection.
- Non-Apple CI must compile `mod-calendar` and return deterministic unsupported
  errors without linking Apple frameworks.
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
- Public module name is `calendar` despite overlap with Python's standard
  library module name.
- Event identifiers are not treated as durable sync IDs. Callers should re-query
  by time range if an ID goes stale.

## References

- `docs/module-api-design.md` for CPSL cross-language module API rules.
- `docs/modules.md` for built-in module registration and feature-gating rules.
- Apple EventKit `EKEventStore`, full-access request, status, event query,
  save, and remove APIs.
- Apple Calendar usage description keys and Calendar entitlement docs.
- `objc2-event-kit` crate metadata and EventKit bindings.
