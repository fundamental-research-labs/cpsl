#![cfg(feature = "mod-edgar")]

use cpsl_core::Sandbox;
use native_http::HttpGateway;
use std::sync::Arc;

fn sb_no_domains() -> Sandbox {
    let gw = Arc::new(HttpGateway::builder().build());
    Sandbox::builder().http_gateway(gw).build().unwrap()
}

fn sb_edgar() -> Sandbox {
    let gw = Arc::new(
        HttpGateway::builder()
            .allow_domain("efts.sec.gov")
            .allow_domain("data.sec.gov")
            .allow_domain("www.sec.gov")
            .build(),
    );
    Sandbox::builder().http_gateway(gw).build().unwrap()
}

// ── Module registration ─────────────────────────────────────────

#[test]
fn edgar_global_exists_with_http() {
    let sb = sb_no_domains();
    let r = sb.exec("return type(edgar)").unwrap();
    assert_eq!(r, "table");
}

#[test]
fn edgar_not_present_without_http() {
    let sb = Sandbox::new().unwrap();
    let r = sb.exec("return type(edgar)").unwrap();
    assert_eq!(r, "nil");
}

#[test]
fn edgar_has_expected_functions() {
    let sb = sb_no_domains();
    let r = sb
        .exec(
            r#"
            return type(edgar.search) .. " " ..
                   type(edgar.filings) .. " " ..
                   type(edgar.company)
        "#,
        )
        .unwrap();
    assert_eq!(r, "function function function");
}

// ── Help ────────────────────────────────────────────────────────

#[test]
fn edgar_help_returns_help_text() {
    let sb = sb_no_domains();
    let r = sb.exec("return edgar.help()").unwrap();
    assert!(r.contains("edgar"), "help: {}", r);
    assert!(r.contains("edgar.search"), "help: {}", r);
    assert!(r.contains("edgar.filings"), "help: {}", r);
    assert!(r.contains("edgar.company"), "help: {}", r);
}

#[test]
fn edgar_help_bare_call() {
    let sb = sb_no_domains();
    let r = sb.exec("edgar.help()").unwrap();
    assert!(r.contains("edgar"), "help: {}", r);
}

#[test]
fn global_help_includes_edgar() {
    let sb = sb_no_domains();
    let r = sb.exec("return help()").unwrap();
    assert!(r.contains("edgar"), "global help should list edgar: {}", r);
}

#[test]
fn edgar_nonexistent_fn_hint() {
    let sb = sb_no_domains();
    let err = sb.exec("edgar.foo()").unwrap_err();
    assert!(
        err.message.contains("edgar.foo does not exist"),
        "msg: {}",
        err.message
    );
    assert!(
        err.message.contains("hint: call edgar.help() for usage"),
        "msg: {}",
        err.message
    );
}

// ── Argument validation — search ─────────────────────────────────

#[test]
fn search_no_args_errors() {
    let sb = sb_no_domains();
    let err = sb.exec("edgar.search()").unwrap_err();
    assert!(
        err.message.contains("missing") || err.message.contains("bad argument"),
        "msg: {}",
        err.message
    );
}

#[test]
fn search_wrong_type_errors() {
    let sb = sb_no_domains();
    let err = sb.exec("edgar.search(123)").unwrap_err();
    assert!(
        err.message.contains("string") || err.message.contains("expected"),
        "msg: {}",
        err.message
    );
}

