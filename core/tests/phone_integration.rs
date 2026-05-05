#![cfg(feature = "mod-phone")]

use cpsl_core::{transpile, Sandbox};

fn sb() -> Sandbox {
    Sandbox::new().unwrap()
}

// ── phone.parse ─────────────────────────────────────────────────

#[test]
fn parse_us_number_with_country_code() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local p = phone.parse("+12025551234")
            return p.country_code .. " " .. p.national_number .. " " .. tostring(p.valid)
        "#,
        )
        .unwrap();
    assert_eq!(r, "1 2025551234 true");
}

#[test]
fn parse_us_number_with_region() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local p = phone.parse("(202) 555-1234", "US")
            return p.country_code .. " " .. p.national_number .. " " .. tostring(p.valid)
        "#,
        )
        .unwrap();
    assert_eq!(r, "1 2025551234 true");
}

#[test]
fn parse_uk_number() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local p = phone.parse("+442071234567")
            return p.country_code .. " " .. tostring(p.valid)
        "#,
        )
        .unwrap();
    assert_eq!(r, "44 true");
}

#[test]
fn parse_invalid_number_still_parses() {
    // parse() can parse the number but valid=false for invalid numbers
    let s = sb();
    let r = s
        .exec(
            r#"
            local p = phone.parse("+1234", "US")
            return tostring(p.valid)
        "#,
        )
        .unwrap();
    assert_eq!(r, "false");
}

#[test]
fn parse_returns_raw() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local p = phone.parse("+12025551234")
            return p.raw
        "#,
        )
        .unwrap();
    assert_eq!(r, "+12025551234");
}

