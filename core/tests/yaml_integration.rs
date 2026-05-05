#![cfg(feature = "mod-yaml")]

use cpsl_core::{transpile, MountTable, Sandbox};

fn sb() -> Sandbox {
    Sandbox::new().unwrap()
}

// ── decode ──────────────────────────────────────────────────────

#[test]
fn decode_simple_mapping() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local data = yaml.decode("name: Alice\nage: 30")
            return data.name .. " " .. tostring(data.age)
        "#,
        )
        .unwrap();
    assert_eq!(r, "Alice 30");
}

#[test]
fn decode_sequence() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local data = yaml.decode("- a\n- b\n- c")
            return #data .. " " .. data[1] .. " " .. data[3]
        "#,
        )
        .unwrap();
    assert_eq!(r, "3 a c");
}

#[test]
fn decode_nested() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local data = yaml.decode("person:\n  name: Bob\n  age: 25")
            return data.person.name .. " " .. tostring(data.person.age)
        "#,
        )
        .unwrap();
    assert_eq!(r, "Bob 25");
}

#[test]
fn decode_boolean_and_null() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local data = yaml.decode("flag: true\nother: false\nmissing: null")
            return tostring(data.flag) .. " " .. tostring(data.other) .. " " .. tostring(data.missing)
        "#,
        )
        .unwrap();
    assert_eq!(r, "true false nil");
}

#[test]
fn decode_numbers() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local data = yaml.decode("int: 42\nfloat: 3.14")
            return tostring(data.int) .. " " .. tostring(data.float)
        "#,
        )
        .unwrap();
    assert_eq!(r, "42 3.14");
}

#[test]
fn decode_empty_string() {
    let s = sb();
    let r = s
        .exec(r#"local data = yaml.decode(""); return tostring(data)"#)
        .unwrap();
    assert_eq!(r, "nil");
}

#[test]
fn decode_list_of_mappings() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local data = yaml.decode("- name: Alice\n  age: 30\n- name: Bob\n  age: 25")
            return #data .. " " .. data[1].name .. " " .. tostring(data[2].age)
        "#,
        )
        .unwrap();
    assert_eq!(r, "2 Alice 25");
}

// ── encode ──────────────────────────────────────────────────────

#[test]
fn encode_simple_table() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local result = yaml.encode({name = "Alice", age = 30})
            return result
        "#,
        )
        .unwrap();
    assert!(r.contains("name:"), "got: {}", r);
    assert!(r.contains("Alice"), "got: {}", r);
    assert!(r.contains("age:"), "got: {}", r);
    assert!(r.contains("30"), "got: {}", r);
}

#[test]
fn encode_array() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local result = yaml.encode({"a", "b", "c"})
            return result
        "#,
        )
        .unwrap();
    assert!(r.contains("- a"), "got: {}", r);
    assert!(r.contains("- b"), "got: {}", r);
    assert!(r.contains("- c"), "got: {}", r);
}

#[test]
fn encode_nested() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local result = yaml.encode({person = {name = "Bob", age = 25}})
            return result
        "#,
        )
        .unwrap();
    assert!(r.contains("person:"), "got: {}", r);
    assert!(r.contains("name:"), "got: {}", r);
    assert!(r.contains("Bob"), "got: {}", r);
}

#[test]
fn encode_booleans_and_nil() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local result = yaml.encode({flag = true, other = false})
            return result
        "#,
        )
        .unwrap();
    assert!(r.contains("true"), "got: {}", r);
    assert!(r.contains("false"), "got: {}", r);
}

// ── roundtrip ───────────────────────────────────────────────────

#[test]
fn roundtrip_decode_encode() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local original = "name: Alice\nage: 30\nitems:\n- one\n- two"
            local data = yaml.decode(original)
            local encoded = yaml.encode(data)
            local reparsed = yaml.decode(encoded)
            return reparsed.name .. " " .. tostring(reparsed.age) .. " " .. reparsed.items[1] .. " " .. reparsed.items[2]
        "#,
        )
        .unwrap();
    assert_eq!(r, "Alice 30 one two");
}

// ── error handling ──────────────────────────────────────────────

#[test]
fn decode_no_args_errors() {
    let s = sb();
    let err = s.exec("yaml.decode()").unwrap_err();
    assert!(
        err.message.contains("missing required argument") || err.message.contains("Usage:"),
        "msg: {}",
        err.message
    );
}

