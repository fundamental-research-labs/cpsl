#![cfg(feature = "mod-apple-calendar")]

use apple_calendar::{
    AccessStatus, AppleCalendarBackend, AppleCalendarError, AppleCalendarGateway,
    CalendarAccessResponse, CalendarInfo, CalendarStatus, CreateEventRequest, EventInfo,
    EventQuery, Result as CalendarResult, UpdateEventRequest,
};
use cpsl_core::{sh_transpile, transpile, Sandbox};
use std::sync::{Arc, Mutex};

const START: &str = "2026-07-01T15:00:00Z";
const END: &str = "2026-07-01T16:00:00Z";
const START_MS: i64 = 1_782_918_000_000;
const END_MS: i64 = 1_782_921_600_000;

#[derive(Debug)]
struct MockCalendarBackend {
    access: Mutex<AccessStatus>,
    calls: Mutex<Vec<String>>,
    last_query: Mutex<Option<EventQuery>>,
    last_create: Mutex<Option<CreateEventRequest>>,
    last_update: Mutex<Option<UpdateEventRequest>>,
}

impl MockCalendarBackend {
    fn new(access: AccessStatus) -> Arc<Self> {
        Arc::new(Self {
            access: Mutex::new(access),
            calls: Mutex::new(Vec::new()),
            last_query: Mutex::new(None),
            last_create: Mutex::new(None),
            last_update: Mutex::new(None),
        })
    }

    fn record(&self, call: &str) {
        self.calls.lock().unwrap().push(call.to_string());
    }

    fn require_full_access(&self) -> CalendarResult<()> {
        if *self.access.lock().unwrap() == AccessStatus::FullAccess {
            Ok(())
        } else {
            Err(AppleCalendarError::FullAccessRequired)
        }
    }
}

#[derive(Clone)]
struct MockBackendHandle(Arc<MockCalendarBackend>);

impl AppleCalendarBackend for MockBackendHandle {
    fn status(&self) -> CalendarResult<CalendarStatus> {
        self.0.record("status");
        let access = *self.0.access.lock().unwrap();
        Ok(CalendarStatus {
            access,
            full_access: access.has_full_access(),
            supported: true,
            platform: "test".into(),
        })
    }

    fn request_full_access(&self) -> CalendarResult<CalendarAccessResponse> {
        self.0.record("request_full_access");
        *self.0.access.lock().unwrap() = AccessStatus::FullAccess;
        Ok(CalendarAccessResponse {
            access: AccessStatus::FullAccess,
            granted: true,
        })
    }

    fn calendars(&self) -> CalendarResult<Vec<CalendarInfo>> {
        self.0.record("calendars");
        self.0.require_full_access()?;
        Ok(vec![calendar("cal-1", "Work"), calendar("cal-2", "Home")])
    }

    fn default_calendar(&self) -> CalendarResult<CalendarInfo> {
        self.0.record("default_calendar");
        self.0.require_full_access()?;
        Ok(calendar("cal-1", "Work"))
    }

    fn events(&self, query: EventQuery) -> CalendarResult<Vec<EventInfo>> {
        self.0.record("events");
        self.0.require_full_access()?;
        *self.0.last_query.lock().unwrap() = Some(query.clone());
        let mut events = vec![
            event("event-1", "Design review", query.start_time, query.end_time),
            event("event-2", "Follow up", query.start_time, query.end_time),
        ];
        if let Some(limit) = query.limit {
            events.truncate(limit);
        }
        Ok(events)
    }

    fn get(&self, event_id: &str) -> CalendarResult<EventInfo> {
        self.0.record("get");
        self.0.require_full_access()?;
        Ok(event(event_id, "Fetched", START_MS, END_MS))
    }

    fn create(&self, request: CreateEventRequest) -> CalendarResult<EventInfo> {
        self.0.record("create");
        self.0.require_full_access()?;
        *self.0.last_create.lock().unwrap() = Some(request.clone());
        Ok(EventInfo {
            id: "created-1".into(),
            calendar_id: request.calendar_id.unwrap_or_else(|| "cal-1".into()),
            title: request.title,
            start_time: request.start_time,
            end_time: request.end_time,
            all_day: request.all_day,
            location: request.location,
            notes: request.notes,
            url: request.url,
        })
    }

    fn update(&self, request: UpdateEventRequest) -> CalendarResult<EventInfo> {
        self.0.record("update");
        self.0.require_full_access()?;
        *self.0.last_update.lock().unwrap() = Some(request.clone());
        Ok(EventInfo {
            id: request.event_id,
            calendar_id: request.calendar_id.unwrap_or_else(|| "cal-1".into()),
            title: request.title.unwrap_or_else(|| "Updated".into()),
            start_time: request.start_time.unwrap_or(START_MS),
            end_time: request.end_time.unwrap_or(END_MS),
            all_day: request.all_day.unwrap_or(false),
            location: request.location,
            notes: request.notes,
            url: request.url,
        })
    }

