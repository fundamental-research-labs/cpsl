//! Country/currency lookup module for the Luau sandbox.
//!
//! Exposes `country.lookup`, `country.byName`, `country.all`, `country.currency`
//! and `currency.lookup`, `currency.all` as globals.
//! Uses `isocountry` (ISO 3166-1) and `iso_currency` (ISO 4217).

use crate::lua_util::register_help_functions;
use crate::sandbox::{
    validate_args, wrap_module_with_help_hints, FnDoc, ModuleDoc, Param, ParamType, ReturnType,
};
use iso_currency::Currency;
use isocountry::CountryCode;
use mlua::{Lua, MultiValue, Value};
use strum::IntoEnumIterator;

// ---------------------------------------------------------------------------
// Documentation
// ---------------------------------------------------------------------------

pub(crate) static COUNTRY_DOC: ModuleDoc = ModuleDoc {
    name: "country",
    summary: "ISO 3166 country & ISO 4217 currency lookup",
    functions: &[
        FnDoc {
            name: "lookup",
            description:
                "Look up a country by alpha-2, alpha-3, or numeric code. Returns {name, alpha2, alpha3, numeric} or nil.",
            params: &[Param {
                name: "code",
                short: Some('c'),
                typ: ParamType::String,
                required: true,
                    fields: None,
            }],
            returns: ReturnType::Table,
            example: Some(r#"country.lookup("US") -- {name="United States of America", alpha2="US", ...}"#),
        },
        FnDoc {
            name: "byName",
            description:
                "Search countries by name (case-insensitive substring match). Returns array of {name, alpha2, alpha3, numeric}.",
            params: &[Param {
                name: "name",
                short: Some('n'),
                typ: ParamType::String,
                required: true,
                    fields: None,
            }],
            returns: ReturnType::Table,
            example: None,
        },
        FnDoc {
            name: "all",
            description:
                "List all ISO 3166-1 countries. Optional query filters by name/code substring. Returns array of {name, alpha2, alpha3, numeric}.",
            params: &[Param {
                name: "query",
                short: Some('q'),
                typ: ParamType::String,
                required: false,
                    fields: None,
            }],
            returns: ReturnType::Table,
            example: None,
        },
        FnDoc {
            name: "currency",
            description:
                "Get currencies used in a country (by alpha-2 code). Returns array of {code, name, symbol, minor_units}.",
            params: &[Param {
                name: "country_code",
                short: Some('c'),
                typ: ParamType::String,
                required: true,
                    fields: None,
            }],
            returns: ReturnType::Table,
            example: None,
        },
    ],
};

pub(crate) static CURRENCY_DOC: ModuleDoc = ModuleDoc {
    name: "currency",
    summary: "ISO 4217 currency lookup",
    functions: &[
        FnDoc {
            name: "lookup",
            description:
                "Look up a currency by ISO 4217 code. Returns {code, name, symbol, minor_units, countries} or nil.",
            params: &[Param {
                name: "code",
                short: Some('c'),
                typ: ParamType::String,
                required: true,
                    fields: None,
            }],
            returns: ReturnType::Table,
            example: Some(r#"currency.lookup("USD") -- {code="USD", name="US Dollar", symbol="$", ...}"#),
        },
        FnDoc {
            name: "all",
            description:
                "List all ISO 4217 currencies. Optional query filters by name/code substring. Returns array of {code, name, symbol, minor_units}.",
            params: &[Param {
                name: "query",
                short: Some('q'),
                typ: ParamType::String,
                required: false,
                    fields: None,
            }],
            returns: ReturnType::Table,
            example: None,
        },
    ],
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Look up a CountryCode from a string that can be alpha-2, alpha-3, or numeric.
fn resolve_country(code: &str) -> Option<CountryCode> {
    let code = code.trim();
    // Try numeric first
    if let Ok(num) = code.parse::<u32>() {
        return CountryCode::for_id(num).ok();
    }
    match code.len() {
        2 => CountryCode::for_alpha2_caseless(code).ok(),
        3 => CountryCode::for_alpha3_caseless(code).ok(),
        _ => None,
    }
}

/// Build a Lua table from a CountryCode.
fn country_to_table(lua: &Lua, cc: &CountryCode) -> mlua::Result<mlua::Table> {
    let t = lua.create_table()?;
    t.set("name", cc.name())?;
    t.set("alpha2", cc.alpha2())?;
    t.set("alpha3", cc.alpha3())?;
    t.set("numeric", cc.numeric_id())?;
    Ok(t)
}

/// Build a Lua table from a Currency.
fn currency_to_table(lua: &Lua, cur: &Currency) -> mlua::Result<mlua::Table> {
    let t = lua.create_table()?;
    t.set("code", cur.code())?;
    t.set("name", cur.name())?;
    t.set("symbol", cur.symbol().to_string())?;
    t.set("minor_units", cur.exponent().unwrap_or(0))?;
    Ok(t)
}

/// Build a Lua table from a Currency with the countries list.
fn currency_to_table_with_countries(lua: &Lua, cur: &Currency) -> mlua::Result<mlua::Table> {
    let t = currency_to_table(lua, cur)?;
    let countries = lua.create_table()?;
    for (i, c) in cur.used_by().iter().enumerate() {
        countries.set(i + 1, format!("{:?}", c))?;
    }
    t.set("countries", countries)?;
    Ok(t)
}

/// Get all currencies used in a given country (by alpha2 code).
fn currencies_for_country(alpha2: &str) -> Vec<Currency> {
    let upper = alpha2.to_uppercase();
    Currency::iter()
        .filter(|cur| {
            cur.used_by()
                .iter()
                .any(|c| format!("{:?}", c) == upper)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/// Register `country.*` and `currency.*` globals in the Lua VM.
pub fn register_country_globals(lua: &Lua) -> Result<(), mlua::Error> {
    // ── country table ──────────────────────────────────────────
    let country_table = lua.create_table()?;

    // country.lookup(code) → {name, alpha2, alpha3, numeric} or nil
    country_table.set(
        "lookup",
        lua.create_function(|lua, args: MultiValue| {
            let validated =
                validate_args(&args, COUNTRY_DOC.params("lookup"), "country.lookup")?;
            let code = match &validated[0] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };
            match resolve_country(&code) {
                Some(cc) => Ok(Value::Table(country_to_table(lua, &cc)?)),
                None => Ok(Value::Nil),
            }
        })?,
    )?;

    // country.byName(name) → array of {name, alpha2, alpha3, numeric}
    country_table.set(
        "byName",
        lua.create_function(|lua, args: MultiValue| {
            let validated =
                validate_args(&args, COUNTRY_DOC.params("byName"), "country.byName")?;
            let name = match &validated[0] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };
            let lower = name.to_lowercase();
            let result = lua.create_table()?;
            let mut i = 1;
            for cc in CountryCode::iter() {
                if cc.name().to_lowercase().contains(&lower) {
                    result.set(i, country_to_table(lua, cc)?)?;
                    i += 1;
                }
            }
            Ok(Value::Table(result))
        })?,
    )?;

    // country.all(query?) → array of {name, alpha2, alpha3, numeric}
    country_table.set(
        "all",
        lua.create_function(|lua, args: MultiValue| {
            let validated =
                validate_args(&args, COUNTRY_DOC.params("all"), "country.all")?;
            let query = match &validated[0] {
                Value::String(s) => Some(s.to_string_lossy().to_string().to_lowercase()),
                _ => None,
            };
            let result = lua.create_table()?;
            let mut i = 1;
            for cc in CountryCode::iter() {
                if let Some(ref q) = query {
                    let name_lower = cc.name().to_lowercase();
                    let a2_lower = cc.alpha2().to_lowercase();
                    let a3_lower = cc.alpha3().to_lowercase();
                    if !name_lower.contains(q) && !a2_lower.contains(q) && !a3_lower.contains(q) {
                        continue;
                    }
                }
                result.set(i, country_to_table(lua, cc)?)?;
                i += 1;
            }
            Ok(Value::Table(result))
        })?,
    )?;

    // country.currency(country_code) → array of {code, name, symbol, minor_units}
    country_table.set(
        "currency",
        lua.create_function(|lua, args: MultiValue| {
            let validated =
                validate_args(&args, COUNTRY_DOC.params("currency"), "country.currency")?;
            let code = match &validated[0] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };
            // Validate the country code first
            let cc = resolve_country(&code).ok_or_else(|| {
                mlua::Error::external(format!(
                    "country.currency: unknown country code '{}'",
                    code
                ))
            })?;
            let alpha2 = cc.alpha2();
            let currencies = currencies_for_country(alpha2);
            let result = lua.create_table()?;
            for (i, cur) in currencies.iter().enumerate() {
                result.set(i + 1, currency_to_table(lua, cur)?)?;
            }
            Ok(Value::Table(result))
        })?,
    )?;

    register_help_functions(lua, &country_table, &COUNTRY_DOC)?;
    lua.globals().set("country", country_table)?;
    wrap_module_with_help_hints(lua, "country")?;

    // ── currency table ─────────────────────────────────────────
    let currency_table = lua.create_table()?;

    // currency.lookup(code) → {code, name, symbol, minor_units, countries} or nil
    currency_table.set(
        "lookup",
        lua.create_function(|lua, args: MultiValue| {
            let validated =
                validate_args(&args, CURRENCY_DOC.params("lookup"), "currency.lookup")?;
            let code = match &validated[0] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };
            match Currency::from_code(&code.to_uppercase()) {
                Some(cur) => Ok(Value::Table(currency_to_table_with_countries(lua, &cur)?)),
                None => Ok(Value::Nil),
            }
        })?,
    )?;

    // currency.all(query?) → array of {code, name, symbol, minor_units}
    currency_table.set(
        "all",
        lua.create_function(|lua, args: MultiValue| {
            let validated =
                validate_args(&args, CURRENCY_DOC.params("all"), "currency.all")?;
            let query = match &validated[0] {
                Value::String(s) => Some(s.to_string_lossy().to_string().to_lowercase()),
                _ => None,
            };
            let result = lua.create_table()?;
            let mut i = 1;
            for cur in Currency::iter() {
                if let Some(ref q) = query {
                    let code_lower = cur.code().to_lowercase();
                    let name_lower = cur.name().to_lowercase();
                    if !code_lower.contains(q) && !name_lower.contains(q) {
                        continue;
                    }
                }
                result.set(i, currency_to_table(lua, &cur)?)?;
                i += 1;
            }
            Ok(Value::Table(result))
        })?,
    )?;

    register_help_functions(lua, &currency_table, &CURRENCY_DOC)?;
    lua.globals().set("currency", currency_table)?;
    wrap_module_with_help_hints(lua, "currency")?;

    Ok(())
}
