#![cfg(feature = "mod-yfinance")]

use native_http::HttpGateway;
use std::sync::Arc;
use cpsl_core::Sandbox;

fn sb_no_domains() -> Sandbox {
    let gw = Arc::new(HttpGateway::builder().build());
    Sandbox::builder().http_gateway(gw).build().unwrap()
}

fn sb_yahoo() -> Sandbox {
    let gw = Arc::new(
        HttpGateway::builder()
            .allow_domain("query1.finance.yahoo.com")
            .allow_domain("query2.finance.yahoo.com")
            .build(),
    );
    Sandbox::builder().http_gateway(gw).build().unwrap()
}

// ── Module registration ─────────────────────────────────────────

#[test]
fn yfinance_global_exists_with_http() {
    let sb = sb_no_domains();
    let r = sb.exec("return type(yfinance)").unwrap();
    assert_eq!(r, "table");
}

#[test]
fn yfinance_not_present_without_http() {
    // Without http gateway, yfinance should not be registered
    let sb = Sandbox::new().unwrap();
    let r = sb.exec("return type(yfinance)").unwrap();
    assert_eq!(r, "nil");
}

#[test]
fn yfinance_has_expected_functions() {
    let sb = sb_no_domains();
    let r = sb
        .exec(
            r#"
            return type(yfinance.history) .. " " ..
                   type(yfinance.quote) .. " " ..
                   type(yfinance.info) .. " " ..
                   type(yfinance.search)
        "#,
        )
        .unwrap();
    assert_eq!(r, "function function function function");
}

// ── Help ────────────────────────────────────────────────────────

#[test]
fn yfinance_help_returns_help_text() {
    let sb = sb_no_domains();
    let r = sb.exec("return yfinance.help()").unwrap();
    assert!(r.contains("yfinance"), "help: {}", r);
    assert!(r.contains("yfinance.history"), "help: {}", r);
    assert!(r.contains("yfinance.quote"), "help: {}", r);
    assert!(r.contains("yfinance.info"), "help: {}", r);
    assert!(r.contains("yfinance.search"), "help: {}", r);
}

#[test]
fn yfinance_help_bare_call() {
    let sb = sb_no_domains();
    let r = sb.exec("yfinance.help()").unwrap();
    assert!(r.contains("yfinance"), "help: {}", r);
}

#[test]
fn global_help_includes_yfinance() {
    let sb = sb_no_domains();
    let r = sb.exec("return help()").unwrap();
    assert!(r.contains("yfinance"), "global help should list yfinance: {}", r);
}

#[test]
fn yfinance_nonexistent_fn_hint() {
    let sb = sb_no_domains();
    let err = sb.exec("yfinance.foo()").unwrap_err();
    assert!(
        err.message.contains("yfinance.foo does not exist"),
        "msg: {}",
        err.message
    );
    assert!(
        err.message.contains("hint: call yfinance.help() for usage"),
        "msg: {}",
        err.message
    );
}

// ── Argument validation ─────────────────────────────────────────

#[test]
fn history_no_args_errors() {
    let sb = sb_no_domains();
    let err = sb.exec("yfinance.history()").unwrap_err();
    assert!(
        err.message.contains("missing") || err.message.contains("bad argument"),
        "msg: {}",
        err.message
    );
}

#[test]
fn quote_no_args_errors() {
    let sb = sb_no_domains();
    let err = sb.exec("yfinance.quote()").unwrap_err();
    assert!(
        err.message.contains("missing") || err.message.contains("bad argument"),
        "msg: {}",
        err.message
    );
}

#[test]
fn info_no_args_errors() {
    let sb = sb_no_domains();
    let err = sb.exec("yfinance.info()").unwrap_err();
    assert!(
        err.message.contains("missing") || err.message.contains("bad argument"),
        "msg: {}",
        err.message
    );
}

#[test]
fn search_no_args_errors() {
    let sb = sb_no_domains();
    let err = sb.exec("yfinance.search()").unwrap_err();
    assert!(
        err.message.contains("missing") || err.message.contains("bad argument"),
        "msg: {}",
        err.message
    );
}

