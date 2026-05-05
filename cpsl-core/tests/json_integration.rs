#![cfg(feature = "mod-json")]

use cpsl_core::Sandbox;

fn sb() -> Sandbox {
    Sandbox::new().unwrap()
}

// ── decode ──────────────────────────────────────────────────────

#[test]
fn decode_object() {
    let s = sb();
    let r = s.exec(r#"local t = json.decode('{"a":1,"b":"hello"}'); return t.a .. " " .. t.b"#).unwrap();
    assert_eq!(r, "1 hello");
}

#[test]
fn decode_array() {
    let s = sb();
    let r = s.exec(r#"local t = json.decode('[10,20,30]'); return t[1] .. "," .. t[2] .. "," .. t[3]"#).unwrap();
    assert_eq!(r, "10,20,30");
}

#[test]
fn decode_nested() {
    let s = sb();
    let r = s
        .exec(r#"local t = json.decode('{"x":{"y":[1,2]}}'); return t.x.y[2]"#)
        .unwrap();
    assert_eq!(r, "2");
}

#[test]
fn decode_null_becomes_nil() {
    let s = sb();
    let r = s.exec(r#"local t = json.decode('{"a":null}'); return t.a == nil"#).unwrap();
    assert_eq!(r, "true");
}

#[test]
fn decode_booleans() {
    let s = sb();
    let r = s.exec(r#"local t = json.decode('{"a":true,"b":false}'); return tostring(t.a) .. " " .. tostring(t.b)"#).unwrap();
    assert_eq!(r, "true false");
}

#[test]
fn decode_number_types() {
    let s = sb();
    // Integer
    let r = s.exec(r#"return type(json.decode("42"))"#).unwrap();
    assert_eq!(r, "number");
    // Float
    let r = s.exec(r#"return json.decode("3.14")"#).unwrap();
    assert_eq!(r, "3.14");
}

#[test]
fn decode_empty_object() {
    let s = sb();
    // Empty object should decode without error
    let r = s.exec(r#"local t = json.decode('{}'); return type(t)"#).unwrap();
    assert_eq!(r, "table");
}

#[test]
fn decode_empty_array() {
    let s = sb();
    let r = s.exec(r#"local t = json.decode('[]'); return type(t)"#).unwrap();
    assert_eq!(r, "table");
}

#[test]
fn decode_invalid_json_errors() {
    let s = sb();
    let err = s.exec(r#"json.decode("not json")"#).unwrap_err();
    assert!(err.message.contains("expected"), "should have parse error: {}", err.message);
}

#[test]
fn decode_no_args_errors() {
    let s = sb();
    let err = s.exec("json.decode()").unwrap_err();
    assert!(err.message.contains("missing required argument") || err.message.contains("Usage:"),
        "should error on missing arg with inline help: {}", err.message);
}

// ── encode ──────────────────────────────────────────────────────

#[test]
fn encode_table_as_object() {
    let s = sb();
    let r = s.exec(r#"return json.encode({name = "test", val = 42})"#).unwrap();
    // Parse back to verify it's valid JSON with correct values
    let v: serde_json::Value = serde_json::from_str(&r).unwrap();
    assert_eq!(v["name"], "test");
    assert_eq!(v["val"], 42);
}

#[test]
fn encode_array() {
    let s = sb();
    let r = s.exec(r#"return json.encode({10, 20, 30})"#).unwrap();
    assert_eq!(r, "[10,20,30]");
}

#[test]
fn encode_nested() {
    let s = sb();
    let r = s.exec(r#"return json.encode({a = {1, 2}, b = "hi"})"#).unwrap();
    let v: serde_json::Value = serde_json::from_str(&r).unwrap();
    assert_eq!(v["a"], serde_json::json!([1, 2]));
    assert_eq!(v["b"], "hi");
}

#[test]
fn encode_nil() {
    let s = sb();
    let r = s.exec(r#"return json.encode(nil)"#).unwrap();
    assert_eq!(r, "null");
}

#[test]
fn encode_boolean() {
    let s = sb();
    assert_eq!(s.exec("return json.encode(true)").unwrap(), "true");
    assert_eq!(s.exec("return json.encode(false)").unwrap(), "false");
}

#[test]
fn encode_string() {
    let s = sb();
    let r = s.exec(r#"return json.encode("hello")"#).unwrap();
    assert_eq!(r, r#""hello""#);
}

#[test]
fn encode_number() {
    let s = sb();
    assert_eq!(s.exec("return json.encode(42)").unwrap(), "42");
    assert_eq!(s.exec("return json.encode(3.14)").unwrap(), "3.14");
}

#[test]
fn encode_pretty() {
    let s = sb();
    let r = s.exec(r#"return json.encode({a = 1}, {pretty = true})"#).unwrap();
    assert!(r.contains('\n'), "pretty output should have newlines: {}", r);
    assert!(r.contains("  "), "pretty output should be indented: {}", r);
}

#[test]
fn encode_function_errors() {
    let s = sb();
    let err = s.exec("json.encode(print)").unwrap_err();
    assert!(err.message.contains("function"), "should mention function: {}", err.message);
}

// ── roundtrip ───────────────────────────────────────────────────

#[test]
fn roundtrip_object() {
    let s = sb();
    let r = s
        .exec(r#"
            local original = '{"key":"value","num":123,"arr":[1,2,3],"nested":{"a":true}}'
            local decoded = json.decode(original)
            local reencoded = json.encode(decoded)
            local redecoded = json.decode(reencoded)
            return redecoded.key .. " " .. tostring(redecoded.num) .. " " .. tostring(redecoded.nested.a)
        "#)
        .unwrap();
    assert_eq!(r, "value 123 true");
}

#[test]
fn roundtrip_array() {
    let s = sb();
    // Note: trailing null becomes nil in Lua which truncates the array.
    // This is expected Lua behavior — nil terminates arrays.
    let r = s
        .exec(r#"
            local arr = json.decode('[1,"two",true]')
            return json.encode(arr)
        "#)
        .unwrap();
    assert_eq!(r, r#"[1,"two",true]"#);
}

// ── doc ─────────────────────────────────────────────────────────

#[test]
fn json_help_returns_help() {
    let s = sb();
    let r = s.exec("return json.help()").unwrap();
    assert!(r.contains("json — JSON encode & decode"), "help: {}", r);
    assert!(r.contains("json.decode"), "help: {}", r);
    assert!(r.contains("json.encode"), "help: {}", r);
}

#[test]
fn json_help_bare_call() {
    let s = sb();
    let r = s.exec("json.help()").unwrap();
    assert!(r.contains("json"), "bare help() should print: {}", r);
}

#[test]
fn json_nonexistent_fn_hint() {
    let s = sb();
    let err = s.exec("json.foo()").unwrap_err();
    assert!(err.message.contains("json.foo does not exist"), "msg: {}", err.message);
    assert!(err.message.contains("hint: call json.help() for usage"), "msg: {}", err.message);
}

#[test]
fn global_help_mentions_json() {
    let s = sb();
    let r = s.exec("return help()").unwrap();
    assert!(r.contains("json"), "global help should list json: {}", r);
}

// ── Python transpiler e2e ───────────────────────────────────────

#[test]
fn python_json_decode() {
    let s = sb();
    let pyrt = include_str!("../../pyrt.luau");
    s.setup_python_runtime(pyrt).unwrap();

    let py_code = r#"
data = json.decode('{"x": 42}')
print(data.x)
"#;
    let transpiled = cpsl_core::transpile::transpile(py_code).unwrap();
    let r = s.exec(&transpiled.luau_source).unwrap();
    assert_eq!(r, "42");
}

#[test]
fn python_json_encode() {
    let s = sb();
    let pyrt = include_str!("../../pyrt.luau");
    s.setup_python_runtime(pyrt).unwrap();

    // Python dict literal {"key": "value"} transpiles via pyrt dict constructor,
    // so use a simpler approach: build a table and pass it
    let py_code = r#"
data = json.decode('{"key": "value"}')
result = json.encode(data)
print(result)
"#;
    let transpiled = cpsl_core::transpile::transpile(py_code).unwrap();
    let r = s.exec(&transpiled.luau_source).unwrap();
    let v: serde_json::Value = serde_json::from_str(&r).unwrap();
    assert_eq!(v["key"], "value");
}
