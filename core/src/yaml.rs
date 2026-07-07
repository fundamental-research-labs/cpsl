//! YAML module for the Luau sandbox.
//!
//! Exposes `yaml.decode`, `yaml.decodeFile`, `yaml.encode`, `yaml.writeFile` as globals.
//! Uses `yaml-rust2` (pure Rust, YAML 1.2 compliant).

use crate::lua_util::{is_lua_array, register_help_functions};
use crate::mount::MountTable;
use crate::pyrt_compat::{py_type, unwrap_py_dict, unwrap_py_seq};
use crate::sandbox::{
    arg_error, validate_args, wrap_module_with_help_hints, FnDoc, ModuleDoc, Param, ParamType,
    ReturnType,
};
use mlua::{Lua, MultiValue, Value};
use std::sync::Arc;
use yaml_rust2::{Yaml, YamlEmitter, YamlLoader};

pub(crate) static YAML_DOC: ModuleDoc = ModuleDoc {
    name: "yaml",
    summary: "YAML parse & emit",
    functions: &[
        FnDoc {
            name: "decode",
            description: "Parse a YAML string into a native value.",
            params: &[Param {
                name: "text",
                short: Some('t'),
                typ: ParamType::String,
                required: true,
                fields: None,
            }],
            returns: ReturnType::Value,
            example: Some(r#"yaml.decode("name: Alice\nage: 30")"#),
        },
        FnDoc {
            name: "decodeFile",
            description: "Parse a YAML file into a native value.",
            params: &[Param {
                name: "path",
                short: None,
                typ: ParamType::String,
                required: true,
                fields: None,
            }],
            returns: ReturnType::Value,
            example: None,
        },
        FnDoc {
            name: "encode",
            description: "Serialize a value to a YAML string. Opts: {multiDoc?: boolean}",
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
                    fields: None,
                },
            ],
            returns: ReturnType::String,
            example: Some(r#"yaml.encode({name="Alice", scores={95, 87, 92}})"#),
        },
        FnDoc {
            name: "writeFile",
            description: "Write a value as YAML to a file. Same options as yaml.encode.",
            params: &[
                Param {
                    name: "path",
                    short: None,
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "value",
                    short: None,
                    typ: ParamType::Value,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "opts",
                    short: None,
                    typ: ParamType::Table,
                    required: false,
                    fields: None,
                },
            ],
            returns: ReturnType::Void,
            example: Some(
                r#"yaml.writeFile({path="/artifacts/config.yaml", value={host="localhost", port=8080}})"#,
            ),
        },
    ],
};

/// Convert a yaml_rust2::Yaml to a Lua value.
fn yaml_to_lua(lua: &Lua, val: &Yaml) -> Result<Value, mlua::Error> {
    match val {
        Yaml::Null | Yaml::BadValue => Ok(Value::Nil),
        Yaml::Boolean(b) => Ok(Value::Boolean(*b)),
        Yaml::Integer(i) => {
            if *i >= i32::MIN as i64 && *i <= i32::MAX as i64 {
                Ok(Value::Integer(*i as mlua::Integer))
            } else {
                Ok(Value::Number(*i as f64))
            }
        }
        Yaml::Real(s) => {
            let f: f64 = s.parse().map_err(mlua::Error::external)?;
            Ok(Value::Number(f))
        }
        Yaml::String(s) => Ok(Value::String(lua.create_string(s)?)),
        Yaml::Array(arr) => {
            let table = lua.create_table()?;
            for (i, item) in arr.iter().enumerate() {
                table.set(i + 1, yaml_to_lua(lua, item)?)?;
            }
            Ok(Value::Table(table))
        }
        Yaml::Hash(map) => {
            let table = lua.create_table()?;
            for (k, v) in map {
                let key = match k {
                    Yaml::String(s) => s.clone(),
                    Yaml::Integer(i) => i.to_string(),
                    Yaml::Real(s) => s.clone(),
                    Yaml::Boolean(b) => b.to_string(),
                    _ => continue,
                };
                table.set(key, yaml_to_lua(lua, v)?)?;
            }
            Ok(Value::Table(table))
        }
        Yaml::Alias(_) => Ok(Value::Nil),
    }
}

