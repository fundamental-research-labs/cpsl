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
            let result = if pretty {
                serde_json::to_string_pretty(&lua_to_json(value)?).map_err(mlua::Error::external)?
            } else {
                lua_to_json_string(value)?
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

/// Maximum nesting depth when encoding Lua values to JSON. Lua tables can
/// contain reference cycles, which would otherwise recurse until the process
/// stack overflows and aborts. Matches serde_json's default recursion limit.
const MAX_ENCODE_DEPTH: usize = 128;

/// Encode a Lua value as a compact JSON string.
pub(crate) fn lua_to_json_string(value: &Value) -> Result<String, mlua::Error> {
    serde_json::to_string(&lua_to_json(value)?).map_err(mlua::Error::external)
}

/// Convert a Lua value to serde_json::Value.
fn lua_to_json(value: &Value) -> Result<serde_json::Value, mlua::Error> {
    lua_to_json_at_depth(value, 0)
}

fn lua_to_json_at_depth(value: &Value, depth: usize) -> Result<serde_json::Value, mlua::Error> {
    if depth > MAX_ENCODE_DEPTH {
        return Err(mlua::Error::external(
            "maximum nesting depth exceeded while encoding JSON (cyclic table?)",
        ));
    }
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
                        arr.push(lua_to_json_at_depth(&val, depth + 1)?);
                    }
                    return Ok(serde_json::Value::Array(arr));
                }
                Some("dict") => {
                    let data = unwrap_py_dict(t)?;
                    let mut map = serde_json::Map::new();
                    for pair in data.pairs::<Value, Value>() {
                        let (k, v) = pair?;
                        let key = json_object_key(&k)?;
                        let value = lua_to_json_at_depth(&v, depth + 1)?;
                        insert_json_object_value(&mut map, key, value)?;
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
                    arr.push(lua_to_json_at_depth(&val, depth + 1)?);
                }
                Ok(serde_json::Value::Array(arr))
            } else {
                let mut map = serde_json::Map::new();
                for pair in t.clone().pairs::<Value, Value>() {
                    let (k, v) = pair?;
                    let key = json_object_key(&k)?;
                    let value = lua_to_json_at_depth(&v, depth + 1)?;
                    insert_json_object_value(&mut map, key, value)?;
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

fn json_object_key(key: &Value) -> Result<String, mlua::Error> {
    match key {
        Value::String(s) => Ok(s.to_string_lossy().to_string()),
        Value::Integer(i) => Ok(i.to_string()),
        Value::Number(n) => Ok(n.to_string()),
        _ => Err(mlua::Error::external(format!(
            "JSON object keys must be strings or numbers, got {}",
            value_type_name(key)
        ))),
    }
}

fn insert_json_object_value(
    map: &mut serde_json::Map<String, serde_json::Value>,
    key: String,
    value: serde_json::Value,
) -> Result<(), mlua::Error> {
    if map.contains_key(&key) {
        return Err(mlua::Error::external(format!(
            "multiple Lua keys map to JSON object key {key:?}"
        )));
    }
    map.insert(key, value);
    Ok(())
}
