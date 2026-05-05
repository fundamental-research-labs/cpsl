#![cfg(feature = "mod-url")]

use cpsl_core::{transpile, Sandbox};

fn sb() -> Sandbox {
    Sandbox::new().unwrap()
}

// ── 1. url.parse tests ─────────────────────────────────────────

#[test]
fn parse_https_url() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local u = url.parse("https://example.com/path?q=1#frag")
            return u.scheme .. "|" .. u.host .. "|" .. u.path .. "|" .. u.query .. "|" .. u.fragment
        "#,
        )
        .unwrap();
    assert_eq!(r, "https|example.com|/path|q=1|frag");
}

#[test]
fn parse_http_with_port() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local u = url.parse("http://localhost:8080/api")
            return u.scheme .. "|" .. u.host .. "|" .. tostring(u.port) .. "|" .. u.path
        "#,
        )
        .unwrap();
    assert_eq!(r, "http|localhost|8080|/api");
}

#[test]
fn parse_origin() {
    let s = sb();
    // url crate normalizes default ports (443 for https), so use non-default port
    let r = s
        .exec(
            r#"
            local u = url.parse("https://example.com:8443/path")
            return u.origin
        "#,
        )
        .unwrap();
    assert_eq!(r, "https://example.com:8443");
}

#[test]
fn parse_no_port_nil() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local u = url.parse("https://example.com/path")
            return tostring(u.port)
        "#,
        )
        .unwrap();
    assert_eq!(r, "nil");
}

#[test]
fn parse_no_query_nil() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local u = url.parse("https://example.com/path")
            return tostring(u.query)
        "#,
        )
        .unwrap();
    assert_eq!(r, "nil");
}

#[test]
fn parse_no_fragment_nil() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local u = url.parse("https://example.com/path")
            return tostring(u.fragment)
        "#,
        )
        .unwrap();
    assert_eq!(r, "nil");
}

#[test]
fn parse_invalid_url_returns_nil() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local u = url.parse("not a url")
            return tostring(u)
        "#,
        )
        .unwrap();
    assert_eq!(r, "nil");
}

#[test]
fn parse_ftp_scheme() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local u = url.parse("ftp://files.example.com/pub")
            return u.scheme .. "|" .. u.host
        "#,
        )
        .unwrap();
    assert_eq!(r, "ftp|files.example.com");
}

#[test]
fn parse_empty_path() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local u = url.parse("https://example.com")
            return u.path
        "#,
        )
        .unwrap();
    assert_eq!(r, "/");
}

// ── 2. url.build tests ─────────────────────────────────────────

#[test]
fn build_basic_url() {
    let s = sb();
    let r = s
        .exec(
            r#"
            return url.build({scheme="https", host="example.com", path="/path"})
        "#,
        )
        .unwrap();
    assert_eq!(r, "https://example.com/path");
}

#[test]
fn build_with_port() {
    let s = sb();
    let r = s
        .exec(
            r#"
            return url.build({scheme="http", host="localhost", port=8080, path="/api"})
        "#,
        )
        .unwrap();
    assert_eq!(r, "http://localhost:8080/api");
}

