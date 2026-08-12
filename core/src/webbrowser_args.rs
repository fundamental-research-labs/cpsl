//! Shared Lua argument and value conversion helpers for the webbrowser module.

use crate::lua_util::{is_lua_array, value_type_name};
use mlua::{Lua, MultiValue, Table, Value};
use serde_json::{Map, Number};
use std::path::Path;

pub(super) fn set_optional_string_field(
    request: &mut Map<String, serde_json::Value>,
    key: &str,
    value: Option<String>,
) {
    if let Some(value) = value {
        request.insert(key.to_string(), serde_json::Value::String(value));
    }
}

pub(super) fn optional_only_table(args: &MultiValue, fn_name: &str) -> Result<Option<Table>, mlua::Error> {
    match args.len() {
        0 => Ok(None),
        1 => Ok(Some(value_table(&args[0], fn_name, "opts")?)),
        _ => Err(mlua::Error::external(format!(
            "{fn_name}: expected optional opts table, got {} arguments",
            args.len()
        ))),
    }
}

pub(super) fn ensure_no_args(args: &MultiValue, fn_name: &str) -> Result<(), mlua::Error> {
    if args.is_empty() {
        Ok(())
    } else {
        Err(mlua::Error::external(format!(
            "{fn_name}: expected no arguments, got {}",
            args.len()
        )))
    }
}

pub(super) fn single_table_arg(args: &MultiValue) -> Option<Table> {
    if args.len() == 1 {
        if let Some(Value::Table(table)) = args.get(0) {
            return Some(table.clone());
        }
    }
    None
}

pub(super) fn table_field(table: &Table, aliases: &[&str]) -> Result<Value, mlua::Error> {
    for alias in aliases {
        let value = table.get::<Value>(*alias)?;
        if !matches!(value, Value::Nil) {
            return Ok(value);
        }
    }
    Ok(Value::Nil)
}

pub(super) fn required_string(table: &Table, fn_name: &str, aliases: &[&str]) -> Result<String, mlua::Error> {
    match table_field(table, aliases)? {
        Value::String(s) => Ok(s.to_string_lossy().to_string()),
        Value::Nil => Err(mlua::Error::external(format!(
            "{fn_name}: missing required argument '{}' (string)",
            aliases[0]
        ))),
        other => Err(mlua::Error::external(format!(
            "{fn_name}: argument '{}' expected string, got {}",
            aliases[0],
            other.type_name()
        ))),
    }
}

pub(super) fn required_number(table: &Table, fn_name: &str, aliases: &[&str]) -> Result<f64, mlua::Error> {
    match table_field(table, aliases)? {
        Value::Integer(value) => Ok(value as f64),
        Value::Number(value) => Ok(value),
        Value::Nil => Err(mlua::Error::external(format!(
            "{fn_name}: missing required argument '{}' (number)",
            aliases[0]
        ))),
        other => Err(mlua::Error::external(format!(
            "{fn_name}: argument '{}' expected number, got {}",
            aliases[0],
            other.type_name()
        ))),
    }
}

pub(super) fn optional_string(
    table: &Option<Table>,
    fn_name: &str,
    aliases: &[&str],
) -> Result<Option<String>, mlua::Error> {
    let Some(table) = table else {
        return Ok(None);
    };
    match table_field(table, aliases)? {
        Value::Nil => Ok(None),
        Value::String(s) => Ok(Some(s.to_string_lossy().to_string())),
        other => Err(mlua::Error::external(format!(
            "{fn_name}: option '{}' expected string, got {}",
            aliases[0],
            other.type_name()
        ))),
    }
}

pub(super) fn optional_bool(
    table: &Option<Table>,
    fn_name: &str,
    aliases: &[&str],
) -> Result<Option<bool>, mlua::Error> {
    let Some(table) = table else {
        return Ok(None);
    };
    match table_field(table, aliases)? {
        Value::Nil => Ok(None),
        Value::Boolean(value) => Ok(Some(value)),
        other => Err(mlua::Error::external(format!(
            "{fn_name}: option '{}' expected boolean, got {}",
            aliases[0],
            other.type_name()
        ))),
    }
}