    fn delete(&self, event_id: &str) -> CalendarResult<bool> {
        self.0.record(&format!("delete:{event_id}"));
        self.0.require_full_access()?;
        Ok(true)
    }
}

fn calendar(id: &str, title: &str) -> CalendarInfo {
    CalendarInfo {
        id: id.into(),
        title: title.into(),
        source: "iCloud".into(),
        color: "#3366ff".into(),
        allows_modifications: true,
    }
}

fn event(id: &str, title: &str, start_time: i64, end_time: i64) -> EventInfo {
    EventInfo {
        id: id.into(),
        calendar_id: "cal-1".into(),
        title: title.into(),
        start_time,
        end_time,
        all_day: false,
        location: Some("Main St".into()),
        notes: Some("Bring forms".into()),
        url: Some("https://example.com/event".into()),
    }
}

fn sandbox_with(mock: Arc<MockCalendarBackend>) -> Sandbox {
    let gateway = Arc::new(AppleCalendarGateway::new(Box::new(MockBackendHandle(mock))));
    Sandbox::builder()
        .calendar_gateway(gateway)
        .build()
        .unwrap()
}

#[test]
fn status_request_access_and_help_work() {
    let mock = MockCalendarBackend::new(AccessStatus::NotDetermined);
    let sandbox = sandbox_with(mock.clone());

    let status = sandbox
        .exec("local s = calendar.status(); return s.access .. '|' .. tostring(s.full_access) .. '|' .. s.platform")
        .unwrap();
    assert_eq!(status, "not_determined|false|test");

    let access = sandbox
        .exec(r#"local r = calendar.request_access("full"); return r.access .. '|' .. tostring(r.granted)"#)
        .unwrap();
    assert_eq!(access, "full_access|true");

    let help = sandbox.exec("calendar.help()").unwrap();
    assert!(help.contains("calendar.events"));

    let global_help = sandbox.exec("help()").unwrap();
    assert!(global_help.contains("calendar"));

    let shrt = include_str!("../../runtime/shrt.luau");
    sandbox.register_module("shrt", shrt).unwrap();
    let shell_help = sandbox
        .exec(r#"local sh = require("shrt"); sh.shell_help()"#)
        .unwrap();
    assert!(shell_help.contains("calendar"));

    let shell_status = sandbox
        .exec(
            &sh_transpile::transpile_sh("calendar status")
                .unwrap()
                .luau_source,
        )
        .unwrap();
    assert!(shell_status.contains("full_access"), "{shell_status}");

    assert_eq!(
        mock.calls.lock().unwrap().as_slice(),
        ["status", "request_full_access", "status"]
    );
}

#[test]
fn calendar_and_event_apis_return_plain_tables() {
    let mock = MockCalendarBackend::new(AccessStatus::FullAccess);
    let sandbox = sandbox_with(mock.clone());

    let calendars = sandbox
        .exec("local c = calendar.calendars(); return #c .. '|' .. c[1].id .. '|' .. c[2].title")
        .unwrap();
    assert_eq!(calendars, "2|cal-1|Home");

    let default_calendar = sandbox
        .exec("local c = calendar.default_calendar(); return c.id .. '|' .. tostring(c.allows_modifications)")
        .unwrap();
    assert_eq!(default_calendar, "cal-1|true");

    let events = sandbox
        .exec(&format!(
            r#"local e = calendar.events("{START}", "{END}", {{calendar_id="cal-1", limit=1}})
return #e .. '|' .. e[1].title .. '|' .. e[1].start_time .. '|' .. e[1].location"#
        ))
        .unwrap();
    assert_eq!(events, "1|Design review|2026-07-01T15:00:00Z|Main St");

    let get = sandbox
        .exec(r#"local e = calendar.get("event-1"); return e.id .. '|' .. e.url"#)
        .unwrap();
    assert_eq!(get, "event-1|https://example.com/event");

    let create = sandbox
        .exec(&format!(
            r#"local e = calendar.create("Dentist", "{START}", "{END}", {{calendar_id="cal-2", location="Clinic", all_day=true}})
return e.id .. '|' .. e.calendar_id .. '|' .. e.title .. '|' .. tostring(e.all_day) .. '|' .. e.location"#
        ))
        .unwrap();
    assert_eq!(create, "created-1|cal-2|Dentist|true|Clinic");

    let update = sandbox
        .exec(&format!(
            r#"local e = calendar.update("event-1", {{title="Moved", start_time="{START}", end_time="{END}"}})
return e.id .. '|' .. e.title .. '|' .. e.start_time"#
        ))
        .unwrap();
    assert_eq!(update, "event-1|Moved|2026-07-01T15:00:00Z");

    let delete = sandbox
        .exec(r#"return tostring(calendar.delete("event-1"))"#)
        .unwrap();
    assert_eq!(delete, "true");

    let query = mock.last_query.lock().unwrap().clone().unwrap();
    assert_eq!(query.start_time, START_MS);
    assert_eq!(query.end_time, END_MS);
    assert_eq!(query.calendar_id.as_deref(), Some("cal-1"));
    assert_eq!(query.limit, Some(1));

    let create = mock.last_create.lock().unwrap().clone().unwrap();
    assert_eq!(create.calendar_id.as_deref(), Some("cal-2"));
    assert_eq!(create.location.as_deref(), Some("Clinic"));
    assert!(create.all_day);

    let update = mock.last_update.lock().unwrap().clone().unwrap();
    assert_eq!(update.title.as_deref(), Some("Moved"));
    assert_eq!(update.start_time, Some(START_MS));
    assert_eq!(update.end_time, Some(END_MS));
}

#[test]
fn table_form_shell_style_and_python_kwargs_are_supported() {
    let mock = MockCalendarBackend::new(AccessStatus::FullAccess);
    let sandbox = sandbox_with(mock.clone());

    let table_form = sandbox
        .exec(&format!(
            r#"local e = calendar.events({{start_time="{START}", end_time="{END}", calendar_id="cal-2", limit=2}})
return #e"#
        ))
        .unwrap();
    assert_eq!(table_form, "2");
    assert_eq!(
        mock.last_query
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .calendar_id
            .as_deref(),
        Some("cal-2")
    );

    let shrt = include_str!("../../runtime/shrt.luau");
    sandbox.register_module("shrt", shrt).unwrap();
    let luau = sh_transpile::transpile_sh(&format!(
        "calendar events {START} {END} --calendar-id cal-1 --limit 1"
    ))
    .unwrap()
    .luau_source;
    let shell_output = sandbox.exec(&luau).unwrap();
    assert!(shell_output.contains("Design review"), "{shell_output}");
    assert_eq!(
        mock.last_query.lock().unwrap().as_ref().unwrap().limit,
        Some(1)
    );

    let py = transpile::transpile(&format!(
        r#"calendar.create("Python", "{START}", "{END}", location="Office")"#
    ))
    .unwrap()
    .luau_source;
    let pyrt = include_str!("../../runtime/pyrt.luau");
    sandbox.setup_python_runtime(pyrt).unwrap();
    sandbox.exec(&py).unwrap();
    assert_eq!(
        mock.last_create
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .location
            .as_deref(),
        Some("Office")
    );
}

#[test]
fn validation_errors_are_clear() {
    let mock = MockCalendarBackend::new(AccessStatus::FullAccess);
    let sandbox = sandbox_with(mock);

    let err = sandbox
        .exec(r#"calendar.request_access("write")"#)
        .unwrap_err();
    assert!(
        err.message.contains("access must be \"full\""),
        "msg: {}",
        err.message
    );

    let err = sandbox
        .exec(&format!(r#"calendar.events("not-time", "{END}")"#))
        .unwrap_err();
    assert!(
        err.message.contains("expected RFC3339 timestamp"),
        "msg: {}",
        err.message
    );

    let err = sandbox
        .exec(&format!(r#"calendar.events("{END}", "{START}")"#))
        .unwrap_err();
    assert!(
        err.message.contains("end_time must be after start_time"),
        "msg: {}",
        err.message
    );

    let err = sandbox
        .exec(&format!(
            r#"calendar.events("{START}", "{END}", {{limit="many"}})"#
        ))
        .unwrap_err();
    assert!(
        err.message.contains("option 'limit' expected number"),
        "msg: {}",
        err.message
    );
}

#[test]
fn missing_access_errors_are_returned_from_gateway() {
    for status in [
        AccessStatus::NotDetermined,
        AccessStatus::Restricted,
        AccessStatus::Denied,
    ] {
        let mock = MockCalendarBackend::new(status);
        let sandbox = sandbox_with(mock);
        let err = sandbox
            .exec(&format!(r#"calendar.events("{START}", "{END}")"#))
            .unwrap_err();
        assert!(
            err.message.contains("full access required"),
            "status {:?}, msg: {}",
            status,
            err.message
        );
    }
}
