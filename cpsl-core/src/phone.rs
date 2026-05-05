//! Phone number parsing, validation, and formatting module for the Luau sandbox.
//!
//! Exposes `phone.parse`, `phone.format`, `phone.isValid`, `phone.type`, `phone.region`
//! as globals. Uses the `phonenumber` crate (Rust binding of Google's libphonenumber).

use crate::lua_util::register_help_functions;
use crate::sandbox::{
    validate_args, wrap_module_with_help_hints, FnDoc, ModuleDoc, Param, ParamType, ReturnType,
};
use mlua::{Lua, MultiValue, Value};

pub(crate) static PHONE_DOC: ModuleDoc = ModuleDoc {
    name: "phone",
    summary: "Phone number parsing, validation & formatting (libphonenumber)",
    functions: &[
        FnDoc {
            name: "parse",
            description:
                "Parse a phone number string. Returns {country_code, national_number, raw, valid}.",
            params: &[
                Param {
                    name: "number",
                    short: Some('n'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "region",
                    short: Some('r'),
                    typ: ParamType::String,
                    required: false,
                    fields: None,
                },
            ],
            returns: ReturnType::Table,
            example: Some(r#"phone.parse("+14155552671")"#),
        },
        FnDoc {
            name: "format",
            description: "Format a phone number. Styles: \"international\", \"national\", \"e164\", \"rfc3966\".",
            params: &[
                Param {
                    name: "number",
                    short: Some('n'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "style",
                    short: Some('s'),
                    typ: ParamType::String,
                    required: false,
                    fields: None,
                },
                Param {
                    name: "region",
                    short: Some('r'),
                    typ: ParamType::String,
                    required: false,
                    fields: None,
                },
            ],
            returns: ReturnType::String,
            example: Some(r#"phone.format({number="+14155552671", style="international"})"#),
        },
        FnDoc {
            name: "isValid",
            description: "Check if a phone number string is valid.",
            params: &[
                Param {
                    name: "number",
                    short: Some('n'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "region",
                    short: Some('r'),
                    typ: ParamType::String,
                    required: false,
                    fields: None,
                },
            ],
            returns: ReturnType::Boolean,
            example: None,
        },
        FnDoc {
            name: "type",
            description:
                "Get the type of a phone number: \"mobile\", \"fixed_line\", \"voip\", etc.",
            params: &[
                Param {
                    name: "number",
                    short: Some('n'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "region",
                    short: Some('r'),
                    typ: ParamType::String,
                    required: false,
                    fields: None,
                },
            ],
            returns: ReturnType::String,
            example: None,
        },
        FnDoc {
            name: "region",
            description: "Get the ISO country code (e.g. \"US\") for a phone number.",
            params: &[Param {
                name: "number",
                short: Some('n'),
                typ: ParamType::String,
                required: true,
                    fields: None,
            }],
            returns: ReturnType::String,
            example: None,
        },
    ],
};

/// Parse a phone number string with an optional region default.
fn parse_phone(
    number: &str,
    region: Option<&str>,
) -> Result<phonenumber::PhoneNumber, mlua::Error> {
    let country = region
        .map(|r| {
            r.to_uppercase()
                .parse::<phonenumber::country::Id>()
                .map_err(|e| {
                    mlua::Error::external(format!("phone: invalid region '{}': {}", r, e))
                })
        })
        .transpose()?;

    phonenumber::parse(country, number)
        .map_err(|e| mlua::Error::external(format!("phone: failed to parse '{}': {}", number, e)))
}

/// Convert phonenumber::Type to a human-readable string.
fn type_to_string(t: phonenumber::Type) -> &'static str {
    use phonenumber::Type;
    match t {
        Type::FixedLine => "fixed_line",
        Type::Mobile => "mobile",
        Type::FixedLineOrMobile => "fixed_line_or_mobile",
        Type::TollFree => "toll_free",
        Type::PremiumRate => "premium_rate",
        Type::SharedCost => "shared_cost",
        Type::PersonalNumber => "personal_number",
        Type::Voip => "voip",
        Type::Pager => "pager",
        Type::Uan => "uan",
        Type::Emergency => "emergency",
        Type::Voicemail => "voicemail",
        Type::ShortCode => "short_code",
        Type::StandardRate => "standard_rate",
        Type::Carrier => "carrier",
        Type::NoInternational => "no_international",
        Type::Unknown => "unknown",
    }
}

/// Register `phone.*` globals in the Lua VM.
pub fn register_phone_globals(lua: &Lua) -> Result<(), mlua::Error> {
    let phone_table = lua.create_table()?;

    // phone.parse(number, region?) -> {country_code, national_number, raw, valid}
    phone_table.set(
        "parse",
        lua.create_function(|lua, args: MultiValue| {
            let validated = validate_args(&args, PHONE_DOC.params("parse"), "phone.parse")?;
            let number = match &validated[0] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };
            let region = match &validated[1] {
                Value::String(s) => Some(s.to_string_lossy().to_string()),
                _ => None,
            };

            let parsed = parse_phone(&number, region.as_deref())?;
            let result = lua.create_table()?;
            result.set(
                "country_code",
                parsed.code().value() as i32,
            )?;
            result.set(
                "national_number",
                parsed.national().value().to_string(),
            )?;
            result.set("raw", number)?;
            result.set("valid", parsed.is_valid())?;
            Ok(Value::Table(result))
        })?,
    )?;

    // phone.format(number, style?, region?) -> string
    phone_table.set(
        "format",
        lua.create_function(|_, args: MultiValue| {
            let validated = validate_args(&args, PHONE_DOC.params("format"), "phone.format")?;
            let number = match &validated[0] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };
            let style = match &validated[1] {
                Value::String(s) => Some(s.to_string_lossy().to_string()),
                _ => None,
            };
            let region = match &validated[2] {
                Value::String(s) => Some(s.to_string_lossy().to_string()),
                _ => None,
            };

            let parsed = parse_phone(&number, region.as_deref())?;

            let mode = match style.as_deref() {
                Some("e164") | Some("E164") => phonenumber::Mode::E164,
                Some("international") | Some("International") | None => {
                    phonenumber::Mode::International
                }
                Some("national") | Some("National") => phonenumber::Mode::National,
                Some("rfc3966") | Some("RFC3966") => phonenumber::Mode::Rfc3966,
                Some(other) => {
                    return Err(mlua::Error::external(format!(
                        "phone.format: unknown style '{}'. Valid styles: international, national, e164, rfc3966",
                        other
                    )));
                }
            };

            Ok(parsed.format().mode(mode).to_string())
        })?,
    )?;

    // phone.isValid(number, region?) -> boolean
    phone_table.set(
        "isValid",
        lua.create_function(|_, args: MultiValue| {
            let validated =
                validate_args(&args, PHONE_DOC.params("isValid"), "phone.isValid")?;
            let number = match &validated[0] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };
            let region = match &validated[1] {
                Value::String(s) => Some(s.to_string_lossy().to_string()),
                _ => None,
            };

            // Try to parse — if parsing fails, the number is not valid
            match parse_phone(&number, region.as_deref()) {
                Ok(parsed) => Ok(parsed.is_valid()),
                Err(_) => Ok(false),
            }
        })?,
    )?;

    // phone.type(number, region?) -> string
    // Note: "type" is a Lua keyword, but table keys can be any string
    phone_table.set(
        "type",
        lua.create_function(|_, args: MultiValue| {
            let validated = validate_args(&args, PHONE_DOC.params("type"), "phone.type")?;
            let number = match &validated[0] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };
            let region = match &validated[1] {
                Value::String(s) => Some(s.to_string_lossy().to_string()),
                _ => None,
            };

            let parsed = parse_phone(&number, region.as_deref())?;
            let db = phonenumber::metadata::DATABASE.clone();
            let phone_type = parsed.number_type(&db);
            Ok(type_to_string(phone_type).to_string())
        })?,
    )?;

    // phone.region(number) -> string (ISO country code)
    phone_table.set(
        "region",
        lua.create_function(|_, args: MultiValue| {
            let validated =
                validate_args(&args, PHONE_DOC.params("region"), "phone.region")?;
            let number = match &validated[0] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };

            let parsed = parse_phone(&number, None)?;
            match parsed.country().id() {
                Some(id) => Ok(id.as_ref().to_string()),
                None => Err(mlua::Error::external(format!(
                    "phone.region: could not determine region for '{}'",
                    number
                ))),
            }
        })?,
    )?;

    register_help_functions(lua, &phone_table, &PHONE_DOC)?;

    lua.globals().set("phone", phone_table)?;
    wrap_module_with_help_hints(lua, "phone")?;

    Ok(())
}
