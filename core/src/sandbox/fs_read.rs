//! Parsing, range I/O, and output conversion for `fs.read`.

use super::{arg_error, slice_lines, FS_DOC};
use mlua::{Lua, MultiValue, Table, Value};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReadMode {
    Text,
    Binary,
    Base64,
}

pub(super) struct ReadRequest {
    pub(super) path: String,
    mode: ReadMode,
    line_offset: Option<usize>,
    line_limit: Option<usize>,
    byte_offset: Option<u64>,
    byte_limit: Option<u64>,
}

impl ReadRequest {
    pub(super) fn parse(args: &MultiValue) -> Result<Self, mlua::Error> {
        let first = args
            .front()
            .ok_or_else(|| arg_error("fs.read", FS_DOC.params("read")))?;

        let request = match first {
            Value::String(path) => {
                let path = path.to_string_lossy().to_string();
                match args.get(1) {
                    Some(Value::Table(opts)) => Self::from_options(path, opts)?,
                    Some(Value::Integer(_) | Value::Number(_) | Value::Nil) | None => {
                        Self::from_legacy_args(path, args.get(1), args.get(2))?
                    }
                    Some(value) => {
                        return Err(type_error("offset or opts", "number or table", value))
                    }
                }
            }
            Value::Table(args) => {
                let args = crate::pyrt_compat::unwrap_py_dict(args)?;
                let path = table_value(&args, "path", 1)?;
                let path = match path {
                    Value::String(path) => path.to_string_lossy().to_string(),
                    Value::Nil => return Err(arg_error("fs.read", FS_DOC.params("read"))),
                    value => return Err(type_error("path", "string", &value)),
                };
                match table_value(&args, "opts", 2)? {
                    Value::Table(opts) => {
                        validate_wrapper_keys(&args)?;
                        Self::from_options(path, &opts)?
                    }
                    Value::Nil | Value::Integer(_) | Value::Number(_) => {
                        Self::from_options(path, &args)?
                    }
                    value => return Err(type_error("opts", "table", &value)),
                }
            }
            value => return Err(type_error("path", "string", value)),
        };

        request.validate()?;
        Ok(request)
    }

    fn from_legacy_args(
        path: String,
        offset: Option<&Value>,
        limit: Option<&Value>,
    ) -> Result<Self, mlua::Error> {
        Ok(Self {
            path,
            mode: ReadMode::Text,
            line_offset: parse_line_range(offset, "offset")?,
            line_limit: parse_line_range(limit, "limit")?,
            byte_offset: None,
            byte_limit: None,
        })
    }

    fn from_options(path: String, opts: &Table) -> Result<Self, mlua::Error> {
        let opts = crate::pyrt_compat::unwrap_py_dict(opts)?;
        validate_option_keys(&opts)?;
        Ok(Self {
            path,
            mode: parse_mode(&opts.get::<Value>("mode")?)?,
            line_offset: parse_line_range(Some(&table_value(&opts, "offset", 2)?), "offset")?,
            line_limit: parse_line_range(Some(&table_value(&opts, "limit", 3)?), "limit")?,
            byte_offset: parse_byte_range(&opts.get::<Value>("byte_offset")?, "byte_offset")?,
            byte_limit: parse_byte_range(&opts.get::<Value>("byte_limit")?, "byte_limit")?,
        })
    }

    fn validate(&self) -> Result<(), mlua::Error> {
        match self.mode {
            ReadMode::Text if self.byte_offset.is_some() || self.byte_limit.is_some() => {
                Err(mlua::Error::external(
                    "fs.read: byte_offset/byte_limit require mode=\"binary\" or mode=\"base64\"",
                ))
            }
            ReadMode::Binary | ReadMode::Base64
                if self.line_offset.is_some() || self.line_limit.is_some() =>
            {
                Err(mlua::Error::external(
                    "fs.read: offset/limit are line-based and only valid in text mode; use byte_offset/byte_limit",
                ))
            }
            _ => Ok(()),
        }
    }

    /// Read host-backed data, applying byte ranges at the I/O layer.
    pub(super) fn read_host(&self, path: &Path) -> std::io::Result<Vec<u8>> {
        if self.mode == ReadMode::Text {
            return std::fs::read(path);
        }

        let mut file = File::open(path)?;
        if let Some(offset) = self.byte_offset {
            file.seek(SeekFrom::Start(offset))?;
        }

        let mut bytes = Vec::new();
        match self.byte_limit {
            Some(limit) => file.take(limit).read_to_end(&mut bytes)?,
            None => file.read_to_end(&mut bytes)?,
        };
        Ok(bytes)
    }

    /// Convert host data whose byte range was already applied by `read_host`.
    pub(super) fn host_value(&self, lua: &Lua, bytes: &[u8]) -> Result<Value, mlua::Error> {
        self.to_lua(lua, bytes)
    }

    /// Convert synthetic data, applying its byte range in memory first.
    pub(super) fn synthetic_value(&self, lua: &Lua, bytes: &[u8]) -> Result<Value, mlua::Error> {
        let bytes = if self.mode == ReadMode::Text {
            bytes
        } else {
            slice_bytes(bytes, self.byte_offset, self.byte_limit)
        };
        self.to_lua(lua, bytes)
    }

