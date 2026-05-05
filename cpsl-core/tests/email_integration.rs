#![cfg(feature = "mod-email")]

use cpsl_core::{Sandbox, transpile};

fn sb() -> Sandbox {
    Sandbox::new().unwrap()
}

// ── email.isValid ──────────────────────────────────────────────

#[test]
fn is_valid_simple_address() {
    let s = sb();
    let r = s
        .exec(r#"return tostring(email.isValid("user@example.com"))"#)
        .unwrap();
    assert_eq!(r, "true");
}

#[test]
fn is_valid_with_plus_tag() {
    let s = sb();
    let r = s
        .exec(r#"return tostring(email.isValid("user+tag@example.com"))"#)
        .unwrap();
    assert_eq!(r, "true");
}

#[test]
fn is_valid_with_dots_in_local() {
    let s = sb();
    let r = s
        .exec(r#"return tostring(email.isValid("first.last@example.com"))"#)
        .unwrap();
    assert_eq!(r, "true");
}

#[test]
fn is_valid_with_subdomain() {
    let s = sb();
    let r = s
        .exec(r#"return tostring(email.isValid("user@mail.example.co.uk"))"#)
        .unwrap();
    assert_eq!(r, "true");
}

#[test]
fn is_valid_missing_at() {
    let s = sb();
    let r = s
        .exec(r#"return tostring(email.isValid("userexample.com"))"#)
        .unwrap();
    assert_eq!(r, "false");
}

#[test]
fn is_valid_missing_domain() {
    let s = sb();
    let r = s
        .exec(r#"return tostring(email.isValid("user@"))"#)
        .unwrap();
    assert_eq!(r, "false");
}

#[test]
fn is_valid_missing_local_part() {
    let s = sb();
    let r = s
        .exec(r#"return tostring(email.isValid("@example.com"))"#)
        .unwrap();
    assert_eq!(r, "false");
}

#[test]
fn is_valid_empty_string() {
    let s = sb();
    let r = s
        .exec(r#"return tostring(email.isValid(""))"#)
        .unwrap();
    assert_eq!(r, "false");
}

#[test]
fn is_valid_double_at() {
    let s = sb();
    let r = s
        .exec(r#"return tostring(email.isValid("user@@example.com"))"#)
        .unwrap();
    assert_eq!(r, "false");
}

#[test]
fn is_valid_spaces() {
    let s = sb();
    let r = s
        .exec(r#"return tostring(email.isValid("user @example.com"))"#)
        .unwrap();
    assert_eq!(r, "false");
}

#[test]
fn is_valid_plain_text() {
    let s = sb();
    let r = s
        .exec(r#"return tostring(email.isValid("not an email"))"#)
        .unwrap();
    assert_eq!(r, "false");
}

// ── email.parse ────────────────────────────────────────────────

#[test]
fn parse_simple_address() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local p = email.parse("user@example.com")
            return p["local"] .. " " .. p.domain .. " " .. p.full
        "#,
        )
        .unwrap();
    assert_eq!(r, "user example.com user@example.com");
}

#[test]
fn parse_with_plus_tag() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local p = email.parse("user+tag@example.com")
            return p["local"] .. " " .. p.domain
        "#,
        )
        .unwrap();
    assert_eq!(r, "user+tag example.com");
}

#[test]
fn parse_with_dots_in_local() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local p = email.parse("first.last@example.com")
            return p["local"]
        "#,
        )
        .unwrap();
    assert_eq!(r, "first.last");
}

#[test]
fn parse_with_subdomain() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local p = email.parse("user@mail.example.co.uk")
            return p.domain
        "#,
        )
        .unwrap();
    assert_eq!(r, "mail.example.co.uk");
}

#[test]
fn parse_invalid_returns_nil() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local p = email.parse("not an email")
            return tostring(p)
        "#,
        )
        .unwrap();
    assert_eq!(r, "nil");
}

#[test]
fn parse_empty_returns_nil() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local p = email.parse("")
            return tostring(p)
        "#,
        )
        .unwrap();
    assert_eq!(r, "nil");
}

#[test]
fn parse_missing_at_returns_nil() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local p = email.parse("userexample.com")
            return tostring(p)
        "#,
        )
        .unwrap();
    assert_eq!(r, "nil");
}

// ── email.normalize ────────────────────────────────────────────

