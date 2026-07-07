//! URL parsing and manipulation module for the Luau sandbox.
//!
//! Exposes `url.parse`, `url.build`, `url.encode`, `url.decode`,
//! `url.query_parse`, `url.query_build`, and `url.join` as globals.
//! Uses the `url` and `percent-encoding` crates — pure computation,
//! no filesystem or network access.

use crate::lua_util::register_help_functions;
use crate::sandbox::{
    validate_args, wrap_module_with_help_hints, FnDoc, ModuleDoc, Param, ParamType, ReturnType,
};
use mlua::{Lua, MultiValue, Value};
use percent_encoding::{percent_decode_str, utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};
use url::Url;

// ---------------------------------------------------------------------------
// Module documentation
// ---------------------------------------------------------------------------

pub(crate) static URL_DOC: ModuleDoc = ModuleDoc {
    name: "url",
    summary: "URL parsing, building, encoding/decoding, query string manipulation, and joining",
    functions: &[
        FnDoc {
            name: "parse",
            description:
                "Parse a URL string into a table {scheme, host, port, path, query, fragment, origin}. Returns nil on invalid URL.",
            params: &[Param {
                name: "str",
                short: Some('s'),
                typ: ParamType::String,
                required: true,
                    fields: None,
            }],
            returns: ReturnType::Table,
            example: Some(r#"url.parse("https://example.com:8080/path?q=hello#top")"#),
        },
        FnDoc {
            name: "build",
            description:
                "Build a URL string from a table {scheme, host, port?, path?, query?, fragment?}.",
            params: &[Param {
                name: "parts",
                short: Some('p'),
                typ: ParamType::Table,
                required: true,
                    fields: None,
            }],
            returns: ReturnType::String,
            example: Some(r#"url.build({scheme="https", host="example.com", path="/api", query="key=val"})"#),
        },
        FnDoc {
            name: "encode",
            description: "Percent-encode a string (all non-alphanumeric characters except - _ . ~).",
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
        FnDoc {
            name: "decode",
            description: "Decode a percent-encoded string.",
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
        FnDoc {
            name: "query_parse",
            description:
                "Parse a query string like \"a=1&b=2\" into a list of {key, value} tables. Preserves order and duplicates.",
            params: &[Param {
                name: "str",
                short: Some('s'),
                typ: ParamType::String,
                required: true,
                    fields: None,
            }],
            returns: ReturnType::Table,
            example: None,
        },
        FnDoc {
            name: "query_build",
            description:
                "Build a query string from a table of key-value pairs. Accepts {a=\"1\", b=\"2\"} or {{\"a\",\"1\"},{\"b\",\"2\"}}.",
            params: &[Param {
                name: "params",
                short: Some('p'),
                typ: ParamType::Table,
                required: true,
                    fields: None,
            }],
            returns: ReturnType::String,
            example: None,
        },
        FnDoc {
            name: "join",
            description: "Resolve a relative URL against a base URL. Returns the resolved absolute URL string.",
            params: &[
                Param {
                    name: "base",
                    short: Some('b'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "relative",
                    short: Some('r'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
            ],
            returns: ReturnType::String,
            example: None,
        },
    ],
};

/// The set of characters to percent-encode (everything except unreserved chars).
/// Unreserved = ALPHA / DIGIT / "-" / "." / "_" / "~" (RFC 3986 Section 2.3)
const ENCODE_SET: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'~');

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/// Register `url.*` globals in the Lua VM.
pub fn register_url_globals(lua: &Lua) -> Result<(), mlua::Error> {
    let url_table = lua.create_table()?;

    // --- url.parse(str) -> table or nil ---
    url_table.set(
        "parse",
        lua.create_function(|lua, args: MultiValue| {
            let validated = validate_args(&args, URL_DOC.params("parse"), "url.parse")?;
            let url_str = match &validated[0] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };

            match Url::parse(&url_str) {
                Ok(parsed) => {
                    let result = lua.create_table()?;
                    result.set("scheme", lua.create_string(parsed.scheme())?)?;
                    result.set(
                        "host",
                        match parsed.host_str() {
                            Some(h) => Value::String(lua.create_string(h)?),
                            None => Value::Nil,
                        },
                    )?;
                    result.set(
                        "port",
                        match parsed.port() {
                            Some(p) => Value::Integer(p as mlua::Integer),
                            None => Value::Nil,
                        },
                    )?;
                    result.set("path", lua.create_string(parsed.path())?)?;
                    result.set(
                        "query",
                        match parsed.query() {
                            Some(q) => Value::String(lua.create_string(q)?),
                            None => Value::Nil,
                        },
                    )?;
                    result.set(
                        "fragment",
                        match parsed.fragment() {
                            Some(f) => Value::String(lua.create_string(f)?),
                            None => Value::Nil,
                        },
                    )?;
                    // origin — scheme + host + port (e.g., "https://example.com:8080")
                    let origin = if parsed.has_host() {
                        parsed.origin().ascii_serialization()
                    } else {
                        String::new()
                    };
                    result.set(
                        "origin",
                        if origin.is_empty() {
                            Value::Nil
                        } else {
                            Value::String(lua.create_string(&origin)?)
                        },
                    )?;

                    Ok(Value::Table(result))
                }
                Err(_) => Ok(Value::Nil),
            }
        })?,
    )?;

    // --- url.build(parts) -> string ---
    url_table.set(
        "build",
        lua.create_function(|_, args: MultiValue| {
            let validated = validate_args(&args, URL_DOC.params("build"), "url.build")?;
            let parts = match &validated[0] {
                Value::Table(t) => t.clone(),
                _ => unreachable!(),
            };

            let scheme: String = parts
                .get::<Value>("scheme")
                .ok()
                .and_then(|v| match v {
                    Value::String(s) => Some(s.to_string_lossy().to_string()),
                    _ => None,
                })
                .unwrap_or_else(|| "https".to_string());

            let host: String = parts
                .get::<Value>("host")
                .ok()
                .and_then(|v| match v {
                    Value::String(s) => Some(s.to_string_lossy().to_string()),
                    _ => None,
                })
                .ok_or_else(|| {
                    mlua::Error::external("url.build: 'host' is required in the parts table")
                })?;

            let port: Option<u16> = parts.get::<Value>("port").ok().and_then(|v| match v {
                Value::Integer(n) => Some(n as u16),
                Value::Number(n) => Some(n as u16),
                _ => None,
            });

            let path: String = parts
                .get::<Value>("path")
                .ok()
                .and_then(|v| match v {
                    Value::String(s) => Some(s.to_string_lossy().to_string()),
                    _ => None,
                })
                .unwrap_or_default();

            let query: Option<String> = parts.get::<Value>("query").ok().and_then(|v| match v {
                Value::String(s) => Some(s.to_string_lossy().to_string()),
                _ => None,
            });

            let fragment: Option<String> =
                parts.get::<Value>("fragment").ok().and_then(|v| match v {
                    Value::String(s) => Some(s.to_string_lossy().to_string()),
                    _ => None,
                });

            // Build the URL string
            let mut url_str = format!("{}://{}", scheme, host);
            if let Some(p) = port {
                url_str.push_str(&format!(":{}", p));
            }
            if !path.is_empty() {
                if !path.starts_with('/') {
                    url_str.push('/');
                }
                url_str.push_str(&path);
            }
            if let Some(q) = query {
                url_str.push('?');
                url_str.push_str(&q);
            }
            if let Some(f) = fragment {
                url_str.push('#');
                url_str.push_str(&f);
            }

            Ok(url_str)
        })?,
    )?;

    // --- url.encode(str) -> string ---
    url_table.set(
        "encode",
        lua.create_function(|_, args: MultiValue| {
            let validated = validate_args(&args, URL_DOC.params("encode"), "url.encode")?;
            let s = match &validated[0] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };

            let encoded = utf8_percent_encode(&s, ENCODE_SET).to_string();
            Ok(encoded)
        })?,
    )?;

    // --- url.decode(str) -> string ---
    url_table.set(
        "decode",
        lua.create_function(|_, args: MultiValue| {
            let validated = validate_args(&args, URL_DOC.params("decode"), "url.decode")?;
            let s = match &validated[0] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };

            let decoded = percent_decode_str(&s)
                .decode_utf8()
                .map_err(|e| mlua::Error::external(format!("url.decode: invalid UTF-8: {}", e)))?
                .to_string();
            Ok(decoded)
        })?,
    )?;

    // --- url.query_parse(str) -> list of {key, value} ---
    url_table.set(
        "query_parse",
        lua.create_function(|lua, args: MultiValue| {
            let validated = validate_args(&args, URL_DOC.params("query_parse"), "url.query_parse")?;
            let s = match &validated[0] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };

            // Strip leading '?' if present
            let qs = s.strip_prefix('?').unwrap_or(&s);

            let result = lua.create_table()?;
            let mut idx = 1i64;

            for pair_str in qs.split('&') {
                if pair_str.is_empty() {
                    continue;
                }
                let entry = lua.create_table()?;
                if let Some((k, v)) = pair_str.split_once('=') {
                    let key_decoded = percent_decode_str(k).decode_utf8_lossy().to_string();
                    let val_decoded = percent_decode_str(v).decode_utf8_lossy().to_string();
                    entry.set("key", lua.create_string(&key_decoded)?)?;
                    entry.set("value", lua.create_string(&val_decoded)?)?;
                } else {
                    let key_decoded = percent_decode_str(pair_str).decode_utf8_lossy().to_string();
                    entry.set("key", lua.create_string(&key_decoded)?)?;
                    entry.set("value", lua.create_string("")?)?;
                }
                result.raw_set(idx, entry)?;
                idx += 1;
            }

            Ok(Value::Table(result))
        })?,
    )?;

    // --- url.query_build(table) -> string ---
    url_table.set(
        "query_build",
        lua.create_function(|_, args: MultiValue| {
            let validated = validate_args(&args, URL_DOC.params("query_build"), "url.query_build")?;
            let params = match &validated[0] {
                Value::Table(t) => t.clone(),
                _ => unreachable!(),
            };

            let mut pairs: Vec<(String, String)> = Vec::new();

            // Check if it's an array of {key, value} pairs or a dict of k=v
            let raw_len = params.raw_len();
            if raw_len > 0 {
                // Check if first element is a table (array of pairs)
                let first: Value = params.raw_get(1)?;
                if matches!(first, Value::Table(_)) {
                    // Array of {key, value} tables or {[1]=key, [2]=value}
                    for i in 1..=raw_len {
                        let entry: mlua::Table = params.raw_get(i)?;
                        let key: String = entry
                            .get::<Value>("key")
                            .ok()
                            .and_then(|v| match v {
                                Value::String(s) => Some(s.to_string_lossy().to_string()),
                                _ => None,
                            })
                            .or_else(|| {
                                entry.raw_get::<Value>(1).ok().and_then(|v| match v {
                                    Value::String(s) => Some(s.to_string_lossy().to_string()),
                                    _ => None,
                                })
                            })
                            .unwrap_or_default();
                        let value: String = entry
                            .get::<Value>("value")
                            .ok()
                            .and_then(|v| match v {
                                Value::String(s) => Some(s.to_string_lossy().to_string()),
                                _ => None,
                            })
                            .or_else(|| {
                                entry.raw_get::<Value>(2).ok().and_then(|v| match v {
                                    Value::String(s) => Some(s.to_string_lossy().to_string()),
                                    _ => None,
                                })
                            })
                            .unwrap_or_default();
                        pairs.push((key, value));
                    }
                } else {
                    // Not a table-of-tables; treat as dict if there are string keys
                    // Fall through to dict handling below
                    for pair in params.clone().pairs::<String, Value>() {
                        let (k, v) = pair?;
                        let val_str = match v {
                            Value::String(s) => s.to_string_lossy().to_string(),
                            Value::Integer(n) => n.to_string(),
                            Value::Number(n) => n.to_string(),
                            Value::Boolean(b) => b.to_string(),
                            _ => String::new(),
                        };
                        pairs.push((k, val_str));
                    }
                }
            } else {
                // Dict form: {key1 = "val1", key2 = "val2"}
                for pair in params.pairs::<String, Value>() {
                    let (k, v) = pair?;
                    let val_str = match v {
                        Value::String(s) => s.to_string_lossy().to_string(),
                        Value::Integer(n) => n.to_string(),
                        Value::Number(n) => n.to_string(),
                        Value::Boolean(b) => b.to_string(),
                        _ => String::new(),
                    };
                    pairs.push((k, val_str));
                }
            }

            // Sort pairs by key for deterministic output when from dict form
            pairs.sort_by(|a, b| a.0.cmp(&b.0));

            let encoded: Vec<String> = pairs
                .iter()
                .map(|(k, v)| {
                    let ek = utf8_percent_encode(k, ENCODE_SET).to_string();
                    let ev = utf8_percent_encode(v, ENCODE_SET).to_string();
                    format!("{}={}", ek, ev)
                })
                .collect();

            Ok(encoded.join("&"))
        })?,
    )?;

    // --- url.join(base, relative) -> string ---
    url_table.set(
        "join",
        lua.create_function(|_, args: MultiValue| {
            let validated = validate_args(&args, URL_DOC.params("join"), "url.join")?;
            let base_str = match &validated[0] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };
            let relative_str = match &validated[1] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };

            let base = Url::parse(&base_str).map_err(|e| {
                mlua::Error::external(format!("url.join: invalid base URL '{}': {}", base_str, e))
            })?;
            let joined = base.join(&relative_str).map_err(|e| {
                mlua::Error::external(format!(
                    "url.join: cannot join '{}' with '{}': {}",
                    base_str, relative_str, e
                ))
            })?;

            Ok(joined.to_string())
        })?,
    )?;

    register_help_functions(lua, &url_table, &URL_DOC)?;

    lua.globals().set("url", url_table)?;
    wrap_module_with_help_hints(lua, "url")?;

    Ok(())
}