#[test]
fn history_wrong_type_errors() {
    let sb = sb_no_domains();
    let err = sb.exec("yfinance.history(123)").unwrap_err();
    assert!(
        err.message.contains("string") || err.message.contains("expected"),
        "msg: {}",
        err.message
    );
}

// ── Ticker validation ───────────────────────────────────────────

#[test]
fn history_invalid_ticker_errors() {
    let sb = sb_no_domains();
    let err = sb
        .exec(r#"yfinance.history("AAPL; DROP TABLE")"#)
        .unwrap_err();
    assert!(
        err.message.contains("invalid character"),
        "msg: {}",
        err.message
    );
}

#[test]
fn history_empty_ticker_errors() {
    let sb = sb_no_domains();
    let err = sb.exec(r#"yfinance.history("")"#).unwrap_err();
    assert!(
        err.message.contains("empty"),
        "msg: {}",
        err.message
    );
}

#[test]
fn history_long_ticker_errors() {
    let sb = sb_no_domains();
    let err = sb
        .exec(r#"yfinance.history("AAAAAAAAAAAAAAAAAAAAA")"#)
        .unwrap_err();
    assert!(
        err.message.contains("too long"),
        "msg: {}",
        err.message
    );
}

// ── Period/interval validation ──────────────────────────────────

#[test]
fn history_invalid_period_errors() {
    let sb = sb_no_domains();
    let err = sb
        .exec(r#"yfinance.history("AAPL", "invalid_period")"#)
        .unwrap_err();
    assert!(
        err.message.contains("invalid period"),
        "msg: {}",
        err.message
    );
}

#[test]
fn history_invalid_interval_errors() {
    let sb = sb_no_domains();
    let err = sb
        .exec(r#"yfinance.history("AAPL", "1mo", "invalid_interval")"#)
        .unwrap_err();
    assert!(
        err.message.contains("invalid interval"),
        "msg: {}",
        err.message
    );
}

// ── Domain enforcement (sandbox security) ───────────────────────

#[test]
fn history_denied_without_yahoo_domain() {
    let sb = sb_no_domains();
    let err = sb
        .exec(r#"yfinance.history("AAPL")"#)
        .unwrap_err();
    // The http gateway should deny the request because query1.finance.yahoo.com is not allowed
    assert!(
        err.message.contains("denied") || err.message.contains("HTTP"),
        "expected domain denial: {}",
        err.message
    );
}

#[test]
fn quote_denied_without_yahoo_domain() {
    let sb = sb_no_domains();
    let err = sb
        .exec(r#"yfinance.quote("AAPL")"#)
        .unwrap_err();
    assert!(
        err.message.contains("denied") || err.message.contains("HTTP"),
        "expected domain denial: {}",
        err.message
    );
}

#[test]
fn search_denied_without_yahoo_domain() {
    let sb = sb_no_domains();
    let err = sb
        .exec(r#"yfinance.search("Apple")"#)
        .unwrap_err();
    assert!(
        err.message.contains("denied") || err.message.contains("HTTP"),
        "expected domain denial: {}",
        err.message
    );
}

// ── Sandbox safety ──────────────────────────────────────────────

#[test]
fn yfinance_no_dangerous_globals() {
    let sb = sb_no_domains();
    let r = sb
        .exec(
            r#"
            return tostring(rawget(yfinance, "io")) .. " " ..
                   tostring(rawget(yfinance, "os")) .. " " ..
                   tostring(rawget(yfinance, "loadstring"))
        "#,
        )
        .unwrap();
    assert_eq!(r, "nil nil nil");
}

#[test]
fn yfinance_metatable_safe() {
    let sb = sb_no_domains();
    let r = sb
        .exec(
            r#"
            local mt = getmetatable(yfinance)
            if mt then
                local idx = rawget(mt, "__index")
                if type(idx) == "table" then
                    if rawget(idx, "io") or rawget(idx, "os") then
                        return "metatable leaks dangerous globals"
                    end
                end
            end
            local count = 0
            for k, v in pairs(yfinance) do
                count = count + 1
            end
            return "safe:" .. count
        "#,
        )
        .unwrap();
    assert!(r.starts_with("safe:"), "expected safe, got: {}", r);
}

#[test]
fn yfinance_cannot_access_filesystem() {
    // Verify that yfinance functions don't expose any filesystem access
    let sb = sb_no_domains();
    let r = sb
        .exec(
            r#"
            -- Ensure yfinance table only has expected function keys
            local allowed = {history=true, quote=true, info=true, search=true, help=true}
            for k, v in pairs(yfinance) do
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

// ── Dual-signature (table form) ─────────────────────────────────

#[test]
fn history_table_form_validates_ticker() {
    let sb = sb_no_domains();
    let err = sb
        .exec(r#"yfinance.history({ticker="AAPL; DROP"})"#)
        .unwrap_err();
    assert!(
        err.message.contains("invalid character"),
        "msg: {}",
        err.message
    );
}

#[test]
fn quote_table_form_validates_ticker() {
    let sb = sb_no_domains();
    let err = sb
        .exec(r#"yfinance.quote({ticker=""})"#)
        .unwrap_err();
    assert!(
        err.message.contains("empty"),
        "msg: {}",
        err.message
    );
}

#[test]
fn history_table_form_invalid_period() {
    let sb = sb_no_domains();
    let err = sb
        .exec(r#"yfinance.history({ticker="AAPL", period="bad"})"#)
        .unwrap_err();
    assert!(
        err.message.contains("invalid period"),
        "msg: {}",
        err.message
    );
}

// ── Shell dispatch ──────────────────────────────────────────────

#[test]
fn shell_yfinance_help() {
    let sb = sb_no_domains();
    let shrt = include_str!("../../shrt.luau");
    sb.register_module("shrt", shrt).unwrap();
    let result = cpsl_core::sh_transpile::transpile_sh(r#"yfinance help"#);
    assert!(result.is_ok(), "transpile err: {:?}", result.err());
    let luau = result.unwrap().luau_source;
    let r = sb.exec(&luau);
    assert!(r.is_ok(), "shell yfinance help should not error: {:?}", r.err());
}

// ── Python transpiler e2e ────────────────────────────────────────

#[test]
fn python_import_yfinance() {
    let py_code = r#"
import yfinance as yf
data = yf.history("AAPL")
"#;
    let transpiled = cpsl_core::transpile::transpile(py_code).unwrap();
    assert!(
        transpiled.luau_source.contains("yfinance"),
        "transpiled: {}",
        transpiled.luau_source
    );
    // The alias 'yf' should map to yfinance
    assert!(
        transpiled.luau_source.contains("yfinance") || transpiled.luau_source.contains("yf"),
        "transpiled: {}",
        transpiled.luau_source
    );
}

#[test]
fn python_import_yfinance_no_alias() {
    let py_code = r#"
import yfinance
data = yfinance.history("AAPL")
"#;
    let transpiled = cpsl_core::transpile::transpile(py_code).unwrap();
    assert!(
        transpiled.luau_source.contains("yfinance"),
        "transpiled: {}",
        transpiled.luau_source
    );
}

#[test]
fn python_from_yfinance_import() {
    let py_code = r#"
from yfinance import Ticker
t = Ticker("AAPL")
"#;
    let transpiled = cpsl_core::transpile::transpile(py_code).unwrap();
    assert!(
        transpiled.luau_source.contains("yfinance"),
        "transpiled: {}",
        transpiled.luau_source
    );
}

#[test]
fn python_yfinance_history_transpiles() {
    let sb = sb_no_domains();
    let pyrt = include_str!("../../pyrt.luau");
    sb.setup_python_runtime(pyrt).unwrap();

    let py_code = r#"
import yfinance as yf
# This should fail at HTTP level (domain denied) but transpile fine
try:
    data = yf.history("AAPL")
except:
    pass
print("transpiled_ok")
"#;
    let transpiled = cpsl_core::transpile::transpile(py_code).unwrap();
    // The transpiled code should reference yfinance
    assert!(
        transpiled.luau_source.contains("yfinance"),
        "transpiled: {}",
        transpiled.luau_source
    );
}

// ── Live API tests (skipped by default, run with --ignored) ─────

#[test]
#[ignore]
fn live_history_aapl() {
    let sb = sb_yahoo();
    let r = sb
        .exec(
            r#"
            local data = yfinance.history("AAPL", "5d")
            -- Should have some rows
            local count = 0
            for _ in ipairs(data) do count = count + 1 end
            if count == 0 then error("no history data") end
            -- First row should have expected fields
            local row = data[1]
            if not row.date then error("missing date") end
            if not row.open then error("missing open") end
            if not row.close then error("missing close") end
            if not row.volume then error("missing volume") end
            return "ok:" .. count
        "#,
        )
        .unwrap();
    assert!(r.starts_with("ok:"), "got: {}", r);
}

#[test]
#[ignore]
fn live_quote_aapl() {
    let sb = sb_yahoo();
    let r = sb
        .exec(
            r#"
            local q = yfinance.quote("AAPL")
            if not q.price then error("missing price") end
            if not q.name then error("missing name") end
            if not q.currency then error("missing currency") end
            return string.format("%.2f %s", q.price, q.currency)
        "#,
        )
        .unwrap();
    // Should have a price and "USD"
    assert!(r.contains("USD"), "got: {}", r);
}

#[test]
#[ignore]
fn live_info_aapl() {
    let sb = sb_yahoo();
    let r = sb
        .exec(
            r#"
            local info = yfinance.info("AAPL")
            if not info.name then error("missing name") end
            if not info.sector then error("missing sector") end
            if not info.industry then error("missing industry") end
            return info.name .. "|" .. info.sector
        "#,
        )
        .unwrap();
    assert!(r.contains("Apple"), "got: {}", r);
}

#[test]
#[ignore]
fn live_search_apple() {
    let sb = sb_yahoo();
    let r = sb
        .exec(
            r#"
            local results = yfinance.search("Apple", 5)
            local count = 0
            for _ in ipairs(results) do count = count + 1 end
            if count == 0 then error("no search results") end
            -- Should find AAPL
            local found = false
            for _, r in ipairs(results) do
                if r.symbol == "AAPL" then found = true end
            end
            return "found:" .. tostring(found) .. " count:" .. count
        "#,
        )
        .unwrap();
    assert!(r.contains("found:true"), "got: {}", r);
}

#[test]
#[ignore]
fn live_history_with_dates() {
    let sb = sb_yahoo();
    let r = sb
        .exec(
            r#"
            local data = yfinance.history("MSFT", nil, "1d", "2024-01-02", "2024-01-10")
            local count = 0
            for _ in ipairs(data) do count = count + 1 end
            return "rows:" .. count
        "#,
        )
        .unwrap();
    // ~5-6 trading days in that range
    assert!(r.starts_with("rows:"), "got: {}", r);
    let count: i32 = r.split(':').nth(1).unwrap().parse().unwrap();
    assert!(count >= 3 && count <= 8, "unexpected row count: {}", count);
}

#[test]
#[ignore]
fn live_history_crypto() {
    let sb = sb_yahoo();
    let r = sb
        .exec(
            r#"
            local data = yfinance.history("BTC-USD", "5d")
            local count = 0
            for _ in ipairs(data) do count = count + 1 end
            return "rows:" .. count
        "#,
        )
        .unwrap();
    assert!(r.starts_with("rows:"), "got: {}", r);
}

#[test]
#[ignore]
fn live_history_index() {
    let sb = sb_yahoo();
    let r = sb
        .exec(
            r#"
            local data = yfinance.history("^GSPC", "5d")
            local count = 0
            for _ in ipairs(data) do count = count + 1 end
            return "rows:" .. count
        "#,
        )
        .unwrap();
    assert!(r.starts_with("rows:"), "got: {}", r);
}
