//! Email validation and parsing module for the Luau sandbox.
//!
//! Exposes `email.isValid`, `email.parse`, `email.normalize`
//! as globals. Uses the `email_address` crate (RFC 5321 compliant, pure Rust).

use crate::lua_util::register_help_functions;
use crate::sandbox::{
    validate_args, wrap_module_with_help_hints, FnDoc, ModuleDoc, Param, ParamType, ReturnType,
};
use email_address::EmailAddress;
use mlua::{Lua, MultiValue, Value};

pub(crate) static EMAIL_DOC: ModuleDoc = ModuleDoc {
    name: "email",
    summary: "Email validation & parsing (RFC 5321)",
    functions: &[
        FnDoc {
            name: "isValid",
            description: "Check if an email address string is valid per RFC 5321.",
            params: &[Param {
                name: "address",
                short: Some('a'),
                typ: ParamType::String,
                required: true,
                fields: None,
            }],
            returns: ReturnType::Boolean,
            example: Some(r#"email.isValid("user@example.com") -- true"#),
        },
        FnDoc {
            name: "parse",
            description: "Parse an email address. Returns {local, domain, full} or nil if invalid.",
            params: &[Param {
                name: "address",
                short: Some('a'),
                typ: ParamType::String,
                required: true,
                fields: None,
            }],
            returns: ReturnType::Table,
            example: None,
        },
        FnDoc {
            name: "normalize",
            description:
                "Normalize an email address: trim whitespace and lowercase the domain part.",
            params: &[Param {
                name: "address",
                short: Some('a'),
                typ: ParamType::String,
                required: true,
                fields: None,
            }],
            returns: ReturnType::String,
            example: None,
        },
    ],
};

/// Register `email.*` globals in the Lua VM.
pub fn register_email_globals(lua: &Lua) -> Result<(), mlua::Error> {
    let email_table = lua.create_table()?;

    // email.isValid(address) -> boolean
    email_table.set(
        "isValid",
        lua.create_function(|_, args: MultiValue| {
            let validated = validate_args(&args, EMAIL_DOC.params("isValid"), "email.isValid")?;
            let address = match &validated[0] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };
            Ok(EmailAddress::is_valid(&address))
        })?,
    )?;

    // email.parse(address) -> {local, domain, full} or nil
    email_table.set(
        "parse",
        lua.create_function(|lua, args: MultiValue| {
            let validated = validate_args(&args, EMAIL_DOC.params("parse"), "email.parse")?;
            let address = match &validated[0] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };

            match address.parse::<EmailAddress>() {
                Ok(parsed) => {
                    let result = lua.create_table()?;
                    result.set("local", parsed.local_part().to_string())?;
                    result.set("domain", parsed.domain().to_string())?;
                    result.set("full", parsed.to_string())?;
                    Ok(Value::Table(result))
                }
                Err(_) => Ok(Value::Nil),
            }
        })?,
    )?;

    // email.normalize(address) -> string (trim whitespace, lowercase domain)
    email_table.set(
        "normalize",
        lua.create_function(|_, args: MultiValue| {
            let validated = validate_args(&args, EMAIL_DOC.params("normalize"), "email.normalize")?;
            let address = match &validated[0] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };

            let trimmed = address.trim();
            match trimmed.parse::<EmailAddress>() {
                Ok(parsed) => {
                    let local = parsed.local_part();
                    let domain = parsed.domain().to_lowercase();
                    Ok(format!("{}@{}", local, domain))
                }
                Err(e) => Err(mlua::Error::external(format!(
                    "email.normalize: invalid email '{}': {}",
                    trimmed, e
                ))),
            }
        })?,
    )?;

    register_help_functions(lua, &email_table, &EMAIL_DOC)?;

    lua.globals().set("email", email_table)?;
    wrap_module_with_help_hints(lua, "email")?;

    Ok(())
}
