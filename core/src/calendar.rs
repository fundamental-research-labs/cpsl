//! Lua-facing Apple Calendar module backed by an injectable EventKit gateway.

use crate::lua_util::register_help_functions;
use crate::sandbox::{
    arg_error, validate_args, wrap_module_with_help_hints, FieldDoc, FnDoc, ModuleDoc, Param,
    ParamType, ReturnType,
};
use apple_calendar::{
    AccessStatus, AppleCalendarGateway, CalendarAccessResponse, CalendarInfo, CalendarStatus,
    CreateEventRequest, EventInfo, EventQuery, UnixMillis, UpdateEventRequest,
};
use chrono::{DateTime, SecondsFormat, Utc};
use mlua::{Lua, MultiValue, Table, Value};
use std::sync::Arc;

pub type CalendarActivityCallback = Arc<dyn Fn(&str) + Send + Sync>;

const QUERY_OPTS_FIELDS: &[FieldDoc] = &[
    FieldDoc {
        name: "calendar_id",
        typ: "string",
        required: false,
        description: "Limit query to one calendar",
    },
    FieldDoc {
        name: "limit",
        typ: "number",
        required: false,
        description: "Maximum events to return",
    },
];

const EVENT_OPTS_FIELDS: &[FieldDoc] = &[
    FieldDoc {
        name: "calendar_id",
        typ: "string",
        required: false,
        description: "Target calendar id; defaults to the system default calendar",
    },
    FieldDoc {
        name: "notes",
        typ: "string",
        required: false,
        description: "Event notes",
    },
    FieldDoc {
        name: "location",
        typ: "string",
        required: false,
        description: "Event location",
    },
    FieldDoc {
        name: "url",
        typ: "string",
        required: false,
        description: "Event URL",
    },
    FieldDoc {
        name: "all_day",
        typ: "boolean",
        required: false,
        description: "Whether this is an all-day event",
    },
];

const UPDATE_OPTS_FIELDS: &[FieldDoc] = &[
    FieldDoc {
        name: "title",
        typ: "string",
        required: false,
        description: "New title",
    },
    FieldDoc {
        name: "start_time",
        typ: "string",
        required: false,
        description: "New RFC3339 start time",
    },
    FieldDoc {
        name: "end_time",
        typ: "string",
        required: false,
        description: "New RFC3339 end time",
    },
    FieldDoc {
        name: "calendar_id",
        typ: "string",
        required: false,
        description: "Move event to another calendar",
    },
    FieldDoc {
        name: "notes",
        typ: "string",
        required: false,
        description: "New notes",
    },
    FieldDoc {
        name: "location",
        typ: "string",
        required: false,
        description: "New location",
    },
    FieldDoc {
        name: "url",
        typ: "string",
        required: false,
        description: "New URL",
    },
    FieldDoc {
        name: "all_day",
        typ: "boolean",
        required: false,
        description: "Whether this is an all-day event",
    },
];

