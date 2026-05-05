#![cfg(feature = "mod-datetime")]

use cpsl_core::{Sandbox, transpile};

fn sb() -> Sandbox {
    Sandbox::new().unwrap()
}

// ── datetime.now ────────────────────────────────────────────────

#[test]
fn now_returns_iso8601() {
    let s = sb();
    let r = s.exec("return datetime.now()").unwrap();
    // Should match YYYY-MM-DDTHH:MM:SSZ pattern
    assert!(r.contains("T"), "expected ISO 8601, got: {}", r);
    assert!(r.ends_with("Z"), "expected UTC suffix Z, got: {}", r);
    assert!(r.len() == 20, "expected 20 chars (YYYY-MM-DDTHH:MM:SSZ), got {} chars: {}", r.len(), r);
}

#[test]
fn now_is_a_parseable_datetime() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local n = datetime.now()
            local y = datetime.year(n)
            return tostring(y)
        "#,
        )
        .unwrap();
    let year: i32 = r.parse().unwrap();
    assert!(year >= 2026, "expected year >= 2026, got: {}", year);
}

// ── datetime.parse ──────────────────────────────────────────────

#[test]
fn parse_iso8601_with_z() {
    let s = sb();
    let r = s
        .exec(r#"return datetime.parse("2024-06-15T10:30:00Z")"#)
        .unwrap();
    assert_eq!(r, "2024-06-15T10:30:00Z");
}

#[test]
fn parse_iso8601_without_tz() {
    let s = sb();
    let r = s
        .exec(r#"return datetime.parse("2024-06-15T10:30:00")"#)
        .unwrap();
    assert_eq!(r, "2024-06-15T10:30:00Z");
}

#[test]
fn parse_date_only() {
    let s = sb();
    let r = s
        .exec(r#"return datetime.parse("2024-06-15")"#)
        .unwrap();
    assert_eq!(r, "2024-06-15T00:00:00Z");
}

#[test]
fn parse_date_with_space_separator() {
    let s = sb();
    let r = s
        .exec(r#"return datetime.parse("2024-06-15 10:30:00")"#)
        .unwrap();
    assert_eq!(r, "2024-06-15T10:30:00Z");
}

#[test]
fn parse_us_date_format() {
    let s = sb();
    let r = s
        .exec(r#"return datetime.parse("06/15/2024")"#)
        .unwrap();
    assert_eq!(r, "2024-06-15T00:00:00Z");
}

#[test]
fn parse_european_date_format() {
    let s = sb();
    let r = s
        .exec(r#"return datetime.parse("15.06.2024")"#)
        .unwrap();
    assert_eq!(r, "2024-06-15T00:00:00Z");
}

#[test]
fn parse_with_explicit_format() {
    let s = sb();
    let r = s
        .exec(r#"return datetime.parse("June 15, 2024", "%B %d, %Y")"#)
        .unwrap();
    assert_eq!(r, "2024-06-15T00:00:00Z");
}

#[test]
fn parse_with_timezone_offset() {
    let s = sb();
    let r = s
        .exec(r#"return datetime.parse("2024-06-15T10:30:00+05:00")"#)
        .unwrap();
    // Should be converted to UTC: 10:30 - 5:00 = 05:30
    assert_eq!(r, "2024-06-15T05:30:00Z");
}

#[test]
fn parse_invalid_string_errors() {
    let s = sb();
    let err = s.exec(r#"datetime.parse("not a date")"#).unwrap_err();
    assert!(
        err.message.contains("could not parse"),
        "msg: {}",
        err.message
    );
}

#[test]
fn parse_no_args_errors() {
    let s = sb();
    let err = s.exec("datetime.parse()").unwrap_err();
    assert!(
        err.message.contains("bad argument") || err.message.contains("missing"),
        "msg: {}",
        err.message
    );
}

// ── datetime.format ─────────────────────────────────────────────

#[test]
fn format_strftime_pattern() {
    let s = sb();
    let r = s
        .exec(r#"return datetime.format("2024-06-15T10:30:00Z", "%Y/%m/%d")"#)
        .unwrap();
    assert_eq!(r, "2024/06/15");
}

#[test]
fn format_with_time_components() {
    let s = sb();
    let r = s
        .exec(r#"return datetime.format("2024-06-15T10:30:45Z", "%H:%M:%S")"#)
        .unwrap();
    assert_eq!(r, "10:30:45");
}

#[test]
fn format_weekday_name() {
    let s = sb();
    let r = s
        .exec(r#"return datetime.format("2024-06-15T10:30:00Z", "%A")"#)
        .unwrap();
    assert_eq!(r, "Saturday");
}

#[test]
fn format_month_name() {
    let s = sb();
    let r = s
        .exec(r#"return datetime.format("2024-06-15T00:00:00Z", "%B")"#)
        .unwrap();
    assert_eq!(r, "June");
}

// ── datetime.add ────────────────────────────────────────────────

#[test]
fn add_days() {
    let s = sb();
    let r = s
        .exec(r#"return datetime.add("2024-06-15T10:00:00Z", {days=3})"#)
        .unwrap();
    assert_eq!(r, "2024-06-18T10:00:00Z");
}

#[test]
fn add_hours() {
    let s = sb();
    let r = s
        .exec(r#"return datetime.add("2024-06-15T10:00:00Z", {hours=5})"#)
        .unwrap();
    assert_eq!(r, "2024-06-15T15:00:00Z");
}

#[test]
fn add_minutes_and_seconds() {
    let s = sb();
    let r = s
        .exec(r#"return datetime.add("2024-06-15T10:00:00Z", {minutes=30, seconds=15})"#)
        .unwrap();
    assert_eq!(r, "2024-06-15T10:30:15Z");
}

#[test]
fn add_negative_days() {
    let s = sb();
    let r = s
        .exec(r#"return datetime.add("2024-06-15T10:00:00Z", {days=-5})"#)
        .unwrap();
    assert_eq!(r, "2024-06-10T10:00:00Z");
}

#[test]
fn add_mixed_positive_and_negative() {
    let s = sb();
    let r = s
        .exec(r#"return datetime.add("2024-06-15T10:00:00Z", {days=1, hours=-2})"#)
        .unwrap();
    assert_eq!(r, "2024-06-16T08:00:00Z");
}

#[test]
fn add_crossing_month_boundary() {
    let s = sb();
    let r = s
        .exec(r#"return datetime.add("2024-01-30T00:00:00Z", {days=3})"#)
        .unwrap();
    assert_eq!(r, "2024-02-02T00:00:00Z");
}

#[test]
fn add_crossing_year_boundary() {
    let s = sb();
    let r = s
        .exec(r#"return datetime.add("2024-12-30T00:00:00Z", {days=5})"#)
        .unwrap();
    assert_eq!(r, "2025-01-04T00:00:00Z");
}

#[test]
fn add_zero_delta() {
    let s = sb();
    let r = s
        .exec(r#"return datetime.add("2024-06-15T10:00:00Z", {days=0})"#)
        .unwrap();
    assert_eq!(r, "2024-06-15T10:00:00Z");
}

// ── datetime.diff ───────────────────────────────────────────────

#[test]
fn diff_seconds_default() {
    let s = sb();
    let r = s
        .exec(r#"return tostring(datetime.diff("2024-06-15T10:00:30Z", "2024-06-15T10:00:00Z"))"#)
        .unwrap();
    assert_eq!(r, "30");
}

#[test]
fn diff_minutes() {
    let s = sb();
    let r = s
        .exec(r#"return tostring(datetime.diff("2024-06-15T10:30:00Z", "2024-06-15T10:00:00Z", "minutes"))"#)
        .unwrap();
    assert_eq!(r, "30");
}

#[test]
fn diff_hours() {
    let s = sb();
    let r = s
        .exec(r#"return tostring(datetime.diff("2024-06-15T15:00:00Z", "2024-06-15T10:00:00Z", "hours"))"#)
        .unwrap();
    assert_eq!(r, "5");
}

#[test]
fn diff_days() {
    let s = sb();
    let r = s
        .exec(r#"return tostring(datetime.diff("2024-06-20T10:00:00Z", "2024-06-15T10:00:00Z", "days"))"#)
        .unwrap();
    assert_eq!(r, "5");
}

#[test]
fn diff_negative() {
    let s = sb();
    let r = s
        .exec(r#"return tostring(datetime.diff("2024-06-10T00:00:00Z", "2024-06-15T00:00:00Z", "days"))"#)
        .unwrap();
    assert_eq!(r, "-5");
}

#[test]
fn diff_unknown_unit_errors() {
    let s = sb();
    let err = s
        .exec(r#"datetime.diff("2024-06-15T10:00:00Z", "2024-06-15T00:00:00Z", "weeks")"#)
        .unwrap_err();
    assert!(
        err.message.contains("unknown unit"),
        "msg: {}",
        err.message
    );
}

// ── datetime.timestamp / datetime.fromtimestamp ─────────────────

#[test]
fn timestamp_epoch() {
    let s = sb();
    let r = s
        .exec(r#"return tostring(datetime.timestamp("1970-01-01T00:00:00Z"))"#)
        .unwrap();
    assert_eq!(r, "0");
}

#[test]
fn timestamp_known_date() {
    let s = sb();
    let r = s
        .exec(r#"return tostring(datetime.timestamp("2024-01-01T00:00:00Z"))"#)
        .unwrap();
    // 2024-01-01 00:00:00 UTC = 1704067200
    assert_eq!(r, "1704067200");
}

#[test]
fn fromtimestamp_epoch() {
    let s = sb();
    let r = s
        .exec(r#"return datetime.fromtimestamp(0)"#)
        .unwrap();
    assert_eq!(r, "1970-01-01T00:00:00Z");
}

#[test]
fn fromtimestamp_known() {
    let s = sb();
    let r = s
        .exec(r#"return datetime.fromtimestamp(1704067200)"#)
        .unwrap();
    assert_eq!(r, "2024-01-01T00:00:00Z");
}

#[test]
fn timestamp_roundtrip() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local dt = "2024-06-15T12:30:00Z"
            local ts = datetime.timestamp(dt)
            local back = datetime.fromtimestamp(ts)
            return back
        "#,
        )
        .unwrap();
    assert_eq!(r, "2024-06-15T12:30:00Z");
}

// ── datetime.weekday ────────────────────────────────────────────

#[test]
fn weekday_saturday() {
    let s = sb();
    // 2024-06-15 is a Saturday
    let r = s
        .exec(r#"return tostring(datetime.weekday("2024-06-15T00:00:00Z"))"#)
        .unwrap();
    assert_eq!(r, "5"); // Saturday = 5 (Monday=0)
}

#[test]
fn weekday_monday() {
    let s = sb();
    // 2024-06-10 is a Monday
    let r = s
        .exec(r#"return tostring(datetime.weekday("2024-06-10T00:00:00Z"))"#)
        .unwrap();
    assert_eq!(r, "0"); // Monday = 0
}

#[test]
fn weekday_sunday() {
    let s = sb();
    // 2024-06-16 is a Sunday
    let r = s
        .exec(r#"return tostring(datetime.weekday("2024-06-16T00:00:00Z"))"#)
        .unwrap();
    assert_eq!(r, "6"); // Sunday = 6
}

// ── datetime.year/month/day/hour/minute/second ──────────────────

#[test]
fn year_extraction() {
    let s = sb();
    let r = s
        .exec(r#"return tostring(datetime.year("2024-06-15T10:30:45Z"))"#)
        .unwrap();
    assert_eq!(r, "2024");
}

#[test]
fn month_extraction() {
    let s = sb();
    let r = s
        .exec(r#"return tostring(datetime.month("2024-06-15T10:30:45Z"))"#)
        .unwrap();
    assert_eq!(r, "6");
}

#[test]
fn day_extraction() {
    let s = sb();
    let r = s
        .exec(r#"return tostring(datetime.day("2024-06-15T10:30:45Z"))"#)
        .unwrap();
    assert_eq!(r, "15");
}

#[test]
fn hour_extraction() {
    let s = sb();
    let r = s
        .exec(r#"return tostring(datetime.hour("2024-06-15T10:30:45Z"))"#)
        .unwrap();
    assert_eq!(r, "10");
}

#[test]
fn minute_extraction() {
    let s = sb();
    let r = s
        .exec(r#"return tostring(datetime.minute("2024-06-15T10:30:45Z"))"#)
        .unwrap();
    assert_eq!(r, "30");
}

#[test]
fn second_extraction() {
    let s = sb();
    let r = s
        .exec(r#"return tostring(datetime.second("2024-06-15T10:30:45Z"))"#)
        .unwrap();
    assert_eq!(r, "45");
}

#[test]
fn all_components_from_same_datetime() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local dt = "2024-06-15T10:30:45Z"
            return datetime.year(dt) .. "-" ..
                   datetime.month(dt) .. "-" ..
                   datetime.day(dt) .. " " ..
                   datetime.hour(dt) .. ":" ..
                   datetime.minute(dt) .. ":" ..
                   datetime.second(dt)
        "#,
        )
        .unwrap();
    assert_eq!(r, "2024-6-15 10:30:45");
}

// ── datetime.isoformat ──────────────────────────────────────────

#[test]
fn isoformat_already_iso() {
    let s = sb();
    let r = s
        .exec(r#"return datetime.isoformat("2024-06-15T10:30:00Z")"#)
        .unwrap();
    assert_eq!(r, "2024-06-15T10:30:00Z");
}

#[test]
fn isoformat_from_date_only() {
    let s = sb();
    let r = s
        .exec(r#"return datetime.isoformat("2024-06-15")"#)
        .unwrap();
    assert_eq!(r, "2024-06-15T00:00:00Z");
}

#[test]
fn isoformat_from_us_format() {
    let s = sb();
    let r = s
        .exec(r#"return datetime.isoformat("06/15/2024")"#)
        .unwrap();
    assert_eq!(r, "2024-06-15T00:00:00Z");
}

// ── datetime.strftime / datetime.strptime ──────────────────────

#[test]
fn strftime_basic() {
    let s = sb();
    let r = s
        .exec(r#"return datetime.strftime("2024-06-15T10:30:00Z", "%d %B %Y")"#)
        .unwrap();
    assert_eq!(r, "15 June 2024");
}

#[test]
fn strptime_basic() {
    let s = sb();
    let r = s
        .exec(r#"return datetime.strptime("15 June 2024", "%d %B %Y")"#)
        .unwrap();
    assert_eq!(r, "2024-06-15T00:00:00Z");
}

#[test]
fn strftime_strptime_roundtrip() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local dt = "2024-06-15T10:30:00Z"
            local formatted = datetime.strftime(dt, "%Y-%m-%d %H:%M:%S")
            local parsed = datetime.strptime(formatted, "%Y-%m-%d %H:%M:%S")
            return parsed
        "#,
        )
        .unwrap();
    assert_eq!(r, "2024-06-15T10:30:00Z");
}

// ── datetime.date ───────────────────────────────────────────────

#[test]
fn date_basic() {
    let s = sb();
    let r = s
        .exec(r#"return datetime.date(2024, 6, 15)"#)
        .unwrap();
    assert_eq!(r, "2024-06-15T00:00:00Z");
}

#[test]
fn date_first_day_of_year() {
    let s = sb();
    let r = s
        .exec(r#"return datetime.date(2024, 1, 1)"#)
        .unwrap();
    assert_eq!(r, "2024-01-01T00:00:00Z");
}

#[test]
fn date_leap_year() {
    let s = sb();
    let r = s
        .exec(r#"return datetime.date(2024, 2, 29)"#)
        .unwrap();
    assert_eq!(r, "2024-02-29T00:00:00Z");
}

#[test]
fn date_invalid_feb_29_non_leap() {
    let s = sb();
    let err = s.exec(r#"datetime.date(2023, 2, 29)"#).unwrap_err();
    assert!(
        err.message.contains("invalid date"),
        "msg: {}",
        err.message
    );
}

#[test]
fn date_invalid_month() {
    let s = sb();
    let err = s.exec(r#"datetime.date(2024, 13, 1)"#).unwrap_err();
    assert!(
        err.message.contains("invalid date"),
        "msg: {}",
        err.message
    );
}

// ── Dual-signature tests (table form for shell dispatch) ────────

#[test]
fn parse_table_form_positional() {
    let s = sb();
    let r = s
        .exec(r#"return datetime.parse({[1]="2024-06-15"})"#)
        .unwrap();
    assert_eq!(r, "2024-06-15T00:00:00Z");
}

#[test]
fn parse_table_form_named() {
    let s = sb();
    let r = s
        .exec(r#"return datetime.parse({str="2024-06-15"})"#)
        .unwrap();
    assert_eq!(r, "2024-06-15T00:00:00Z");
}

#[test]
fn timestamp_table_form() {
    let s = sb();
    let r = s
        .exec(r#"return tostring(datetime.timestamp({[1]="1970-01-01T00:00:00Z"}))"#)
        .unwrap();
    assert_eq!(r, "0");
}

#[test]
fn weekday_table_form() {
    let s = sb();
    let r = s
        .exec(r#"return tostring(datetime.weekday({[1]="2024-06-10T00:00:00Z"}))"#)
        .unwrap();
    assert_eq!(r, "0"); // Monday
}

// ── Shell dispatch tests ────────────────────────────────────────

#[test]
fn shell_datetime_now() {
    let s = sb();
    let shrt = include_str!("../../shrt.luau");
    s.register_module("shrt", shrt).unwrap();
    let result = cpsl_core::sh_transpile::transpile_sh(r#"datetime now"#);
    assert!(result.is_ok(), "transpile err: {:?}", result.err());
    let luau = result.unwrap().luau_source;
    let r = s.exec(&luau);
    assert!(r.is_ok(), "shell datetime now should not error: {:?}", r.err());
}

#[test]
fn shell_datetime_parse() {
    let s = sb();
    let shrt = include_str!("../../shrt.luau");
    s.register_module("shrt", shrt).unwrap();
    let result = cpsl_core::sh_transpile::transpile_sh(r#"datetime parse "2024-06-15""#);
    assert!(result.is_ok(), "transpile err: {:?}", result.err());
    let luau = result.unwrap().luau_source;
    let r = s.exec(&luau).unwrap();
    assert!(
        r.contains("2024-06-15"),
        "expected parsed date, got: {}",
        r
    );
}

#[test]
fn shell_datetime_timestamp() {
    let s = sb();
    let shrt = include_str!("../../shrt.luau");
    s.register_module("shrt", shrt).unwrap();
    let result = cpsl_core::sh_transpile::transpile_sh(r#"datetime timestamp "2024-01-01T00:00:00Z""#);
    assert!(result.is_ok(), "transpile err: {:?}", result.err());
    let luau = result.unwrap().luau_source;
    let r = s.exec(&luau).unwrap();
    assert!(
        r.contains("1704067200"),
        "expected unix timestamp, got: {}",
        r
    );
}

// ── Error handling ──────────────────────────────────────────────

#[test]
fn format_no_args_errors() {
    let s = sb();
    let err = s.exec("datetime.format()").unwrap_err();
    assert!(
        err.message.contains("bad argument") || err.message.contains("missing"),
        "msg: {}",
        err.message
    );
}

#[test]
fn add_no_args_errors() {
    let s = sb();
    let err = s.exec("datetime.add()").unwrap_err();
    assert!(
        err.message.contains("bad argument") || err.message.contains("missing"),
        "msg: {}",
        err.message
    );
}

#[test]
fn diff_no_args_errors() {
    let s = sb();
    let err = s.exec("datetime.diff()").unwrap_err();
    assert!(
        err.message.contains("bad argument") || err.message.contains("missing"),
        "msg: {}",
        err.message
    );
}

#[test]
fn timestamp_wrong_type_errors() {
    let s = sb();
    let err = s.exec("datetime.timestamp(42)").unwrap_err();
    assert!(
        err.message.contains("string") || err.message.contains("expected"),
        "msg: {}",
        err.message
    );
}

#[test]
fn fromtimestamp_wrong_type_errors() {
    let s = sb();
    let err = s.exec(r#"datetime.fromtimestamp("not a number")"#).unwrap_err();
    assert!(
        err.message.contains("number") || err.message.contains("expected"),
        "msg: {}",
        err.message
    );
}

#[test]
fn year_no_args_errors() {
    let s = sb();
    let err = s.exec("datetime.year()").unwrap_err();
    assert!(
        err.message.contains("bad argument") || err.message.contains("missing"),
        "msg: {}",
        err.message
    );
}

#[test]
fn strptime_bad_format_errors() {
    let s = sb();
    let err = s.exec(r#"datetime.strptime("not a date", "%Y-%m-%d")"#).unwrap_err();
    assert!(
        err.message.contains("could not parse"),
        "msg: {}",
        err.message
    );
}

// ── Help ────────────────────────────────────────────────────────

#[test]
fn datetime_help_returns_help() {
    let s = sb();
    let r = s.exec("return datetime.help()").unwrap();
    assert!(r.contains("datetime"), "help: {}", r);
    assert!(r.contains("datetime.now"), "help: {}", r);
    assert!(r.contains("datetime.parse"), "help: {}", r);
    assert!(r.contains("datetime.add"), "help: {}", r);
    assert!(r.contains("datetime.diff"), "help: {}", r);
    assert!(r.contains("datetime.timestamp"), "help: {}", r);
}

#[test]
fn datetime_nonexistent_fn_hint() {
    let s = sb();
    let err = s.exec("datetime.foo()").unwrap_err();
    assert!(
        err.message.contains("datetime.foo does not exist"),
        "msg: {}",
        err.message
    );
    assert!(
        err.message.contains("hint: call datetime.help() for usage"),
        "msg: {}",
        err.message
    );
}

#[test]
fn global_help_mentions_datetime() {
    let s = sb();
    let r = s.exec("return help()").unwrap();
    assert!(
        r.contains("datetime"),
        "global help should list datetime: {}",
        r
    );
}

// ── Sandbox safety: no filesystem or network access ─────────────

#[test]
fn datetime_does_not_access_filesystem() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local n = datetime.now()
            local p = datetime.parse("2024-06-15T10:30:00Z")
            local f = datetime.format(p, "%Y-%m-%d")
            local a = datetime.add(p, {days=1})
            local d = datetime.diff(a, p, "days")
            local ts = datetime.timestamp(p)
            local back = datetime.fromtimestamp(ts)
            local wd = datetime.weekday(p)
            local y = datetime.year(p)
            local m = datetime.month(p)
            local day = datetime.day(p)
            return f .. " " .. tostring(d) .. " " .. tostring(wd)
        "#,
        )
        .unwrap();
    assert_eq!(r, "2024-06-15 1 5");
}

#[test]
fn datetime_no_dangerous_globals_exposed() {
    let s = sb();
    let r = s
        .exec(
            r#"
            return tostring(type(datetime.now)) .. " " ..
                   tostring(type(datetime.parse)) .. " " ..
                   tostring(type(datetime.format)) .. " " ..
                   tostring(type(datetime.add)) .. " " ..
                   tostring(rawget(datetime, "io")) .. " " ..
                   tostring(rawget(datetime, "os"))
        "#,
        )
        .unwrap();
    assert_eq!(r, "function function function function nil nil");
}

#[test]
fn datetime_sandbox_no_io_access() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local mt = getmetatable(datetime)
            if mt then
                local idx = rawget(mt, "__index")
                if type(idx) == "table" then
                    if rawget(idx, "io") or rawget(idx, "os") then
                        return "metatable leaks dangerous globals"
                    end
                end
            end
            local count = 0
            for k, v in pairs(datetime) do
                count = count + 1
            end
            return "safe:" .. count
        "#,
        )
        .unwrap();
    assert!(
        r.starts_with("safe:"),
        "expected safe table, got: {}",
        r
    );
}

#[test]
fn datetime_sandbox_no_network_access() {
    let s = sb();
    let r = s
        .exec(
            r#"
            -- All datetime operations should work without any network
            local results = {}
            table.insert(results, datetime.parse("2024-06-15"))
            table.insert(results, datetime.format("2024-06-15T10:00:00Z", "%Y"))
            table.insert(results, datetime.add("2024-06-15T00:00:00Z", {days=1}))
            table.insert(results, tostring(datetime.diff("2024-06-16T00:00:00Z", "2024-06-15T00:00:00Z", "days")))
            return table.concat(results, ",")
        "#,
        )
        .unwrap();
    assert_eq!(r, "2024-06-15T00:00:00Z,2024,2024-06-16T00:00:00Z,1");
}

// ── Edge cases ──────────────────────────────────────────────────

#[test]
fn add_large_duration() {
    let s = sb();
    let r = s
        .exec(r#"return datetime.add("2024-01-01T00:00:00Z", {days=365})"#)
        .unwrap();
    // 2024 is a leap year, so 365 days from Jan 1 = Dec 31
    assert_eq!(r, "2024-12-31T00:00:00Z");
}

#[test]
fn diff_same_datetime_is_zero() {
    let s = sb();
    let r = s
        .exec(r#"return tostring(datetime.diff("2024-06-15T10:00:00Z", "2024-06-15T10:00:00Z"))"#)
        .unwrap();
    assert_eq!(r, "0");
}

#[test]
fn parse_with_fractional_seconds() {
    let s = sb();
    let r = s
        .exec(r#"return datetime.parse("2024-06-15T10:30:45.123Z")"#)
        .unwrap();
    // Output is truncated to whole seconds
    assert_eq!(r, "2024-06-15T10:30:45Z");
}

#[test]
fn weekday_date_only_input() {
    let s = sb();
    let r = s
        .exec(r#"return tostring(datetime.weekday("2024-06-15"))"#)
        .unwrap();
    assert_eq!(r, "5"); // Saturday
}

#[test]
fn complex_workflow() {
    let s = sb();
    let r = s
        .exec(
            r#"
            -- Create a date, add some time, check components
            local d = datetime.date(2024, 3, 15)
            local d2 = datetime.add(d, {days=10, hours=14, minutes=30})
            local y = datetime.year(d2)
            local m = datetime.month(d2)
            local day = datetime.day(d2)
            local h = datetime.hour(d2)
            local min = datetime.minute(d2)
            return y .. "-" .. m .. "-" .. day .. " " .. h .. ":" .. min
        "#,
        )
        .unwrap();
    assert_eq!(r, "2024-3-25 14:30");
}

#[test]
fn diff_across_daylight_saving_irrelevant_utc() {
    // Since we work in UTC, DST doesn't affect calculations
    let s = sb();
    let r = s
        .exec(
            r#"
            local d1 = "2024-03-10T00:00:00Z"
            local d2 = "2024-03-11T00:00:00Z"
            return tostring(datetime.diff(d2, d1, "hours"))
        "#,
        )
        .unwrap();
    assert_eq!(r, "24"); // Exactly 24 hours in UTC
}

#[test]
fn fromtimestamp_negative() {
    // Negative timestamp = before epoch
    let s = sb();
    let r = s
        .exec(r#"return datetime.fromtimestamp(-86400)"#)
        .unwrap();
    assert_eq!(r, "1969-12-31T00:00:00Z");
}

#[test]
fn parse_and_format_preserve_date() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local dates = {"2024-01-01", "2024-06-15", "2024-12-31"}
            local results = {}
            for _, d in ipairs(dates) do
                local parsed = datetime.parse(d)
                local formatted = datetime.format(parsed, "%Y-%m-%d")
                table.insert(results, formatted)
            end
            return table.concat(results, ",")
        "#,
        )
        .unwrap();
    assert_eq!(r, "2024-01-01,2024-06-15,2024-12-31");
}

// ── Python transpiler e2e ───────────────────────────────────────

#[test]
fn python_datetime_now() {
    let s = sb();
    let pyrt = include_str!("../../pyrt.luau");
    s.setup_python_runtime(pyrt).unwrap();

    let py_code = r#"
import datetime
result = datetime.now()
print(result)
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    let r = s.exec(&transpiled.luau_source).unwrap();
    assert!(r.contains("T") && r.contains("Z"), "expected ISO 8601, got: {}", r);
}

#[test]
fn python_datetime_parse() {
    let s = sb();
    let pyrt = include_str!("../../pyrt.luau");
    s.setup_python_runtime(pyrt).unwrap();

    let py_code = r#"
import datetime
result = datetime.parse("2024-06-15")
print(result)
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    let r = s.exec(&transpiled.luau_source).unwrap();
    assert_eq!(r, "2024-06-15T00:00:00Z");
}

#[test]
fn python_from_datetime_import() {
    let py_code = r#"
from datetime import datetime, timedelta
result = datetime.parse("2024-06-15")
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    assert!(
        transpiled.luau_source.contains("datetime"),
        "transpiled: {}",
        transpiled.luau_source
    );
}

#[test]
fn python_dateutil_import() {
    let py_code = r#"
import dateutil
result = dateutil.parse("2024-06-15")
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    assert!(
        transpiled.luau_source.contains("datetime"),
        "transpiled: {}",
        transpiled.luau_source
    );
}

#[test]
fn python_datetime_timestamp() {
    let s = sb();
    let pyrt = include_str!("../../pyrt.luau");
    s.setup_python_runtime(pyrt).unwrap();

    let py_code = r#"
import datetime
ts = datetime.timestamp("2024-01-01T00:00:00Z")
print(ts)
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    let r = s.exec(&transpiled.luau_source).unwrap();
    assert!(r.contains("1704067200"), "expected timestamp, got: {}", r);
}