#[test]
fn search_empty_query_errors() {
    let sb = sb_no_domains();
    let err = sb.exec(r#"edgar.search("")"#).unwrap_err();
    assert!(err.message.contains("empty"), "msg: {}", err.message);
}

#[test]
fn search_whitespace_query_errors() {
    let sb = sb_no_domains();
    let err = sb.exec(r#"edgar.search("   ")"#).unwrap_err();
    assert!(err.message.contains("empty"), "msg: {}", err.message);
}

#[test]
fn search_invalid_form_type_errors() {
    let sb = sb_no_domains();
    let err = sb
        .exec(r#"edgar.search("apple", "10-K; DROP TABLE")"#)
        .unwrap_err();
    assert!(
        err.message.contains("invalid character"),
        "msg: {}",
        err.message
    );
}

#[test]
fn search_invalid_date_errors() {
    let sb = sb_no_domains();
    let err = sb
        .exec(r#"edgar.search("apple", nil, "not-a-date")"#)
        .unwrap_err();
    assert!(err.message.contains("invalid"), "msg: {}", err.message);
}

#[test]
fn search_date_before_edgar_errors() {
    let sb = sb_no_domains();
    let err = sb
        .exec(r#"edgar.search("apple", nil, "1990-01-01")"#)
        .unwrap_err();
    assert!(
        err.message.contains("out of range") || err.message.contains("1993"),
        "msg: {}",
        err.message
    );
}

// ── Argument validation — filings ────────────────────────────────

#[test]
fn filings_no_args_errors() {
    let sb = sb_no_domains();
    let err = sb.exec("edgar.filings()").unwrap_err();
    assert!(
        err.message.contains("missing") || err.message.contains("bad argument"),
        "msg: {}",
        err.message
    );
}

#[test]
fn filings_empty_cik_errors() {
    let sb = sb_no_domains();
    let err = sb.exec(r#"edgar.filings("")"#).unwrap_err();
    assert!(err.message.contains("empty"), "msg: {}", err.message);
}

#[test]
fn filings_non_numeric_cik_errors() {
    let sb = sb_no_domains();
    let err = sb.exec(r#"edgar.filings("AAPL")"#).unwrap_err();
    assert!(
        err.message.contains("numeric") || err.message.contains("invalid CIK"),
        "msg: {}",
        err.message
    );
}

#[test]
fn filings_path_traversal_cik_errors() {
    let sb = sb_no_domains();
    let err = sb.exec(r#"edgar.filings("../../etc/passwd")"#).unwrap_err();
    assert!(
        err.message.contains("invalid") || err.message.contains("numeric"),
        "msg: {}",
        err.message
    );
}

#[test]
fn filings_too_long_cik_errors() {
    let sb = sb_no_domains();
    let err = sb.exec(r#"edgar.filings("12345678901")"#).unwrap_err();
    assert!(err.message.contains("out of range"), "msg: {}", err.message);
}

#[test]
fn filings_cik_prefix_accepted() {
    // CIK prefix should be stripped — this should only fail at HTTP level (domain denied)
    let sb = sb_no_domains();
    let err = sb.exec(r#"edgar.filings("CIK0000320193")"#).unwrap_err();
    // Should NOT be a CIK validation error — should be an HTTP/domain error
    assert!(
        err.message.contains("denied") || err.message.contains("HTTP"),
        "CIK prefix should be accepted, but got: {}",
        err.message
    );
}

#[test]
fn filings_numeric_cik_accepted() {
    let sb = sb_no_domains();
    let err = sb.exec(r#"edgar.filings("320193")"#).unwrap_err();
    // Should fail at HTTP level, not validation
    assert!(
        err.message.contains("denied") || err.message.contains("HTTP"),
        "numeric CIK should be accepted, but got: {}",
        err.message
    );
}

#[test]
fn filings_integer_cik_as_string() {
    // CIK must be passed as string — integers get rejected by param validation
    let sb = sb_no_domains();
    let err = sb.exec(r#"edgar.filings(320193)"#).unwrap_err();
    assert!(
        err.message.contains("string") || err.message.contains("expected"),
        "should hint at string type: {}",
        err.message
    );
}

#[test]
fn filings_numeric_string_cik_accepted() {
    let sb = sb_no_domains();
    let err = sb.exec(r#"edgar.filings("320193")"#).unwrap_err();
    // Should fail at HTTP level, not validation
    assert!(
        err.message.contains("denied") || err.message.contains("HTTP"),
        "numeric string CIK should be accepted, but got: {}",
        err.message
    );
}

// ── Argument validation — company ────────────────────────────────

#[test]
fn company_no_args_errors() {
    let sb = sb_no_domains();
    let err = sb.exec("edgar.company()").unwrap_err();
    assert!(
        err.message.contains("missing") || err.message.contains("bad argument"),
        "msg: {}",
        err.message
    );
}

#[test]
fn company_empty_cik_errors() {
    let sb = sb_no_domains();
    let err = sb.exec(r#"edgar.company("")"#).unwrap_err();
    assert!(err.message.contains("empty"), "msg: {}", err.message);
}

#[test]
fn company_invalid_cik_errors() {
    let sb = sb_no_domains();
    let err = sb.exec(r#"edgar.company("abc")"#).unwrap_err();
    assert!(
        err.message.contains("numeric") || err.message.contains("invalid"),
        "msg: {}",
        err.message
    );
}

// ── Domain enforcement (sandbox security) ────────────────────────

#[test]
fn search_denied_without_edgar_domain() {
    let sb = sb_no_domains();
    let err = sb.exec(r#"edgar.search("apple")"#).unwrap_err();
    assert!(
        err.message.contains("denied") || err.message.contains("HTTP"),
        "expected domain denial: {}",
        err.message
    );
}

#[test]
fn filings_denied_without_edgar_domain() {
    let sb = sb_no_domains();
    let err = sb.exec(r#"edgar.filings("320193")"#).unwrap_err();
    assert!(
        err.message.contains("denied") || err.message.contains("HTTP"),
        "expected domain denial: {}",
        err.message
    );
}

#[test]
fn company_denied_without_edgar_domain() {
    let sb = sb_no_domains();
    let err = sb.exec(r#"edgar.company("320193")"#).unwrap_err();
    assert!(
        err.message.contains("denied") || err.message.contains("HTTP"),
        "expected domain denial: {}",
        err.message
    );
}

// ── Sandbox safety ───────────────────────────────────────────────

#[test]
fn edgar_no_dangerous_globals() {
    let sb = sb_no_domains();
    let r = sb
        .exec(
            r#"
            return tostring(rawget(edgar, "io")) .. " " ..
                   tostring(rawget(edgar, "os")) .. " " ..
                   tostring(rawget(edgar, "loadstring"))
        "#,
        )
        .unwrap();
    assert_eq!(r, "nil nil nil");
}

#[test]
fn edgar_metatable_safe() {
    let sb = sb_no_domains();
    let r = sb
        .exec(
            r#"
            local mt = getmetatable(edgar)
            if mt then
                local idx = rawget(mt, "__index")
                if type(idx) == "table" then
                    if rawget(idx, "io") or rawget(idx, "os") then
                        return "metatable leaks dangerous globals"
                    end
                end
            end
            local count = 0
            for k, v in pairs(edgar) do
                count = count + 1
            end
            return "safe:" .. count
        "#,
        )
        .unwrap();
    assert!(r.starts_with("safe:"), "expected safe, got: {}", r);
}

#[test]
fn edgar_cannot_access_filesystem() {
    let sb = sb_no_domains();
    let r = sb
        .exec(
            r#"
            local allowed = {search=true, filings=true, company=true, help=true}
            for k, v in pairs(edgar) do
                if type(v) == "function" and not allowed[k] then
                    return "unexpected function: " .. k
                end
            end
            return "clean"
        "#,
        )
        .unwrap();
    assert_eq!(r, "clean");
}

#[test]
fn edgar_no_network_access_beyond_api() {
    // Verify edgar can't be used to access arbitrary URLs
    let sb = sb_no_domains();
    let r = sb
        .exec(
            r#"
            -- edgar table should only have expected functions, no raw HTTP access
            local has_raw = rawget(edgar, "get") or rawget(edgar, "request") or rawget(edgar, "fetch")
            return tostring(has_raw)
        "#,
        )
        .unwrap();
    assert_eq!(r, "nil");
}

// ── Dual-signature (table form) ──────────────────────────────────

#[test]
fn search_table_form_validates_query() {
    let sb = sb_no_domains();
    let err = sb.exec(r#"edgar.search({query=""})"#).unwrap_err();
    assert!(err.message.contains("empty"), "msg: {}", err.message);
}

#[test]
fn filings_table_form_validates_cik() {
    let sb = sb_no_domains();
    let err = sb.exec(r#"edgar.filings({cik="abc"})"#).unwrap_err();
    assert!(
        err.message.contains("numeric") || err.message.contains("invalid"),
        "msg: {}",
        err.message
    );
}

#[test]
fn company_table_form_validates_cik() {
    let sb = sb_no_domains();
    let err = sb.exec(r#"edgar.company({cik=""})"#).unwrap_err();
    assert!(err.message.contains("empty"), "msg: {}", err.message);
}

#[test]
fn search_table_form_invalid_form_type() {
    let sb = sb_no_domains();
    let err = sb
        .exec(r#"edgar.search({query="apple", type="10-K;DROP"})"#)
        .unwrap_err();
    assert!(
        err.message.contains("invalid character"),
        "msg: {}",
        err.message
    );
}

#[test]
fn filings_table_form_with_type_filter() {
    // Table form with CIK and type filter — should fail at HTTP level
    let sb = sb_no_domains();
    let err = sb
        .exec(r#"edgar.filings({cik="320193", type="10-K"})"#)
        .unwrap_err();
    assert!(
        err.message.contains("denied") || err.message.contains("HTTP"),
        "msg: {}",
        err.message
    );
}

// ── Shell dispatch ───────────────────────────────────────────────

#[test]
fn shell_edgar_help() {
    let sb = sb_no_domains();
    let shrt = include_str!("../../runtime/shrt.luau");
    sb.register_module("shrt", shrt).unwrap();
    let result = cpsl_core::sh_transpile::transpile_sh(r#"edgar help"#);
    assert!(result.is_ok(), "transpile err: {:?}", result.err());
    let luau = result.unwrap().luau_source;
    let r = sb.exec(&luau);
    assert!(
        r.is_ok(),
        "shell edgar help should not error: {:?}",
        r.err()
    );
}

// ── Python transpiler e2e ────────────────────────────────────────

#[test]
fn python_import_sec_edgar_downloader() {
    let py_code = r#"
from sec_edgar_downloader import Downloader
dl = Downloader()
filings = dl.get("10-K", "AAPL")
"#;
    let transpiled = cpsl_core::transpile::transpile(py_code).unwrap();
    assert!(
        transpiled.luau_source.contains("edgar"),
        "transpiled: {}",
        transpiled.luau_source
    );
}

#[test]
fn python_import_edgar() {
    let py_code = r#"
import edgar
filings = edgar.get_filings(form="10-K")
"#;
    let transpiled = cpsl_core::transpile::transpile(py_code).unwrap();
    assert!(
        transpiled.luau_source.contains("edgar"),
        "transpiled: {}",
        transpiled.luau_source
    );
}

#[test]
fn python_from_edgar_import() {
    let py_code = r#"
from edgar import Company
c = Company("Apple Inc.", "0000320193")
"#;
    let transpiled = cpsl_core::transpile::transpile(py_code).unwrap();
    assert!(
        transpiled.luau_source.contains("edgar"),
        "transpiled: {}",
        transpiled.luau_source
    );
}

#[test]
fn python_edgar_search_transpiles() {
    let sb = sb_no_domains();
    let pyrt = include_str!("../../runtime/pyrt.luau");
    sb.setup_python_runtime(pyrt).unwrap();

    let py_code = r#"
import edgar
# This should fail at HTTP level (domain denied) but transpile fine
try:
    data = edgar.search("apple")
except:
    pass
print("transpiled_ok")
"#;
    let transpiled = cpsl_core::transpile::transpile(py_code).unwrap();
    assert!(
        transpiled.luau_source.contains("edgar"),
        "transpiled: {}",
        transpiled.luau_source
    );
}

// ── Live API tests (skipped by default, run with --ignored) ──────

#[test]
#[ignore]
fn live_search_apple() {
    let sb = sb_edgar();
    let r = sb
        .exec(
            r#"
            local results = edgar.search("Apple Inc", "10-K", nil, nil, 5)
            local count = 0
            for _ in ipairs(results) do count = count + 1 end
            if count == 0 then error("no search results") end
            -- First result should have expected fields
            local row = results[1]
            if not row.accession then error("missing accession") end
            if not row.form then error("missing form") end
            if not row.filed then error("missing filed") end
            return "ok:" .. count
        "#,
        )
        .unwrap();
    assert!(r.starts_with("ok:"), "got: {}", r);
}

#[test]
#[ignore]
fn live_filings_apple() {
    let sb = sb_edgar();
    let r = sb
        .exec(
            r#"
            -- Apple CIK: 0000320193
            local filings = edgar.filings("320193", "10-K", 5)
            local count = 0
            for _ in ipairs(filings) do count = count + 1 end
            if count == 0 then error("no filings") end
            local row = filings[1]
            if not row.accession then error("missing accession") end
            if not row.form then error("missing form") end
            if not row.filed then error("missing filed") end
            if not row.url then error("missing url") end
            return "ok:" .. count
        "#,
        )
        .unwrap();
    assert!(r.starts_with("ok:"), "got: {}", r);
}

#[test]
#[ignore]
fn live_company_apple() {
    let sb = sb_edgar();
    let r = sb
        .exec(
            r#"
            local info = edgar.company("320193")
            if not info.name then error("missing name") end
            if not info.sic then error("missing sic") end
            -- Check tickers
            local tickers = info.tickers
            if not tickers or #tickers == 0 then error("missing tickers") end
            return info.name .. "|" .. tickers[1]
        "#,
        )
        .unwrap();
    assert!(r.contains("APPLE") || r.contains("Apple"), "got: {}", r);
    assert!(r.contains("AAPL"), "should have AAPL ticker: {}", r);
}

#[test]
#[ignore]
fn live_search_with_date_range() {
    let sb = sb_edgar();
    let r = sb
        .exec(
            r#"
            local results = edgar.search("annual report", "10-K", "2024-01-01", "2024-06-30", 5)
            local count = 0
            for _ in ipairs(results) do count = count + 1 end
            return "rows:" .. count
        "#,
        )
        .unwrap();
    assert!(r.starts_with("rows:"), "got: {}", r);
}

#[test]
#[ignore]
fn live_filings_with_cik_prefix() {
    let sb = sb_edgar();
    let r = sb
        .exec(
            r#"
            -- Should accept CIK prefix
            local filings = edgar.filings("CIK0000320193", nil, 3)
            local count = 0
            for _ in ipairs(filings) do count = count + 1 end
            return "rows:" .. count
        "#,
        )
        .unwrap();
    assert!(r.starts_with("rows:"), "got: {}", r);
    let count: i32 = r.split(':').nth(1).unwrap().parse().unwrap();
    assert!(count > 0, "should have some filings: {}", count);
}
