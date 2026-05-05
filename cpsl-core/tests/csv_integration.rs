#![cfg(feature = "mod-csv")]

use cpsl_core::{Sandbox, transpile, sh_transpile};

fn sb() -> Sandbox {
    Sandbox::new().unwrap()
}

// ── parse (no header) ──────────────────────────────────────────

#[test]
fn parse_simple_csv() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local data = csv.parse("a,b,c\n1,2,3\n4,5,6")
            return #data .. " " .. data[1][1] .. " " .. data[3][3]
        "#,
        )
        .unwrap();
    assert_eq!(r, "3 a 6");
}

#[test]
fn parse_empty_csv() {
    let s = sb();
    let r = s.exec(r#"local data = csv.parse(""); return #data"#).unwrap();
    assert_eq!(r, "0");
}

#[test]
fn parse_single_row() {
    let s = sb();
    let r = s
        .exec(r#"local data = csv.parse("x,y,z"); return data[1][2]"#)
        .unwrap();
    assert_eq!(r, "y");
}

#[test]
fn parse_with_quotes() {
    let s = sb();
    let r = s
        .exec(
            r#"local data = csv.parse('"hello, world",42'); return data[1][1] .. "|" .. data[1][2]"#,
        )
        .unwrap();
    assert_eq!(r, "hello, world|42");
}

// ── parse (with header) ────────────────────────────────────────

#[test]
fn parse_with_header() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local data = csv.parse("name,age\nAlice,30\nBob,25", {header = true})
            return #data .. " " .. data[1].name .. " " .. data[2].age
        "#,
        )
        .unwrap();
    assert_eq!(r, "2 Alice 25");
}

// ── parse (custom delimiter) ───────────────────────────────────

#[test]
fn parse_custom_delimiter() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local data = csv.parse("a\tb\tc", {delimiter = "\t"})
            return data[1][1] .. " " .. data[1][3]
        "#,
        )
        .unwrap();
    assert_eq!(r, "a c");
}

// ── parse (skipRows) ───────────────────────────────────────────

#[test]
fn parse_skip_rows() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local data = csv.parse("a,b\n1,2\n3,4\n5,6", {skipRows = 1})
            return #data .. " " .. data[1][1]
        "#,
        )
        .unwrap();
    assert_eq!(r, "3 1");
}

// ── stringify ──────────────────────────────────────────────────

#[test]
fn stringify_array_of_arrays() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local rows = {{"a", "b"}, {"1", "2"}}
            local result = csv.stringify(rows)
            return result
        "#,
        )
        .unwrap();
    assert!(r.contains("a,b"), "got: {}", r);
    assert!(r.contains("1,2"), "got: {}", r);
}

#[test]
fn stringify_with_headers() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local rows = {{name = "Alice", age = "30"}, {name = "Bob", age = "25"}}
            local result = csv.stringify(rows, {headers = {"name", "age"}})
            return result
        "#,
        )
        .unwrap();
    assert!(r.contains("name,age"), "got: {}", r);
    assert!(r.contains("Alice,30"), "got: {}", r);
    assert!(r.contains("Bob,25"), "got: {}", r);
}

#[test]
fn stringify_custom_delimiter() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local rows = {{"a", "b"}, {"1", "2"}}
            local result = csv.stringify(rows, {delimiter = "\t"})
            return result
        "#,
        )
        .unwrap();
    assert!(r.contains("a\tb"), "got: {}", r);
}

// ── stringify with numbers ─────────────────────────────────────

#[test]
fn stringify_numbers() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local rows = {{1, 2.5, true}}
            local result = csv.stringify(rows)
            return result
        "#,
        )
        .unwrap();
    assert!(r.contains("1,2.5,true"), "got: {}", r);
}

// ── roundtrip ──────────────────────────────────────────────────

#[test]
fn roundtrip_parse_stringify() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local original = "name,age\nAlice,30\nBob,25"
            local data = csv.parse(original, {header = true})
            local result = csv.stringify(data, {headers = {"name", "age"}})
            local reparsed = csv.parse(result, {header = true})
            return reparsed[1].name .. " " .. reparsed[2].age
        "#,
        )
        .unwrap();
    assert_eq!(r, "Alice 25");
}

// ── error handling ─────────────────────────────────────────────

#[test]
fn parse_no_args_errors() {
    let s = sb();
    let err = s.exec("csv.parse()").unwrap_err();
    assert!(
        err.message.contains("missing required argument") || err.message.contains("Usage:"),
        "msg: {}",
        err.message
    );
}

// ── help ───────────────────────────────────────────────────────

#[test]
fn csv_help_returns_help() {
    let s = sb();
    let r = s.exec("return csv.help()").unwrap();
    assert!(r.contains("csv — CSV parse & write"), "help: {}", r);
    assert!(r.contains("csv.parse"), "help: {}", r);
    assert!(r.contains("csv.stringify"), "help: {}", r);
}

#[test]
fn csv_nonexistent_fn_hint() {
    let s = sb();
    let err = s.exec("csv.foo()").unwrap_err();
    assert!(
        err.message.contains("csv.foo does not exist"),
        "msg: {}",
        err.message
    );
    assert!(
        err.message.contains("hint: call csv.help() for usage"),
        "msg: {}",
        err.message
    );
}

#[test]
fn global_help_mentions_csv() {
    let s = sb();
    let r = s.exec("return help()").unwrap();
    assert!(r.contains("csv"), "global help should list csv: {}", r);
}

// ── Python transpiler e2e ───────────────────────────────────────