/// Convert a Lua value to yaml_rust2::Yaml.
fn lua_to_yaml(value: &Value) -> Result<Yaml, mlua::Error> {
    match value {
        Value::Nil => Ok(Yaml::Null),
        Value::Boolean(b) => Ok(Yaml::Boolean(*b)),
        Value::Integer(i) => Ok(Yaml::Integer(*i as i64)),
        Value::Number(n) => Ok(Yaml::Real(n.to_string())),
        Value::String(s) => Ok(Yaml::String(s.to_string_lossy().to_string())),
        Value::Table(t) => {
            // Detect py.list/py.tuple/py.dict wrappers from the Python runtime
            match py_type(t).as_deref() {
                Some("list") | Some("tuple") => {
                    let data = unwrap_py_seq(t)?;
                    let len = data.raw_len();
                    let mut arr = Vec::with_capacity(len);
                    for i in 1..=len {
                        let val: Value = data.raw_get(i)?;
                        arr.push(lua_to_yaml(&val)?);
                    }
                    return Ok(Yaml::Array(arr));
                }
                Some("dict") => {
                    let data = unwrap_py_dict(t)?;
                    let mut map = yaml_rust2::yaml::Hash::new();
                    for pair in data.pairs::<Value, Value>() {
                        let (k, v) = pair?;
                        let key = match &k {
                            Value::String(s) => Yaml::String(s.to_string_lossy().to_string()),
                            Value::Integer(i) => Yaml::Integer(*i as i64),
                            Value::Number(n) => Yaml::Real(n.to_string()),
                            _ => continue,
                        };
                        map.insert(key, lua_to_yaml(&v)?);
                    }
                    return Ok(Yaml::Hash(map));
                }
                _ => {}
            }

            let len = t.raw_len();
            if len > 0 && is_lua_array(t, len) {
                let mut arr = Vec::with_capacity(len);
                for i in 1..=len {
                    let val: Value = t.raw_get(i)?;
                    arr.push(lua_to_yaml(&val)?);
                }
                Ok(Yaml::Array(arr))
            } else {
                let mut map = yaml_rust2::yaml::Hash::new();
                for pair in t.clone().pairs::<Value, Value>() {
                    let (k, v) = pair?;
                    let key = match &k {
                        Value::String(s) => Yaml::String(s.to_string_lossy().to_string()),
                        Value::Integer(i) => Yaml::Integer(*i as i64),
                        Value::Number(n) => Yaml::Real(n.to_string()),
                        _ => continue,
                    };
                    map.insert(key, lua_to_yaml(&v)?);
                }
                Ok(Yaml::Hash(map))
            }
        }
        Value::Function(_) => Err(mlua::Error::external("cannot encode function as YAML")),
        _ => Err(mlua::Error::external(format!(
            "cannot encode {} as YAML",
            value.type_name()
        ))),
    }
}

/// Parse YAML text, returning the first document.
fn parse_yaml(lua: &Lua, text: &str) -> Result<Value, mlua::Error> {
    let docs = YamlLoader::load_from_str(text).map_err(mlua::Error::external)?;
    match docs.into_iter().next() {
        Some(doc) => yaml_to_lua(lua, &doc),
        None => Ok(Value::Nil),
    }
}

/// Encode a Lua value to YAML string.
fn encode_yaml(value: &Value, _multi_doc: bool) -> Result<String, mlua::Error> {
    let yaml_val = lua_to_yaml(value)?;
    let mut out = String::new();
    let mut emitter = YamlEmitter::new(&mut out);
    emitter.multiline_strings(true);
    emitter.dump(&yaml_val).map_err(mlua::Error::external)?;
    // YamlEmitter prepends "---\n"; strip it for cleaner output
    let result = out.strip_prefix("---\n").unwrap_or(&out);
    Ok(result.to_string())
}

