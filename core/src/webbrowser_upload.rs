//! Upload argument parsing and control-key validation for the webbrowser module.

use super::{
    arg_error, required_string, single_table_arg, string_array, value_string, WEBBROWSER_DOC,
};
use crate::mount::MountTable;
use mlua::{MultiValue, Value};

const SUPPORTED_CONTROL_KEYS: &[&str] = &[
    "Enter",
    "Escape",
    "Tab",
    "Backspace",
    "Delete",
    "ArrowLeft",
    "ArrowUp",
    "ArrowRight",
    "ArrowDown",
    "Home",
    "End",
    "PageUp",
    "PageDown",
];

pub(super) fn validate_control_key(key: &str) -> Result<(), mlua::Error> {
    if SUPPORTED_CONTROL_KEYS.contains(&key) {
        return Ok(());
    }
    Err(mlua::Error::external(format!(
        "webbrowser.key_press: unsupported control key {key:?}; use one of: {}",
        SUPPORTED_CONTROL_KEYS.join(", ")
    )))
}

pub(super) fn parse_upload_args(
    args: &MultiValue,
) -> Result<(String, String, Vec<String>), mlua::Error> {
    if let Some(table) = single_table_arg(args) {
        let mut paths: Value = table.get("paths")?;
        if matches!(paths, Value::Nil) {
            paths = table.get("path")?;
        }
        return Ok((
            required_string(&table, "webbrowser.upload", &["browser"])?,
            required_string(&table, "webbrowser.upload", &["action"])?,
            string_or_array(&paths, "webbrowser.upload", "paths")?,
        ));
    }
    if args.len() != 3 {
        return Err(arg_error(
            "webbrowser.upload",
            WEBBROWSER_DOC.params("upload"),
        ));
    }
    Ok((
        value_string(&args[0], "webbrowser.upload", "browser")?,
        value_string(&args[1], "webbrowser.upload", "action")?,
        string_or_array(&args[2], "webbrowser.upload", "paths")?,
    ))
}

pub(super) fn parse_arm_upload_args(
    args: &MultiValue,
) -> Result<(String, Vec<String>), mlua::Error> {
    if let Some(table) = single_table_arg(args) {
        let mut paths: Value = table.get("paths")?;
        if matches!(paths, Value::Nil) {
            paths = table.get("path")?;
        }
        return Ok((
            required_string(&table, "webbrowser.arm_upload", &["browser"])?,
            string_or_array(&paths, "webbrowser.arm_upload", "paths")?,
        ));
    }
    if args.len() != 2 {
        return Err(arg_error(
            "webbrowser.arm_upload",
            WEBBROWSER_DOC.params("arm_upload"),
        ));
    }
    Ok((
        value_string(&args[0], "webbrowser.arm_upload", "browser")?,
        string_or_array(&args[1], "webbrowser.arm_upload", "paths")?,
    ))
}

pub(super) fn resolve_upload_paths(
    mounts: &MountTable,
    paths: &[String],
    function_name: &str,
) -> Result<Vec<serde_json::Value>, mlua::Error> {
    let mut host_paths = Vec::with_capacity(paths.len());
    for path in paths {
        let host_path = mounts.resolve_read(path).map_err(mlua::Error::external)?;
        if !host_path.is_file() {
            return Err(mlua::Error::external(format!(
                "{function_name}: path is not a file: {path}"
            )));
        }
        host_paths.push(serde_json::Value::String(
            host_path.to_string_lossy().to_string(),
        ));
    }
    Ok(host_paths)
}

fn string_or_array(value: &Value, fn_name: &str, name: &str) -> Result<Vec<String>, mlua::Error> {
    match value {
        Value::String(value) => Ok(vec![value.to_string_lossy().to_string()]),
        Value::Table(table) => string_array(table, fn_name, name),
        Value::Nil => Err(mlua::Error::external(format!(
            "{fn_name}: missing required argument '{name}' (string or array table)"
        ))),
        other => Err(mlua::Error::external(format!(
            "{fn_name}: argument '{name}' expected string or array table, got {}",
            other.type_name()
        ))),
    }
}
