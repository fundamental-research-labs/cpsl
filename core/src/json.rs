//! JSON module for the Luau sandbox.
//!
//! Exposes `json.decode(str)` and `json.encode(value)` as globals.
//! Uses mlua's built-in serialize feature for Lua↔JSON conversion.

use crate::lua_util::{is_lua_array, register_help_functions, value_type_name};
use crate::pyrt_compat::{py_type, unwrap_py_dict, unwrap_py_seq};
use crate::sandbox::{
    arg_error, validate_args, wrap_module_with_help_hints, FieldDoc, FnDoc, ModuleDoc, Param,
    ParamType, ReturnType,
};
use mlua::{Lua, MultiValue, Value};

const JSON_ENCODE_OPTS_FIELDS: &[FieldDoc] = &[FieldDoc {
    name: "pretty",
    typ: "boolean",
    required: false,
    description: "Pretty-print with indentation (default false)",
}];

pub(crate) static JSON_DOC: ModuleDoc = ModuleDoc {
    name: "json",
    summary: "JSON encode & decode",
    functions: &[
        FnDoc {
            name: "decode",
            description: "Parse a JSON string into a native value.",
            params: &[Param {
                name: "text",
                short: Some('t'),
                typ: ParamType::String,
                required: true,
                fields: None,
            }],
            returns: ReturnType::Value,
            example: Some(r#"local data = json.decode('{"name":"Alice","age":30}')"#),
        },
        FnDoc {
            name: "encode",
            description:
                "Serialize a value to a JSON string. Pass pretty=true for indented output.",
            params: &[
                Param {
                    name: "value",
                    short: Some('v'),
                    typ: ParamType::Value,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "opts",
                    short: None,
                    typ: ParamType::Table,
                    required: false,
                    fields: Some(JSON_ENCODE_OPTS_FIELDS),
                },
            ],
            returns: ReturnType::String,
            example: Some(r#"json.encode({name = "Alice", scores = {95, 87, 92}})"#),
        },
    ],
};

/// Register `json.*` globals in the Lua VM.
pub(crate) fn register_json_globals(lua: &Lua) -> Result<(), mlua::Error> {
    let json = lua.create_table()?;

    // json.decode(str) or json.decode({[1]=str}) -> value
    json.set(
        "decode",
        lua.create_function(|lua, args: MultiValue| {
            let validated = validate_args(&args, JSON_DOC.params("decode"), "json.decode")?;
            let text = match &validated[0] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!("validate_args ensures string"),
            };
            let serde_val: serde_json::Value =
                serde_json::from_str(&text).map_err(mlua::Error::external)?;
            json_to_lua(lua, &serde_val)
        })?,
    )?;

    // json.encode(value, opts?) -> string
    // Cannot use validate_args: first arg is Value type, tables are valid JSON values
    // and would be misinterpreted as table-form calling convention.
    json.set(
        "encode",
        lua.create_function(|_, args: MultiValue| {
            if args.is_empty() {
                return Err(arg_error("json.encode", JSON_DOC.params("encode")));
            }
            let value = &args[0];
            let opts = args.get(1).and_then(|v| match v {
                Value::Table(t) => Some(t.clone()),
                _ => None,
            });
            let pretty = opts
                .and_then(|t| t.get::<bool>("pretty").ok())
                .unwrap_or(false);
            let serde_val = lua_to_json(value)?;
            let result = if pretty {
                serde_json::to_string_pretty(&serde_val).map_err(mlua::Error::external)?
            } else {
                serde_json::to_string(&serde_val).map_err(mlua::Error::external)?
            };
            Ok(result)
        })?,
    )?;

    register_help_functions(lua, &json, &JSON_DOC)?;

    lua.globals().set("json", json)?;
    wrap_module_with_help_hints(lua, "json")?;

    Ok(())
}

/// Convert a serde_json::Value to a Lua value.
fn json_to_lua(lua: &Lua, val: &serde_json::Value) -> Result<Value, mlua::Error> {
    match val {
        serde_json::Value::Null => Ok(Value::Nil),
        serde_json::Value::Bool(b) => Ok(Value::Boolean(*b)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                // Keep integers as integers when possible
                if i >= i32::MIN as i64 && i <= i32::MAX as i64 {
                    Ok(Value::Integer(i as mlua::Integer))
                } else {
                    Ok(Value::Number(i as f64))
                }
            } else if let Some(f) = n.as_f64() {
                Ok(Value::Number(f))
            } else {
                Ok(Value::Nil)
            }
        }
        serde_json::Value::String(s) => Ok(Value::String(lua.create_string(s)?)),
        serde_json::Value::Array(arr) => {
            let table = lua.create_table()?;
            for (i, item) in arr.iter().enumerate() {
                table.set(i + 1, json_to_lua(lua, item)?)?;
            }
            Ok(Value::Table(table))
        }
        serde_json::Value::Object(map) => {
            let table = lua.create_table()?;
            for (k, v) in map {
                table.set(k.as_str(), json_to_lua(lua, v)?)?;
            }
            Ok(Value::Table(table))
        }
    }
}