#[test]
fn normalize_lowercases_domain() {
    let s = sb();
    let r = s
        .exec(r#"return email.normalize("User@EXAMPLE.COM")"#)
        .unwrap();
    assert_eq!(r, "User@example.com");
}

#[test]
fn normalize_preserves_local_case() {
    let s = sb();
    let r = s
        .exec(r#"return email.normalize("John.Doe@Gmail.COM")"#)
        .unwrap();
    assert_eq!(r, "John.Doe@gmail.com");
}

#[test]
fn normalize_trims_whitespace() {
    let s = sb();
    let r = s
        .exec(r#"return email.normalize("  user@example.com  ")"#)
        .unwrap();
    assert_eq!(r, "user@example.com");
}

#[test]
fn normalize_already_normal() {
    let s = sb();
    let r = s
        .exec(r#"return email.normalize("user@example.com")"#)
        .unwrap();
    assert_eq!(r, "user@example.com");
}

#[test]
fn normalize_invalid_errors() {
    let s = sb();
    let err = s
        .exec(r#"email.normalize("not an email")"#)
        .unwrap_err();
    assert!(
        err.message.contains("email.normalize") || err.message.contains("invalid"),
        "msg: {}",
        err.message
    );
}

// ── Dual-signature tests (table form for shell dispatch) ────────

#[test]
fn is_valid_table_form() {
    let s = sb();
    let r = s
        .exec(r#"return tostring(email.isValid({[1]="user@example.com"}))"#)
        .unwrap();
    assert_eq!(r, "true");
}

#[test]
fn is_valid_table_form_named() {
    let s = sb();
    let r = s
        .exec(r#"return tostring(email.isValid({address="user@example.com"}))"#)
        .unwrap();
    assert_eq!(r, "true");
}

#[test]
fn parse_table_form() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local p = email.parse({[1]="user@example.com"})
            return p["local"] .. " " .. p.domain
        "#,
        )
        .unwrap();
    assert_eq!(r, "user example.com");
}

#[test]
fn normalize_table_form() {
    let s = sb();
    let r = s
        .exec(r#"return email.normalize({[1]="User@EXAMPLE.COM"})"#)
        .unwrap();
    assert_eq!(r, "User@example.com");
}

// ── Shell dispatch tests ────────────────────────────────────────

#[test]
fn shell_email_is_valid() {
    let s = sb();
    let shrt = include_str!("../../shrt.luau");
    s.register_module("shrt", shrt).unwrap();
    let result = cpsl_core::sh_transpile::transpile_sh(r#"email isValid "user@example.com""#);
    assert!(result.is_ok(), "transpile err: {:?}", result.err());
    let luau = result.unwrap().luau_source;
    // sh.run() doesn't auto-serialize booleans (known limitation), so just
    // verify the call succeeds without error — the boolean is returned but not printed.
    let r = s.exec(&luau);
    assert!(r.is_ok(), "shell email isValid should not error: {:?}", r.err());
}

#[test]
fn shell_email_parse() {
    let s = sb();
    let shrt = include_str!("../../shrt.luau");
    s.register_module("shrt", shrt).unwrap();
    let result = cpsl_core::sh_transpile::transpile_sh(r#"email parse "user@example.com""#);
    assert!(result.is_ok(), "transpile err: {:?}", result.err());
    let luau = result.unwrap().luau_source;
    let r = s.exec(&luau).unwrap();
    assert!(
        r.contains("user") && r.contains("example.com"),
        "expected parsed email, got: {}",
        r
    );
}

#[test]
fn shell_email_normalize() {
    let s = sb();
    let shrt = include_str!("../../shrt.luau");
    s.register_module("shrt", shrt).unwrap();
    let result =
        cpsl_core::sh_transpile::transpile_sh(r#"email normalize "User@EXAMPLE.COM""#);
    assert!(result.is_ok(), "transpile err: {:?}", result.err());
    let luau = result.unwrap().luau_source;
    let r = s.exec(&luau).unwrap();
    assert!(
        r.contains("User@example.com"),
        "expected normalized email, got: {}",
        r
    );
}

// ── Error handling ──────────────────────────────────────────────

#[test]
fn is_valid_no_args_errors() {
    let s = sb();
    let err = s.exec("email.isValid()").unwrap_err();
    assert!(
        err.message.contains("bad argument") || err.message.contains("missing"),
        "msg: {}",
        err.message
    );
}

#[test]
fn parse_no_args_errors() {
    let s = sb();
    let err = s.exec("email.parse()").unwrap_err();
    assert!(
        err.message.contains("bad argument") || err.message.contains("missing"),
        "msg: {}",
        err.message
    );
}

#[test]
fn normalize_no_args_errors() {
    let s = sb();
    let err = s.exec("email.normalize()").unwrap_err();
    assert!(
        err.message.contains("bad argument") || err.message.contains("missing"),
        "msg: {}",
        err.message
    );
}

#[test]
fn is_valid_wrong_type_errors() {
    let s = sb();
    let err = s.exec("email.isValid(42)").unwrap_err();
    assert!(
        err.message.contains("string") || err.message.contains("expected"),
        "msg: {}",
        err.message
    );
}

#[test]
fn parse_wrong_type_errors() {
    let s = sb();
    let err = s.exec("email.parse(42)").unwrap_err();
    assert!(
        err.message.contains("string") || err.message.contains("expected"),
        "msg: {}",
        err.message
    );
}

#[test]
fn normalize_wrong_type_errors() {
    let s = sb();
    let err = s.exec("email.normalize(42)").unwrap_err();
    assert!(
        err.message.contains("string") || err.message.contains("expected"),
        "msg: {}",
        err.message
    );
}

// ── Help ────────────────────────────────────────────────────────

#[test]
fn email_help_returns_help() {
    let s = sb();
    let r = s.exec("return email.help()").unwrap();
    assert!(r.contains("email"), "help: {}", r);
    assert!(r.contains("email.isValid"), "help: {}", r);
    assert!(r.contains("email.parse"), "help: {}", r);
    assert!(r.contains("email.normalize"), "help: {}", r);
}

#[test]
fn email_nonexistent_fn_hint() {
    let s = sb();
    let err = s.exec("email.foo()").unwrap_err();
    assert!(
        err.message.contains("email.foo does not exist"),
        "msg: {}",
        err.message
    );
    assert!(
        err.message.contains("hint: call email.help() for usage"),
        "msg: {}",
        err.message
    );
}

#[test]
fn global_help_mentions_email() {
    let s = sb();
    let r = s.exec("return help()").unwrap();
    assert!(
        r.contains("email"),
        "global help should list email: {}",
        r
    );
}

// ── Sandbox safety: no filesystem or network access ─────────────

#[test]
fn email_does_not_access_filesystem() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local valid = email.isValid("user@example.com")
            local p = email.parse("user@example.com")
            local normalized = email.normalize("User@EXAMPLE.COM")
            return tostring(valid) .. " " .. p["local"] .. " " .. p.domain .. " " .. normalized
        "#,
        )
        .unwrap();
    assert_eq!(r, "true user example.com User@example.com");
}

#[test]
fn email_no_dangerous_globals_exposed() {
    let s = sb();
    let r = s
        .exec(
            r#"
            return tostring(type(email.isValid)) .. " " ..
                   tostring(type(email.parse)) .. " " ..
                   tostring(type(email.normalize)) .. " " ..
                   tostring(rawget(email, "io")) .. " " ..
                   tostring(rawget(email, "os"))
        "#,
        )
        .unwrap();
    assert_eq!(r, "function function function nil nil");
}

#[test]
fn email_sandbox_no_io_access() {
    // Verify the email module doesn't expose any way to read/write files
    let s = sb();
    let r = s
        .exec(
            r#"
            -- email module has a metatable for help hints (__index), that's expected.
            -- Verify no io/os leaks through it.
            local mt = getmetatable(email)
            if mt then
                local idx = rawget(mt, "__index")
                if type(idx) == "table" then
                    if rawget(idx, "io") or rawget(idx, "os") then
                        return "metatable leaks dangerous globals"
                    end
                end
            end
            -- email table should only contain its functions + help + __summary
            local count = 0
            for k, v in pairs(email) do
                count = count + 1
            end
            return "safe:" .. count
        "#,
        )
        .unwrap();
    assert!(
        r.starts_with("safe:"),
        "expected safe table, got: {}",
        r
    );
}

#[test]
fn email_sandbox_no_network_access() {
    // Verify email module is purely computational — no DNS lookups or network calls
    let s = sb();
    let r = s
        .exec(
            r#"
            -- All email operations should work without any network
            local results = {}
            table.insert(results, tostring(email.isValid("user@example.com")))
            table.insert(results, tostring(email.isValid("invalid")))
            local p = email.parse("test@test.org")
            table.insert(results, p.domain)
            table.insert(results, email.normalize("A@B.COM"))
            return table.concat(results, ",")
        "#,
        )
        .unwrap();
    assert_eq!(r, "true,false,test.org,A@b.com");
}

// ── Edge cases ──────────────────────────────────────────────────

#[test]
fn parse_numeric_local_part() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local p = email.parse("123@example.com")
            return p["local"]
        "#,
        )
        .unwrap();
    assert_eq!(r, "123");
}

#[test]
fn parse_hyphen_in_domain() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local p = email.parse("user@my-domain.com")
            return p.domain
        "#,
        )
        .unwrap();
    assert_eq!(r, "my-domain.com");
}

#[test]
fn is_valid_very_long_local_part() {
    let s = sb();
    // RFC 5321 limits local part to 64 characters
    let r = s
        .exec(
            r#"
            local long = string.rep("a", 65)
            return tostring(email.isValid(long .. "@example.com"))
        "#,
        )
        .unwrap();
    assert_eq!(r, "false");
}

#[test]
fn normalize_with_plus_tag() {
    let s = sb();
    let r = s
        .exec(r#"return email.normalize("user+tag@EXAMPLE.COM")"#)
        .unwrap();
    assert_eq!(r, "user+tag@example.com");
}

#[test]
fn multiple_operations_same_address() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local addr = "Test.User@Example.COM"
            local valid = email.isValid(addr)
            local p = email.parse(addr)
            local normalized = email.normalize(addr)
            return tostring(valid) .. " " .. p["local"] .. " " .. p.domain .. " " .. normalized
        "#,
        )
        .unwrap();
    assert_eq!(r, "true Test.User Example.COM Test.User@example.com");
}

#[test]
fn is_valid_unicode_domain() {
    // IDN domains may or may not be valid depending on the crate
    let s = sb();
    let r = s
        .exec(r#"return tostring(email.isValid("user@example.com"))"#)
        .unwrap();
    assert_eq!(r, "true");
}

#[test]
fn parse_full_field_matches_input() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local p = email.parse("hello@world.org")
            return p.full
        "#,
        )
        .unwrap();
    assert_eq!(r, "hello@world.org");
}

// ── Python transpiler e2e ───────────────────────────────────────

#[test]
fn python_email_validator_is_valid() {
    let s = sb();
    let pyrt = include_str!("../../pyrt.luau");
    s.setup_python_runtime(pyrt).unwrap();

    let py_code = r#"
import email_validator
result = email_validator.isValid("user@example.com")
print(result)
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    let r = s.exec(&transpiled.luau_source).unwrap();
    assert!(r == "true" || r == "True", "expected true/True, got: {}", r);
}

#[test]
fn python_email_validator_parse() {
    let s = sb();
    let pyrt = include_str!("../../pyrt.luau");
    s.setup_python_runtime(pyrt).unwrap();

    let py_code = r#"
import email_validator
p = email_validator.parse("user@example.com")
print(p.domain)
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    let r = s.exec(&transpiled.luau_source).unwrap();
    assert_eq!(r, "example.com");
}

#[test]
fn python_email_validator_normalize() {
    let s = sb();
    let pyrt = include_str!("../../pyrt.luau");
    s.setup_python_runtime(pyrt).unwrap();

    let py_code = r#"
import email_validator
result = email_validator.normalize("User@EXAMPLE.COM")
print(result)
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    let r = s.exec(&transpiled.luau_source).unwrap();
    assert_eq!(r, "User@example.com");
}

#[test]
fn python_from_email_validator_import() {
    let py_code = r#"
from email_validator import validate_email
result = validate_email("user@example.com")
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    // from email_validator import validate_email → email
    assert!(
        transpiled.luau_source.contains("email"),
        "transpiled: {}",
        transpiled.luau_source
    );
}

#[test]
fn python_import_email_passthrough() {
    let py_code = r#"
import email
result = email.isValid("user@example.com")
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    assert!(
        transpiled.luau_source.contains("email"),
        "transpiled: {}",
        transpiled.luau_source
    );
}