#[test]
fn build_with_query_and_fragment() {
    let s = sb();
    let r = s
        .exec(r#"
            return url.build({scheme="https", host="example.com", path="/search", query="q=hello", fragment="top"})
        "#)
        .unwrap();
    assert_eq!(r, "https://example.com/search?q=hello#top");
}

#[test]
fn build_defaults_to_https() {
    let s = sb();
    let r = s
        .exec(
            r#"
            return url.build({host="example.com"})
        "#,
        )
        .unwrap();
    assert_eq!(r, "https://example.com");
}

// ── 3. url.encode / url.decode tests ───────────────────────────

#[test]
fn encode_spaces() {
    let s = sb();
    let r = s.exec(r#"return url.encode("hello world")"#).unwrap();
    assert_eq!(r, "hello%20world");
}

#[test]
fn encode_special_chars() {
    let s = sb();
    let r = s.exec(r#"return url.encode("a=1&b=2")"#).unwrap();
    assert_eq!(r, "a%3D1%26b%3D2");
}

#[test]
fn encode_unicode() {
    let s = sb();
    let r = s.exec(r#"return url.encode("caf\195\169")"#).unwrap();
    assert_eq!(r, "caf%C3%A9");
}

#[test]
fn encode_unreserved_chars_not_encoded() {
    let s = sb();
    let r = s.exec(r#"return url.encode("a-b_c.d~e")"#).unwrap();
    assert_eq!(r, "a-b_c.d~e");
}

#[test]
fn decode_percent_encoded() {
    let s = sb();
    let r = s.exec(r#"return url.decode("hello%20world")"#).unwrap();
    assert_eq!(r, "hello world");
}

#[test]
fn decode_special_chars() {
    let s = sb();
    let r = s.exec(r#"return url.decode("a%3D1%26b%3D2")"#).unwrap();
    assert_eq!(r, "a=1&b=2");
}

#[test]
fn decode_unicode() {
    let s = sb();
    let r = s.exec(r#"return url.decode("caf%C3%A9")"#).unwrap();
    assert_eq!(r, "caf\u{00e9}");
}

#[test]
fn encode_decode_roundtrip() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local original = "hello world & foo=bar"
            local encoded = url.encode(original)
            local decoded = url.decode(encoded)
            return decoded
        "#,
        )
        .unwrap();
    assert_eq!(r, "hello world & foo=bar");
}

// ── 4. url.query_parse tests ───────────────────────────────────

#[test]
fn query_parse_basic() {
    let s = sb();
    let r = s
        .exec(r#"
            local pairs = url.query_parse("a=1&b=2")
            return pairs[1].key .. "=" .. pairs[1].value .. "&" .. pairs[2].key .. "=" .. pairs[2].value
        "#)
        .unwrap();
    assert_eq!(r, "a=1&b=2");
}

#[test]
fn query_parse_with_leading_question_mark() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local pairs = url.query_parse("?x=10&y=20")
            return pairs[1].key .. "=" .. pairs[1].value
        "#,
        )
        .unwrap();
    assert_eq!(r, "x=10");
}

#[test]
fn query_parse_encoded_values() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local pairs = url.query_parse("name=hello%20world&q=a%26b")
            return pairs[1].value .. "|" .. pairs[2].value
        "#,
        )
        .unwrap();
    assert_eq!(r, "hello world|a&b");
}

#[test]
fn query_parse_empty_value() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local pairs = url.query_parse("key=")
            return pairs[1].key .. "|" .. pairs[1].value
        "#,
        )
        .unwrap();
    assert_eq!(r, "key|");
}

#[test]
fn query_parse_no_value() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local pairs = url.query_parse("flag")
            return pairs[1].key .. "|" .. pairs[1].value
        "#,
        )
        .unwrap();
    assert_eq!(r, "flag|");
}

#[test]
fn query_parse_empty_string() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local pairs = url.query_parse("")
            return tostring(#pairs)
        "#,
        )
        .unwrap();
    assert_eq!(r, "0");
}

// ── 5. url.query_build tests ───────────────────────────────────

#[test]
fn query_build_from_dict() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local qs = url.query_build({a="1", b="2"})
            return qs
        "#,
        )
        .unwrap();
    // Dict form: sorted by key
    assert_eq!(r, "a=1&b=2");
}

#[test]
fn query_build_from_list_of_pairs() {
    let s = sb();
    // Use named table form to avoid table-form ambiguity with single-table params
    let r = s
        .exec(
            r#"
            local pairs = {{key="x", value="10"}, {key="y", value="20"}}
            local qs = url.query_build({params=pairs})
            return qs
        "#,
        )
        .unwrap();
    // List of pairs: sorted by key
    assert_eq!(r, "x=10&y=20");
}

#[test]
fn query_build_encodes_special_chars() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local qs = url.query_build({q="hello world"})
            return qs
        "#,
        )
        .unwrap();
    assert_eq!(r, "q=hello%20world");
}

// ── 6. url.join tests ──────────────────────────────────────────

#[test]
fn join_relative_path() {
    let s = sb();
    let r = s
        .exec(
            r#"
            return url.join("https://example.com/a/b", "../c")
        "#,
        )
        .unwrap();
    assert_eq!(r, "https://example.com/c");
}

#[test]
fn join_absolute_path() {
    let s = sb();
    let r = s
        .exec(
            r#"
            return url.join("https://example.com/a/b", "/d")
        "#,
        )
        .unwrap();
    assert_eq!(r, "https://example.com/d");
}

#[test]
fn join_full_url_override() {
    let s = sb();
    let r = s
        .exec(
            r#"
            return url.join("https://example.com/a", "https://other.com/b")
        "#,
        )
        .unwrap();
    assert_eq!(r, "https://other.com/b");
}