/// Convert a Lua value to serde_json::Value.
fn lua_to_json(value: &Value) -> Result<serde_json::Value, mlua::Error> {
    match value {
        Value::Nil => Ok(serde_json::Value::Null),
        Value::Boolean(b) => Ok(serde_json::Value::Bool(*b)),
        Value::Integer(i) => Ok(serde_json::Value::Number((*i).into())),
        Value::Number(n) => {
            let n = serde_json::Number::from_f64(*n)
                .ok_or_else(|| mlua::Error::external("cannot encode NaN or Infinity as JSON"))?;
            Ok(serde_json::Value::Number(n))
        }
        Value::String(s) => Ok(serde_json::Value::String(s.to_string_lossy().to_string())),
        Value::Table(t) => {
            // Detect py.list/py.tuple/py.dict wrappers from the Python runtime
            match py_type(t).as_deref() {
                Some("list") | Some("tuple") => {
                    let data = unwrap_py_seq(t)?;
                    let len = data.raw_len();
                    let mut arr = Vec::with_capacity(len);
                    for i in 1..=len {
                        let val: Value = data.raw_get(i)?;
                        arr.push(lua_to_json(&val)?);
                    }
                    return Ok(serde_json::Value::Array(arr));
                }
                Some("dict") => {
                    let data = unwrap_py_dict(t)?;
                    let mut map = serde_json::Map::new();
                    for pair in data.pairs::<Value, Value>() {
                        let (k, v) = pair?;
                        let key = match &k {
                            Value::String(s) => s.to_string_lossy().to_string(),
                            Value::Integer(i) => i.to_string(),
                            Value::Number(n) => n.to_string(),
                            _ => continue,
                        };
                        map.insert(key, lua_to_json(&v)?);
                    }
                    return Ok(serde_json::Value::Object(map));
                }
                _ => {}
            }

            // Determine if it's an array (consecutive integer keys 1..n) or object
            let len = t.raw_len();
            if len > 0 && is_lua_array(t, len) {
                let mut arr = Vec::with_capacity(len);
                for i in 1..=len {
                    let val: Value = t.raw_get(i)?;
                    arr.push(lua_to_json(&val)?);
                }
                Ok(serde_json::Value::Array(arr))
            } else {
                let mut map = serde_json::Map::new();
                for pair in t.clone().pairs::<Value, Value>() {
                    let (k, v) = pair?;
                    let key = match &k {
                        Value::String(s) => s.to_string_lossy().to_string(),
                        Value::Integer(i) => i.to_string(),
                        Value::Number(n) => n.to_string(),
                        _ => {
                            return Err(mlua::Error::external(format!(
                                "JSON object keys must be strings, got {}",
                                value_type_name(&k)
                            )))
                        }
                    };
                    map.insert(key, lua_to_json(&v)?);
                }
                Ok(serde_json::Value::Object(map))
            }
        }
        Value::Function(_) => Err(mlua::Error::external("cannot encode function as JSON")),
        _ => Err(mlua::Error::external(format!(
            "cannot encode {} as JSON",
            value_type_name(value)
        ))),
    }
}
