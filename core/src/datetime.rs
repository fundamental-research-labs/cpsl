//! Date/time module for the Luau sandbox.
//!
//! Exposes `datetime.now`, `datetime.parse`, `datetime.format`, `datetime.add`,
//! `datetime.diff`, `datetime.timestamp`, `datetime.fromtimestamp`, `datetime.weekday`,
//! `datetime.year`, `datetime.month`, `datetime.day`, `datetime.isoformat`,
//! `datetime.strftime`, `datetime.strptime` as globals.
//!
//! Uses the `chrono` crate (pure Rust). All operations are purely computational —
//! no filesystem or network access.

use crate::lua_util::register_help_functions;
use crate::sandbox::{
    validate_args, wrap_module_with_help_hints, FnDoc, ModuleDoc, Param, ParamType, ReturnType,
};
use chrono::{
    DateTime, Datelike, Duration, NaiveDate, NaiveDateTime, TimeZone, Timelike, Utc, Weekday,
};
use mlua::{Lua, MultiValue, Value};

// ---------------------------------------------------------------------------
// Documentation
// ---------------------------------------------------------------------------

pub(crate) static DATETIME_DOC: ModuleDoc = ModuleDoc {
    name: "datetime",
    summary: "Date/time parsing, formatting & arithmetic (chrono)",
    functions: &[
        FnDoc {
            name: "now",
            description: "Return the current UTC datetime as an ISO 8601 string.",
            params: &[],
            returns: ReturnType::String,
            example: Some(r#"local now = datetime.now() -- "2026-03-10T12:00:00Z""#),
        },
        FnDoc {
            name: "parse",
            description: "Parse a datetime string. Auto-detects common formats, or use an explicit strftime format as second arg. Returns ISO 8601 string.",
            params: &[
                Param { name: "str", short: Some('s'), typ: ParamType::String, required: true, fields: None },
                Param { name: "fmt", short: Some('f'), typ: ParamType::String, required: false, fields: None },
            ],
            returns: ReturnType::String,
            example: Some(r#"datetime.parse("March 10, 2026") -- auto-detect format"#),
        },
        FnDoc {
            name: "format",
            description: "Format a datetime string using a strftime pattern.",
            params: &[
                Param { name: "dt", short: Some('d'), typ: ParamType::String, required: true, fields: None },
                Param { name: "fmt", short: Some('f'), typ: ParamType::String, required: true, fields: None },
            ],
            returns: ReturnType::String,
            example: Some(r#"datetime.format({dt="2026-03-10T12:00:00Z", fmt="%B %d, %Y"})"#),
        },
        FnDoc {
            name: "add",
            description: "Add a duration to a datetime. Returns new ISO 8601 string. Opts: {days?, hours?, minutes?, seconds?}",
            params: &[
                Param { name: "dt", short: Some('d'), typ: ParamType::String, required: true, fields: None },
                Param { name: "delta", short: None, typ: ParamType::Table, required: true, fields: None },
            ],
            returns: ReturnType::String,
            example: Some(r#"datetime.add({dt="2026-03-10T12:00:00Z", delta={days=7, hours=3}})"#),
        },
        FnDoc {
            name: "diff",
            description: "Compute the difference between two datetimes. Returns number. Unit: \"seconds\" (default), \"minutes\", \"hours\", \"days\".",
            params: &[
                Param { name: "dt1", short: None, typ: ParamType::String, required: true, fields: None },
                Param { name: "dt2", short: None, typ: ParamType::String, required: true, fields: None },
                Param { name: "unit", short: Some('u'), typ: ParamType::String, required: false, fields: None },
            ],
            returns: ReturnType::Number,
            example: Some(r#"datetime.diff({dt1="2026-03-17T00:00:00Z", dt2="2026-03-10T00:00:00Z", unit="days"}) -- 7"#),
        },
        FnDoc {
            name: "timestamp",
            description: "Convert a datetime string to a Unix timestamp (seconds since epoch, float).",
            params: &[
                Param { name: "dt", short: Some('d'), typ: ParamType::String, required: true, fields: None },
            ],
            returns: ReturnType::Number,
            example: None,
        },
        FnDoc {
            name: "fromtimestamp",
            description: "Convert a Unix timestamp (number) to an ISO 8601 datetime string (UTC).",
            params: &[
                Param { name: "ts", short: Some('t'), typ: ParamType::Number, required: true, fields: None },
            ],
            returns: ReturnType::String,
            example: None,
        },
        FnDoc {
            name: "weekday",
            description: "Return the weekday of a datetime as 0-6 (Monday=0).",
            params: &[
                Param { name: "dt", short: Some('d'), typ: ParamType::String, required: true, fields: None },
            ],
            returns: ReturnType::Number,
            example: None,
        },
        FnDoc {
            name: "year",
            description: "Extract the year from a datetime string.",
            params: &[
                Param { name: "dt", short: Some('d'), typ: ParamType::String, required: true, fields: None },
            ],
            returns: ReturnType::Number,
            example: None,
        },
        FnDoc {
            name: "month",
            description: "Extract the month (1-12) from a datetime string.",
            params: &[
                Param { name: "dt", short: Some('d'), typ: ParamType::String, required: true, fields: None },
            ],
            returns: ReturnType::Number,
            example: None,
        },
        FnDoc {
            name: "day",
            description: "Extract the day of the month from a datetime string.",
            params: &[
                Param { name: "dt", short: Some('d'), typ: ParamType::String, required: true, fields: None },
            ],
            returns: ReturnType::Number,
            example: None,
        },
        FnDoc {
            name: "hour",
            description: "Extract the hour (0-23) from a datetime string.",
            params: &[
                Param { name: "dt", short: Some('d'), typ: ParamType::String, required: true, fields: None },
            ],
            returns: ReturnType::Number,
            example: None,
        },
        FnDoc {
            name: "minute",
            description: "Extract the minute (0-59) from a datetime string.",
            params: &[
                Param { name: "dt", short: Some('d'), typ: ParamType::String, required: true, fields: None },
            ],
            returns: ReturnType::Number,
            example: None,
        },
        FnDoc {
            name: "second",
            description: "Extract the second (0-59) from a datetime string.",
            params: &[
                Param { name: "dt", short: Some('d'), typ: ParamType::String, required: true, fields: None },
            ],
            returns: ReturnType::Number,
            example: None,
        },
        FnDoc {
            name: "isoformat",
            description: "Return a datetime string in strict ISO 8601 format.",
            params: &[
                Param { name: "dt", short: Some('d'), typ: ParamType::String, required: true, fields: None },
            ],
            returns: ReturnType::String,
            example: None,
        },
        FnDoc {
            name: "strftime",
            description: "Format a datetime using a strftime pattern (alias for datetime.format).",
            params: &[
                Param { name: "dt", short: Some('d'), typ: ParamType::String, required: true, fields: None },
                Param { name: "fmt", short: Some('f'), typ: ParamType::String, required: true, fields: None },
            ],
            returns: ReturnType::String,
            example: None,
        },
        FnDoc {
            name: "strptime",
            description: "Parse a datetime string using a strftime pattern (alias for datetime.parse with explicit format).",
            params: &[
                Param { name: "str", short: Some('s'), typ: ParamType::String, required: true, fields: None },
                Param { name: "fmt", short: Some('f'), typ: ParamType::String, required: true, fields: None },
            ],
            returns: ReturnType::String,
            example: None,
        },
        FnDoc {
            name: "date",
            description: "Create a date-only datetime from year, month, day components.",
            params: &[
                Param { name: "year", short: Some('y'), typ: ParamType::Number, required: true, fields: None },
                Param { name: "month", short: Some('m'), typ: ParamType::Number, required: true, fields: None },
                Param { name: "day", short: Some('d'), typ: ParamType::Number, required: true, fields: None },
            ],
            returns: ReturnType::String,
            example: Some(r#"datetime.date({year=2026, month=3, day=10})"#),
        },
    ],
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Try to parse a datetime string in various common formats.
/// Returns a UTC DateTime on success.
fn parse_datetime(s: &str) -> Result<DateTime<Utc>, String> {
    let s = s.trim();

    // Try ISO 8601 with timezone (e.g. 2026-03-03T12:00:00Z or 2026-03-03T12:00:00+00:00)
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Ok(dt.with_timezone(&Utc));
    }

    // Try ISO 8601 without timezone — treat as UTC
    if let Ok(ndt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S") {
        return Ok(Utc.from_utc_datetime(&ndt));
    }
    if let Ok(ndt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f") {
        return Ok(Utc.from_utc_datetime(&ndt));
    }

    // Date + time without T separator
    if let Ok(ndt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
        return Ok(Utc.from_utc_datetime(&ndt));
    }
    if let Ok(ndt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S%.f") {
        return Ok(Utc.from_utc_datetime(&ndt));
    }

    // Date only → midnight UTC
    if let Ok(nd) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        let ndt = nd.and_hms_opt(0, 0, 0).unwrap();
        return Ok(Utc.from_utc_datetime(&ndt));
    }

    // US format: MM/DD/YYYY
    if let Ok(nd) = NaiveDate::parse_from_str(s, "%m/%d/%Y") {
        let ndt = nd.and_hms_opt(0, 0, 0).unwrap();
        return Ok(Utc.from_utc_datetime(&ndt));
    }

    // European format: DD.MM.YYYY
    if let Ok(nd) = NaiveDate::parse_from_str(s, "%d.%m.%Y") {
        let ndt = nd.and_hms_opt(0, 0, 0).unwrap();
        return Ok(Utc.from_utc_datetime(&ndt));
    }

    // RFC 2822
    if let Ok(dt) = DateTime::parse_from_rfc2822(s) {
        return Ok(dt.with_timezone(&Utc));
    }

    Err(format!(
        "datetime.parse: could not parse '{}' — try providing an explicit format string",
        s
    ))
}

/// Parse with an explicit strftime format.
fn parse_datetime_with_fmt(s: &str, fmt: &str) -> Result<DateTime<Utc>, String> {
    let s = s.trim();
    // Try full datetime first
    if let Ok(ndt) = NaiveDateTime::parse_from_str(s, fmt) {
        return Ok(Utc.from_utc_datetime(&ndt));
    }
    // Try date-only
    if let Ok(nd) = NaiveDate::parse_from_str(s, fmt) {
        let ndt = nd.and_hms_opt(0, 0, 0).unwrap();
        return Ok(Utc.from_utc_datetime(&ndt));
    }
    Err(format!(
        "datetime.parse: could not parse '{}' with format '{}'",
        s, fmt
    ))
}

/// Format a DateTime to ISO 8601 (no fractional seconds, with Z suffix).
fn to_iso(dt: &DateTime<Utc>) -> String {
    dt.format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

/// Extract an optional i64 field from a Lua table, defaulting to 0.
fn table_get_i64(tbl: &mlua::Table, key: &str) -> Result<i64, mlua::Error> {
    match tbl.get::<Value>(key)? {
        Value::Nil => Ok(0),
        Value::Integer(n) => Ok(n as i64),
        Value::Number(n) => Ok(n as i64),
        other => Err(mlua::Error::external(format!(
            "datetime.add: expected number for '{}', got {:?}",
            key, other
        ))),
    }
}

/// Extract a number from a Value (Integer or Number).
fn value_to_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Integer(n) => Some(*n as f64),
        Value::Number(n) => Some(*n),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

pub fn register_datetime_globals(lua: &Lua) -> Result<(), mlua::Error> {
    let dt_table = lua.create_table()?;

    // datetime.now() -> ISO 8601 UTC string
    dt_table.set(
        "now",
        lua.create_function(|_, _: MultiValue| Ok(to_iso(&Utc::now())))?,
    )?;

    // datetime.parse(str, fmt?) -> ISO 8601 string
    dt_table.set(
        "parse",
        lua.create_function(|_, args: MultiValue| {
            let validated = validate_args(&args, DATETIME_DOC.params("parse"), "datetime.parse")?;
            let s = match &validated[0] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };
            let fmt = match &validated[1] {
                Value::String(f) => Some(f.to_string_lossy().to_string()),
                _ => None,
            };

            let dt = if let Some(f) = fmt {
                parse_datetime_with_fmt(&s, &f)
            } else {
                parse_datetime(&s)
            };

            match dt {
                Ok(d) => Ok(to_iso(&d)),
                Err(e) => Err(mlua::Error::external(e)),
            }
        })?,
    )?;

    // datetime.format(dt, fmt) -> formatted string
    dt_table.set(
        "format",
        lua.create_function(|_, args: MultiValue| {
            let validated = validate_args(&args, DATETIME_DOC.params("format"), "datetime.format")?;
            let dt_str = match &validated[0] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };
            let fmt = match &validated[1] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };
            let dt = parse_datetime(&dt_str).map_err(mlua::Error::external)?;
            Ok(dt.format(&fmt).to_string())
        })?,
    )?;

    // datetime.add(dt, {days?, hours?, minutes?, seconds?}) -> ISO 8601 string
    dt_table.set(
        "add",
        lua.create_function(|_, args: MultiValue| {
            let validated = validate_args(&args, DATETIME_DOC.params("add"), "datetime.add")?;
            let dt_str = match &validated[0] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };
            let delta_tbl = match &validated[1] {
                Value::Table(t) => t,
                _ => unreachable!(),
            };

            let dt = parse_datetime(&dt_str).map_err(mlua::Error::external)?;

            let days = table_get_i64(delta_tbl, "days")?;
            let hours = table_get_i64(delta_tbl, "hours")?;
            let minutes = table_get_i64(delta_tbl, "minutes")?;
            let seconds = table_get_i64(delta_tbl, "seconds")?;

            let total_seconds = days * 86400 + hours * 3600 + minutes * 60 + seconds;
            let new_dt = dt + Duration::seconds(total_seconds);
            Ok(to_iso(&new_dt))
        })?,
    )?;

    // datetime.diff(dt1, dt2, unit?) -> number
    dt_table.set(
        "diff",
        lua.create_function(|_, args: MultiValue| {
            let validated = validate_args(&args, DATETIME_DOC.params("diff"), "datetime.diff")?;
            let dt1_str = match &validated[0] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };
            let dt2_str = match &validated[1] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };
            let unit = match &validated[2] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => "seconds".to_string(),
            };

            let dt1 = parse_datetime(&dt1_str).map_err(mlua::Error::external)?;
            let dt2 = parse_datetime(&dt2_str).map_err(mlua::Error::external)?;
            let diff = dt1 - dt2;
            let total_secs = diff.num_seconds() as f64;

            let result = match unit.as_str() {
                "seconds" | "s" => total_secs,
                "minutes" | "m" => total_secs / 60.0,
                "hours" | "h" => total_secs / 3600.0,
                "days" | "d" => total_secs / 86400.0,
                other => {
                    return Err(mlua::Error::external(format!(
                        "datetime.diff: unknown unit '{}' — use 'seconds', 'minutes', 'hours', or 'days'",
                        other
                    )));
                }
            };
            Ok(result)
        })?,
    )?;

    // datetime.timestamp(dt) -> Unix timestamp (float)
    dt_table.set(
        "timestamp",
        lua.create_function(|_, args: MultiValue| {
            let validated = validate_args(
                &args,
                DATETIME_DOC.params("timestamp"),
                "datetime.timestamp",
            )?;
            let dt_str = match &validated[0] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };
            let dt = parse_datetime(&dt_str).map_err(mlua::Error::external)?;
            Ok(dt.timestamp() as f64)
        })?,
    )?;

    // datetime.fromtimestamp(ts) -> ISO 8601 string
    dt_table.set(
        "fromtimestamp",
        lua.create_function(|_, args: MultiValue| {
            let validated = validate_args(
                &args,
                DATETIME_DOC.params("fromtimestamp"),
                "datetime.fromtimestamp",
            )?;
            let ts = match value_to_f64(&validated[0]) {
                Some(n) => n,
                None => unreachable!(),
            };
            let secs = ts as i64;
            let nsecs = ((ts - secs as f64) * 1_000_000_000.0) as u32;
            match DateTime::from_timestamp(secs, nsecs) {
                Some(dt) => Ok(to_iso(&dt)),
                None => Err(mlua::Error::external(format!(
                    "datetime.fromtimestamp: invalid timestamp {}",
                    ts
                ))),
            }
        })?,
    )?;

    // datetime.weekday(dt) -> 0-6 (Monday=0)
    dt_table.set(
        "weekday",
        lua.create_function(|_, args: MultiValue| {
            let validated =
                validate_args(&args, DATETIME_DOC.params("weekday"), "datetime.weekday")?;
            let dt_str = match &validated[0] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };
            let dt = parse_datetime(&dt_str).map_err(mlua::Error::external)?;
            let wd = match dt.weekday() {
                Weekday::Mon => 0,
                Weekday::Tue => 1,
                Weekday::Wed => 2,
                Weekday::Thu => 3,
                Weekday::Fri => 4,
                Weekday::Sat => 5,
                Weekday::Sun => 6,
            };
            Ok(wd)
        })?,
    )?;

    // datetime.year(dt) -> number
    dt_table.set(
        "year",
        lua.create_function(|_, args: MultiValue| {
            let validated = validate_args(&args, DATETIME_DOC.params("year"), "datetime.year")?;
            let dt_str = match &validated[0] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };
            let dt = parse_datetime(&dt_str).map_err(mlua::Error::external)?;
            Ok(dt.year())
        })?,
    )?;

    // datetime.month(dt) -> number (1-12)
    dt_table.set(
        "month",
        lua.create_function(|_, args: MultiValue| {
            let validated = validate_args(&args, DATETIME_DOC.params("month"), "datetime.month")?;
            let dt_str = match &validated[0] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };
            let dt = parse_datetime(&dt_str).map_err(mlua::Error::external)?;
            Ok(dt.month())
        })?,
    )?;

    // datetime.day(dt) -> number
    dt_table.set(
        "day",
        lua.create_function(|_, args: MultiValue| {
            let validated = validate_args(&args, DATETIME_DOC.params("day"), "datetime.day")?;
            let dt_str = match &validated[0] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };
            let dt = parse_datetime(&dt_str).map_err(mlua::Error::external)?;
            Ok(dt.day())
        })?,
    )?;

    // datetime.hour(dt) -> number (0-23)
    dt_table.set(
        "hour",
        lua.create_function(|_, args: MultiValue| {
            let validated = validate_args(&args, DATETIME_DOC.params("hour"), "datetime.hour")?;
            let dt_str = match &validated[0] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };
            let dt = parse_datetime(&dt_str).map_err(mlua::Error::external)?;
            Ok(dt.hour())
        })?,
    )?;

    // datetime.minute(dt) -> number (0-59)
    dt_table.set(
        "minute",
        lua.create_function(|_, args: MultiValue| {
            let validated = validate_args(&args, DATETIME_DOC.params("minute"), "datetime.minute")?;
            let dt_str = match &validated[0] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };
            let dt = parse_datetime(&dt_str).map_err(mlua::Error::external)?;
            Ok(dt.minute())
        })?,
    )?;

    // datetime.second(dt) -> number (0-59)
    dt_table.set(
        "second",
        lua.create_function(|_, args: MultiValue| {
            let validated = validate_args(&args, DATETIME_DOC.params("second"), "datetime.second")?;
            let dt_str = match &validated[0] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };
            let dt = parse_datetime(&dt_str).map_err(mlua::Error::external)?;
            Ok(dt.second())
        })?,
    )?;

    // datetime.isoformat(dt) -> ISO 8601 string
    dt_table.set(
        "isoformat",
        lua.create_function(|_, args: MultiValue| {
            let validated = validate_args(
                &args,
                DATETIME_DOC.params("isoformat"),
                "datetime.isoformat",
            )?;
            let dt_str = match &validated[0] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };
            let dt = parse_datetime(&dt_str).map_err(mlua::Error::external)?;
            Ok(to_iso(&dt))
        })?,
    )?;

    // datetime.strftime(dt, fmt) -> formatted string (alias for format)
    dt_table.set(
        "strftime",
        lua.create_function(|_, args: MultiValue| {
            let validated =
                validate_args(&args, DATETIME_DOC.params("strftime"), "datetime.strftime")?;
            let dt_str = match &validated[0] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };
            let fmt = match &validated[1] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };
            let dt = parse_datetime(&dt_str).map_err(mlua::Error::external)?;
            Ok(dt.format(&fmt).to_string())
        })?,
    )?;

    // datetime.strptime(str, fmt) -> ISO 8601 string
    dt_table.set(
        "strptime",
        lua.create_function(|_, args: MultiValue| {
            let validated =
                validate_args(&args, DATETIME_DOC.params("strptime"), "datetime.strptime")?;
            let s = match &validated[0] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };
            let fmt = match &validated[1] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };
            let dt = parse_datetime_with_fmt(&s, &fmt).map_err(mlua::Error::external)?;
            Ok(to_iso(&dt))
        })?,
    )?;

    // datetime.date(year, month, day) -> ISO 8601 string
    dt_table.set(
        "date",
        lua.create_function(|_, args: MultiValue| {
            let validated = validate_args(&args, DATETIME_DOC.params("date"), "datetime.date")?;
            let year = match value_to_f64(&validated[0]) {
                Some(n) => n as i32,
                None => unreachable!(),
            };
            let month = match value_to_f64(&validated[1]) {
                Some(n) => n as u32,
                None => unreachable!(),
            };
            let day = match value_to_f64(&validated[2]) {
                Some(n) => n as u32,
                None => unreachable!(),
            };
            match NaiveDate::from_ymd_opt(year, month, day) {
                Some(nd) => {
                    let ndt = nd.and_hms_opt(0, 0, 0).unwrap();
                    let dt = Utc.from_utc_datetime(&ndt);
                    Ok(to_iso(&dt))
                }
                None => Err(mlua::Error::external(format!(
                    "datetime.date: invalid date {}-{}-{}",
                    year, month, day
                ))),
            }
        })?,
    )?;

    register_help_functions(lua, &dt_table, &DATETIME_DOC)?;
    lua.globals().set("datetime", dt_table)?;
    wrap_module_with_help_hints(lua, "datetime")?;

    Ok(())
}
