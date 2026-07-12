//! Lua-facing Apple Calendar module backed by an injectable EventKit gateway.

use crate::lua_util::{is_lua_array, register_help_functions};
use crate::mount::MountTable;
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
use std::collections::{hash_map::DefaultHasher, HashSet};
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::Arc;

pub type CalendarActivityCallback = Arc<dyn Fn(&str) + Send + Sync>;

const ATTACHMENT_BLOCK_START: &str = "[Herm attachments]";
const ATTACHMENT_BLOCK_END: &str = "[/Herm attachments]";
const CALENDAR_ATTACHMENT_ROOT: &str = "/home/herm/calendar-attachments";

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
            name: "attach",
            description: "Associate sandbox files with an event in Herm. Files are copied to durable storage and referenced from event notes because EventKit has no native file-attachment API.",
            params: &[
                Param {
                    name: "event_id",
                    short: Some('i'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "paths",
                    short: Some('p'),
                    typ: ParamType::Value,
                    required: true,
                    fields: None,
                },
            ],
            returns: ReturnType::Table,
            example: Some(
                r#"calendar.attach(event.id, {"/home/herm/report.pdf", "/attachments/conversation/photo.jpg"})"#,
            ),
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
    mounts: Arc<MountTable>,
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
        let mounts = mounts.clone();
        let activity_callback = activity_callback.clone();
        calendar.set(
            "attach",
            lua.create_function(move |lua, args: MultiValue| {
                let (event_id, paths) = parse_attach_args(&args)?;
                let event = gateway.get(&event_id).map_err(mlua::Error::external)?;
                let existing_paths = attachment_paths(event.notes.as_deref());
                let copied =
                    copy_calendar_attachments(&mounts, &event_id, &paths, &existing_paths)?;
                let notes = notes_with_attachments(event.notes.as_deref(), &copied);
                let request = UpdateEventRequest {
                    event_id,
                    title: None,
                    start_time: None,
                    end_time: None,
                    calendar_id: None,
                    notes: Some(notes),
                    location: None,
                    url: None,
                    all_day: None,
                };
                notify_calendar_activity(&activity_callback, "attach");
                match gateway.update(request) {
                    Ok(event) => event_to_table(lua, &event),
                    Err(error) => {
                        let new_paths: Vec<String> = copied
                            .into_iter()
                            .filter(|path| !existing_paths.contains(path))
                            .collect();
                        cleanup_copied_attachments(&mounts, &new_paths);
                        Err(mlua::Error::external(error))
                    }
                }
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
                let notes = optional_string(&opts, "calendar.update", "notes")?;
                let notes = if let Some(notes) = notes {
                    let existing = gateway.get(&event_id).map_err(mlua::Error::external)?;
                    let attachments = attachment_paths(existing.notes.as_deref());
                    Some(notes_with_attachments(Some(&notes), &attachments))
                } else {
                    None
                };
                let request = UpdateEventRequest {
                    event_id,
                    title: optional_string(&opts, "calendar.update", "title")?,
                    start_time,
                    end_time,
                    calendar_id: optional_string(&opts, "calendar.update", "calendar_id")?,
                    notes,
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
        let mounts = mounts.clone();
        calendar.set(
            "delete",
            lua.create_function(move |_, args: MultiValue| {
                let validated =
                    validate_args(&args, CALENDAR_DOC.params("delete"), "calendar.delete")?;
                let event_id = value_string(&validated[0], "calendar.delete", "event_id")?;
                let attachments = gateway
                    .get(&event_id)
                    .ok()
                    .map(|event| attachment_paths(event.notes.as_deref()))
                    .unwrap_or_default();
                notify_calendar_activity(&activity_callback, "delete");
                let deleted = gateway.delete(&event_id).map_err(mlua::Error::external)?;
                if deleted {
                    cleanup_copied_attachments(&mounts, &attachments);
                }
                Ok(deleted)
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
        let mut event_id = table.get::<Value>("event_id")?;
        if matches!(event_id, Value::Nil) {
            event_id = table.get::<Value>(1)?;
        }
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

fn parse_attach_args(args: &MultiValue) -> Result<(String, Vec<String>), mlua::Error> {
    if let Some(table) = single_table_arg(args) {
        let mut event_id = table.get::<Value>("event_id")?;
        if matches!(event_id, Value::Nil) {
            event_id = table.get::<Value>(1)?;
        }
        let mut paths = table.get::<Value>("paths")?;
        if matches!(paths, Value::Nil) {
            paths = table.get::<Value>(2)?;
        }
        return Ok((
            value_string(&event_id, "calendar.attach", "event_id")?,
            string_or_array(&paths, "calendar.attach", "paths")?,
        ));
    }
    let validated = validate_args(args, CALENDAR_DOC.params("attach"), "calendar.attach")?;
    Ok((
        value_string(&validated[0], "calendar.attach", "event_id")?,
        string_or_array(&validated[1], "calendar.attach", "paths")?,
    ))
}

fn string_or_array(value: &Value, fn_name: &str, name: &str) -> Result<Vec<String>, mlua::Error> {
    match value {
        Value::String(value) => Ok(vec![value.to_string_lossy().to_string()]),
        Value::Table(table) => {
            let len = table.raw_len();
            if len == 0 || !is_lua_array(table, len) {
                return Err(mlua::Error::external(format!(
                    "{fn_name}: argument '{name}' expected a non-empty array table"
                )));
            }
            let mut paths = Vec::with_capacity(len);
            for index in 1..=len {
                let value: Value = table.raw_get(index)?;
                paths.push(value_string(&value, fn_name, name)?);
            }
            Ok(paths)
        }
        Value::Nil => Err(mlua::Error::external(format!(
            "{fn_name}: missing required argument '{name}' (string or array table)"
        ))),
        other => Err(mlua::Error::external(format!(
            "{fn_name}: argument '{name}' expected string or array table, got {}",
            other.type_name()
        ))),
    }
}

fn copy_calendar_attachments(
    mounts: &MountTable,
    event_id: &str,
    source_paths: &[String],
    existing_paths: &[String],
) -> Result<Vec<String>, mlua::Error> {
    let scope = calendar_attachment_scope(event_id);
    let virtual_directory = format!("{CALENDAR_ATTACHMENT_ROOT}/{scope}");
    let mut created = Vec::with_capacity(source_paths.len());
    let result = (|| {
        let mut copied = Vec::with_capacity(source_paths.len());

        for source_path in source_paths {
            if existing_paths.contains(source_path) {
                let source = mounts
                    .resolve_read(source_path)
                    .map_err(mlua::Error::external)?;
                if !source.is_file() {
                    return Err(mlua::Error::external(format!(
                        "calendar.attach: path is not a file: {source_path}"
                    )));
                }
                copied.push(source_path.clone());
                continue;
            }

            let source = mounts
                .resolve_read(source_path)
                .map_err(mlua::Error::external)?;
            if !source.is_file() {
                return Err(mlua::Error::external(format!(
                    "calendar.attach: path is not a file: {source_path}"
                )));
            }
            let name = safe_attachment_name(
                Path::new(source_path)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("attachment"),
            );
            let virtual_destination = unique_attachment_path(mounts, &virtual_directory, &name)?;
            let destination = mounts
                .resolve_write_deep(&virtual_destination)
                .map_err(mlua::Error::external)?;
            if let Some(parent) = destination.parent() {
                std::fs::create_dir_all(parent).map_err(mlua::Error::external)?;
            }
            created.push(virtual_destination.clone());
            std::fs::copy(&source, &destination).map_err(mlua::Error::external)?;
            copied.push(virtual_destination);
        }
        Ok(copied)
    })();
    if result.is_err() {
        cleanup_copied_attachments(mounts, &created);
    }
    result
}

fn calendar_attachment_scope(event_id: &str) -> String {
    let mut hasher = DefaultHasher::new();
    event_id.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn safe_attachment_name(value: &str) -> String {
    let name: String = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_') {
                character
            } else {
                '-'
            }
        })
        .collect();
    let name = name.trim_matches(|character| character == '.' || character == '-');
    if name.is_empty() {
        "attachment".to_string()
    } else {
        name.to_string()
    }
}

fn unique_attachment_path(
    mounts: &MountTable,
    virtual_directory: &str,
    name: &str,
) -> Result<String, mlua::Error> {
    let path = Path::new(name);
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("attachment");
    let extension = path.extension().and_then(|value| value.to_str());
    let mut suffix = 1;
    loop {
        let candidate = if suffix == 1 {
            name.to_string()
        } else if let Some(extension) = extension {
            format!("{stem}-{suffix}.{extension}")
        } else {
            format!("{stem}-{suffix}")
        };
        let virtual_path = format!("{virtual_directory}/{candidate}");
        match mounts.resolve_read(&virtual_path) {
            Ok(host_path) if host_path.exists() => suffix += 1,
            Ok(_) | Err(_) => return Ok(virtual_path),
        }
    }
}

fn notes_with_attachments(notes: Option<&str>, new_paths: &[String]) -> String {
    let (plain_notes, mut paths) = split_attachment_notes(notes);
    for path in new_paths {
        if !paths.contains(path) {
            paths.push(path.clone());
        }
    }
    if paths.is_empty() {
        return plain_notes;
    }
    let block = format!(
        "{ATTACHMENT_BLOCK_START}\n{}\n{ATTACHMENT_BLOCK_END}",
        paths.join("\n")
    );
    if plain_notes.is_empty() {
        block
    } else {
        format!("{plain_notes}\n\n{block}")
    }
}

fn attachment_paths(notes: Option<&str>) -> Vec<String> {
    split_attachment_notes(notes).1
}

fn split_attachment_notes(notes: Option<&str>) -> (String, Vec<String>) {
    let notes = notes.unwrap_or("");
    let Some(start) = notes.find(ATTACHMENT_BLOCK_START) else {
        return (notes.trim().to_string(), Vec::new());
    };
    let paths_start = start + ATTACHMENT_BLOCK_START.len();
    let Some(relative_end) = notes[paths_start..].find(ATTACHMENT_BLOCK_END) else {
        return (notes.trim().to_string(), Vec::new());
    };
    let end = paths_start + relative_end;
    let mut seen = HashSet::new();
    let paths = notes[paths_start..end]
        .lines()
        .map(str::trim)
        .filter(|line| is_managed_attachment_path(line))
        .map(str::to_string)
        .filter(|path| seen.insert(path.clone()))
        .collect();
    let plain_notes = format!(
        "{}{}",
        &notes[..start],
        &notes[end + ATTACHMENT_BLOCK_END.len()..]
    )
    .trim()
    .to_string();
    (plain_notes, paths)
}

fn is_managed_attachment_path(path: &str) -> bool {
    let Some(relative) = path.strip_prefix(&format!("{CALENDAR_ATTACHMENT_ROOT}/")) else {
        return false;
    };
    let mut components = relative.split('/');
    matches!(
        (components.next(), components.next(), components.next()),
        (Some(scope), Some(name), None)
            if !scope.is_empty()
                && !name.is_empty()
                && scope != "."
                && scope != ".."
                && name != "."
                && name != ".."
    )
}

fn cleanup_copied_attachments(mounts: &MountTable, paths: &[String]) {
    let Ok(root) = mounts.resolve_write_deep(CALENDAR_ATTACHMENT_ROOT) else {
        return;
    };
    for path in paths {
        if !is_managed_attachment_path(path) {
            continue;
        }
        if let Ok(host_path) = mounts.resolve_write(path) {
            if !host_path.starts_with(&root) {
                continue;
            }
            let _ = std::fs::remove_file(&host_path);
            if let Some(parent) = host_path.parent() {
                if parent != root && parent.starts_with(&root) {
                    let _ = std::fs::remove_dir(parent);
                }
            }
        }
    }
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
    let (notes, attachment_paths) = split_attachment_notes(event.notes.as_deref());
    if !notes.is_empty() {
        table.set("notes", notes)?;
    }
    if let Some(url) = &event.url {
        table.set("url", url.as_str())?;
    }
    let attachments = lua.create_table()?;
    for (index, path) in attachment_paths.iter().enumerate() {
        attachments.set(index + 1, path.as_str())?;
    }
    table.set("attachments", attachments)?;
    Ok(table)
}

#[allow(dead_code)]
fn _assert_access_status_is_used(status: AccessStatus) -> &'static str {
    status.as_str()
}