#[test]
fn join_with_query() {
    let s = sb();
    let r = s
        .exec(
            r#"
            return url.join("https://example.com/base/", "page?q=1")
        "#,
        )
        .unwrap();
    assert_eq!(r, "https://example.com/base/page?q=1");
}

#[test]
fn join_invalid_base_errors() {
    let s = sb();
    let err = s.exec(r#"url.join("not-a-url", "/path")"#).unwrap_err();
    assert!(
        err.message.contains("url.join") || err.message.contains("invalid"),
        "msg: {}",
        err.message
    );
}

// ── 7. Error handling tests ────────────────────────────────────

#[test]
fn parse_no_args_errors() {
    let s = sb();
    let err = s.exec("url.parse()").unwrap_err();
    assert!(
        err.message.contains("bad argument") || err.message.contains("missing"),
        "msg: {}",
        err.message
    );
}

#[test]
fn encode_no_args_errors() {
    let s = sb();
    let err = s.exec("url.encode()").unwrap_err();
    assert!(
        err.message.contains("bad argument") || err.message.contains("missing"),
        "msg: {}",
        err.message
    );
}

#[test]
fn decode_no_args_errors() {
    let s = sb();
    let err = s.exec("url.decode()").unwrap_err();
    assert!(
        err.message.contains("bad argument") || err.message.contains("missing"),
        "msg: {}",
        err.message
    );
}

#[test]
fn build_no_args_errors() {
    let s = sb();
    let err = s.exec("url.build()").unwrap_err();
    assert!(
        err.message.contains("bad argument") || err.message.contains("missing"),
        "msg: {}",
        err.message
    );
}

#[test]
fn join_no_args_errors() {
    let s = sb();
    let err = s.exec("url.join()").unwrap_err();
    assert!(
        err.message.contains("bad argument") || err.message.contains("missing"),
        "msg: {}",
        err.message
    );
}

#[test]
fn parse_wrong_type_errors() {
    let s = sb();
    let err = s.exec("url.parse(42)").unwrap_err();
    assert!(
        err.message.contains("string") || err.message.contains("expected"),
        "msg: {}",
        err.message
    );
}

#[test]
fn encode_wrong_type_errors() {
    let s = sb();
    let err = s.exec("url.encode(42)").unwrap_err();
    assert!(
        err.message.contains("string") || err.message.contains("expected"),
        "msg: {}",
        err.message
    );
}

// ── 8. Table form (shell dispatch) tests ───────────────────────

#[test]
fn parse_table_form_positional() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local u = url.parse({[1]="https://example.com/path"})
            return u.host
        "#,
        )
        .unwrap();
    assert_eq!(r, "example.com");
}

#[test]
fn parse_table_form_named() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local u = url.parse({str="https://example.com/path"})
            return u.host
        "#,
        )
        .unwrap();
    assert_eq!(r, "example.com");
}

#[test]
fn join_table_form() {
    let s = sb();
    let r = s
        .exec(
            r#"
            return url.join({base="https://example.com/a/", relative="../b"})
        "#,
        )
        .unwrap();
    assert_eq!(r, "https://example.com/b");
}

// ── 9. Help tests ──────────────────────────────────────────────

#[test]
fn url_help_returns_help_text() {
    let s = sb();
    let r = s.exec("return url.help()").unwrap();
    assert!(r.contains("url"), "help: {}", r);
    assert!(r.contains("url.parse"), "help: {}", r);
    assert!(r.contains("url.build"), "help: {}", r);
    assert!(r.contains("url.encode"), "help: {}", r);
    assert!(r.contains("url.decode"), "help: {}", r);
    assert!(r.contains("url.query_parse"), "help: {}", r);
    assert!(r.contains("url.query_build"), "help: {}", r);
    assert!(r.contains("url.join"), "help: {}", r);
}

#[test]
fn url_nonexistent_fn_hint() {
    let s = sb();
    let err = s.exec("url.foo()").unwrap_err();
    assert!(
        err.message.contains("url.foo does not exist"),
        "msg: {}",
        err.message
    );
    assert!(
        err.message.contains("hint: call url.help() for usage"),
        "msg: {}",
        err.message
    );
}

#[test]
fn global_help_mentions_url() {
    let s = sb();
    let r = s.exec("return help()").unwrap();
    assert!(r.contains("url"), "global help should list url: {}", r);
}

// ── 10. Shell dispatch tests ───────────────────────────────────