pub(crate) static CALENDAR_DOC: ModuleDoc = ModuleDoc {
    name: "calendar",
    summary: "Apple Calendar events via EventKit",
    functions: &[
        FnDoc {
            name: "status",
            description: "Return Calendar authorization status.",
            params: &[],
            returns: ReturnType::Table,
            example: Some("local status = calendar.status()"),
        },
        FnDoc {
            name: "request_access",
            description: "Request full Calendar event access. V1 accepts only \"full\".",
            params: &[Param {
                name: "access",
                short: Some('a'),
                typ: ParamType::String,
                required: true,
                fields: None,
            }],
            returns: ReturnType::Table,
            example: Some(r#"calendar.request_access("full")"#),
        },
        FnDoc {
            name: "calendars",
            description: "List event calendars.",
            params: &[],
            returns: ReturnType::Table,
            example: Some("local calendars = calendar.calendars()"),
        },
        FnDoc {
            name: "default_calendar",
            description: "Return the default calendar for new events.",
            params: &[],
            returns: ReturnType::Table,
            example: Some("local cal = calendar.default_calendar()"),
        },
        FnDoc {
            name: "events",
            description: "List events in a time range.",
            params: &[
                Param {
                    name: "start_time",
                    short: Some('s'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "end_time",
                    short: Some('e'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "opts",
                    short: None,
                    typ: ParamType::Table,
                    required: false,
                    fields: Some(QUERY_OPTS_FIELDS),
                },
            ],
            returns: ReturnType::Table,
            example: Some(
                r#"calendar.events("2026-07-01T00:00:00Z", "2026-07-08T00:00:00Z", {limit=50})"#,
            ),
        },
        FnDoc {
            name: "get",
            description: "Fetch one event by EventKit event id.",
            params: &[Param {
                name: "event_id",
                short: Some('i'),
                typ: ParamType::String,
                required: true,
                fields: None,
            }],
            returns: ReturnType::Table,
            example: Some(r#"local event = calendar.get("event-id")"#),
        },
        FnDoc {
            name: "create",
            description: "Create an event.",
            params: &[
                Param {
                    name: "title",
                    short: Some('t'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "start_time",
                    short: Some('s'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "end_time",
                    short: Some('e'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "opts",
                    short: None,
                    typ: ParamType::Table,
                    required: false,
                    fields: Some(EVENT_OPTS_FIELDS),
                },
            ],
            returns: ReturnType::Table,
            example: Some(
                r#"calendar.create("Dentist", start_time, end_time, {location="Main St"})"#,
            ),
        },
        FnDoc {
            name: "update",
            description:
                "Update a non-recurring event. Recurring event mutations are rejected in V1.",
            params: &[
                Param {
                    name: "event_id",
                    short: Some('i'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "opts",
                    short: None,
                    typ: ParamType::Table,
                    required: true,
                    fields: Some(UPDATE_OPTS_FIELDS),
                },
            ],
            returns: ReturnType::Table,
            example: Some(r#"calendar.update(event_id, {title="Updated title"})"#),
        },
        FnDoc {
            name: "delete",
            description: "Delete a non-recurring event.",
            params: &[Param {
                name: "event_id",
                short: Some('i'),
                typ: ParamType::String,
                required: true,
                fields: None,
            }],
            returns: ReturnType::Boolean,
            example: Some(r#"calendar.delete(event_id)"#),
        },
    ],
};

pub(crate) fn register_calendar_globals(
    lua: &Lua,
    gateway: Arc<AppleCalendarGateway>,
    activity_callback: Option<CalendarActivityCallback>,
) -> Result<(), mlua::Error> {
    let calendar = lua.create_table()?;

    {
        let gateway = gateway.clone();
        let activity_callback = activity_callback.clone();
        calendar.set(
            "status",
            lua.create_function(move |lua, args: MultiValue| {
                validate_no_args(&args, "calendar.status")?;
                notify_calendar_activity(&activity_callback, "status");
                status_to_table(lua, gateway.status().map_err(mlua::Error::external)?)
            })?,
        )?;
    }

    {
        let gateway = gateway.clone();
        let activity_callback = activity_callback.clone();
        calendar.set(
            "request_access",
            lua.create_function(move |lua, args: MultiValue| {
                let validated = validate_args(
                    &args,
                    CALENDAR_DOC.params("request_access"),
                    "calendar.request_access",
                )?;
                let access = value_string(&validated[0], "calendar.request_access", "access")?;
                if access != "full" {
                    return Err(mlua::Error::external(format!(
                        "calendar.request_access: access must be \"full\" in V1, got \"{}\"",
                        access
                    )));
                }
                notify_calendar_activity(&activity_callback, "request_access");
                access_to_table(
                    lua,
                    gateway
                        .request_full_access()
                        .map_err(mlua::Error::external)?,
                )
            })?,
        )?;
    }

    {
        let gateway = gateway.clone();
        let activity_callback = activity_callback.clone();
        calendar.set(
            "calendars",
            lua.create_function(move |lua, args: MultiValue| {
                validate_no_args(&args, "calendar.calendars")?;
                notify_calendar_activity(&activity_callback, "calendars");
                calendars_to_table(lua, gateway.calendars().map_err(mlua::Error::external)?)
            })?,
        )?;
    }

    {
        let gateway = gateway.clone();
        let activity_callback = activity_callback.clone();
        calendar.set(
            "default_calendar",
            lua.create_function(move |lua, args: MultiValue| {
                validate_no_args(&args, "calendar.default_calendar")?;
                notify_calendar_activity(&activity_callback, "default_calendar");
                calendar_to_table(
                    lua,
                    &gateway.default_calendar().map_err(mlua::Error::external)?,
                )
            })?,
        )?;
    }

    {
        let gateway = gateway.clone();
        let activity_callback = activity_callback.clone();
        calendar.set(
            "events",
            lua.create_function(move |lua, args: MultiValue| {
                let table_form = single_table_arg(&args);
                let validated =
                    validate_args(&args, CALENDAR_DOC.params("events"), "calendar.events")?;
                let start_time =
                    parse_timestamp_value(&validated[0], "calendar.events", "start_time")?;
                let end_time = parse_timestamp_value(&validated[1], "calendar.events", "end_time")?;
                validate_range(start_time, end_time, "calendar.events")?;
                let opts = opts_table(&validated[2], table_form)?;
                let query = EventQuery {
                    start_time,
                    end_time,
                    calendar_id: optional_string(&opts, "calendar.events", "calendar_id")?,
                    limit: optional_limit(&opts, "calendar.events", "limit")?,
                };
                notify_calendar_activity(&activity_callback, "events");
                events_to_table(lua, gateway.events(query).map_err(mlua::Error::external)?)
            })?,
        )?;
    }

    {
        let gateway = gateway.clone();
        let activity_callback = activity_callback.clone();
        calendar.set(
            "get",
            lua.create_function(move |lua, args: MultiValue| {
                let validated = validate_args(&args, CALENDAR_DOC.params("get"), "calendar.get")?;
                let event_id = value_string(&validated[0], "calendar.get", "event_id")?;
                notify_calendar_activity(&activity_callback, "get");
                event_to_table(lua, &gateway.get(&event_id).map_err(mlua::Error::external)?)
            })?,
        )?;
    }

    {
        let gateway = gateway.clone();
        let activity_callback = activity_callback.clone();
        calendar.set(
            "create",
            lua.create_function(move |lua, args: MultiValue| {
                let table_form = single_table_arg(&args);
                let validated =
                    validate_args(&args, CALENDAR_DOC.params("create"), "calendar.create")?;
                let title = value_string(&validated[0], "calendar.create", "title")?;
                let start_time =
                    parse_timestamp_value(&validated[1], "calendar.create", "start_time")?;
                let end_time = parse_timestamp_value(&validated[2], "calendar.create", "end_time")?;
                validate_range(start_time, end_time, "calendar.create")?;
                let opts = opts_table(&validated[3], table_form)?;
                let request = CreateEventRequest {
                    title,
                    start_time,
                    end_time,
                    calendar_id: optional_string(&opts, "calendar.create", "calendar_id")?,
                    notes: optional_string(&opts, "calendar.create", "notes")?,
                    location: optional_string(&opts, "calendar.create", "location")?,
                    url: optional_string(&opts, "calendar.create", "url")?,
                    all_day: optional_bool(&opts, "calendar.create", "all_day")?.unwrap_or(false),
                };
                notify_calendar_activity(&activity_callback, "create");
                event_to_table(
                    lua,
                    &gateway.create(request).map_err(mlua::Error::external)?,
                )
            })?,
        )?;
    }

    {
        let gateway = gateway.clone();
        let activity_callback = activity_callback.clone();
        calendar.set(
            "update",
            lua.create_function(move |lua, args: MultiValue| {
                let (event_id, opts) = parse_update_args(&args)?;
                let start_time = optional_timestamp(&opts, "calendar.update", "start_time")?;
                let end_time = optional_timestamp(&opts, "calendar.update", "end_time")?;
                if let (Some(start_time), Some(end_time)) = (start_time, end_time) {
                    validate_range(start_time, end_time, "calendar.update")?;
                }
                let request = UpdateEventRequest {
                    event_id,
                    title: optional_string(&opts, "calendar.update", "title")?,
                    start_time,
                    end_time,
                    calendar_id: optional_string(&opts, "calendar.update", "calendar_id")?,
                    notes: optional_string(&opts, "calendar.update", "notes")?,
                    location: optional_string(&opts, "calendar.update", "location")?,
                    url: optional_string(&opts, "calendar.update", "url")?,
                    all_day: optional_bool(&opts, "calendar.update", "all_day")?,
                };
                notify_calendar_activity(&activity_callback, "update");
                event_to_table(
                    lua,
                    &gateway.update(request).map_err(mlua::Error::external)?,
                )
            })?,
        )?;
    }

    {
        let gateway = gateway.clone();
        let activity_callback = activity_callback.clone();
        calendar.set(
            "delete",
            lua.create_function(move |_, args: MultiValue| {
                let validated =
                    validate_args(&args, CALENDAR_DOC.params("delete"), "calendar.delete")?;
                let event_id = value_string(&validated[0], "calendar.delete", "event_id")?;
                notify_calendar_activity(&activity_callback, "delete");
                gateway.delete(&event_id).map_err(mlua::Error::external)
            })?,
        )?;
    }

    register_help_functions(lua, &calendar, &CALENDAR_DOC)?;
    lua.globals().set("calendar", calendar)?;
    wrap_module_with_help_hints(lua, "calendar")?;

    Ok(())
}

fn notify_calendar_activity(callback: &Option<CalendarActivityCallback>, operation: &str) {
    if let Some(callback) = callback {
        callback(operation);
    }
}

fn validate_no_args(args: &MultiValue, fn_name: &str) -> Result<(), mlua::Error> {
    if args.is_empty() {
        Ok(())
    } else {
        Err(mlua::Error::external(format!(
            "{}: expected no arguments, got {}",
            fn_name,
            args.len()
        )))
    }
}

fn single_table_arg(args: &MultiValue) -> Option<Table> {
    if args.len() == 1 {
        if let Some(Value::Table(table)) = args.get(0) {
            return Some(table.clone());
        }
    }
    None
}

fn opts_table(value: &Value, table_form: Option<Table>) -> Result<Option<Table>, mlua::Error> {
    match value {
        Value::Table(table) => Ok(Some(table.clone())),
        Value::Nil => {
            if let Some(table) = table_form {
                match table.get::<Value>("opts")? {
                    Value::Table(opts) => Ok(Some(opts)),
                    Value::Nil => Ok(Some(table)),
                    other => Err(mlua::Error::external(format!(
                        "calendar: argument 'opts' expected table, got {}",
                        other.type_name()
                    ))),
                }
            } else {
                Ok(None)
            }
        }
        other => Err(mlua::Error::external(format!(
            "calendar: argument 'opts' expected table, got {}",
            other.type_name()
        ))),
    }
}

fn parse_update_args(args: &MultiValue) -> Result<(String, Option<Table>), mlua::Error> {
    if let Some(table) = single_table_arg(args) {
        let event_id = table
            .get::<Value>("event_id")
            .or_else(|_| table.get::<Value>(1))?;
        let event_id = value_string(&event_id, "calendar.update", "event_id")?;
        let opts = match table.get::<Value>("opts")? {
            Value::Table(opts) => Some(opts),
            Value::Nil => Some(table),
            other => {
                return Err(mlua::Error::external(format!(
                    "calendar.update: argument 'opts' expected table, got {}",
                    other.type_name()
                )))
            }
        };
        return Ok((event_id, opts));
    }

    let validated = validate_args(args, CALENDAR_DOC.params("update"), "calendar.update")?;
    let event_id = value_string(&validated[0], "calendar.update", "event_id")?;
    let opts = match &validated[1] {
        Value::Table(table) => Some(table.clone()),
        Value::Nil => return Err(arg_error("calendar.update", CALENDAR_DOC.params("update"))),
        other => {
            return Err(mlua::Error::external(format!(
                "calendar.update: argument 'opts' expected table, got {}",
                other.type_name()
            )))
        }
    };
    Ok((event_id, opts))
}

fn value_string(value: &Value, fn_name: &str, name: &str) -> Result<String, mlua::Error> {
    match value {
        Value::String(s) => Ok(s.to_string_lossy().to_string()),
        Value::Nil => Err(mlua::Error::external(format!(
            "{}: missing required argument '{}' (string)",
            fn_name, name
        ))),
        other => Err(mlua::Error::external(format!(
            "{}: argument '{}' expected string, got {}",
            fn_name,
            name,
            other.type_name()
        ))),
    }
}

fn optional_string(
    table: &Option<Table>,
    fn_name: &str,
    key: &str,
) -> Result<Option<String>, mlua::Error> {
    let Some(table) = table else {
        return Ok(None);
    };
    match table.get::<Value>(key)? {
        Value::Nil => Ok(None),
        Value::String(s) => Ok(Some(s.to_string_lossy().to_string())),
        other => Err(mlua::Error::external(format!(
            "{}: option '{}' expected string, got {}",
            fn_name,
            key,
            other.type_name()
        ))),
    }
}

fn optional_bool(
    table: &Option<Table>,
    fn_name: &str,
    key: &str,
) -> Result<Option<bool>, mlua::Error> {
    let Some(table) = table else {
        return Ok(None);
    };
    match table.get::<Value>(key)? {
        Value::Nil => Ok(None),
        Value::Boolean(value) => Ok(Some(value)),
        other => Err(mlua::Error::external(format!(
            "{}: option '{}' expected boolean, got {}",
            fn_name,
            key,
            other.type_name()
        ))),
    }
}

fn optional_limit(
    table: &Option<Table>,
    fn_name: &str,
    key: &str,
) -> Result<Option<usize>, mlua::Error> {
    let Some(table) = table else {
        return Ok(None);
    };
    match table.get::<Value>(key)? {
        Value::Nil => Ok(None),
        Value::Integer(value) if value >= 0 => Ok(Some(value as usize)),
        Value::Number(value) if value >= 0.0 => Ok(Some(value as usize)),
        Value::Integer(_) | Value::Number(_) => Err(mlua::Error::external(format!(
            "{}: option '{}' must be non-negative",
            fn_name, key
        ))),
        other => Err(mlua::Error::external(format!(
            "{}: option '{}' expected number, got {}",
            fn_name,
            key,
            other.type_name()
        ))),
    }
}

fn optional_timestamp(
    table: &Option<Table>,
    fn_name: &str,
    key: &str,
) -> Result<Option<UnixMillis>, mlua::Error> {
    let Some(table) = table else {
        return Ok(None);
    };
    match table.get::<Value>(key)? {
        Value::Nil => Ok(None),
        value => parse_timestamp_value(&value, fn_name, key).map(Some),
    }
}

fn parse_timestamp_value(
    value: &Value,
    fn_name: &str,
    name: &str,
) -> Result<UnixMillis, mlua::Error> {
    let raw = value_string(value, fn_name, name)?;
    DateTime::parse_from_rfc3339(&raw)
        .map(|dt| dt.timestamp_millis())
        .map_err(|_| {
            mlua::Error::external(format!(
                "{}: argument '{}' expected RFC3339 timestamp, got \"{}\"",
                fn_name, name, raw
            ))
        })
}

fn validate_range(
    start_time: UnixMillis,
    end_time: UnixMillis,
    fn_name: &str,
) -> Result<(), mlua::Error> {
    if end_time > start_time {
        Ok(())
    } else {
        Err(mlua::Error::external(format!(
            "{}: end_time must be after start_time",
            fn_name
        )))
    }
}

fn format_timestamp(ms: UnixMillis) -> Result<String, mlua::Error> {
    let dt = DateTime::<Utc>::from_timestamp_millis(ms).ok_or_else(|| {
        mlua::Error::external(format!("calendar: timestamp out of range: {}", ms))
    })?;
    Ok(dt.to_rfc3339_opts(SecondsFormat::Secs, true))
}

fn status_to_table(lua: &Lua, status: CalendarStatus) -> Result<Table, mlua::Error> {
    let table = lua.create_table()?;
    table.set("access", status.access.as_str())?;
    table.set("state", access_state(status.access))?;
    table.set("full_access", status.full_access)?;
    table.set("supported", status.supported)?;
    table.set("platform", status.platform)?;
    Ok(table)
}

fn access_to_table(lua: &Lua, access: CalendarAccessResponse) -> Result<Table, mlua::Error> {
    let table = lua.create_table()?;
    table.set("access", access.access.as_str())?;
    table.set("state", access_state(access.access))?;
    table.set("granted", access.granted)?;
    Ok(table)
}

fn access_state(access: AccessStatus) -> &'static str {
    match access {
        AccessStatus::FullAccess => "granted",
        AccessStatus::NotDetermined => "undefined",
        AccessStatus::Denied
        | AccessStatus::Restricted
        | AccessStatus::WriteOnly
        | AccessStatus::Unknown => "denied",
    }
}

fn calendars_to_table(lua: &Lua, calendars: Vec<CalendarInfo>) -> Result<Table, mlua::Error> {
    let table = lua.create_table()?;
    for (idx, calendar) in calendars.iter().enumerate() {
        table.set(idx + 1, calendar_to_table(lua, calendar)?)?;
    }
    Ok(table)
}

fn calendar_to_table(lua: &Lua, calendar: &CalendarInfo) -> Result<Table, mlua::Error> {
    let table = lua.create_table()?;
    table.set("id", calendar.id.as_str())?;
    table.set("title", calendar.title.as_str())?;
    table.set("source", calendar.source.as_str())?;
    table.set("color", calendar.color.as_str())?;
    table.set("allows_modifications", calendar.allows_modifications)?;
    Ok(table)
}

fn events_to_table(lua: &Lua, events: Vec<EventInfo>) -> Result<Table, mlua::Error> {
    let table = lua.create_table()?;
    for (idx, event) in events.iter().enumerate() {
        table.set(idx + 1, event_to_table(lua, event)?)?;
    }
    Ok(table)
}

fn event_to_table(lua: &Lua, event: &EventInfo) -> Result<Table, mlua::Error> {
    let table = lua.create_table()?;
    table.set("id", event.id.as_str())?;
    table.set("calendar_id", event.calendar_id.as_str())?;
    table.set("title", event.title.as_str())?;
    table.set("start_time", format_timestamp(event.start_time)?)?;
    table.set("end_time", format_timestamp(event.end_time)?)?;
    table.set("all_day", event.all_day)?;
    if let Some(location) = &event.location {
        table.set("location", location.as_str())?;
    }
    if let Some(notes) = &event.notes {
        table.set("notes", notes.as_str())?;
    }
    if let Some(url) = &event.url {
        table.set("url", url.as_str())?;
    }
    Ok(table)
}

#[allow(dead_code)]
fn _assert_access_status_is_used(status: AccessStatus) -> &'static str {
    status.as_str()
}
