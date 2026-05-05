#![cfg(feature = "mod-base64")]

use cpsl_core::Sandbox;

fn sb() -> Sandbox {
    Sandbox::new().unwrap()
}

// ── encode ──────────────────────────────────────────────────────

#[test]
fn encode_simple_string() {
    let s = sb();
    let r = s.exec(r#"return base64.encode("Hello, World!")"#).unwrap();
    assert_eq!(r, "SGVsbG8sIFdvcmxkIQ==");
}

#[test]
fn encode_empty_string() {
    let s = sb();
    let r = s.exec(r#"return base64.encode("")"#).unwrap();
    assert_eq!(r, "");
}

#[test]
fn encode_single_char() {
    let s = sb();
    let r = s.exec(r#"return base64.encode("a")"#).unwrap();
    assert_eq!(r, "YQ==");
}

#[test]
fn encode_two_chars() {
    let s = sb();
    let r = s.exec(r#"return base64.encode("ab")"#).unwrap();
    assert_eq!(r, "YWI=");
}

#[test]
fn encode_three_chars() {
    let s = sb();
    let r = s.exec(r#"return base64.encode("abc")"#).unwrap();
    assert_eq!(r, "YWJj");
}

#[test]
fn encode_no_args_errors() {
    let s = sb();
    let err = s.exec("base64.encode()").unwrap_err();
    assert!(
        err.message.contains("missing required argument") || err.message.contains("Usage:"),
        "should error on missing arg with inline help: {}",
        err.message
    );
}

// ── decode ──────────────────────────────────────────────────────

#[test]
fn decode_simple_string() {
    let s = sb();
    let r = s
        .exec(r#"return base64.decode("SGVsbG8sIFdvcmxkIQ==")"#)
        .unwrap();
    assert_eq!(r, "Hello, World!");
}

#[test]
fn decode_empty_string() {
    let s = sb();
    let r = s.exec(r#"return base64.decode("")"#).unwrap();
    assert_eq!(r, "");
}

#[test]
fn decode_no_padding() {
    let s = sb();
    let r = s.exec(r#"return base64.decode("YWJj")"#).unwrap();
    assert_eq!(r, "abc");
}

#[test]
fn decode_one_padding() {
    let s = sb();
    let r = s.exec(r#"return base64.decode("YWI=")"#).unwrap();
    assert_eq!(r, "ab");
}

#[test]
fn decode_two_padding() {
    let s = sb();
    let r = s.exec(r#"return base64.decode("YQ==")"#).unwrap();
    assert_eq!(r, "a");
}

#[test]
fn decode_with_whitespace() {
    let s = sb();
    // base64.decode should handle embedded newlines (like multiline base64)
    let r = s.exec(r#"return base64.decode("SGVs\nbG8=")"#).unwrap();
    assert_eq!(r, "Hello");
}

#[test]
fn decode_invalid_errors() {
    let s = sb();
    let err = s.exec(r#"base64.decode("!!!")"#).unwrap_err();
    assert!(
        err.message.contains("invalid base64"),
        "should have decode error: {}",
        err.message
    );
}

#[test]
fn decode_no_args_errors() {
    let s = sb();
    let err = s.exec("base64.decode()").unwrap_err();
    assert!(
        err.message.contains("missing required argument") || err.message.contains("Usage:"),
        "should error on missing arg with inline help: {}",
        err.message
    );
}

// ── Python-compatible aliases ───────────────────────────────────

#[test]
fn b64encode_alias() {
    let s = sb();
    let r = s.exec(r#"return base64.b64encode("test")"#).unwrap();
    assert_eq!(r, "dGVzdA==");
}

#[test]
fn b64decode_alias() {
    let s = sb();
    let r = s.exec(r#"return base64.b64decode("dGVzdA==")"#).unwrap();
    assert_eq!(r, "test");
}

// ── roundtrip ───────────────────────────────────────────────────

#[test]
fn roundtrip() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local original = "Hello, World! 123 ~!@#$%"
            local encoded = base64.encode(original)
            local decoded = base64.decode(encoded)
            return decoded
        "#,
        )
        .unwrap();
    assert_eq!(r, "Hello, World! 123 ~!@#$%");
}

#[test]
fn roundtrip_empty() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local encoded = base64.encode("")
            local decoded = base64.decode(encoded)
            return decoded
        "#,
        )
        .unwrap();
    assert_eq!(r, "");
}

// ── doc ─────────────────────────────────────────────────────────

#[test]
fn help_returns_help() {
    let s = sb();
    let r = s.exec("return base64.help()").unwrap();
    assert!(
        r.contains("base64 — Base64 encoding & decoding"),
        "help: {}",
        r
    );
    assert!(r.contains("base64.encode"), "help: {}", r);
    assert!(r.contains("base64.decode"), "help: {}", r);
}

#[test]
fn help_bare_call() {
    let s = sb();
    let r = s.exec("base64.help()").unwrap();
    assert!(r.contains("base64"), "bare help() should print: {}", r);
}

#[test]
fn nonexistent_fn_hint() {
    let s = sb();
    let err = s.exec("base64.foo()").unwrap_err();
    assert!(
        err.message.contains("base64.foo does not exist"),
        "msg: {}",
        err.message
    );
    assert!(
        err.message.contains("hint: call base64.help() for usage"),
        "msg: {}",
        err.message
    );
}

#[test]
fn global_help_mentions_base64() {
    let s = sb();
    let r = s.exec("return help()").unwrap();
    assert!(
        r.contains("base64"),
        "global help should list base64: {}",
        r
    );
}

// ── Python transpiler e2e ───────────────────────────────────────

#[test]
fn python_import_base64() {
    let s = sb();
    let pyrt = include_str!("../../runtime/pyrt.luau");
    s.setup_python_runtime(pyrt).unwrap();

    let py_code = r#"
import base64
result = base64.b64encode("hello")
print(result)
"#;
    let transpiled = cpsl_core::transpile::transpile(py_code).unwrap();
    let r = s.exec(&transpiled.luau_source).unwrap();
    assert_eq!(r, "aGVsbG8=");
}

#[test]
fn python_from_base64_import() {
    let s = sb();
    let pyrt = include_str!("../../runtime/pyrt.luau");
    s.setup_python_runtime(pyrt).unwrap();

    let py_code = r#"
import base64
encoded = base64.b64encode("world")
decoded = base64.b64decode(encoded)
print(decoded)
"#;
    let transpiled = cpsl_core::transpile::transpile(py_code).unwrap();
    let r = s.exec(&transpiled.luau_source).unwrap();
    assert_eq!(r, "world");
}

#[test]
fn python_encode_decode_roundtrip() {
    let s = sb();
    let pyrt = include_str!("../../runtime/pyrt.luau");
    s.setup_python_runtime(pyrt).unwrap();

    let py_code = r#"
import base64
data = "The quick brown fox jumps over the lazy dog"
encoded = base64.encode(data)
decoded = base64.decode(encoded)
print(decoded)
"#;
    let transpiled = cpsl_core::transpile::transpile(py_code).unwrap();
    let r = s.exec(&transpiled.luau_source).unwrap();
    assert_eq!(r, "The quick brown fox jumps over the lazy dog");
}
