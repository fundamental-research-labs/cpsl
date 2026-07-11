//! Parsing, range I/O, and output conversion for `fs.read`.

use super::{arg_error, slice_lines, FS_DOC};
use mlua::{Lua, MultiValue, Table, Value};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

const LUAU_BUFFER_MAX_BYTES: u64 = 1 << 30;
const BASE64_MAX_INPUT_BYTES: u64 = (LUAU_BUFFER_MAX_BYTES / 4) * 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReadMode {
    Text,
    Buffer,
    Base64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OptionsContext {
    OptionsOnly,
    CombinedArguments,
}

pub(super) struct ReadRequest {
    pub(super) path: String,
    mode: ReadMode,
    line_offset: Option<usize>,
    line_limit: Option<usize>,
    byte_offset: Option<u64>,
    byte_count: Option<u64>,
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
                    Some(Value::Table(opts)) => {
                        Self::from_options(path, opts, OptionsContext::OptionsOnly)?
                    }
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
                        Self::from_options(path, &opts, OptionsContext::OptionsOnly)?
                    }
                    Value::Nil | Value::Integer(_) | Value::Number(_) => {
                        Self::from_options(path, &args, OptionsContext::CombinedArguments)?
                    }
                    value => return Err(type_error("opts", "table", &value)),
                }
            }
            value => return Err(type_error("path", "string", value)),
        };

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
            byte_count: None,
        })
    }

    fn from_options(
        path: String,
        opts: &Table,
        context: OptionsContext,
    ) -> Result<Self, mlua::Error> {
        let opts = crate::pyrt_compat::unwrap_py_dict(opts)?;
        validate_option_keys(&opts, context)?;
        let mode = parse_mode(&opts.get::<Value>("mode")?)?;
        let offset = table_value(&opts, "offset", 2)?;

        match mode {
            ReadMode::Text => {
                if !matches!(opts.get::<Value>("count")?, Value::Nil) {
                    return Err(mlua::Error::external(
                        "fs.read: count is byte-based and requires mode=\"buffer\" or mode=\"base64\"; use limit for text lines",
                    ));
                }
                Ok(Self {
                    path,
                    mode,
                    line_offset: parse_line_range(Some(&offset), "offset")?,
                    line_limit: parse_line_range(Some(&table_value(&opts, "limit", 3)?), "limit")?,
                    byte_offset: None,
                    byte_count: None,
                })
            }
            ReadMode::Buffer | ReadMode::Base64 => {
                if !matches!(opts.get::<Value>("limit")?, Value::Nil) {
                    return Err(mlua::Error::external(
                        "fs.read: limit is line-based and only valid in text mode; use count for bytes",
                    ));
                }
                Ok(Self {
                    path,
                    mode,
                    line_offset: None,
                    line_limit: None,
                    byte_offset: parse_byte_range(&offset, "offset")?,
                    byte_count: parse_byte_range(&table_value(&opts, "count", 3)?, "count")?,
                })
            }
        }
    }

    /// Read host-backed data and convert it to the requested Luau representation.
    pub(super) fn read_host_value(&self, lua: &Lua, path: &Path) -> Result<Value, mlua::Error> {
        if self.mode == ReadMode::Buffer {
            return self.read_host_buffer(lua, path);
        }

        let bytes = self.read_host_bytes(path).map_err(mlua::Error::external)?;
        self.to_lua(lua, &bytes)
    }

    fn read_host_bytes(&self, path: &Path) -> std::io::Result<Vec<u8>> {
        if self.mode == ReadMode::Text {
            return std::fs::read(path);
        }

        let mut file = File::open(path)?;
        let metadata = file.metadata()?;
        let offset = self.byte_offset.unwrap_or(0);
        if metadata.is_file() && offset != 0 && offset >= metadata.len() {
            // Some platforms reject offsets beyond their signed seek range. For a
            // regular file, its known EOF lets us return the usual empty slice first.
            return Ok(Vec::new());
        }
        if metadata.is_file() && metadata.len() > 0 {
            let selected_bytes = self
                .byte_count
                .map(|count| count.min(metadata.len().saturating_sub(offset)))
                .unwrap_or_else(|| metadata.len().saturating_sub(offset));
            self.ensure_mode_input_size(selected_bytes)?;
        }
        if offset != 0 {
            file.seek(SeekFrom::Start(offset))?;
        }

        let mut bytes = Vec::new();
        let read_limit = self
            .mode_input_limit()
            .map(|limit| limit.saturating_add(1))
            .unwrap_or(u64::MAX);
        let read_limit = self
            .byte_count
            .map(|count| count.min(read_limit))
            .unwrap_or(read_limit);
        file.take(read_limit).read_to_end(&mut bytes)?;
        self.ensure_mode_input_size(bytes.len() as u64)?;
        Ok(bytes)
    }

    fn read_host_buffer(&self, lua: &Lua, path: &Path) -> Result<Value, mlua::Error> {
        let mut file = File::open(path).map_err(mlua::Error::external)?;
        let metadata = file.metadata().map_err(mlua::Error::external)?;
        let offset = self.byte_offset.unwrap_or(0);

        if metadata.is_file() && offset != 0 && offset >= metadata.len() {
            return lua.create_buffer([]).map(Value::Buffer);
        }

        // A regular file with a known non-zero length can be streamed directly
        // into its final Luau buffer, avoiding a second full-size allocation.
        if metadata.is_file() && metadata.len() > 0 {
            let selected_bytes = self
                .byte_count
                .map(|count| count.min(metadata.len().saturating_sub(offset)))
                .unwrap_or_else(|| metadata.len().saturating_sub(offset));
            self.ensure_mode_input_size(selected_bytes)
                .map_err(mlua::Error::external)?;
            if offset != 0 {
                file.seek(SeekFrom::Start(offset))
                    .map_err(mlua::Error::external)?;
            }

            let selected_len = usize::try_from(selected_bytes).map_err(mlua::Error::external)?;
            let buffer = lua.create_buffer_with_capacity(selected_len)?;
            let copied = {
                let mut cursor = buffer.clone().cursor();
                let mut selected = (&mut file).take(selected_bytes);
                std::io::copy(&mut selected, &mut cursor).map_err(mlua::Error::external)?
            };
            if copied == selected_bytes {
                let requested_more = self
                    .byte_count
                    .map(|count| count > selected_bytes)
                    .unwrap_or(true);
                if requested_more {
                    let mut probe = [0_u8; 1];
                    if file.read(&mut probe).map_err(mlua::Error::external)? != 0 {
                        return Err(mlua::Error::external(
                            "fs.read: file changed size while reading; retry the operation",
                        ));
                    }
                }
                return Ok(Value::Buffer(buffer));
            }

            // If the file shrank after metadata was read, return only the bytes
            // actually observed instead of exposing zero-filled tail bytes.
            let bytes = buffer.to_vec();
            return lua
                .create_buffer(&bytes[..copied as usize])
                .map(Value::Buffer);
        }

        // Streams and dynamically-sized files cannot be pre-sized. Bound the
        // actual read with one sentinel byte so growth cannot bypass the limit.
        if offset != 0 {
            file.seek(SeekFrom::Start(offset))
                .map_err(mlua::Error::external)?;
        }
        let read_limit = self
            .byte_count
            .map(|count| count.min(LUAU_BUFFER_MAX_BYTES + 1))
            .unwrap_or(LUAU_BUFFER_MAX_BYTES + 1);
        let mut bytes = Vec::new();
        file.take(read_limit)
            .read_to_end(&mut bytes)
            .map_err(mlua::Error::external)?;
        self.ensure_mode_input_size(bytes.len() as u64)
            .map_err(mlua::Error::external)?;
        lua.create_buffer(bytes).map(Value::Buffer)
    }

    /// Convert synthetic data, applying its byte range in memory first.
    pub(super) fn synthetic_value(&self, lua: &Lua, bytes: &[u8]) -> Result<Value, mlua::Error> {
        let bytes = if self.mode == ReadMode::Text {
            bytes
        } else {
            slice_bytes(bytes, self.byte_offset, self.byte_count)
        };
        self.to_lua(lua, bytes)
    }

    fn to_lua(&self, lua: &Lua, bytes: &[u8]) -> Result<Value, mlua::Error> {
        self.ensure_mode_input_size(bytes.len() as u64)
            .map_err(mlua::Error::external)?;
        match self.mode {
            ReadMode::Text => {
                let text = std::str::from_utf8(bytes).map_err(|_| {
                    mlua::Error::external(format!(
                        "fs.read: {} is not valid UTF-8; use mode=\"buffer\" for in-sandbox bytes or mode=\"base64\" for safe output",
                        self.path
                    ))
                })?;
                let sliced = slice_lines(text, self.line_offset, self.line_limit);
                lua.create_string(&sliced).map(Value::String)
            }
            ReadMode::Buffer => lua.create_buffer(bytes).map(Value::Buffer),
            ReadMode::Base64 => {
                let encoded = crate::base64_codec::encode(bytes);
                lua.create_string(encoded).map(Value::String)
            }
        }
    }

    fn mode_input_limit(&self) -> Option<u64> {
        match self.mode {
            ReadMode::Text => None,
            ReadMode::Buffer => Some(LUAU_BUFFER_MAX_BYTES),
            ReadMode::Base64 => Some(BASE64_MAX_INPUT_BYTES),
        }
    }

    fn ensure_mode_input_size(&self, size: u64) -> std::io::Result<()> {
        let Some(limit) = self.mode_input_limit() else {
            return Ok(());
        };
        if size <= limit {
            return Ok(());
        }

        let message = match self.mode {
            ReadMode::Buffer => {
                "fs.read: selected range exceeds Luau's 1 GiB buffer limit; use count to read a smaller range"
            }
            ReadMode::Base64 => {
                "fs.read: selected range is too large for base64 output in a Luau string; use count to read at most 805306368 bytes"
            }
            ReadMode::Text => unreachable!(),
        };
        Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            message,
        ))
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
        Value::String(mode) if mode.as_bytes() == b"buffer" => Ok(ReadMode::Buffer),
        Value::String(mode) if mode.as_bytes() == b"base64" => Ok(ReadMode::Base64),
        Value::String(mode) => Err(mlua::Error::external(format!(
            "fs.read: invalid mode '{}'; expected text, buffer, or base64",
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

fn slice_bytes(bytes: &[u8], offset: Option<u64>, count: Option<u64>) -> &[u8] {
    let start = offset
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(if offset.is_some() { usize::MAX } else { 0 })
        .min(bytes.len());
    let count = count
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(usize::MAX);
    let end = start.saturating_add(count).min(bytes.len());
    &bytes[start..end]
}

fn validate_option_keys(options: &Table, context: OptionsContext) -> Result<(), mlua::Error> {
    for pair in options.clone().pairs::<Value, Value>() {
        let (key, _) = pair?;
        let known = match &key {
            Value::String(key) if key.as_bytes() == b"path" => {
                context == OptionsContext::CombinedArguments
            }
            Value::String(key) => matches!(
                key.as_bytes().as_ref(),
                b"offset" | b"limit" | b"count" | b"mode"
            ),
            Value::Integer(1) => context == OptionsContext::CombinedArguments,
            Value::Integer(2..=3) => true,
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
