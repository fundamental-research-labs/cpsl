//! Helpers for interoperating with py.list / py.dict / py.tuple wrappers
//! produced by the Python runtime (pyrt.luau).
//!
//! The transpiled Python code wraps lists, dicts, and tuples in structured
//! tables: `{ __py_type = "list"|"tuple"|"dict", data = {...}, ... }`.
//! Rust-side modules that receive these tables need to unwrap them to access
//! the underlying data.

use mlua::Value;

/// If the table is a py.list or py.tuple wrapper, return its `.data` sub-table;
/// otherwise return the table as-is.
pub(crate) fn unwrap_py_seq(t: &mlua::Table) -> Result<mlua::Table, mlua::Error> {
    if let Ok(Value::String(s)) = t.get::<Value>("__py_type") {
        let ty = s.to_string_lossy();
        if ty == "list" || ty == "tuple" {
            return t.get::<mlua::Table>("data").map_err(|_| {
                mlua::Error::external(format!("py.{} wrapper missing .data field", ty))
            });
        }
    }
    Ok(t.clone())
}

/// If the table is a py.dict wrapper, return its `.data` sub-table;
/// otherwise return the table as-is.
pub(crate) fn unwrap_py_dict(t: &mlua::Table) -> Result<mlua::Table, mlua::Error> {
    if let Ok(Value::String(s)) = t.get::<Value>("__py_type") {
        if s.to_string_lossy() == "dict" {
            return t
                .get::<mlua::Table>("data")
                .map_err(|_| mlua::Error::external("py.dict wrapper missing .data field"));
        }
    }
    Ok(t.clone())
}

/// Returns the `__py_type` string if this table is a py wrapper, or None.
pub(crate) fn py_type(t: &mlua::Table) -> Option<String> {
    if let Ok(Value::String(s)) = t.get::<Value>("__py_type") {
        Some(s.to_string_lossy().to_string())
    } else {
        None
    }
}
