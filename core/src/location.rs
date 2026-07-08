//! Lua-facing host location module.

use crate::lua_util::register_help_functions;
use crate::sandbox::{wrap_module_with_help_hints, FnDoc, ModuleDoc, ReturnType};
use mlua::{Lua, MultiValue, Value};
use serde_json::json;
use std::sync::Arc;

pub trait LocationGateway: Send + Sync {
    fn handle_json(&self, request_json: &str) -> Result<String, String>;
}

pub(crate) static LOCATION_DOC: ModuleDoc = ModuleDoc {
    name: "location",
    summary: "Current device location with app permission handling",
    functions: &[
        FnDoc {
            name: "status",
            description: "Return Location authorization status. access is granted, denied, or undefined.",
            params: &[],
            returns: ReturnType::Table,
            example: Some("local status = location.status()"),
        },
        FnDoc {
            name: "request_access",
            description: "Prompt for Location access when access is undefined; denied access must be changed in Settings.",
            params: &[],
            returns: ReturnType::Table,
            example: Some("local status = location.request_access()"),
        },
        FnDoc {
            name: "current",
            description: "Return the current device location. Prompts when access is undefined.",
            params: &[],
            returns: ReturnType::Table,
            example: Some("local here = location.current()"),
        },
    ],
};

pub(crate) fn register_location_globals(
    lua: &Lua,
    gateway: Arc<dyn LocationGateway>,
) -> Result<(), mlua::Error> {
    let location = lua.create_table()?;

    {
        let gateway = gateway.clone();
        location.set(
            "status",
            lua.create_function(move |lua, args: MultiValue| {
                validate_no_args(&args, "location.status")?;
                call_gateway(lua, gateway.as_ref(), "status")
            })?,
        )?;
    }

    {
        let gateway = gateway.clone();
        location.set(
            "request_access",
            lua.create_function(move |lua, args: MultiValue| {
                validate_no_args(&args, "location.request_access")?;
                call_gateway(lua, gateway.as_ref(), "request_access")
            })?,
        )?;
    }

    {
        let gateway = gateway.clone();
        location.set(
            "current",
            lua.create_function(move |lua, args: MultiValue| {
                validate_no_args(&args, "location.current")?;
                call_gateway(lua, gateway.as_ref(), "current")
            })?,
        )?;
    }

    register_help_functions(lua, &location, &LOCATION_DOC)?;
    lua.globals().set("location", location)?;
    wrap_module_with_help_hints(lua, "location")?;

    Ok(())
}

fn validate_no_args(args: &MultiValue, fn_name: &str) -> Result<(), mlua::Error> {
    if args.is_empty() {
        Ok(())
    } else {
        Err(mlua::Error::external(format!(
            "{}: expected no arguments, got {}",
            fn_name,
            args.len()
        )))
    }
}

fn call_gateway(
    lua: &Lua,
    gateway: &dyn LocationGateway,
    command: &str,
) -> Result<Value, mlua::Error> {
    let request = json!({ "command": command }).to_string();
    let response = gateway
        .handle_json(&request)
        .map_err(mlua::Error::external)?;
    let value: serde_json::Value =
        serde_json::from_str(&response).map_err(mlua::Error::external)?;
    if value
        .get("ok")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
    {
        json_to_lua(lua, &value)
    } else {
        let error = value
            .get("error")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("location command failed");
        Err(mlua::Error::external(error.to_string()))
    }
}

fn json_to_lua(lua: &Lua, value: &serde_json::Value) -> Result<Value, mlua::Error> {
    match value {
        serde_json::Value::Null => Ok(Value::Nil),
        serde_json::Value::Bool(value) => Ok(Value::Boolean(*value)),
        serde_json::Value::Number(number) => {
            if let Some(value) = number.as_i64() {
                Ok(Value::Integer(value as mlua::Integer))
            } else if let Some(value) = number.as_f64() {
                Ok(Value::Number(value))
            } else {
                Ok(Value::Nil)
            }
        }
        serde_json::Value::String(value) => Ok(Value::String(lua.create_string(value)?)),
        serde_json::Value::Array(values) => {
            let table = lua.create_table()?;
            for (index, value) in values.iter().enumerate() {
                table.set(index + 1, json_to_lua(lua, value)?)?;
            }
            Ok(Value::Table(table))
        }
        serde_json::Value::Object(values) => {
            let table = lua.create_table()?;
            for (key, value) in values {
                table.set(key.as_str(), json_to_lua(lua, value)?)?;
            }
            Ok(Value::Table(table))
        }
    }
}
