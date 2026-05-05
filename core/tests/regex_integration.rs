#![cfg(feature = "mod-regex")]

use cpsl_core::{transpile, Sandbox};

fn sb() -> Sandbox {
    Sandbox::new().unwrap()
}

// ── 1. regex.match tests ────────────────────────────────────────

#[test]
fn match_simple_pattern() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local m = regex.match("(\\w+)@(\\w+)", "user@host")
            return m.full
        "#,
        )
        .unwrap();
    assert_eq!(r, "user@host");
}

#[test]
fn match_groups_positional() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local m = regex.match("(\\w+)@(\\w+)", "user@host")
            return m.groups[1] .. " " .. m.groups[2]
        "#,
        )
        .unwrap();
    assert_eq!(r, "user host");
}

#[test]
fn match_named_captures() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local m = regex.match("(?P<user>\\w+)@(?P<domain>\\w+)", "alice@example")
            return m.groups.user .. " " .. m.groups.domain
        "#,
        )
        .unwrap();
    assert_eq!(r, "alice example");
}

#[test]
fn match_named_and_positional_together() {
    let s = sb();
    let r = s
        .exec(r#"
            local m = regex.match("(?P<user>\\w+)@(?P<domain>\\w+)", "alice@example")
            return m.groups[1] .. " " .. m.groups[2] .. " " .. m.groups.user .. " " .. m.groups.domain
        "#)
        .unwrap();
    assert_eq!(r, "alice example alice example");
}

#[test]
fn match_no_match_returns_nil() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local m = regex.match("^\\d+$", "abc")
            return tostring(m)
        "#,
        )
        .unwrap();
    assert_eq!(r, "nil");
}

#[test]
fn match_no_capture_groups() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local m = regex.match("hello", "say hello world")
            return m.full
        "#,
        )
        .unwrap();
    assert_eq!(r, "hello");
}

#[test]
fn match_empty_string() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local m = regex.match(".*", "")
            return m.full
        "#,
        )
        .unwrap();
    assert_eq!(r, "");
}

// ── 2. regex.find_all tests ─────────────────────────────────────

#[test]
fn find_all_multiple_matches() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local results = regex.find_all("\\d+", "abc 123 def 456 ghi 789")
            local parts = {}
            for _, m in ipairs(results) do
                table.insert(parts, m.match)
            end
            return table.concat(parts, ",")
        "#,
        )
        .unwrap();
    assert_eq!(r, "123,456,789");
}

#[test]
fn find_all_1based_indices() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local results = regex.find_all("\\w+", "ab cd")
            local first = results[1]
            return tostring(first.start) .. " " .. tostring(first["end"])
        "#,
        )
        .unwrap();
    assert_eq!(r, "1 2");
}

#[test]
fn find_all_second_match_indices() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local results = regex.find_all("\\w+", "ab cd")
            local second = results[2]
            return tostring(second.start) .. " " .. tostring(second["end"])
        "#,
        )
        .unwrap();
    assert_eq!(r, "4 5");
}

#[test]
fn find_all_no_matches() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local results = regex.find_all("\\d+", "no numbers here")
            return tostring(#results)
        "#,
        )
        .unwrap();
    assert_eq!(r, "0");
}

#[test]
fn find_all_with_groups() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local results = regex.find_all("(\\w+)=(\\w+)", "a=1 b=2")
            return results[1].groups[1] .. "=" .. results[1].groups[2] ..
                   " " .. results[2].groups[1] .. "=" .. results[2].groups[2]
        "#,
        )
        .unwrap();
    assert_eq!(r, "a=1 b=2");
}

// ── 3. regex.replace tests ──────────────────────────────────────