#[test]
fn decode_invalid_yaml() {
    let s = sb();
    let err = s.exec(r#"yaml.decode(":\n  :\n  : [[[[")"#).unwrap_err();
    assert!(
        !err.message.is_empty(),
        "should error on invalid YAML: {}",
        err.message
    );
}

// ── help ────────────────────────────────────────────────────────

#[test]
fn yaml_help_returns_help() {
    let s = sb();
    let r = s.exec("return yaml.help()").unwrap();
    assert!(r.contains("yaml — YAML parse & emit"), "help: {}", r);
    assert!(r.contains("yaml.decode"), "help: {}", r);
    assert!(r.contains("yaml.encode"), "help: {}", r);
}

#[test]
fn yaml_nonexistent_fn_hint() {
    let s = sb();
    let err = s.exec("yaml.foo()").unwrap_err();
    assert!(
        err.message.contains("yaml.foo does not exist"),
        "msg: {}",
        err.message
    );
    assert!(
        err.message.contains("hint: call yaml.help() for usage"),
        "msg: {}",
        err.message
    );
}

#[test]
fn global_help_mentions_yaml() {
    let s = sb();
    let r = s.exec("return help()").unwrap();
    assert!(r.contains("yaml"), "global help should list yaml: {}", r);
}

// ── Python transpiler e2e ───────────────────────────────────────

#[test]
fn python_yaml_decode() {
    let s = sb();
    let pyrt = include_str!("../../runtime/pyrt.luau");
    s.setup_python_runtime(pyrt).unwrap();

    let py_code = r#"
import yaml
data = yaml.decode("name: Alice\nage: 30")
print(data.name)
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    let r = s.exec(&transpiled.luau_source).unwrap();
    assert_eq!(r, "Alice");
}

#[test]
fn python_yaml_encode() {
    let s = sb();
    let pyrt = include_str!("../../runtime/pyrt.luau");
    s.setup_python_runtime(pyrt).unwrap();

    let py_code = r#"
import yaml
result = yaml.encode({"items": ["a", "b"]})
print(result)
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    let r = s.exec(&transpiled.luau_source).unwrap();
    assert!(r.contains("items:"), "got: {}", r);
}

#[test]
// ── Dual-signature tests (table form for shell dispatch) ────────
#[test]
fn decode_table_form() {
    // yaml.decode({[1]="key: value"}) — shell dispatch form
    let s = sb();
    let r = s
        .exec(r#"local d = yaml.decode({[1]="key: value"}); return d.key"#)
        .unwrap();
    assert_eq!(r, "value");
}

#[test]
fn decode_file_table_form() {
    // yaml.decodeFile({[1]="/workspace/data.yaml"}) — shell dispatch form
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::write(dir.path().join("data.yaml"), "name: test\ncount: 42").unwrap();
    let mut table = MountTable::new();
    table
        .parse_and_add(&format!("{}:/workspace", dir.path().display()))
        .unwrap();
    let s = Sandbox::with_mounts(table).unwrap();
    let r = s
        .exec(r#"local d = yaml.decodeFile({[1]="/workspace/data.yaml"}); return d.name"#)
        .unwrap();
    assert_eq!(r, "test");
}

#[test]
fn shell_yaml_decode_roundtrip() {
    // `yaml decode "key: value"` via sh.run() should display the parsed result
    let s = sb();
    let shrt = include_str!("../../runtime/shrt.luau");
    s.register_module("shrt", shrt).unwrap();
    let result = cpsl_core::sh_transpile::transpile_sh(r#"yaml decode "key: value""#);
    assert!(result.is_ok());
    let luau = result.unwrap().luau_source;
    let r = s.exec(&luau).unwrap();
    // sh.run() auto-serializes table to JSON
    assert!(
        r.contains("key") && r.contains("value"),
        "should display parsed yaml, got: {}",
        r
    );
}

#[test]
fn python_pyyaml_import_maps_to_yaml() {
    // `import yaml` should passthrough to the yaml global
    let py_code = r#"
import yaml
data = yaml.decode("key: value")
print(data.key)
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    // The transpiled code should reference yaml directly (passthrough)
    assert!(
        transpiled.luau_source.contains("yaml"),
        "transpiled: {}",
        transpiled.luau_source
    );
}