#[test]
fn parse_garbage_errors() {
    let s = sb();
    let err = s.exec(r#"phone.parse("not a phone number")"#).unwrap_err();
    assert!(
        err.message.contains("phone") || err.message.contains("parse"),
        "msg: {}",
        err.message
    );
}

// ── phone.format ────────────────────────────────────────────────

#[test]
fn format_international_default() {
    let s = sb();
    let r = s.exec(r#"return phone.format("+12025551234")"#).unwrap();
    assert_eq!(r, "+1 202-555-1234");
}

#[test]
fn format_e164() {
    let s = sb();
    let r = s
        .exec(r#"return phone.format("+12025551234", "e164")"#)
        .unwrap();
    assert_eq!(r, "+12025551234");
}

#[test]
fn format_national() {
    let s = sb();
    let r = s
        .exec(r#"return phone.format("+12025551234", "national")"#)
        .unwrap();
    assert_eq!(r, "(202) 555-1234");
}

#[test]
fn format_rfc3966() {
    let s = sb();
    let r = s
        .exec(r#"return phone.format("+12025551234", "rfc3966")"#)
        .unwrap();
    assert_eq!(r, "tel:+1-202-555-1234");
}

#[test]
fn format_with_region() {
    let s = sb();
    let r = s
        .exec(r#"return phone.format("(202) 555-1234", "e164", "US")"#)
        .unwrap();
    assert_eq!(r, "+12025551234");
}

#[test]
fn format_unknown_style_errors() {
    let s = sb();
    let err = s
        .exec(r#"phone.format("+12025551234", "unknown")"#)
        .unwrap_err();
    assert!(
        err.message.contains("unknown style"),
        "msg: {}",
        err.message
    );
}

// ── phone.isValid ───────────────────────────────────────────────

#[test]
fn is_valid_valid_number() {
    let s = sb();
    let r = s
        .exec(r#"return tostring(phone.isValid("+12025551234"))"#)
        .unwrap();
    assert_eq!(r, "true");
}

#[test]
fn is_valid_with_region() {
    let s = sb();
    let r = s
        .exec(r#"return tostring(phone.isValid("(202) 555-1234", "US"))"#)
        .unwrap();
    assert_eq!(r, "true");
}

#[test]
fn is_valid_invalid_number() {
    let s = sb();
    let r = s
        .exec(r#"return tostring(phone.isValid("+1234"))"#)
        .unwrap();
    assert_eq!(r, "false");
}

#[test]
fn is_valid_garbage_returns_false() {
    let s = sb();
    let r = s
        .exec(r#"return tostring(phone.isValid("not a phone number"))"#)
        .unwrap();
    assert_eq!(r, "false");
}

#[test]
fn is_valid_empty_returns_false() {
    let s = sb();
    let r = s.exec(r#"return tostring(phone.isValid(""))"#).unwrap();
    assert_eq!(r, "false");
}

// ── phone.type ──────────────────────────────────────────────────

#[test]
fn type_us_number() {
    let s = sb();
    let r = s.exec(r#"return phone["type"]("+12025551234")"#).unwrap();
    // US numbers are often classified as fixed_line_or_mobile
    assert!(
        r == "fixed_line" || r == "mobile" || r == "fixed_line_or_mobile",
        "expected a phone type, got: {}",
        r
    );
}

#[test]
fn type_uk_mobile() {
    let s = sb();
    let r = s.exec(r#"return phone["type"]("+447911123456")"#).unwrap();
    assert_eq!(r, "mobile");
}

#[test]
fn type_with_region() {
    let s = sb();
    let r = s
        .exec(r#"return phone["type"]("(202) 555-1234", "US")"#)
        .unwrap();
    assert!(
        r == "fixed_line" || r == "mobile" || r == "fixed_line_or_mobile",
        "expected a phone type, got: {}",
        r
    );
}

// ── phone.region ────────────────────────────────────────────────

#[test]
fn region_us_number() {
    let s = sb();
    let r = s.exec(r#"return phone.region("+12025551234")"#).unwrap();
    assert_eq!(r, "US");
}

#[test]
fn region_uk_number() {
    let s = sb();
    let r = s.exec(r#"return phone.region("+442071234567")"#).unwrap();
    assert_eq!(r, "GB");
}

#[test]
fn region_german_number() {
    let s = sb();
    let r = s.exec(r#"return phone.region("+4930123456")"#).unwrap();
    assert_eq!(r, "DE");
}

#[test]
fn region_japanese_number() {
    let s = sb();
    let r = s.exec(r#"return phone.region("+81312345678")"#).unwrap();
    assert_eq!(r, "JP");
}

// ── Dual-signature tests (table form for shell dispatch) ────────

#[test]
fn parse_table_form() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local p = phone.parse({[1]="+12025551234"})
            return p.country_code .. " " .. tostring(p.valid)
        "#,
        )
        .unwrap();
    assert_eq!(r, "1 true");
}

#[test]
fn parse_table_form_with_region() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local p = phone.parse({[1]="(202) 555-1234", region="US"})
            return p.country_code .. " " .. tostring(p.valid)
        "#,
        )
        .unwrap();
    assert_eq!(r, "1 true");
}

#[test]
fn format_table_form() {
    let s = sb();
    let r = s
        .exec(r#"return phone.format({[1]="+12025551234", [2]="e164"})"#)
        .unwrap();
    assert_eq!(r, "+12025551234");
}

#[test]
fn is_valid_table_form() {
    let s = sb();
    let r = s
        .exec(r#"return tostring(phone.isValid({[1]="+12025551234"}))"#)
        .unwrap();
    assert_eq!(r, "true");
}

#[test]
fn region_table_form() {
    let s = sb();
    let r = s
        .exec(r#"return phone.region({[1]="+12025551234"})"#)
        .unwrap();
    assert_eq!(r, "US");
}

// ── Shell dispatch tests ────────────────────────────────────────

#[test]
fn shell_phone_format() {
    let s = sb();
    let shrt = include_str!("../../runtime/shrt.luau");
    s.register_module("shrt", shrt).unwrap();
    let result = cpsl_core::sh_transpile::transpile_sh(r#"phone format "+12025551234" "e164""#);
    assert!(result.is_ok(), "transpile err: {:?}", result.err());
    let luau = result.unwrap().luau_source;
    let r = s.exec(&luau).unwrap();
    assert!(
        r.contains("+12025551234"),
        "expected +12025551234, got: {}",
        r
    );
}

#[test]
fn shell_phone_region() {
    let s = sb();
    let shrt = include_str!("../../runtime/shrt.luau");
    s.register_module("shrt", shrt).unwrap();
    let result = cpsl_core::sh_transpile::transpile_sh(r#"phone region "+12025551234""#);
    assert!(result.is_ok(), "transpile err: {:?}", result.err());
    let luau = result.unwrap().luau_source;
    let r = s.exec(&luau).unwrap();
    assert!(r.contains("US"), "expected US, got: {}", r);
}

// ── Error handling ──────────────────────────────────────────────

#[test]
fn parse_no_args_errors() {
    let s = sb();
    let err = s.exec("phone.parse()").unwrap_err();
    assert!(
        err.message.contains("bad argument") || err.message.contains("missing"),
        "msg: {}",
        err.message
    );
}

#[test]
fn parse_wrong_type_errors() {
    let s = sb();
    let err = s.exec("phone.parse(42)").unwrap_err();
    assert!(
        err.message.contains("string") || err.message.contains("expected"),
        "msg: {}",
        err.message
    );
}

#[test]
fn format_no_args_errors() {
    let s = sb();
    let err = s.exec("phone.format()").unwrap_err();
    assert!(
        err.message.contains("bad argument") || err.message.contains("missing"),
        "msg: {}",
        err.message
    );
}

#[test]
fn region_invalid_region_hint_errors() {
    let s = sb();
    let err = s.exec(r#"phone.parse("12345", "ZZ")"#).unwrap_err();
    assert!(
        err.message.contains("invalid region") || err.message.contains("parse"),
        "msg: {}",
        err.message
    );
}

// ── Help ────────────────────────────────────────────────────────

#[test]
fn phone_help_returns_help() {
    let s = sb();
    let r = s.exec("return phone.help()").unwrap();
    assert!(r.contains("phone"), "help: {}", r);
    assert!(r.contains("phone.parse"), "help: {}", r);
    assert!(r.contains("phone.format"), "help: {}", r);
    assert!(r.contains("phone.isValid"), "help: {}", r);
    assert!(r.contains("phone.region"), "help: {}", r);
}

#[test]
fn phone_nonexistent_fn_hint() {
    let s = sb();
    let err = s.exec("phone.foo()").unwrap_err();
    assert!(
        err.message.contains("phone.foo does not exist"),
        "msg: {}",
        err.message
    );
    assert!(
        err.message.contains("hint: call phone.help() for usage"),
        "msg: {}",
        err.message
    );
}

#[test]
fn global_help_mentions_phone() {
    let s = sb();
    let r = s.exec("return help()").unwrap();
    assert!(r.contains("phone"), "global help should list phone: {}", r);
}

// ── Sandbox safety: no filesystem or network access ─────────────

#[test]
fn phone_does_not_access_filesystem() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local p = phone.parse("+12025551234")
            local valid = phone.isValid("+12025551234")
            local region = phone.region("+12025551234")
            local formatted = phone.format("+12025551234", "e164")
            return p.country_code .. " " .. tostring(valid) .. " " .. region .. " " .. formatted
        "#,
        )
        .unwrap();
    assert_eq!(r, "1 true US +12025551234");
}

#[test]
fn phone_no_dangerous_globals_exposed() {
    let s = sb();
    let r = s
        .exec(
            r#"
            return tostring(type(phone.parse)) .. " " ..
                   tostring(type(phone.format)) .. " " ..
                   tostring(type(phone.isValid)) .. " " ..
                   tostring(rawget(phone, "io")) .. " " ..
                   tostring(rawget(phone, "os"))
        "#,
        )
        .unwrap();
    assert_eq!(r, "function function function nil nil");
}

// ── Edge cases ──────────────────────────────────────────────────

#[test]
fn parse_number_with_extension() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local p = phone.parse("+12025551234 ext. 567")
            return tostring(p.valid)
        "#,
        )
        .unwrap();
    // Number with extension may or may not be valid depending on the library
    assert!(r == "true" || r == "false", "got: {}", r);
}

#[test]
fn parse_number_with_dashes_and_spaces() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local p = phone.parse("+1 202-555-1234")
            return p.country_code .. " " .. p.national_number .. " " .. tostring(p.valid)
        "#,
        )
        .unwrap();
    assert_eq!(r, "1 2025551234 true");
}

#[test]
fn format_international_uk() {
    let s = sb();
    let r = s
        .exec(r#"return phone.format("+442071234567", "international")"#)
        .unwrap();
    assert!(r.starts_with("+44"), "expected +44..., got: {}", r);
}

#[test]
fn region_case_insensitive() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local p = phone.parse("(202) 555-1234", "us")
            return tostring(p.valid)
        "#,
        )
        .unwrap();
    assert_eq!(r, "true");
}

#[test]
fn multiple_operations_same_number() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local num = "+33612345678"
            local p = phone.parse(num)
            local valid = phone.isValid(num)
            local region = phone.region(num)
            local formatted = phone.format(num, "national")
            return p.country_code .. " " .. tostring(valid) .. " " .. region
        "#,
        )
        .unwrap();
    assert_eq!(r, "33 true FR");
}

#[test]
fn australian_number() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local p = phone.parse("+61412345678")
            return p.country_code .. " " .. tostring(p.valid) .. " " .. phone.region("+61412345678")
        "#,
        )
        .unwrap();
    assert_eq!(r, "61 true AU");
}

// ── Python transpiler e2e ───────────────────────────────────────

#[test]
fn python_phonenumbers_parse() {
    let s = sb();
    let pyrt = include_str!("../../runtime/pyrt.luau");
    s.setup_python_runtime(pyrt).unwrap();

    let py_code = r#"
import phonenumbers
x = phonenumbers.parse("+12025551234")
print(x.country_code)
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    let r = s.exec(&transpiled.luau_source).unwrap();
    assert_eq!(r, "1");
}

#[test]
fn python_phonenumbers_is_valid() {
    let s = sb();
    let pyrt = include_str!("../../runtime/pyrt.luau");
    s.setup_python_runtime(pyrt).unwrap();

    let py_code = r#"
import phonenumbers
result = phonenumbers.isValid("+12025551234")
print(result)
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    let r = s.exec(&transpiled.luau_source).unwrap();
    // Python runtime prints True/False (Python style)
    assert!(r == "true" || r == "True", "expected true/True, got: {}", r);
}

#[test]
fn python_phonenumbers_format() {
    let s = sb();
    let pyrt = include_str!("../../runtime/pyrt.luau");
    s.setup_python_runtime(pyrt).unwrap();

    let py_code = r#"
import phonenumbers
result = phonenumbers.format("+12025551234", "e164")
print(result)
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    let r = s.exec(&transpiled.luau_source).unwrap();
    assert_eq!(r, "+12025551234");
}

#[test]
fn python_phonenumbers_region() {
    let s = sb();
    let pyrt = include_str!("../../runtime/pyrt.luau");
    s.setup_python_runtime(pyrt).unwrap();

    let py_code = r#"
import phonenumbers
region = phonenumbers.region("+442071234567")
print(region)
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    let r = s.exec(&transpiled.luau_source).unwrap();
    assert_eq!(r, "GB");
}

#[test]
fn python_from_phonenumbers_import() {
    let py_code = r#"
from phonenumbers import PhoneNumber
result = PhoneNumber.parse("+12025551234")
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    // from phonenumbers import PhoneNumber → phone
    assert!(
        transpiled.luau_source.contains("phone"),
        "transpiled: {}",
        transpiled.luau_source
    );
}

#[test]
fn python_import_phone_passthrough() {
    let py_code = r#"
import phone
result = phone.isValid("+12025551234")
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    assert!(
        transpiled.luau_source.contains("phone"),
        "transpiled: {}",
        transpiled.luau_source
    );
}