#[test]
fn shell_url_parse() {
    let s = sb();
    let shrt = include_str!("../../runtime/shrt.luau");
    s.register_module("shrt", shrt).unwrap();
    let result =
        cpsl_core::sh_transpile::transpile_sh(r#"url parse "https://example.com/path?q=1#frag""#);
    assert!(result.is_ok(), "transpile err: {:?}", result.err());
    let luau = result.unwrap().luau_source;
    let r = s.exec(&luau).unwrap();
    assert!(
        r.contains("example.com"),
        "expected parse result containing example.com, got: {}",
        r
    );
}

#[test]
fn shell_url_encode() {
    let s = sb();
    let shrt = include_str!("../../runtime/shrt.luau");
    s.register_module("shrt", shrt).unwrap();
    let result = cpsl_core::sh_transpile::transpile_sh(r#"url encode "hello world""#);
    assert!(result.is_ok(), "transpile err: {:?}", result.err());
    let luau = result.unwrap().luau_source;
    let r = s.exec(&luau).unwrap();
    assert!(
        r.contains("hello%20world"),
        "expected encoded string, got: {}",
        r
    );
}

#[test]
fn shell_url_decode() {
    let s = sb();
    let shrt = include_str!("../../runtime/shrt.luau");
    s.register_module("shrt", shrt).unwrap();
    let result = cpsl_core::sh_transpile::transpile_sh(r#"url decode "hello%20world""#);
    assert!(result.is_ok(), "transpile err: {:?}", result.err());
    let luau = result.unwrap().luau_source;
    let r = s.exec(&luau).unwrap();
    assert!(
        r.contains("hello world"),
        "expected decoded string, got: {}",
        r
    );
}

#[test]
fn shell_url_join() {
    let s = sb();
    let shrt = include_str!("../../runtime/shrt.luau");
    s.register_module("shrt", shrt).unwrap();
    let result =
        cpsl_core::sh_transpile::transpile_sh(r#"url join "https://example.com/a/" "../b""#);
    assert!(result.is_ok(), "transpile err: {:?}", result.err());
    let luau = result.unwrap().luau_source;
    let r = s.exec(&luau).unwrap();
    assert!(
        r.contains("example.com/b"),
        "expected joined URL, got: {}",
        r
    );
}

#[test]
fn shell_url_query_parse() {
    let s = sb();
    let shrt = include_str!("../../runtime/shrt.luau");
    s.register_module("shrt", shrt).unwrap();
    let result = cpsl_core::sh_transpile::transpile_sh(r#"url query_parse "a=1&b=2""#);
    assert!(result.is_ok(), "transpile err: {:?}", result.err());
    let luau = result.unwrap().luau_source;
    let r = s.exec(&luau).unwrap();
    assert!(
        r.contains("a") && r.contains("1"),
        "expected query_parse result, got: {}",
        r
    );
}

// ── 11. Python transpiler tests ────────────────────────────────

#[test]
fn python_import_urllib_maps_to_url() {
    let py_code = r#"
import urllib
result = urllib.parse("https://example.com")
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    assert!(
        transpiled.luau_source.contains("url"),
        "transpiled: {}",
        transpiled.luau_source
    );
}

#[test]
fn python_from_urllib_parse_import_urlparse() {
    let py_code = r#"
from urllib.parse import urlparse
result = urlparse("https://example.com/path")
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    assert!(
        transpiled.luau_source.contains("url.parse"),
        "transpiled: {}",
        transpiled.luau_source
    );
}

#[test]
fn python_from_urllib_parse_import_quote() {
    let py_code = r#"
from urllib.parse import quote
result = quote("hello world")
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    assert!(
        transpiled.luau_source.contains("url.encode"),
        "transpiled: {}",
        transpiled.luau_source
    );
}

#[test]
fn python_from_urllib_parse_import_unquote() {
    let py_code = r#"
from urllib.parse import unquote
result = unquote("hello%20world")
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    assert!(
        transpiled.luau_source.contains("url.decode"),
        "transpiled: {}",
        transpiled.luau_source
    );
}

#[test]
fn python_from_urllib_parse_import_urlencode() {
    let py_code = r#"
from urllib.parse import urlencode
result = urlencode({"a": "1"})
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    assert!(
        transpiled.luau_source.contains("url.query_build"),
        "transpiled: {}",
        transpiled.luau_source
    );
}

#[test]
fn python_from_urllib_parse_import_parse_qs() {
    let py_code = r#"
from urllib.parse import parse_qs
result = parse_qs("a=1&b=2")
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    assert!(
        transpiled.luau_source.contains("url.query_parse"),
        "transpiled: {}",
        transpiled.luau_source
    );
}

#[test]
fn python_from_urllib_parse_import_urljoin() {
    let py_code = r#"
from urllib.parse import urljoin
result = urljoin("https://example.com/a/", "../b")
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    assert!(
        transpiled.luau_source.contains("url.join"),
        "transpiled: {}",
        transpiled.luau_source
    );
}

#[test]
fn python_urlparse_e2e() {
    let s = sb();
    let pyrt = include_str!("../../runtime/pyrt.luau");
    s.setup_python_runtime(pyrt).unwrap();

    let py_code = r#"
from urllib.parse import urlparse
u = urlparse("https://example.com/path?q=1")
print(u.host)
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    let r = s.exec(&transpiled.luau_source).unwrap();
    assert_eq!(r, "example.com");
}

#[test]
fn python_quote_unquote_e2e() {
    let s = sb();
    let pyrt = include_str!("../../runtime/pyrt.luau");
    s.setup_python_runtime(pyrt).unwrap();

    let py_code = r#"
from urllib.parse import quote, unquote
encoded = quote("hello world")
decoded = unquote(encoded)
print(decoded)
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    let r = s.exec(&transpiled.luau_source).unwrap();
    assert_eq!(r, "hello world");
}

#[test]
fn python_urljoin_e2e() {
    let s = sb();
    let pyrt = include_str!("../../runtime/pyrt.luau");
    s.setup_python_runtime(pyrt).unwrap();

    let py_code = r#"
from urllib.parse import urljoin
result = urljoin("https://example.com/a/b", "../c")
print(result)
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    let r = s.exec(&transpiled.luau_source).unwrap();
    assert_eq!(r, "https://example.com/c");
}

// ── 12. Sandbox safety tests ───────────────────────────────────

#[test]
fn url_no_dangerous_globals_exposed() {
    let s = sb();
    let r = s
        .exec(
            r#"
            return tostring(type(url.parse)) .. " " ..
                   tostring(type(url.encode)) .. " " ..
                   tostring(type(url.join)) .. " " ..
                   tostring(rawget(url, "io")) .. " " ..
                   tostring(rawget(url, "os"))
        "#,
        )
        .unwrap();
    assert_eq!(r, "function function function nil nil");
}

#[test]
fn url_is_purely_computational() {
    let s = sb();
    let r = s
        .exec(
            r#"
            -- All url operations should work without any fs/network
            local results = {}
            local u = url.parse("https://example.com/path?q=1#frag")
            table.insert(results, u.scheme)
            table.insert(results, u.host)
            table.insert(results, url.encode("hello world"))
            table.insert(results, url.decode("hello%20world"))
            table.insert(results, url.join("https://example.com/a/", "../b"))
            return table.concat(results, " ")
        "#,
        )
        .unwrap();
    assert_eq!(
        r,
        "https example.com hello%20world hello world https://example.com/b"
    );
}

// ── 13. Edge cases ─────────────────────────────────────────────

#[test]
fn parse_data_uri() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local u = url.parse("data:text/plain;base64,SGVsbG8=")
            return u.scheme
        "#,
        )
        .unwrap();
    assert_eq!(r, "data");
}

#[test]
fn build_path_without_leading_slash() {
    let s = sb();
    let r = s
        .exec(
            r#"
            return url.build({scheme="https", host="example.com", path="path"})
        "#,
        )
        .unwrap();
    assert_eq!(r, "https://example.com/path");
}

#[test]
fn query_parse_duplicate_keys() {
    let s = sb();
    let r = s
        .exec(r#"
            local pairs = url.query_parse("a=1&a=2&a=3")
            return tostring(#pairs) .. "|" .. pairs[1].value .. "|" .. pairs[2].value .. "|" .. pairs[3].value
        "#)
        .unwrap();
    assert_eq!(r, "3|1|2|3");
}

#[test]
fn encode_empty_string() {
    let s = sb();
    let r = s.exec(r#"return url.encode("")"#).unwrap();
    assert_eq!(r, "");
}

#[test]
fn decode_empty_string() {
    let s = sb();
    let r = s.exec(r#"return url.decode("")"#).unwrap();
    assert_eq!(r, "");
}

#[test]
fn decode_no_percent_encoding() {
    let s = sb();
    let r = s.exec(r#"return url.decode("hello")"#).unwrap();
    assert_eq!(r, "hello");
}