pub(super) fn optional_number(
    table: &Option<Table>,
    fn_name: &str,
    aliases: &[&str],
) -> Result<Option<f64>, mlua::Error> {
    let Some(table) = table else {
        return Ok(None);
    };
    match table_field(table, aliases)? {
        Value::Nil => Ok(None),
        Value::Integer(value) => Ok(Some(value as f64)),
        Value::Number(value) => Ok(Some(value)),
        other => Err(mlua::Error::external(format!(
            "{fn_name}: option '{}' expected number, got {}",
            aliases[0],
            other.type_name()
        ))),
    }
}

pub(super) fn optional_integer(
    table: &Option<Table>,
    fn_name: &str,
    aliases: &[&str],
) -> Result<Option<i64>, mlua::Error> {
    let Some(table) = table else {
        return Ok(None);
    };
    match table_field(table, aliases)? {
        Value::Nil => Ok(None),
        Value::Integer(value) => Ok(Some(value)),
        Value::Number(value) if value.fract() == 0.0 => Ok(Some(value as i64)),
        Value::Number(_) => Err(mlua::Error::external(format!(
            "{fn_name}: option '{}' expected integer",
            aliases[0]
        ))),
        other => Err(mlua::Error::external(format!(
            "{fn_name}: option '{}' expected number, got {}",
            aliases[0],
            other.type_name()
        ))),
    }
}

pub(super) fn value_string(value: &Value, fn_name: &str, name: &str) -> Result<String, mlua::Error> {
    match value {
        Value::String(s) => Ok(s.to_string_lossy().to_string()),
        Value::Nil => Err(mlua::Error::external(format!(
            "{fn_name}: missing required argument '{name}' (string)"
        ))),
        other => Err(mlua::Error::external(format!(
            "{fn_name}: argument '{name}' expected string, got {}",
            other.type_name()
        ))),
    }
}

pub(super) fn value_number(value: &Value, fn_name: &str, name: &str) -> Result<f64, mlua::Error> {
    match value {
        Value::Integer(value) => Ok(*value as f64),
        Value::Number(value) => Ok(*value),
        Value::Nil => Err(mlua::Error::external(format!(
            "{fn_name}: missing required argument '{name}' (number)"
        ))),
        other => Err(mlua::Error::external(format!(
            "{fn_name}: argument '{name}' expected number, got {}",
            other.type_name()
        ))),
    }
}

pub(super) fn value_integer(value: &Value, fn_name: &str, name: &str) -> Result<i64, mlua::Error> {
    match value {
        Value::Integer(value) => Ok(*value),
        Value::Number(value) if value.fract() == 0.0 => Ok(*value as i64),
        Value::Number(_) => Err(mlua::Error::external(format!(
            "{fn_name}: argument '{name}' expected integer"
        ))),
        Value::Nil => Err(mlua::Error::external(format!(
            "{fn_name}: missing required argument '{name}' (number)"
        ))),
        other => Err(mlua::Error::external(format!(
            "{fn_name}: argument '{name}' expected number, got {}",
            other.type_name()
        ))),
    }
}

pub(super) fn value_table(value: &Value, fn_name: &str, name: &str) -> Result<Table, mlua::Error> {
    match value {
        Value::Table(table) => Ok(table.clone()),
        Value::Nil => Err(mlua::Error::external(format!(
            "{fn_name}: missing required argument '{name}' (table)"
        ))),
        other => Err(mlua::Error::external(format!(
            "{fn_name}: argument '{name}' expected table, got {}",
            other.type_name()
        ))),
    }
}

pub(super) fn string_array(table: &Table, fn_name: &str, name: &str) -> Result<Vec<String>, mlua::Error> {
    let len = table.raw_len();
    if len == 0 || !is_lua_array(table, len) {
        return Err(mlua::Error::external(format!(
            "{fn_name}: argument '{name}' expected array table"
        )));
    }
    let mut values = Vec::with_capacity(len);
    for i in 1..=len {
        let value: Value = table.raw_get(i)?;
        values.push(value_string(&value, fn_name, name)?);
    }
    Ok(values)
}