    fn to_lua(&self, lua: &Lua, bytes: &[u8]) -> Result<Value, mlua::Error> {
        match self.mode {
            ReadMode::Text => {
                let text = std::str::from_utf8(bytes).map_err(|_| {
                    mlua::Error::external(format!(
                        "fs.read: {} is not valid UTF-8; use mode=\"binary\" for in-sandbox bytes or mode=\"base64\" for safe output",
                        self.path
                    ))
                })?;
                let sliced = slice_lines(text, self.line_offset, self.line_limit);
                lua.create_string(&sliced).map(Value::String)
            }
            ReadMode::Binary => lua.create_string(bytes).map(Value::String),
            ReadMode::Base64 => {
                let encoded = crate::base64_codec::encode(bytes);
                lua.create_string(encoded).map(Value::String)
            }
        }
    }
}

fn table_value(table: &Table, name: &str, index: usize) -> Result<Value, mlua::Error> {
    let value = table.get::<Value>(name)?;
    if matches!(value, Value::Nil) {
        table.get(index)
    } else {
        Ok(value)
    }
}

fn parse_mode(value: &Value) -> Result<ReadMode, mlua::Error> {
    match value {
        Value::Nil => Ok(ReadMode::Text),
        Value::String(mode) if mode.as_bytes() == b"text" => Ok(ReadMode::Text),
        Value::String(mode) if mode.as_bytes() == b"binary" => Ok(ReadMode::Binary),
        Value::String(mode) if mode.as_bytes() == b"base64" => Ok(ReadMode::Base64),
        Value::String(mode) => Err(mlua::Error::external(format!(
            "fs.read: invalid mode '{}'; expected text, binary, or base64",
            mode.to_string_lossy()
        ))),
        value => Err(type_error("mode", "string", value)),
    }
}

// Preserve the legacy line-range behavior: negative values clamp to the first line
// and fractional values truncate. Oversized values saturate instead of wrapping on wasm32.
fn parse_line_range(value: Option<&Value>, name: &str) -> Result<Option<usize>, mlua::Error> {
    match value {
        None | Some(Value::Nil) => Ok(None),
        Some(Value::Integer(value)) => {
            Ok(Some(usize::try_from((*value).max(0)).unwrap_or(usize::MAX)))
        }
        Some(Value::Number(value)) => Ok(Some(value.max(0.0) as usize)),
        Some(value) => Err(type_error(name, "number", value)),
    }
}

fn parse_byte_range(value: &Value, name: &str) -> Result<Option<u64>, mlua::Error> {
    let parsed = match value {
        Value::Nil => return Ok(None),
        Value::Integer(value) if *value >= 0 => *value as u64,
        Value::Number(value)
            if value.is_finite()
                && *value >= 0.0
                && value.fract() == 0.0
                && *value < u64::MAX as f64 =>
        {
            *value as u64
        }
        Value::Integer(_) | Value::Number(_) => {
            return Err(mlua::Error::external(format!(
                "fs.read: option '{}' must be a non-negative integer",
                name
            )))
        }
        value => return Err(type_error(name, "number", value)),
    };
    Ok(Some(parsed))
}

fn slice_bytes(bytes: &[u8], offset: Option<u64>, limit: Option<u64>) -> &[u8] {
    let start = offset
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(if offset.is_some() { usize::MAX } else { 0 })
        .min(bytes.len());
    let limit = limit
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(usize::MAX);
    let end = start.saturating_add(limit).min(bytes.len());
    &bytes[start..end]
}

fn validate_option_keys(options: &Table) -> Result<(), mlua::Error> {
    for pair in options.clone().pairs::<Value, Value>() {
        let (key, _) = pair?;
        let known = match &key {
            Value::String(key) => matches!(
                key.as_bytes().as_ref(),
                b"path" | b"offset" | b"limit" | b"mode" | b"byte_offset" | b"byte_limit"
            ),
            Value::Integer(1..=3) => true,
            _ => false,
        };
        if !known {
            return Err(mlua::Error::external(format!(
                "fs.read: unknown option {}",
                display_key(&key)
            )));
        }
    }
    Ok(())
}

fn validate_wrapper_keys(wrapper: &Table) -> Result<(), mlua::Error> {
    for pair in wrapper.clone().pairs::<Value, Value>() {
        let (key, _) = pair?;
        let known = match &key {
            Value::String(key) => matches!(key.as_bytes().as_ref(), b"path" | b"opts"),
            Value::Integer(1..=2) => true,
            _ => false,
        };
        if !known {
            return Err(mlua::Error::external(format!(
                "fs.read: unknown argument {}",
                display_key(&key)
            )));
        }
    }
    Ok(())
}

fn display_key(key: &Value) -> String {
    match key {
        Value::String(key) => format!("'{}'", key.to_string_lossy()),
        Value::Integer(key) => key.to_string(),
        _ => format!("of type {}", key.type_name()),
    }
}

fn type_error(name: &str, expected: &str, value: &Value) -> mlua::Error {
    mlua::Error::external(format!(
        "fs.read: argument '{}' expected {}, got {}",
        name,
        expected,
        value.type_name()
    ))
}
