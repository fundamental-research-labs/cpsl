//! Shared utility functions for Luau sandbox modules.
//!
//! Consolidates duplicated helpers (`is_lua_array`, `value_type_name`)
//! and the help registration boilerplate into a single module.

use crate::sandbox::{HelpMode, ModuleDoc};
use mlua::{Lua, Value};

/// Check if a table with raw_len > 0 is a proper Lua array (keys 1..n with no gaps).
pub(crate) fn is_lua_array(t: &mlua::Table, len: usize) -> bool {
    let mut count = 0;
    for pair in t.clone().pairs::<Value, Value>() {
        if pair.is_err() {
            return false;
        }
        count += 1;
        if count > len {
            return false; // more keys than array length → has non-array keys
        }
    }
    count == len
}

/// Return a human-readable type name for a Lua value.
pub(crate) fn value_type_name(v: &Value) -> &'static str {
    match v {
        Value::Nil => "nil",
        Value::Boolean(_) => "boolean",
        Value::Integer(_) | Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Table(_) => "table",
        Value::Function(_) => "function",
        _ => "userdata",
    }
}

/// Register `help(mode?)` on a module table.
///
/// The ModuleDoc is rendered in both Lua and Shell formats at registration
/// time, but stored as data strings (`__help_lua`, `__help_shell`).
/// The single `help()` function picks the right format based on its
/// optional argument: `help()` → Lua format, `help("shell")` → Shell format.
///
/// This means modules have ONE help function. The shell runtime passes
/// `"shell"` when dispatching `json help` etc., so users see the right format
/// without any shell_help() duplication.
pub(crate) fn register_help_functions(
    lua: &Lua,
    table: &mlua::Table,
    doc: &ModuleDoc,
) -> Result<(), mlua::Error> {
    // Pre-render both formats from the same ModuleDoc metadata.
    // Escape for embedding in Luau string literals.
    let lua_text = doc
        .format_help(HelpMode::Lua)
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n");
    let shell_text = doc
        .format_help(HelpMode::Shell)
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n");

    // help(mode?) — single function, picks format from mode arg.
    // Both text variants are captured as upvalues in the closure.
    table.set(
        "help",
        lua.load(format!(
            r#"local lua_t = "{lua_text}"
local sh_t = "{shell_text}"
return function(mode)
    if mode == "shell" then
        print(sh_t)
    else
        print(lua_t)
    end
end"#
        ))
        .eval::<mlua::Function>()?,
    )?;

    // __summary — one-line description used by the global help() to list modules dynamically.
    table.set("__summary", doc.summary)?;

    // __params — maps function names to ordered param name lists.
    // Used by sh.run() to unpack shell named args into positional args.
    let params_table = lua.create_table()?;
    for f in doc.functions {
        let param_list = lua.create_table()?;
        for (i, p) in f.params.iter().enumerate() {
            param_list.set(i + 1, p.name)?;
        }
        params_table.set(f.name, param_list)?;
    }
    table.set("__params", params_table)?;

    // __field_types — maps function names to the declared types of flattened
    // option fields. The shell runtime uses this to coerce option values without
    // guessing from their spelling (for example, `--validate false` must become
    // a boolean while a string option whose value is "false" must stay a string).
    let field_types_table = lua.create_table()?;
    for f in doc.functions {
        let method_field_types = lua.create_table()?;
        let mut has_fields = false;
        for p in f.params {
            if p.name == "opts" {
                let Some(fields) = p.fields else { continue };
                for field in fields {
                    method_field_types.set(field.name, field.typ)?;
                    has_fields = true;
                }
            }
        }
        if has_fields {
            field_types_table.set(f.name, method_field_types)?;
        }
    }
    table.set("__field_types", field_types_table)?;

    // __fn_help — maps function names to compact help strings (signature + example).
    // Used by wrap_module_with_help_hints to inline help into usage error messages,
    // eliminating the round-trip of "call module.help()".
    let fn_help_table = lua.create_table()?;
    for f in doc.functions {
        fn_help_table.set(f.name, f.format_error_help(doc.name))?;
    }
    table.set("__fn_help", fn_help_table)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sandbox::{FieldDoc, FnDoc, Param, ParamType, ReturnType};

    #[test]
    fn test_value_type_name() {
        assert_eq!(value_type_name(&Value::Nil), "nil");
        assert_eq!(value_type_name(&Value::Boolean(true)), "boolean");
        assert_eq!(value_type_name(&Value::Integer(42)), "number");
        assert_eq!(value_type_name(&Value::Number(3.14)), "number");
    }

    #[test]
    fn test_is_lua_array_with_lua() {
        let lua = Lua::new();
        // Pure array {1, 2, 3}
        let arr = lua.create_table().unwrap();
        arr.set(1, "a").unwrap();
        arr.set(2, "b").unwrap();
        arr.set(3, "c").unwrap();
        assert!(is_lua_array(&arr, 3));

        // Mixed table {1="a", "extra"="val"} — raw_len=1 but has non-integer key
        let mixed = lua.create_table().unwrap();
        mixed.set(1, "a").unwrap();
        mixed.set("extra", "val").unwrap();
        assert!(!is_lua_array(&mixed, 1));

        // Empty table
        let empty = lua.create_table().unwrap();
        assert!(is_lua_array(&empty, 0));
    }

    #[test]
    fn test_register_help_exposes_only_opts_field_types() {
        static DIRECT_FIELDS: &[FieldDoc] = &[FieldDoc {
            name: "direct_flag",
            typ: "boolean",
            required: false,
            description: "Not an opts field",
        }];
        static OPTS_FIELDS: &[FieldDoc] = &[FieldDoc {
            name: "validate",
            typ: "boolean",
            required: false,
            description: "Validate input",
        }];
        static PARAMS: &[Param] = &[
            Param {
                name: "payload",
                short: None,
                typ: ParamType::Table,
                required: true,
                fields: Some(DIRECT_FIELDS),
            },
            Param {
                name: "opts",
                short: None,
                typ: ParamType::Table,
                required: false,
                fields: Some(OPTS_FIELDS),
            },
        ];
        static FUNCTIONS: &[FnDoc] = &[FnDoc {
            name: "decode",
            description: "Decode input.",
            params: PARAMS,
            returns: ReturnType::Table,
            example: None,
        }];
        let doc = ModuleDoc {
            name: "test",
            summary: "test module",
            functions: FUNCTIONS,
        };
        let lua = Lua::new();
        let module = lua.create_table().unwrap();

        register_help_functions(&lua, &module, &doc).unwrap();

        let all_types: mlua::Table = module.get("__field_types").unwrap();
        let decode_types: mlua::Table = all_types.get("decode").unwrap();
        assert_eq!(decode_types.get::<String>("validate").unwrap(), "boolean");
        assert!(matches!(
            decode_types.get::<Value>("direct_flag").unwrap(),
            Value::Nil
        ));
    }
}
