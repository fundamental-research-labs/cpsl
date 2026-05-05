//! Tests for Python-to-Luau transpilation edge cases.

use super::*;

fn transpile_py(source: &str) -> String {
    transpile(source).unwrap().luau_source
}

#[test]
fn kwargs_module_call_only_kwargs() {
    // sh.ls(long=True) → sh.ls({long = true})
    let result = transpile_py("sh.ls(long=True)");
    assert!(result.contains("sh.ls({long = true})"), "got: {}", result);
}

#[test]
fn kwargs_module_call_mixed() {
    // sh.head("/file", n=5) → sh.head("/file", {n = 5})
    // Positional args stay positional, kwargs become a trailing opts table
    let result = transpile_py(r#"sh.head("/file", n=5)"#);
    assert!(
        result.contains(r#"sh.head("/file", {n = 5})"#),
        "got: {}",
        result
    );
}

#[test]
fn kwargs_module_call_multiple_kwargs() {
    // sh.ls(long=True, all=True) → sh.ls({long = true, all = true})
    let result = transpile_py("sh.ls(long=True, all=True)");
    assert!(
        result.contains("sh.ls({long = true, all = true})"),
        "got: {}",
        result
    );
}

#[test]
fn kwargs_module_call_no_kwargs_unchanged() {
    // fs.read("/path") → fs.read("/path") (no change)
    let result = transpile_py(r#"fs.read("/path")"#);
    assert!(result.contains(r#"fs.read("/path")"#), "got: {}", result);
}

#[test]
fn kwargs_module_call_bool_false() {
    // sh.sort(reverse=False) → sh.sort({reverse = false})
    let result = transpile_py("sh.sort(reverse=False)");
    assert!(
        result.contains("sh.sort({reverse = false})"),
        "got: {}",
        result
    );
}

#[test]
fn kwargs_fs_module_with_kwargs() {
    // fs.list("/workspace", recursive=True) → fs.list("/workspace", {recursive = true})
    // Positional args stay positional, kwargs become a trailing opts table
    let result = transpile_py(r#"fs.list("/workspace", recursive=True)"#);
    assert!(
        result.contains(r#"fs.list("/workspace", {recursive = true})"#),
        "got: {}",
        result
    );
}

#[test]
fn kwargs_string_value() {
    // doc.renderFile(source="/in.md", target="/out.pdf")
    let result = transpile_py(r#"doc.renderFile(source="/in.md", target="/out.pdf")"#);
    assert!(
        result.contains(r#"doc.renderFile({source = "/in.md", target = "/out.pdf"})"#),
        "got: {}",
        result
    );
}

#[test]
fn kwargs_positional_and_multiple_kwargs() {
    // sh.grep("pattern", i=True, v=True) → sh.grep("pattern", {i = true, v = true})
    let result = transpile_py(r#"sh.grep("pattern", i=True, v=True)"#);
    assert!(
        result.contains(r#"sh.grep("pattern", {i = true, v = true})"#),
        "got: {}",
        result
    );
}

#[test]
fn fstring_escapes_newline() {
    // f"line {i}\n" should escape the newline in the Luau format string
    let result = transpile_py(r#"x = f"line {i}\n""#);
    assert!(
        result.contains(r#"string.format("line %s\n""#),
        "should escape newline in f-string, got: {}",
        result
    );
    // Must NOT contain a literal newline inside the string
    assert!(
        !result.contains("\"line \n"),
        "should not have raw newline in string literal, got: {}",
        result
    );
}

#[test]
fn listcomp_statement_semicolon_prefix() {
    // Statement-level list comp produces an IIFE starting with `(`.
    // Luau needs a `;` prefix to avoid "ambiguous syntax" error.
    let result = transpile_py(r#"[print(i) for i in range(1, 4)]"#);
    assert!(
        result.contains(";(function()"),
        "should prefix IIFE with semicolon, got: {}",
        result
    );
}