#[test]
fn python_csv_parse() {
    let s = sb();
    let pyrt = include_str!("../../pyrt.luau");
    s.setup_python_runtime(pyrt).unwrap();

    // Test csv.parse call from transpiled Python.
    // csv returns plain Lua tables. Use json.encode to inspect the result
    // since Python indexing (data[0]) would use py.index which expects Python objects.
    let py_code = r#"
text = "a,b\n1,2"
data = csv.parse(text)
result = json.encode(data)
print(result)
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    let r = s.exec(&transpiled.luau_source).unwrap();
    let v: serde_json::Value = serde_json::from_str(&r).unwrap();
    assert_eq!(v[0][0], "a");
    assert_eq!(v[1][0], "1");
}

#[test]
fn python_csv_stringify() {
    let s = sb();
    let pyrt = include_str!("../../pyrt.luau");
    s.setup_python_runtime(pyrt).unwrap();

    let py_code = r#"
rows = csv.parse("a,b\n1,2")
result = csv.stringify(rows)
print(result)
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    let r = s.exec(&transpiled.luau_source).unwrap();
    assert!(r.contains("a,b"), "got: {}", r);
    assert!(r.contains("1,2"), "got: {}", r);
}

// ── Dual-signature tests (table form for shell dispatch) ────────

#[test]
fn parse_table_form() {
    // csv.parse({[1]="a,b\n1,2"}) — shell dispatch form
    let s = sb();
    let r = s
        .exec(r#"local data = csv.parse({[1]="a,b\n1,2"}); return #data .. " " .. data[1][1]"#)
        .unwrap();
    assert_eq!(r, "2 a");
}

#[test]
fn parse_table_form_with_header() {
    // csv.parse({[1]="a,b\n1,2", header=true})
    let s = sb();
    let r = s
        .exec(r#"local data = csv.parse({[1]="a,b\n1,2", header=true}); return data[1].a"#)
        .unwrap();
    assert_eq!(r, "1");
}

#[test]
fn parse_table_form_short_aliases() {
    // csv.parse({[1]="a;b\n1;2", d=";", h=true}) — short aliases
    let s = sb();
    let r = s
        .exec(r#"local data = csv.parse({[1]="a;b\n1;2", d=";", h=true}); return data[1].a"#)
        .unwrap();
    assert_eq!(r, "1");
}

#[test]
fn stringify_table_form() {
    // csv.stringify({[1]={{1,"a"},{2,"b"}}}) — named-params form
    let s = sb();
    let r = s
        .exec(
            r#"
            local rows = {{[1]="1",[2]="a"},{[1]="2",[2]="b"}}
            return csv.stringify({[1]=rows})
        "#,
        )
        .unwrap();
    assert!(r.contains("1,a") && r.contains("2,b"), "got: {}", r);
}

// ── Shell transpiler ────────────────────────────────────────────

#[test]
fn shell_csv_parse_dispatch() {
    // `csv parse "a,b"` should dispatch via sh.run()
    let result = sh_transpile::transpile_sh(r#"csv parse "a,b""#);
    assert!(result.is_ok());
    let luau = result.unwrap().luau_source;
    assert!(luau.contains("sh.run(\"csv\", \"parse\""), "got: {}", luau);
}

#[test]
fn shell_csv_help() {
    // `csv` with no args should show help (module dispatch)
    let s = sb();
    let result = sh_transpile::transpile_sh("csv");
    assert!(result.is_ok());
    let luau = result.unwrap().luau_source;
    let shrt = include_str!("../../shrt.luau");
    s.register_module("shrt", shrt).unwrap();
    let r = s.exec(&luau).unwrap();
    assert!(r.contains("csv"), "should show csv help: {}", r);
}

#[test]
fn shell_csv_parse_roundtrip() {
    // `csv parse "a,b\n1,2"` should parse and display JSON via sh.run()
    let s = sb();
    let shrt = include_str!("../../shrt.luau");
    s.register_module("shrt", shrt).unwrap();
    let result = sh_transpile::transpile_sh(r#"csv parse "a,b\n1,2""#);
    assert!(result.is_ok());
    let luau = result.unwrap().luau_source;
    let r = s.exec(&luau).unwrap();
    // sh.run() auto-serializes to JSON; result should contain parsed data
    assert!(r.contains("a") && r.contains("b"), "should display parsed csv, got: {}", r);
}

    #[test]
fn shell_csv_parse_header_roundtrip() {
    // Test with actual multiline CSV content via Lua directly
    // (shell strings with \n produce literal backslash-n in the transpiler)
    let s = sb();
    let shrt = include_str!("../../shrt.luau");
    s.register_module("shrt", shrt).unwrap();
    // Use the table-form directly to test header flag
    let r = s.exec(r#"
        local data = csv.parse({[1]="a,b\n1,2", header=true})
        return json.encode(data, {pretty=true})
    "#).unwrap();
    assert!(r.contains("\"a\"") && r.contains("\"1\""), "should parse with headers, got: {}", r);
}

#[test]
fn shell_csv_parse_with_delimiter_flag() {
    // Test delimiter + header via table-form directly
    let s = sb();
    let r = s.exec(r#"
        local data = csv.parse({[1]="a;b\n1;2", d=";", h=true})
        return data[1].a .. "," .. data[1].b
    "#).unwrap();
    assert_eq!(r, "1,2");
}

#[test]
fn shell_csv_parse_transpile_flags() {
    // Verify the shell transpiler correctly passes --delimiter and --header as flags
    let result = sh_transpile::transpile_sh(r#"csv parse "data" --delimiter ";" --header"#);
    assert!(result.is_ok());
    let luau = result.unwrap().luau_source;
    assert!(luau.contains("delimiter=\";\""), "should have delimiter flag, got: {}", luau);
    assert!(luau.contains("header=true"), "should have header flag, got: {}", luau);
}