/// Register `yaml.*` globals in the Lua VM.
pub fn register_yaml_globals(lua: &Lua, mounts: Arc<MountTable>) -> Result<(), mlua::Error> {
    let yaml_table = lua.create_table()?;

    // yaml.decode(text) -> value
    yaml_table.set(
        "decode",
        lua.create_function(|lua, args: MultiValue| {
            let validated = validate_args(&args, YAML_DOC.params("decode"), "yaml.decode")?;
            let text = match &validated[0] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!("validate_args ensures string"),
            };
            parse_yaml(lua, &text)
        })?,
    )?;

    // yaml.decodeFile(path) -> value
    {
        let m = mounts.clone();
        yaml_table.set(
            "decodeFile",
            lua.create_function(move |lua, args: MultiValue| {
                let validated =
                    validate_args(&args, YAML_DOC.params("decodeFile"), "yaml.decodeFile")?;
                let path = match &validated[0] {
                    Value::String(s) => s.to_string_lossy().to_string(),
                    _ => unreachable!("validate_args ensures string"),
                };
                let host_path = m.resolve_read(&path).map_err(mlua::Error::external)?;
                let data = std::fs::read_to_string(&host_path).map_err(mlua::Error::external)?;
                parse_yaml(lua, &data)
            })?,
        )?;
    }

    // yaml.encode(value, opts?) -> string
    // Cannot use validate_args: first arg is Value type, tables are valid YAML values.
    yaml_table.set(
        "encode",
        lua.create_function(|_, args: MultiValue| {
            if args.is_empty() {
                return Err(arg_error("yaml.encode", YAML_DOC.params("encode")));
            }
            let value = &args[0];
            let opts = args.get(1).and_then(|v| match v {
                Value::Table(t) => Some(t.clone()),
                _ => None,
            });
            let multi_doc = opts
                .and_then(|t| t.get::<bool>("multiDoc").ok())
                .unwrap_or(false);
            encode_yaml(value, multi_doc)
        })?,
    )?;

    // yaml.writeFile(path, value, opts?)
    // Cannot use validate_args: second arg is Value type, tables are valid YAML values.
    {
        let m = mounts.clone();
        yaml_table.set(
            "writeFile",
            lua.create_function(move |_, args: MultiValue| {
                if args.is_empty() {
                    return Err(arg_error("yaml.writeFile", YAML_DOC.params("writeFile")));
                }
                let first = &args[0];
                let value_opt = args.get(1).cloned();
                let opts_arg = args.get(2).and_then(|v| match v {
                    Value::Table(t) => Some(t.clone()),
                    _ => None,
                });
                let (path, value, multi_doc) = match first {
                    Value::String(s) => {
                        let p = s.to_string_lossy().to_string();
                        let v = value_opt.ok_or_else(|| {
                            mlua::Error::external(
                                "yaml.writeFile: missing required argument 'value' (value)",
                            )
                        })?;
                        let md = opts_arg
                            .and_then(|t| t.get::<bool>("multiDoc").ok())
                            .unwrap_or(false);
                        (p, v, md)
                    }
                    Value::Table(t) => {
                        let p: String = t
                            .get::<String>(1)
                            .or_else(|_| t.get::<String>("path"))
                            .map_err(|_| {
                                mlua::Error::external(
                                    "yaml.writeFile: missing required argument 'path' (string)",
                                )
                            })?;
                        let v: Value = t
                            .get::<Value>(2)
                            .ok()
                            .filter(|v| !matches!(v, Value::Nil))
                            .or_else(|| {
                                t.get::<Value>("value")
                                    .ok()
                                    .filter(|v| !matches!(v, Value::Nil))
                            })
                            .unwrap_or(Value::Nil);
                        let md = t.get::<bool>("multiDoc").unwrap_or(false);
                        (p, v, md)
                    }
                    _ => {
                        return Err(mlua::Error::external(
                            "yaml.writeFile: argument 'path' expected string, got ".to_string()
                                + first.type_name(),
                        ));
                    }
                };
                let host_path = m.resolve_write(&path).map_err(mlua::Error::external)?;
                if let Some(parent) = host_path.parent() {
                    std::fs::create_dir_all(parent).map_err(mlua::Error::external)?;
                }
                let yaml_str = encode_yaml(&value, multi_doc)?;
                std::fs::write(&host_path, yaml_str.as_bytes()).map_err(mlua::Error::external)?;
                Ok(())
            })?,
        )?;
    }

    register_help_functions(lua, &yaml_table, &YAML_DOC)?;

    lua.globals().set("yaml", yaml_table)?;
    wrap_module_with_help_hints(lua, "yaml")?;

    Ok(())
}