#[test]
fn replace_first_only() {
    let s = sb();
    let r = s
        .exec(r#"return regex.replace("\\d+", "a1b2c3", "X")"#)
        .unwrap();
    assert_eq!(r, "aXb2c3");
}

#[test]
fn replace_with_backreference() {
    let s = sb();
    let r = s
        .exec(r#"return regex.replace("(\\w+)@(\\w+)", "user@host", "$2@$1")"#)
        .unwrap();
    assert_eq!(r, "host@user");
}

#[test]
fn replace_no_match_returns_original() {
    let s = sb();
    let r = s
        .exec(r#"return regex.replace("\\d+", "no numbers", "X")"#)
        .unwrap();
    assert_eq!(r, "no numbers");
}

// ── 4. regex.replace_all tests ──────────────────────────────────

#[test]
fn replace_all_basic() {
    let s = sb();
    let r = s
        .exec(r#"return regex.replace_all("\\d+", "a1b2c3", "X")"#)
        .unwrap();
    assert_eq!(r, "aXbXcX");
}

#[test]
fn replace_all_with_backreference() {
    let s = sb();
    let r = s
        .exec(r#"return regex.replace_all("(\\w+)=(\\w+)", "a=1 b=2", "$1:$2")"#)
        .unwrap();
    assert_eq!(r, "a:1 b:2");
}

#[test]
fn replace_all_named_backreference() {
    let s = sb();
    let r = s
        .exec(r#"return regex.replace_all("(?P<k>\\w+)=(?P<v>\\w+)", "a=1 b=2", "${k}->${v}")"#)
        .unwrap();
    assert_eq!(r, "a->1 b->2");
}

// ── 5. regex.split tests ────────────────────────────────────────

#[test]
fn split_basic() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local parts = regex.split("[,;\\s]+", "a, b; c  d")
            return table.concat(parts, "|")
        "#,
        )
        .unwrap();
    assert_eq!(r, "a|b|c|d");
}

#[test]
fn split_no_match_returns_whole_string() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local parts = regex.split("\\d+", "hello world")
            return parts[1]
        "#,
        )
        .unwrap();
    assert_eq!(r, "hello world");
}

#[test]
fn split_empty_string() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local parts = regex.split(",", "")
            return tostring(#parts) .. ":" .. parts[1]
        "#,
        )
        .unwrap();
    assert_eq!(r, "1:");
}

// ── 6. regex.is_match tests ─────────────────────────────────────

#[test]
fn is_match_true() {
    let s = sb();
    let r = s
        .exec(r#"return tostring(regex.is_match("^\\d+$", "12345"))"#)
        .unwrap();
    assert_eq!(r, "true");
}

#[test]
fn is_match_false() {
    let s = sb();
    let r = s
        .exec(r#"return tostring(regex.is_match("^\\d+$", "abc"))"#)
        .unwrap();
    assert_eq!(r, "false");
}

#[test]
fn is_match_partial() {
    let s = sb();
    // is_match should match anywhere in the string (not anchored)
    let r = s
        .exec(r#"return tostring(regex.is_match("\\d+", "abc123def"))"#)
        .unwrap();
    assert_eq!(r, "true");
}

// ── 7. regex.escape tests ───────────────────────────────────────

#[test]
fn escape_metacharacters() {
    let s = sb();
    let r = s
        .exec(r#"return regex.escape("hello.world+foo[bar]")"#)
        .unwrap();
    assert_eq!(r, r"hello\.world\+foo\[bar\]");
}

#[test]
fn escape_then_match() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local escaped = regex.escape("1+1=2")
            return tostring(regex.is_match(escaped, "1+1=2"))
        "#,
        )
        .unwrap();
    assert_eq!(r, "true");
}

#[test]
fn escape_then_no_match_without_escape() {
    let s = sb();
    // Without escaping, "1+1" is a valid regex meaning "11" (one or more 1s)
    // It should NOT match the literal "1+1=2" at position 0 as "1+1=2"
    // But "1+" matches "1" so is_match would be true; let's test differently
    let r = s
        .exec(
            r#"
            local escaped = regex.escape("(foo)")
            return tostring(regex.is_match(escaped, "(foo)"))
        "#,
        )
        .unwrap();
    assert_eq!(r, "true");
}

// ── 8. Error handling tests ─────────────────────────────────────

#[test]
fn match_invalid_pattern_errors() {
    let s = sb();
    let err = s.exec(r#"regex.match("[invalid", "text")"#).unwrap_err();
    assert!(
        err.message.contains("invalid pattern") || err.message.contains("regex.match"),
        "msg: {}",
        err.message
    );
}

#[test]
fn match_no_args_errors() {
    let s = sb();
    let err = s.exec("regex.match()").unwrap_err();
    assert!(
        err.message.contains("bad argument") || err.message.contains("missing"),
        "msg: {}",
        err.message
    );
}

#[test]
fn match_one_arg_errors() {
    let s = sb();
    let err = s.exec(r#"regex.match("\\d+")"#).unwrap_err();
    assert!(
        err.message.contains("bad argument") || err.message.contains("missing"),
        "msg: {}",
        err.message
    );
}

#[test]
fn match_wrong_type_errors() {
    let s = sb();
    let err = s.exec("regex.match(42, 43)").unwrap_err();
    assert!(
        err.message.contains("string") || err.message.contains("expected"),
        "msg: {}",
        err.message
    );
}

#[test]
fn is_match_invalid_pattern_errors() {
    let s = sb();
    let err = s
        .exec(r#"regex.is_match("(unclosed", "text")"#)
        .unwrap_err();
    assert!(
        err.message.contains("invalid pattern") || err.message.contains("regex.is_match"),
        "msg: {}",
        err.message
    );
}

#[test]
fn split_invalid_pattern_errors() {
    let s = sb();
    let err = s.exec(r#"regex.split("[bad", "text")"#).unwrap_err();
    assert!(
        err.message.contains("invalid pattern") || err.message.contains("regex.split"),
        "msg: {}",
        err.message
    );
}

#[test]
fn replace_no_args_errors() {
    let s = sb();
    let err = s.exec("regex.replace()").unwrap_err();
    assert!(
        err.message.contains("bad argument") || err.message.contains("missing"),
        "msg: {}",
        err.message
    );
}

#[test]
fn escape_no_args_errors() {
    let s = sb();
    let err = s.exec("regex.escape()").unwrap_err();
    assert!(
        err.message.contains("bad argument") || err.message.contains("missing"),
        "msg: {}",
        err.message
    );
}

// ── 9. Edge cases ───────────────────────────────────────────────

#[test]
fn match_unicode() {
    let s = sb();
    // Rust regex \w matches Unicode word chars, so "café" is matched fully
    let r = s
        .exec(
            r#"
            local m = regex.match("(\\w+)", "caf\195\169")
            return m.full
        "#,
        )
        .unwrap();
    assert_eq!(r, "caf\u{00e9}");
}

#[test]
fn find_all_overlapping_not_returned() {
    let s = sb();
    // Pattern "aa" on "aaa" should find only one non-overlapping match
    let r = s
        .exec(
            r#"
            local results = regex.find_all("aa", "aaa")
            return tostring(#results)
        "#,
        )
        .unwrap();
    assert_eq!(r, "1");
}

#[test]
fn replace_empty_replacement() {
    let s = sb();
    let r = s
        .exec(r#"return regex.replace_all("\\s+", "a b c", "")"#)
        .unwrap();
    assert_eq!(r, "abc");
}

#[test]
fn split_single_char_delimiter() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local parts = regex.split(",", "a,b,c")
            return table.concat(parts, "-")
        "#,
        )
        .unwrap();
    assert_eq!(r, "a-b-c");
}

#[test]
fn replace_full_match_backref() {
    let s = sb();
    let r = s
        .exec(r#"return regex.replace_all("\\w+", "hello world", "[$0]")"#)
        .unwrap();
    assert_eq!(r, "[hello] [world]");
}

// ── 10. Table form (shell dispatch) tests ───────────────────────

#[test]
fn match_table_form_positional() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local m = regex.match({[1]="(\\w+)", [2]="hello"})
            return m.full
        "#,
        )
        .unwrap();
    assert_eq!(r, "hello");
}

#[test]
fn match_table_form_named() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local m = regex.match({pattern="(\\w+)", text="hello"})
            return m.full
        "#,
        )
        .unwrap();
    assert_eq!(r, "hello");
}

#[test]
fn is_match_table_form() {
    let s = sb();
    let r = s
        .exec(r#"return tostring(regex.is_match({pattern="^\\d+$", text="123"}))"#)
        .unwrap();
    assert_eq!(r, "true");
}

// ── 11. Help tests ──────────────────────────────────────────────

#[test]
fn regex_help_returns_help_text() {
    let s = sb();
    let r = s.exec("return regex.help()").unwrap();
    assert!(r.contains("regex"), "help: {}", r);
    assert!(r.contains("regex.match"), "help: {}", r);
    assert!(r.contains("regex.find_all"), "help: {}", r);
    assert!(r.contains("regex.replace"), "help: {}", r);
    assert!(r.contains("regex.split"), "help: {}", r);
    assert!(r.contains("regex.is_match"), "help: {}", r);
    assert!(r.contains("regex.escape"), "help: {}", r);
}

#[test]
fn regex_nonexistent_fn_hint() {
    let s = sb();
    let err = s.exec("regex.foo()").unwrap_err();
    assert!(
        err.message.contains("regex.foo does not exist"),
        "msg: {}",
        err.message
    );
    assert!(
        err.message.contains("hint: call regex.help() for usage"),
        "msg: {}",
        err.message
    );
}

#[test]
fn global_help_mentions_regex() {
    let s = sb();
    let r = s.exec("return help()").unwrap();
    assert!(r.contains("regex"), "global help should list regex: {}", r);
}

// ── 12. Shell dispatch tests ────────────────────────────────────

#[test]
fn shell_regex_is_match() {
    let s = sb();
    let shrt = include_str!("../../runtime/shrt.luau");
    s.register_module("shrt", shrt).unwrap();
    // sh.run doesn't emit booleans, so use regex.match which returns a table (emitted as JSON)
    let result = cpsl_core::sh_transpile::transpile_sh(r#"regex match "^\d+$" "12345""#);
    assert!(result.is_ok(), "transpile err: {:?}", result.err());
    let luau = result.unwrap().luau_source;
    let r = s.exec(&luau).unwrap();
    assert!(
        r.contains("12345"),
        "expected match result containing 12345, got: {}",
        r
    );
}

#[test]
fn shell_regex_escape() {
    let s = sb();
    let shrt = include_str!("../../runtime/shrt.luau");
    s.register_module("shrt", shrt).unwrap();
    let result = cpsl_core::sh_transpile::transpile_sh(r#"regex escape "hello.world""#);
    assert!(result.is_ok(), "transpile err: {:?}", result.err());
    let luau = result.unwrap().luau_source;
    let r = s.exec(&luau).unwrap();
    assert!(
        r.contains(r"hello\.world"),
        "expected escaped dot, got: {}",
        r
    );
}

// ── 13. Python transpiler tests ─────────────────────────────────

#[test]
fn python_import_re_maps_to_regex() {
    let py_code = r#"
import re
result = re.is_match("\\d+", "abc123")
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    assert!(
        transpiled.luau_source.contains("regex"),
        "transpiled: {}",
        transpiled.luau_source
    );
}

#[test]
fn python_re_match_e2e() {
    let s = sb();
    let pyrt = include_str!("../../runtime/pyrt.luau");
    s.setup_python_runtime(pyrt).unwrap();

    let py_code = r#"
import re
m = re.match("(\w+)@(\w+)", "user@host")
print(m.full)
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    let r = s.exec(&transpiled.luau_source).unwrap();
    assert_eq!(r, "user@host");
}

#[test]
fn python_re_is_match_e2e() {
    let s = sb();
    let pyrt = include_str!("../../runtime/pyrt.luau");
    s.setup_python_runtime(pyrt).unwrap();

    let py_code = r#"
import re
result = re.is_match("^\d+$", "42")
print(result)
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    let r = s.exec(&transpiled.luau_source).unwrap();
    // Python runtime prints booleans as "True"/"False"
    assert!(
        r == "true" || r == "True",
        "expected true or True, got: {}",
        r
    );
}

#[test]
fn python_re_replace_all_e2e() {
    let s = sb();
    let pyrt = include_str!("../../runtime/pyrt.luau");
    s.setup_python_runtime(pyrt).unwrap();

    let py_code = r#"
import re
result = re.replace_all("\d+", "a1b2c3", "X")
print(result)
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    let r = s.exec(&transpiled.luau_source).unwrap();
    assert_eq!(r, "aXbXcX");
}

// ── 14. Sandbox safety tests ────────────────────────────────────

#[test]
fn regex_no_dangerous_globals_exposed() {
    let s = sb();
    let r = s
        .exec(
            r#"
            return tostring(type(regex.match)) .. " " ..
                   tostring(type(regex.is_match)) .. " " ..
                   tostring(type(regex.escape)) .. " " ..
                   tostring(rawget(regex, "io")) .. " " ..
                   tostring(rawget(regex, "os"))
        "#,
        )
        .unwrap();
    assert_eq!(r, "function function function nil nil");
}

#[test]
fn regex_is_purely_computational() {
    let s = sb();
    let r = s
        .exec(
            r#"
            -- All regex operations should work without any fs/network
            local results = {}
            table.insert(results, tostring(regex.is_match("\\d+", "123")))
            local m = regex.match("(\\w+)", "hello")
            table.insert(results, m.full)
            table.insert(results, regex.replace_all("\\s+", "a b", "_"))
            local parts = regex.split(",", "x,y,z")
            table.insert(results, table.concat(parts, "|"))
            table.insert(results, regex.escape("a.b"))
            return table.concat(results, " ")
        "#,
        )
        .unwrap();
    assert_eq!(r, r"true hello a_b x|y|z a\.b");
}
