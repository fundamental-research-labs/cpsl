//! Regular expression module for the Luau sandbox.
//!
//! Exposes `regex.match`, `regex.find_all`, `regex.replace`, `regex.replace_all`,
//! `regex.split`, `regex.is_match`, and `regex.escape` as globals.
//! Uses the `regex` crate — pure computation, no filesystem or network access.

use crate::lua_util::register_help_functions;
use crate::sandbox::{
    validate_args, wrap_module_with_help_hints, FnDoc, ModuleDoc, Param, ParamType, ReturnType,
};
use mlua::{Lua, MultiValue, Value};
use regex::Regex;

// ---------------------------------------------------------------------------
// Module documentation
// ---------------------------------------------------------------------------

pub(crate) static REGEX_DOC: ModuleDoc = ModuleDoc {
    name: "regex",
    summary: "Regular expressions: matching, extraction, replacement, splitting",
    functions: &[
        FnDoc {
            name: "match",
            description: "Match pattern against text. Returns {full, groups} with positional and named captures, or nil if no match.",
            params: &[
                Param {
                    name: "pattern",
                    short: Some('p'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "text",
                    short: Some('t'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
            ],
            returns: ReturnType::Table,
            example: Some(r#"local m = regex.match("(\\d+)-(\\d+)", "order 42-7") -- m.groups[1]="42""#),
        },
        FnDoc {
            name: "find_all",
            description: "Find all non-overlapping matches. Returns list of {match, start, end, groups} with 1-based indices.",
            params: &[
                Param {
                    name: "pattern",
                    short: Some('p'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "text",
                    short: Some('t'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
            ],
            returns: ReturnType::Table,
            example: None,
        },
        FnDoc {
            name: "replace",
            description: "Replace the first match of pattern in text with replacement. Supports $1, ${name} backreferences.",
            params: &[
                Param {
                    name: "pattern",
                    short: Some('p'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "text",
                    short: Some('t'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "replacement",
                    short: Some('r'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
            ],
            returns: ReturnType::String,
            example: Some(r#"regex.replace({pattern="\\d+", text="item 42 and 7", replacement="N"}) -- "item N and 7""#),
        },
        FnDoc {
            name: "replace_all",
            description: "Replace all matches of pattern in text with replacement. Supports $1, ${name} backreferences.",
            params: &[
                Param {
                    name: "pattern",
                    short: Some('p'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "text",
                    short: Some('t'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "replacement",
                    short: Some('r'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
            ],
            returns: ReturnType::String,
            example: Some(r#"regex.replace_all({pattern="\\d+", text="item 42 and 7", replacement="N"})"#),
        },
        FnDoc {
            name: "split",
            description: "Split text by pattern. Returns a list of strings.",
            params: &[
                Param {
                    name: "pattern",
                    short: Some('p'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "text",
                    short: Some('t'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
            ],
            returns: ReturnType::Table,
            example: None,
        },
        FnDoc {
            name: "is_match",
            description: "Fast check whether pattern matches text. Returns boolean, no capture overhead.",
            params: &[
                Param {
                    name: "pattern",
                    short: Some('p'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "text",
                    short: Some('t'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
            ],
            returns: ReturnType::Boolean,
            example: None,
        },
        FnDoc {
            name: "escape",
            description: "Escape a string so it can be used as a literal pattern (all regex metacharacters escaped).",
            params: &[Param {
                name: "str",
                short: Some('s'),
                typ: ParamType::String,
                required: true,
                    fields: None,
            }],
            returns: ReturnType::String,
            example: None,
        },
    ],
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Compile a regex pattern, returning a friendly Lua error on failure.
fn compile_regex(pattern: &str, fn_name: &str) -> Result<Regex, mlua::Error> {
    Regex::new(pattern).map_err(|e| {
        mlua::Error::external(format!("{}: invalid pattern '{}': {}", fn_name, pattern, e))
    })
}

/// Build a groups table for a capture match.
/// Positional captures go to integer keys 1, 2, 3, ...
/// Named captures additionally go to string keys.
fn build_groups_table(
    lua: &Lua,
    caps: &regex::Captures<'_>,
    re: &Regex,
) -> Result<mlua::Table, mlua::Error> {
    let groups = lua.create_table()?;
    let names: Vec<Option<&str>> = re.capture_names().collect();

    // Start from 1 (index 0 is the full match)
    let mut positional_idx = 1i64;
    for (i, name_opt) in names.iter().enumerate().skip(1) {
        if let Some(m) = caps.get(i) {
            let val = lua.create_string(m.as_str())?;
            groups.raw_set(positional_idx, val.clone())?;
            if let Some(name) = name_opt {
                groups.raw_set(*name, val)?;
            }
            positional_idx += 1;
        } else {
            // Unmatched optional group — still advance positional index
            positional_idx += 1;
        }
    }

    Ok(groups)
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/// Register `regex.*` globals in the Lua VM.
pub fn register_regex_globals(lua: &Lua) -> Result<(), mlua::Error> {
    let regex_table = lua.create_table()?;

    // --- regex.match(pattern, text) -> {full, groups} or nil ---
    regex_table.set(
        "match",
        lua.create_function(|lua, args: MultiValue| {
            let validated =
                validate_args(&args, REGEX_DOC.params("match"), "regex.match")?;
            let pattern = match &validated[0] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };
            let text = match &validated[1] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };

            let re = compile_regex(&pattern, "regex.match")?;

            match re.captures(&text) {
                Some(caps) => {
                    let result = lua.create_table()?;
                    let full_match = caps.get(0).unwrap().as_str();
                    result.set("full", lua.create_string(full_match)?)?;
                    let groups = build_groups_table(lua, &caps, &re)?;
                    result.set("groups", groups)?;
                    Ok(Value::Table(result))
                }
                None => Ok(Value::Nil),
            }
        })?,
    )?;

    // --- regex.find_all(pattern, text) -> list of {match, start, end, groups} ---
    regex_table.set(
        "find_all",
        lua.create_function(|lua, args: MultiValue| {
            let validated =
                validate_args(&args, REGEX_DOC.params("find_all"), "regex.find_all")?;
            let pattern = match &validated[0] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };
            let text = match &validated[1] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };

            let re = compile_regex(&pattern, "regex.find_all")?;
            let results = lua.create_table()?;
            let mut idx = 1i64;

            for caps in re.captures_iter(&text) {
                let entry = lua.create_table()?;
                let full = caps.get(0).unwrap();
                entry.set("match", lua.create_string(full.as_str())?)?;
                // 1-based indices (Lua convention)
                entry.set("start", (full.start() + 1) as i64)?;
                entry.set("end", full.end() as i64)?;
                let groups = build_groups_table(lua, &caps, &re)?;
                entry.set("groups", groups)?;
                results.raw_set(idx, entry)?;
                idx += 1;
            }

            Ok(Value::Table(results))
        })?,
    )?;

    // --- regex.replace(pattern, text, replacement) -> string ---
    regex_table.set(
        "replace",
        lua.create_function(|_, args: MultiValue| {
            let validated =
                validate_args(&args, REGEX_DOC.params("replace"), "regex.replace")?;
            let pattern = match &validated[0] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };
            let text = match &validated[1] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };
            let replacement = match &validated[2] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };

            let re = compile_regex(&pattern, "regex.replace")?;
            let result = re.replacen(&text, 1, replacement.as_str());
            Ok(result.into_owned())
        })?,
    )?;

    // --- regex.replace_all(pattern, text, replacement) -> string ---
    regex_table.set(
        "replace_all",
        lua.create_function(|_, args: MultiValue| {
            let validated =
                validate_args(&args, REGEX_DOC.params("replace_all"), "regex.replace_all")?;
            let pattern = match &validated[0] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };
            let text = match &validated[1] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };
            let replacement = match &validated[2] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };

            let re = compile_regex(&pattern, "regex.replace_all")?;
            let result = re.replace_all(&text, replacement.as_str());
            Ok(result.into_owned())
        })?,
    )?;

    // --- regex.split(pattern, text) -> list of strings ---
    regex_table.set(
        "split",
        lua.create_function(|lua, args: MultiValue| {
            let validated =
                validate_args(&args, REGEX_DOC.params("split"), "regex.split")?;
            let pattern = match &validated[0] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };
            let text = match &validated[1] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };

            let re = compile_regex(&pattern, "regex.split")?;
            let result = lua.create_table()?;
            for (i, part) in re.split(&text).enumerate() {
                result.raw_set((i + 1) as i64, lua.create_string(part)?)?;
            }
            Ok(Value::Table(result))
        })?,
    )?;

    // --- regex.is_match(pattern, text) -> boolean ---
    regex_table.set(
        "is_match",
        lua.create_function(|_, args: MultiValue| {
            let validated =
                validate_args(&args, REGEX_DOC.params("is_match"), "regex.is_match")?;
            let pattern = match &validated[0] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };
            let text = match &validated[1] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };

            let re = compile_regex(&pattern, "regex.is_match")?;
            Ok(re.is_match(&text))
        })?,
    )?;

    // --- regex.escape(str) -> escaped string ---
    regex_table.set(
        "escape",
        lua.create_function(|_, args: MultiValue| {
            let validated =
                validate_args(&args, REGEX_DOC.params("escape"), "regex.escape")?;
            let s = match &validated[0] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };
            Ok(regex::escape(&s))
        })?,
    )?;

    register_help_functions(lua, &regex_table, &REGEX_DOC)?;

    lua.globals().set("regex", regex_table)?;
    wrap_module_with_help_hints(lua, "regex")?;

    Ok(())
}