pub(super) fn number_value(value: f64, fn_name: &str, name: &str) -> Result<serde_json::Value, mlua::Error> {
    let number = Number::from_f64(value).ok_or_else(|| {
        mlua::Error::external(format!("{fn_name}: argument '{name}' must be finite"))
    })?;
    Ok(serde_json::Value::Number(number))
}

pub(super) fn validate_screenshot_path(path: &str) -> Result<(), mlua::Error> {
    let extension = Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if matches!(extension.as_str(), "png" | "jpg" | "jpeg") {
        Ok(())
    } else {
        Err(mlua::Error::external(
            "webbrowser.screenshot: path must end in .png, .jpg, or .jpeg",
        ))
    }
}

pub(super) fn json_to_lua(lua: &Lua, value: &serde_json::Value) -> Result<Value, mlua::Error> {
    match value {
        serde_json::Value::Null => Ok(Value::Nil),
        serde_json::Value::Bool(value) => Ok(Value::Boolean(*value)),
        serde_json::Value::Number(number) => {
            if let Some(value) = number.as_i64() {
                if value >= i32::MIN as i64 && value <= i32::MAX as i64 {
                    Ok(Value::Integer(value as mlua::Integer))
                } else {
                    Ok(Value::Number(value as f64))
                }
            } else if let Some(value) = number.as_f64() {
                Ok(Value::Number(value))
            } else {
                Ok(Value::Nil)
            }
        }
        serde_json::Value::String(value) => Ok(Value::String(lua.create_string(value)?)),
        serde_json::Value::Array(values) => {
            let table = lua.create_table()?;
            for (index, item) in values.iter().enumerate() {
                table.set(index + 1, json_to_lua(lua, item)?)?;
            }
            Ok(Value::Table(table))
        }
        serde_json::Value::Object(values) => {
            let table = lua.create_table()?;
            for (key, item) in values {
                table.set(key.as_str(), json_to_lua(lua, item)?)?;
            }
            Ok(Value::Table(table))
        }
    }
}

pub(super) fn lua_to_json(value: &Value) -> Result<serde_json::Value, mlua::Error> {
    match value {
        Value::Nil => Ok(serde_json::Value::Null),
        Value::Boolean(value) => Ok(serde_json::Value::Bool(*value)),
        Value::Integer(value) => Ok(serde_json::Value::Number((*value).into())),
        Value::Number(value) => number_value(*value, "webbrowser", "value"),
        Value::String(value) => Ok(serde_json::Value::String(
            value.to_string_lossy().to_string(),
        )),
        Value::Table(table) => {
            let len = table.raw_len();
            if len > 0 && is_lua_array(table, len) {
                let mut values = Vec::with_capacity(len);
                for i in 1..=len {
                    let value: Value = table.raw_get(i)?;
                    values.push(lua_to_json(&value)?);
                }
                Ok(serde_json::Value::Array(values))
            } else {
                let mut map = Map::new();
                for pair in table.clone().pairs::<Value, Value>() {
                    let (key, value) = pair?;
                    let key = match key {
                        Value::String(value) => value.to_string_lossy().to_string(),
                        Value::Integer(value) => value.to_string(),
                        Value::Number(value) => value.to_string(),
                        other => {
                            return Err(mlua::Error::external(format!(
                                "JSON object keys must be strings, got {}",
                                value_type_name(&other)
                            )))
                        }
                    };
                    map.insert(key, lua_to_json(&value)?);
                }
                Ok(serde_json::Value::Object(map))
            }
        }
        Value::Function(_) => Err(mlua::Error::external("cannot encode function as JSON")),
        other => Err(mlua::Error::external(format!(
            "cannot encode {} as JSON",
            value_type_name(other)
        ))),
    }
}
