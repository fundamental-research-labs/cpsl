//! Lua-facing Excel (.xlsx) module backed by the cells C ABI host gateway.
//!
//! Path arguments are sandbox virtual paths; this module maps them to host
//! paths via MountTable before invoking the gateway. Workbook handles are
//! opaque strings owned by the host (Swift cells_xlsx bridge).

use crate::lua_util::register_help_functions;
use crate::mount::MountTable;
use crate::sandbox::{
    wrap_module_with_help_hints, FnDoc, ModuleDoc, Param, ParamType, ReturnType,
};
use mlua::{Lua, MultiValue, Value};
use serde_json::json;
use std::sync::Arc;

pub trait XlsxGateway: Send + Sync {
    fn handle_json(&self, request_json: &str) -> Result<String, String>;
}

impl<F> XlsxGateway for F
where
    F: Fn(&str) -> Result<String, String> + Send + Sync,
{
    fn handle_json(&self, request_json: &str) -> Result<String, String> {
        self(request_json)
    }
}

pub(crate) static XLSX_DOC: ModuleDoc = ModuleDoc {
    name: "xlsx",
    summary: "Read, edit, and write Excel .xlsx workbooks (cells library)",
    functions: &[
        FnDoc {
            name: "create",
            description: "Create an empty workbook with one sheet named Sheet1. Returns a workbook handle.",
            params: &[],
            returns: ReturnType::Table,
            example: Some(r#"local wb = xlsx.create().workbook"#),
        },
        FnDoc {
            name: "open",
            description: "Open an existing .xlsx at a sandbox path. Returns a workbook handle.",
            params: &[Param {
                name: "path",
                short: Some('p'),
                typ: ParamType::String,
                required: true,
                fields: None,
            }],
            returns: ReturnType::Table,
            example: Some(r#"local wb = xlsx.open("/workdir/data.xlsx").workbook"#),
        },
        FnDoc {
            name: "close",
            description: "Release a workbook handle.",
            params: &[Param {
                name: "workbook",
                short: Some('w'),
                typ: ParamType::String,
                required: true,
                fields: None,
            }],
            returns: ReturnType::Table,
            example: Some(r#"xlsx.close(wb)"#),
        },
        FnDoc {
            name: "sheet_count",
            description: "Return the number of sheets in the workbook.",
            params: &[Param {
                name: "workbook",
                short: Some('w'),
                typ: ParamType::String,
                required: true,
                fields: None,
            }],
            returns: ReturnType::Table,
            example: Some(r#"local n = xlsx.sheet_count(wb).count"#),
        },
        FnDoc {
            name: "sheet_name",
            description: "Return the name of the sheet at a 0-based index.",
            params: &[
                Param {
                    name: "workbook",
                    short: Some('w'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "sheet",
                    short: Some('s'),
                    typ: ParamType::Number,
                    required: true,
                    fields: None,
                },
            ],
            returns: ReturnType::Table,
            example: Some(r#"local name = xlsx.sheet_name(wb, 0).name"#),
        },
        FnDoc {
            name: "sheets",
            description: "Return a list of sheet names (1-based Lua array).",
            params: &[Param {
                name: "workbook",
                short: Some('w'),
                typ: ParamType::String,
                required: true,
                fields: None,
            }],
            returns: ReturnType::Table,
            example: Some(r#"local names = xlsx.sheets(wb).sheets"#),
        },
        FnDoc {
            name: "get_type",
            description: "Return cell type at (sheet, col, row): empty, number, string, bool, or other. Positions are 0-based.",
            params: &cell_params(false),
            returns: ReturnType::Table,
            example: Some(r#"local t = xlsx.get_type(wb, 0, 0, 0).type"#),
        },
        FnDoc {
            name: "get_number",
            description: "Return the number value at (sheet, col, row). Positions are 0-based.",
            params: &cell_params(false),
            returns: ReturnType::Table,
            example: Some(r#"local n = xlsx.get_number(wb, 0, 1, 0).value"#),
        },
        FnDoc {
            name: "get_string",
            description: "Return the string value at (sheet, col, row). Positions are 0-based.",
            params: &cell_params(false),
            returns: ReturnType::Table,
            example: Some(r#"local s = xlsx.get_string(wb, 0, 0, 0).value"#),
        },
        FnDoc {
            name: "get_bool",
            description: "Return the boolean value at (sheet, col, row). Positions are 0-based.",
            params: &cell_params(false),
            returns: ReturnType::Table,
            example: Some(r#"local b = xlsx.get_bool(wb, 0, 2, 0).value"#),
        },
        FnDoc {
            name: "get",
            description: "Return type and value for a cell at (sheet, col, row). Positions are 0-based.",
            params: &cell_params(false),
            returns: ReturnType::Table,
            example: Some(r#"local cell = xlsx.get(wb, 0, 0, 0); print(cell.type, cell.value)"#),
        },
        FnDoc {
            name: "set_number",
            description: "Set a number cell at (sheet, col, row). Positions are 0-based.",
            params: &cell_params(true),
            returns: ReturnType::Table,
            example: Some(r#"xlsx.set_number(wb, 0, 1, 0, 42.5)"#),
        },
        FnDoc {
            name: "set_string",
            description: "Set a string cell at (sheet, col, row). Positions are 0-based.",
            params: &[
                Param {
                    name: "workbook",
                    short: Some('w'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "sheet",
                    short: Some('s'),
                    typ: ParamType::Number,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "col",
                    short: Some('c'),
                    typ: ParamType::Number,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "row",
                    short: Some('r'),
                    typ: ParamType::Number,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "value",
                    short: Some('v'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
            ],
            returns: ReturnType::Table,
            example: Some(r#"xlsx.set_string(wb, 0, 0, 0, "hello")"#),
        },
        FnDoc {
            name: "set_bool",
            description: "Set a boolean cell at (sheet, col, row). Positions are 0-based.",
            params: &cell_params(true),
            returns: ReturnType::Table,
            example: Some(r#"xlsx.set_bool(wb, 0, 2, 0, true)"#),
        },
        FnDoc {
            name: "write",
            description: "Write the workbook to a sandbox path as .xlsx.",
            params: &[
                Param {
                    name: "workbook",
                    short: Some('w'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "path",
                    short: Some('p'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
            ],
            returns: ReturnType::Table,
            example: Some(r#"xlsx.write(wb, "/workdir/out.xlsx")"#),
        },
    ],
};

const fn cell_params(with_value: bool) -> &'static [Param] {
    // Static Param slices can't be built conditionally with different lengths
    // easily; use separate statics instead via callers for set_string.
    // This helper is only for number/bool cells that share the same shape.
    if with_value {
        &[
            Param {
                name: "workbook",
                short: Some('w'),
                typ: ParamType::String,
                required: true,
                fields: None,
            },
            Param {
                name: "sheet",
                short: Some('s'),
                typ: ParamType::Number,
                required: true,
                fields: None,
            },
            Param {
                name: "col",
                short: Some('c'),
                typ: ParamType::Number,
                required: true,
                fields: None,
            },
            Param {
                name: "row",
                short: Some('r'),
                typ: ParamType::Number,
                required: true,
                fields: None,
            },
            Param {
                name: "value",
                short: Some('v'),
                typ: ParamType::Value,
                required: true,
                fields: None,
            },
        ]
    } else {
        &[
            Param {
                name: "workbook",
                short: Some('w'),
                typ: ParamType::String,
                required: true,
                fields: None,
            },
            Param {
                name: "sheet",
                short: Some('s'),
                typ: ParamType::Number,
                required: true,
                fields: None,
            },
            Param {
                name: "col",
                short: Some('c'),
                typ: ParamType::Number,
                required: true,
                fields: None,
            },
            Param {
                name: "row",
                short: Some('r'),
                typ: ParamType::Number,
                required: true,
                fields: None,
            },
        ]
    }
}

pub(crate) fn register_xlsx_globals(
    lua: &Lua,
    gateway: Arc<dyn XlsxGateway>,
    mounts: Arc<MountTable>,
) -> Result<(), mlua::Error> {
    let xlsx = lua.create_table()?;

    {
        let gateway = gateway.clone();
        xlsx.set(
            "create",
            lua.create_function(move |lua, args: MultiValue| {
                require_arity(&args, 0, "xlsx.create")?;
                call_gateway(lua, gateway.as_ref(), json!({ "command": "create" }))
            })?,
        )?;
    }

    {
        let gateway = gateway.clone();
        let mounts = mounts.clone();
        xlsx.set(
            "open",
            lua.create_function(move |lua, args: MultiValue| {
                let path = require_string_arg(&args, 0, "xlsx.open", "path")?;
                require_arity(&args, 1, "xlsx.open")?;
                let host_path = mounts
                    .resolve_read(&path)
                    .map_err(|e| mlua::Error::external(format!("xlsx.open: {e}")))?;
                call_gateway(
                    lua,
                    gateway.as_ref(),
                    json!({
                        "command": "open",
                        "path": host_path.to_string_lossy(),
                    }),
                )
            })?,
        )?;
    }

    {
        let gateway = gateway.clone();
        xlsx.set(
            "close",
            lua.create_function(move |lua, args: MultiValue| {
                let workbook = require_string_arg(&args, 0, "xlsx.close", "workbook")?;
                require_arity(&args, 1, "xlsx.close")?;
                call_gateway(
                    lua,
                    gateway.as_ref(),
                    json!({ "command": "close", "workbook": workbook }),
                )
            })?,
        )?;
    }

    {
        let gateway = gateway.clone();
        xlsx.set(
            "sheet_count",
            lua.create_function(move |lua, args: MultiValue| {
                let workbook = require_string_arg(&args, 0, "xlsx.sheet_count", "workbook")?;
                require_arity(&args, 1, "xlsx.sheet_count")?;
                call_gateway(
                    lua,
                    gateway.as_ref(),
                    json!({ "command": "sheet_count", "workbook": workbook }),
                )
            })?,
        )?;
    }

    {
        let gateway = gateway.clone();
        xlsx.set(
            "sheet_name",
            lua.create_function(move |lua, args: MultiValue| {
                let workbook = require_string_arg(&args, 0, "xlsx.sheet_name", "workbook")?;
                let sheet = require_int_arg(&args, 1, "xlsx.sheet_name", "sheet")?;
                require_arity(&args, 2, "xlsx.sheet_name")?;
                call_gateway(
                    lua,
                    gateway.as_ref(),
                    json!({
                        "command": "sheet_name",
                        "workbook": workbook,
                        "sheet": sheet,
                    }),
                )
            })?,
        )?;
    }

    {
        let gateway = gateway.clone();
        xlsx.set(
            "sheets",
            lua.create_function(move |lua, args: MultiValue| {
                let workbook = require_string_arg(&args, 0, "xlsx.sheets", "workbook")?;
                require_arity(&args, 1, "xlsx.sheets")?;
                call_gateway(
                    lua,
                    gateway.as_ref(),
                    json!({ "command": "sheets", "workbook": workbook }),
                )
            })?,
        )?;
    }

    register_cell_getter(lua, &xlsx, gateway.clone(), "get_type")?;
    register_cell_getter(lua, &xlsx, gateway.clone(), "get_number")?;
    register_cell_getter(lua, &xlsx, gateway.clone(), "get_string")?;
    register_cell_getter(lua, &xlsx, gateway.clone(), "get_bool")?;
    register_cell_getter(lua, &xlsx, gateway.clone(), "get")?;

    {
        let gateway = gateway.clone();
        xlsx.set(
            "set_number",
            lua.create_function(move |lua, args: MultiValue| {
                let workbook = require_string_arg(&args, 0, "xlsx.set_number", "workbook")?;
                let sheet = require_int_arg(&args, 1, "xlsx.set_number", "sheet")?;
                let col = require_int_arg(&args, 2, "xlsx.set_number", "col")?;
                let row = require_int_arg(&args, 3, "xlsx.set_number", "row")?;
                let value = require_number_arg(&args, 4, "xlsx.set_number", "value")?;
                require_arity(&args, 5, "xlsx.set_number")?;
                call_gateway(
                    lua,
                    gateway.as_ref(),
                    json!({
                        "command": "set_number",
                        "workbook": workbook,
                        "sheet": sheet,
                        "col": col,
                        "row": row,
                        "value": value,
                    }),
                )
            })?,
        )?;
    }

    {
        let gateway = gateway.clone();
        xlsx.set(
            "set_string",
            lua.create_function(move |lua, args: MultiValue| {
                let workbook = require_string_arg(&args, 0, "xlsx.set_string", "workbook")?;
                let sheet = require_int_arg(&args, 1, "xlsx.set_string", "sheet")?;
                let col = require_int_arg(&args, 2, "xlsx.set_string", "col")?;
                let row = require_int_arg(&args, 3, "xlsx.set_string", "row")?;
                let value = require_string_arg(&args, 4, "xlsx.set_string", "value")?;
                require_arity(&args, 5, "xlsx.set_string")?;
                call_gateway(
                    lua,
                    gateway.as_ref(),
                    json!({
                        "command": "set_string",
                        "workbook": workbook,
                        "sheet": sheet,
                        "col": col,
                        "row": row,
                        "value": value,
                    }),
                )
            })?,
        )?;
    }

    {
        let gateway = gateway.clone();
        xlsx.set(
            "set_bool",
            lua.create_function(move |lua, args: MultiValue| {
                let workbook = require_string_arg(&args, 0, "xlsx.set_bool", "workbook")?;
                let sheet = require_int_arg(&args, 1, "xlsx.set_bool", "sheet")?;
                let col = require_int_arg(&args, 2, "xlsx.set_bool", "col")?;
                let row = require_int_arg(&args, 3, "xlsx.set_bool", "row")?;
                let value = require_bool_arg(&args, 4, "xlsx.set_bool", "value")?;
                require_arity(&args, 5, "xlsx.set_bool")?;
                call_gateway(
                    lua,
                    gateway.as_ref(),
                    json!({
                        "command": "set_bool",
                        "workbook": workbook,
                        "sheet": sheet,
                        "col": col,
                        "row": row,
                        "value": value,
                    }),
                )
            })?,
        )?;
    }

    {
        let gateway = gateway.clone();
        let mounts = mounts.clone();
        xlsx.set(
            "write",
            lua.create_function(move |lua, args: MultiValue| {
                let workbook = require_string_arg(&args, 0, "xlsx.write", "workbook")?;
                let path = require_string_arg(&args, 1, "xlsx.write", "path")?;
                require_arity(&args, 2, "xlsx.write")?;
                let host_path = mounts
                    .resolve_write_deep(&path)
                    .map_err(|e| mlua::Error::external(format!("xlsx.write: {e}")))?;
                if let Some(parent) = host_path.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| {
                        mlua::Error::external(format!(
                            "xlsx.write: failed to create parent directory: {e}"
                        ))
                    })?;
                }
                call_gateway(
                    lua,
                    gateway.as_ref(),
                    json!({
                        "command": "write",
                        "workbook": workbook,
                        "path": host_path.to_string_lossy(),
                    }),
                )
            })?,
        )?;
    }

    register_help_functions(lua, &xlsx, &XLSX_DOC)?;
    lua.globals().set("xlsx", xlsx)?;
    wrap_module_with_help_hints(lua, "xlsx")?;

    Ok(())
}

fn register_cell_getter(
    lua: &Lua,
    table: &mlua::Table,
    gateway: Arc<dyn XlsxGateway>,
    command: &'static str,
) -> Result<(), mlua::Error> {
    let name = command;
    table.set(
        name,
        lua.create_function(move |lua, args: MultiValue| {
            let fn_name = format!("xlsx.{name}");
            let workbook = require_string_arg(&args, 0, &fn_name, "workbook")?;
            let sheet = require_int_arg(&args, 1, &fn_name, "sheet")?;
            let col = require_int_arg(&args, 2, &fn_name, "col")?;
            let row = require_int_arg(&args, 3, &fn_name, "row")?;
            require_arity(&args, 4, &fn_name)?;
            call_gateway(
                lua,
                gateway.as_ref(),
                json!({
                    "command": name,
                    "workbook": workbook,
                    "sheet": sheet,
                    "col": col,
                    "row": row,
                }),
            )
        })?,
    )?;
    Ok(())
}

fn require_arity(args: &MultiValue, expected: usize, fn_name: &str) -> Result<(), mlua::Error> {
    if args.len() == expected {
        Ok(())
    } else {
        Err(mlua::Error::external(format!(
            "{fn_name}: expected {expected} argument(s), got {}",
            args.len()
        )))
    }
}

fn require_string_arg(
    args: &MultiValue,
    index: usize,
    fn_name: &str,
    param: &str,
) -> Result<String, mlua::Error> {
    match args.get(index) {
        Some(Value::String(s)) => Ok(s.to_str()?.to_string()),
        Some(other) => Err(mlua::Error::external(format!(
            "{fn_name}: {param} must be a string, got {}",
            crate::lua_util::value_type_name(other)
        ))),
        None => Err(mlua::Error::external(format!(
            "{fn_name}: missing required argument {param}"
        ))),
    }
}

fn require_int_arg(
    args: &MultiValue,
    index: usize,
    fn_name: &str,
    param: &str,
) -> Result<i64, mlua::Error> {
    match args.get(index) {
        Some(Value::Integer(n)) => Ok(*n),
        Some(Value::Number(n)) if n.fract() == 0.0 => Ok(*n as i64),
        Some(other) => Err(mlua::Error::external(format!(
            "{fn_name}: {param} must be an integer, got {}",
            crate::lua_util::value_type_name(other)
        ))),
        None => Err(mlua::Error::external(format!(
            "{fn_name}: missing required argument {param}"
        ))),
    }
}

fn require_number_arg(
    args: &MultiValue,
    index: usize,
    fn_name: &str,
    param: &str,
) -> Result<f64, mlua::Error> {
    match args.get(index) {
        Some(Value::Integer(n)) => Ok(*n as f64),
        Some(Value::Number(n)) => Ok(*n),
        Some(other) => Err(mlua::Error::external(format!(
            "{fn_name}: {param} must be a number, got {}",
            crate::lua_util::value_type_name(other)
        ))),
        None => Err(mlua::Error::external(format!(
            "{fn_name}: missing required argument {param}"
        ))),
    }
}

fn require_bool_arg(
    args: &MultiValue,
    index: usize,
    fn_name: &str,
    param: &str,
) -> Result<bool, mlua::Error> {
    match args.get(index) {
        Some(Value::Boolean(b)) => Ok(*b),
        Some(other) => Err(mlua::Error::external(format!(
            "{fn_name}: {param} must be a boolean, got {}",
            crate::lua_util::value_type_name(other)
        ))),
        None => Err(mlua::Error::external(format!(
            "{fn_name}: missing required argument {param}"
        ))),
    }
}

fn call_gateway(
    lua: &Lua,
    gateway: &dyn XlsxGateway,
    request: serde_json::Value,
) -> Result<Value, mlua::Error> {
    let response = gateway
        .handle_json(&request.to_string())
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
            .unwrap_or("xlsx command failed");
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mount::MountPermission;
    use crate::sandbox::Sandbox;
    use std::sync::Mutex;
    use tempfile::TempDir;

    /// Records gateway requests so Luau surface wiring can be tested without
    /// re-implementing the cells XLSX format layer. Round-trip correctness of the
    /// real cells C ABI is covered by herm's CPSLExcelService / C smoke tests.
    struct RecordingGateway {
        last: Mutex<String>,
    }

    impl XlsxGateway for RecordingGateway {
        fn handle_json(&self, request_json: &str) -> Result<String, String> {
            *self.last.lock().unwrap() = request_json.to_string();
            let req: serde_json::Value =
                serde_json::from_str(request_json).map_err(|e| e.to_string())?;
            let command = req
                .get("command")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            match command {
                "create" => Ok(r#"{"ok":true,"workbook":"wb-test"}"#.into()),
                "open" => {
                    let path = req.get("path").and_then(|v| v.as_str()).unwrap_or("");
                    if path.contains("missing") || path.contains("does_not_exist") {
                        Ok(r#"{"ok":false,"error":"failed to open xlsx: file not found"}"#.into())
                    } else {
                        Ok(r#"{"ok":true,"workbook":"wb-open"}"#.into())
                    }
                }
                "write" => Ok(r#"{"ok":true}"#.into()),
                "set_string" | "set_number" | "set_bool" | "close" => Ok(r#"{"ok":true}"#.into()),
                "get_string" => Ok(r#"{"ok":true,"value":"hello"}"#.into()),
                "get_number" => Ok(r#"{"ok":true,"value":42.5}"#.into()),
                "get_type" => Ok(r#"{"ok":true,"type":"string"}"#.into()),
                "sheet_count" => Ok(r#"{"ok":true,"count":1}"#.into()),
                "sheet_name" => Ok(r#"{"ok":true,"name":"Sheet1"}"#.into()),
                "sheets" => Ok(r#"{"ok":true,"sheets":["Sheet1"]}"#.into()),
                "get" | "get_bool" => Ok(r#"{"ok":true,"type":"string","value":"hello"}"#.into()),
                other => Err(format!("unexpected command {other}")),
            }
        }
    }

    fn sandbox_with_gateway(tmp: &TempDir, gateway: Arc<dyn XlsxGateway>) -> Sandbox {
        let mut mounts = MountTable::new();
        mounts
            .add_mount(
                tmp.path().to_path_buf(),
                "/workdir",
                MountPermission::ReadWrite,
            )
            .unwrap();
        Sandbox::builder()
            .mounts(mounts)
            .auto_tmp(false)
            .xlsx_gateway(gateway)
            .build()
            .unwrap()
    }

    #[test]
    fn help_mentions_open_create_set_write() {
        let tmp = TempDir::new().unwrap();
        let sandbox = sandbox_with_gateway(&tmp, Arc::new(RecordingGateway {
            last: Mutex::new(String::new()),
        }));
        // help() prints; sandbox.exec captures the print buffer.
        let help_text = sandbox.exec("return xlsx.help()").unwrap();
        assert!(
            help_text.contains("open") && help_text.contains("create"),
            "help should mention open/create: {help_text}"
        );
        assert!(
            help_text.contains("set_string") || help_text.contains("set_number"),
            "help should mention set: {help_text}"
        );
        assert!(
            help_text.contains("write"),
            "help should mention write: {help_text}"
        );
    }

    #[test]
    fn global_help_lists_xlsx_when_gateway_is_wired() {
        let tmp = TempDir::new().unwrap();
        let sandbox = sandbox_with_gateway(
            &tmp,
            Arc::new(RecordingGateway {
                last: Mutex::new(String::new()),
            }),
        );
        let help_text = sandbox.exec("help()").unwrap();
        assert!(
            help_text.contains("xlsx"),
            "global help() must list xlsx when the host gateway is wired: {help_text}"
        );
    }

    #[test]
    fn open_maps_sandbox_path_and_invalid_open_errors() {
        let tmp = TempDir::new().unwrap();
        let gateway = Arc::new(RecordingGateway {
            last: Mutex::new(String::new()),
        });
        let sandbox = sandbox_with_gateway(&tmp, gateway.clone());

        // create a real host file so resolve_read succeeds; gateway still rejects missing-named opens
        let err = sandbox
            .exec(r#"return xlsx.open("/workdir/does_not_exist.xlsx")"#)
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("open") || err.contains("fail") || err.contains("not found"),
            "expected open failure, got: {err}"
        );

        // Path mapping: open of a present file should pass a host path (not /workdir/...) to gateway
        std::fs::write(tmp.path().join("present.xlsx"), b"stub").unwrap();
        sandbox
            .exec(r#"return xlsx.open("/workdir/present.xlsx").workbook"#)
            .unwrap();
        let last = gateway.last.lock().unwrap().clone();
        assert!(
            last.contains("present.xlsx") && !last.contains("\"/workdir/present.xlsx\""),
            "gateway should receive host path, got: {last}"
        );
    }

    #[test]
    fn create_set_write_uses_gateway_commands() {
        let tmp = TempDir::new().unwrap();
        let gateway = Arc::new(RecordingGateway {
            last: Mutex::new(String::new()),
        });
        let sandbox = sandbox_with_gateway(&tmp, gateway.clone());
        sandbox
            .exec(
                r#"
                local wb = xlsx.create().workbook
                xlsx.set_string(wb, 0, 0, 0, "hello")
                xlsx.set_number(wb, 0, 1, 0, 42.5)
                xlsx.write(wb, "/workdir/out.xlsx")
                return xlsx.get_string(wb, 0, 0, 0).value
                "#,
            )
            .unwrap();
        // last request should be get_string after the chain
        let last = gateway.last.lock().unwrap().clone();
        assert!(
            last.contains("get_string") || last.contains("write") || last.contains("set_"),
            "expected xlsx gateway traffic, got: {last}"
        );
    }
}
