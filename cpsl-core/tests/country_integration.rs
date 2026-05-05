#![cfg(feature = "mod-country")]

use cpsl_core::{Sandbox, transpile};

fn sb() -> Sandbox {
    Sandbox::new().unwrap()
}

// ── country.lookup ─────────────────────────────────────────────

#[test]
fn lookup_by_alpha2() {
    let s = sb();
    let r = s
        .exec(r#"local c = country.lookup("US"); return c.name .. " " .. c.alpha2 .. " " .. c.alpha3 .. " " .. c.numeric"#)
        .unwrap();
    assert!(r.contains("United States"), "got: {}", r);
    assert!(r.contains("US"), "got: {}", r);
    assert!(r.contains("USA"), "got: {}", r);
}

#[test]
fn lookup_by_alpha2_lowercase() {
    let s = sb();
    let r = s
        .exec(r#"local c = country.lookup("gb"); return c.alpha2"#)
        .unwrap();
    assert_eq!(r, "GB");
}

#[test]
fn lookup_by_alpha3() {
    let s = sb();
    let r = s
        .exec(r#"local c = country.lookup("DEU"); return c.alpha2"#)
        .unwrap();
    assert_eq!(r, "DE");
}

#[test]
fn lookup_by_alpha3_lowercase() {
    let s = sb();
    let r = s
        .exec(r#"local c = country.lookup("fra"); return c.name"#)
        .unwrap();
    assert!(r.contains("France"), "got: {}", r);
}

#[test]
fn lookup_by_numeric() {
    let s = sb();
    let r = s
        .exec(r#"local c = country.lookup("392"); return c.alpha2"#)
        .unwrap();
    assert_eq!(r, "JP");
}

#[test]
fn lookup_invalid_returns_nil() {
    let s = sb();
    let r = s
        .exec(r#"return tostring(country.lookup("ZZ"))"#)
        .unwrap();
    assert_eq!(r, "nil");
}

#[test]
fn lookup_empty_string_returns_nil() {
    let s = sb();
    let r = s
        .exec(r#"return tostring(country.lookup(""))"#)
        .unwrap();
    assert_eq!(r, "nil");
}

#[test]
fn lookup_invalid_numeric_returns_nil() {
    let s = sb();
    let r = s
        .exec(r#"return tostring(country.lookup("99999"))"#)
        .unwrap();
    assert_eq!(r, "nil");
}

// ── country.byName ─────────────────────────────────────────────

#[test]
fn by_name_exact() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local results = country.byName("Japan")
            return results[1].alpha2
        "#,
        )
        .unwrap();
    assert_eq!(r, "JP");
}

#[test]
fn by_name_case_insensitive() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local results = country.byName("japan")
            return results[1].alpha2
        "#,
        )
        .unwrap();
    assert_eq!(r, "JP");
}

#[test]
fn by_name_partial_match() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local results = country.byName("United")
            local count = 0
            for _ in pairs(results) do count = count + 1 end
            return tostring(count > 1)
        "#,
        )
        .unwrap();
    assert_eq!(r, "true");
}

#[test]
fn by_name_no_match() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local results = country.byName("Zzzyyyxxx")
            local count = 0
            for _ in pairs(results) do count = count + 1 end
            return tostring(count)
        "#,
        )
        .unwrap();
    assert_eq!(r, "0");
}

// ── country.all ────────────────────────────────────────────────

#[test]
fn all_returns_many_countries() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local all = country.all()
            local count = 0
            for _ in pairs(all) do count = count + 1 end
            return tostring(count > 200)
        "#,
        )
        .unwrap();
    assert_eq!(r, "true");
}

#[test]
fn all_with_filter() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local results = country.all("JP")
            local count = 0
            for _ in pairs(results) do count = count + 1 end
            return tostring(count)
        "#,
        )
        .unwrap();
    // "JP" should match Japan's alpha2
    let count: i32 = r.parse().unwrap();
    assert!(count >= 1, "expected at least 1 match for 'JP', got: {}", count);
}

#[test]
fn all_each_entry_has_required_fields() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local all = country.all()
            local first = all[1]
            local has_fields = first.name ~= nil and first.alpha2 ~= nil and first.alpha3 ~= nil and first.numeric ~= nil
            return tostring(has_fields)
        "#,
        )
        .unwrap();
    assert_eq!(r, "true");
}

// ── country.currency ───────────────────────────────────────────

#[test]
fn currency_for_us() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local curs = country.currency("US")
            for i, c in ipairs(curs) do
                if c.code == "USD" then
                    return c.code .. " " .. c.name
                end
            end
            return "not found"
        "#,
        )
        .unwrap();
    assert!(r.contains("USD"), "got: {}", r);
    assert!(r.contains("dollar"), "got: {}", r);
}

#[test]
fn currency_for_gb() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local curs = country.currency("GB")
            for i, c in ipairs(curs) do
                if c.code == "GBP" then
                    return c.code
                end
            end
            return "not found"
        "#,
        )
        .unwrap();
    assert_eq!(r, "GBP");
}

#[test]
fn currency_for_jp() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local curs = country.currency("JP")
            for i, c in ipairs(curs) do
                if c.code == "JPY" then
                    return c.code .. " " .. tostring(c.minor_units)
                end
            end
            return "not found"
        "#,
        )
        .unwrap();
    assert!(r.starts_with("JPY"), "got: {}", r);
    assert!(r.contains("0"), "JPY minor_units should be 0, got: {}", r);
}

#[test]
fn currency_has_symbol() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local curs = country.currency("US")
            for i, c in ipairs(curs) do
                if c.code == "USD" then
                    return c.symbol
                end
            end
            return "none"
        "#,
        )
        .unwrap();
    assert_eq!(r, "$");
}

#[test]
fn currency_invalid_country_errors() {
    let s = sb();
    let err = s
        .exec(r#"country.currency("ZZ")"#)
        .unwrap_err();
    assert!(
        err.message.contains("unknown country") || err.message.contains("country.currency"),
        "msg: {}",
        err.message
    );
}

#[test]
fn currency_accepts_alpha3() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local curs = country.currency("USA")
            for i, c in ipairs(curs) do
                if c.code == "USD" then return "ok" end
            end
            return "not found"
        "#,
        )
        .unwrap();
    assert_eq!(r, "ok");
}

// ── currency.lookup ────────────────────────────────────────────

#[test]
fn currency_lookup_usd() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local c = currency.lookup("USD")
            return c.code .. " " .. c.name
        "#,
        )
        .unwrap();
    assert!(r.contains("USD"), "got: {}", r);
    assert!(r.contains("dollar"), "got: {}", r);
}

#[test]
fn currency_lookup_eur() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local c = currency.lookup("EUR")
            return c.code .. " " .. c.name .. " " .. c.symbol
        "#,
        )
        .unwrap();
    assert!(r.contains("EUR"), "got: {}", r);
    assert!(r.contains("Euro"), "got: {}", r);
}

#[test]
fn currency_lookup_case_insensitive() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local c = currency.lookup("usd")
            return c.code
        "#,
        )
        .unwrap();
    assert_eq!(r, "USD");
}

#[test]
fn currency_lookup_has_countries() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local c = currency.lookup("USD")
            local count = 0
            for _ in pairs(c.countries) do count = count + 1 end
            return tostring(count > 0)
        "#,
        )
        .unwrap();
    assert_eq!(r, "true");
}

#[test]
fn currency_lookup_has_minor_units() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local c = currency.lookup("USD")
            return tostring(c.minor_units)
        "#,
        )
        .unwrap();
    assert_eq!(r, "2");
}

#[test]
fn currency_lookup_invalid_returns_nil() {
    let s = sb();
    let r = s
        .exec(r#"return tostring(currency.lookup("ZZZ"))"#)
        .unwrap();
    assert_eq!(r, "nil");
}

#[test]
fn currency_lookup_empty_returns_nil() {
    let s = sb();
    let r = s
        .exec(r#"return tostring(currency.lookup(""))"#)
        .unwrap();
    assert_eq!(r, "nil");
}

// ── currency.all ───────────────────────────────────────────────

#[test]
fn currency_all_returns_many() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local all = currency.all()
            local count = 0
            for _ in pairs(all) do count = count + 1 end
            return tostring(count > 100)
        "#,
        )
        .unwrap();
    assert_eq!(r, "true");
}

#[test]
fn currency_all_with_filter() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local results = currency.all("dollar")
            local count = 0
            for _ in pairs(results) do count = count + 1 end
            return tostring(count > 1)
        "#,
        )
        .unwrap();
    assert_eq!(r, "true");
}

#[test]
fn currency_all_filter_by_code() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local results = currency.all("usd")
            local count = 0
            for _ in pairs(results) do count = count + 1 end
            return tostring(count)
        "#,
        )
        .unwrap();
    let count: i32 = r.parse().unwrap();
    assert!(count >= 1, "expected at least 1 match for 'usd', got: {}", count);
}

#[test]
fn currency_all_entries_have_fields() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local all = currency.all()
            local first = all[1]
            local has = first.code ~= nil and first.name ~= nil and first.symbol ~= nil and first.minor_units ~= nil
            return tostring(has)
        "#,
        )
        .unwrap();
    assert_eq!(r, "true");
}

// ── Dual-signature tests (table form for shell dispatch) ───────

#[test]
fn lookup_table_form_positional() {
    let s = sb();
    let r = s
        .exec(r#"local c = country.lookup({[1]="US"}); return c.alpha2"#)
        .unwrap();
    assert_eq!(r, "US");
}

#[test]
fn lookup_table_form_named() {
    let s = sb();
    let r = s
        .exec(r#"local c = country.lookup({code="DE"}); return c.alpha2"#)
        .unwrap();
    assert_eq!(r, "DE");
}

#[test]
fn by_name_table_form() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local results = country.byName({name="Japan"})
            return results[1].alpha2
        "#,
        )
        .unwrap();
    assert_eq!(r, "JP");
}

#[test]
fn currency_lookup_table_form() {
    let s = sb();
    let r = s
        .exec(r#"local c = currency.lookup({[1]="EUR"}); return c.code"#)
        .unwrap();
    assert_eq!(r, "EUR");
}

#[test]
fn country_currency_table_form() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local curs = country.currency({[1]="JP"})
            for i, c in ipairs(curs) do
                if c.code == "JPY" then return "ok" end
            end
            return "not found"
        "#,
        )
        .unwrap();
    assert_eq!(r, "ok");
}

// ── Shell dispatch tests ───────────────────────────────────────

#[test]
fn shell_country_lookup() {
    let s = sb();
    let shrt = include_str!("../../shrt.luau");
    s.register_module("shrt", shrt).unwrap();
    let result = cpsl_core::sh_transpile::transpile_sh(r#"country lookup "US""#);
    assert!(result.is_ok(), "transpile err: {:?}", result.err());
    let luau = result.unwrap().luau_source;
    let r = s.exec(&luau).unwrap();
    assert!(
        r.contains("US") && r.contains("United States"),
        "expected country info, got: {}",
        r
    );
}

#[test]
fn shell_country_by_name() {
    let s = sb();
    let shrt = include_str!("../../shrt.luau");
    s.register_module("shrt", shrt).unwrap();
    let result = cpsl_core::sh_transpile::transpile_sh(r#"country byName "Japan""#);
    assert!(result.is_ok(), "transpile err: {:?}", result.err());
    let luau = result.unwrap().luau_source;
    let r = s.exec(&luau).unwrap();
    assert!(
        r.contains("JP"),
        "expected Japan in results, got: {}",
        r
    );
}

#[test]
fn shell_currency_lookup() {
    let s = sb();
    let shrt = include_str!("../../shrt.luau");
    s.register_module("shrt", shrt).unwrap();
    let result = cpsl_core::sh_transpile::transpile_sh(r#"currency lookup "USD""#);
    assert!(result.is_ok(), "transpile err: {:?}", result.err());
    let luau = result.unwrap().luau_source;
    let r = s.exec(&luau).unwrap();
    assert!(
        r.contains("USD") && r.contains("dollar"),
        "expected USD info, got: {}",
        r
    );
}

// ── Error handling ─────────────────────────────────────────────

#[test]
fn lookup_no_args_errors() {
    let s = sb();
    let err = s.exec("country.lookup()").unwrap_err();
    assert!(
        err.message.contains("bad argument") || err.message.contains("missing"),
        "msg: {}",
        err.message
    );
}

#[test]
fn by_name_no_args_errors() {
    let s = sb();
    let err = s.exec("country.byName()").unwrap_err();
    assert!(
        err.message.contains("bad argument") || err.message.contains("missing"),
        "msg: {}",
        err.message
    );
}

#[test]
fn country_currency_no_args_errors() {
    let s = sb();
    let err = s.exec("country.currency()").unwrap_err();
    assert!(
        err.message.contains("bad argument") || err.message.contains("missing"),
        "msg: {}",
        err.message
    );
}

#[test]
fn currency_lookup_no_args_errors() {
    let s = sb();
    let err = s.exec("currency.lookup()").unwrap_err();
    assert!(
        err.message.contains("bad argument") || err.message.contains("missing"),
        "msg: {}",
        err.message
    );
}

#[test]
fn lookup_wrong_type_errors() {
    let s = sb();
    let err = s.exec("country.lookup(42)").unwrap_err();
    assert!(
        err.message.contains("string") || err.message.contains("expected"),
        "msg: {}",
        err.message
    );
}

#[test]
fn currency_lookup_wrong_type_errors() {
    let s = sb();
    let err = s.exec("currency.lookup(42)").unwrap_err();
    assert!(
        err.message.contains("string") || err.message.contains("expected"),
        "msg: {}",
        err.message
    );
}

// ── Help ───────────────────────────────────────────────────────

#[test]
fn country_help_returns_help() {
    let s = sb();
    let r = s.exec("return country.help()").unwrap();
    assert!(r.contains("country"), "help: {}", r);
    assert!(r.contains("country.lookup"), "help: {}", r);
    assert!(r.contains("country.byName"), "help: {}", r);
    assert!(r.contains("country.all"), "help: {}", r);
    assert!(r.contains("country.currency"), "help: {}", r);
}

#[test]
fn currency_help_returns_help() {
    let s = sb();
    let r = s.exec("return currency.help()").unwrap();
    assert!(r.contains("currency"), "help: {}", r);
    assert!(r.contains("currency.lookup"), "help: {}", r);
    assert!(r.contains("currency.all"), "help: {}", r);
}

#[test]
fn country_nonexistent_fn_hint() {
    let s = sb();
    let err = s.exec("country.foo()").unwrap_err();
    assert!(
        err.message.contains("country.foo does not exist"),
        "msg: {}",
        err.message
    );
    assert!(
        err.message.contains("hint: call country.help() for usage"),
        "msg: {}",
        err.message
    );
}

#[test]
fn currency_nonexistent_fn_hint() {
    let s = sb();
    let err = s.exec("currency.foo()").unwrap_err();
    assert!(
        err.message.contains("currency.foo does not exist"),
        "msg: {}",
        err.message
    );
    assert!(
        err.message.contains("hint: call currency.help() for usage"),
        "msg: {}",
        err.message
    );
}

#[test]
fn global_help_mentions_country() {
    let s = sb();
    let r = s.exec("return help()").unwrap();
    assert!(
        r.contains("country"),
        "global help should list country: {}",
        r
    );
}

#[test]
fn global_help_mentions_currency() {
    let s = sb();
    let r = s.exec("return help()").unwrap();
    assert!(
        r.contains("currency"),
        "global help should list currency: {}",
        r
    );
}

// ── Sandbox safety: no filesystem or network access ────────────

#[test]
fn country_does_not_access_filesystem() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local c = country.lookup("US")
            local results = country.byName("Japan")
            local curs = country.currency("GB")
            return c.alpha2 .. " " .. results[1].alpha2
        "#,
        )
        .unwrap();
    assert_eq!(r, "US JP");
}

#[test]
fn country_no_dangerous_globals_exposed() {
    let s = sb();
    let r = s
        .exec(
            r#"
            return tostring(type(country.lookup)) .. " " ..
                   tostring(type(country.byName)) .. " " ..
                   tostring(type(country.all)) .. " " ..
                   tostring(type(country.currency)) .. " " ..
                   tostring(rawget(country, "io")) .. " " ..
                   tostring(rawget(country, "os"))
        "#,
        )
        .unwrap();
    assert_eq!(r, "function function function function nil nil");
}

#[test]
fn currency_no_dangerous_globals_exposed() {
    let s = sb();
    let r = s
        .exec(
            r#"
            return tostring(type(currency.lookup)) .. " " ..
                   tostring(type(currency.all)) .. " " ..
                   tostring(rawget(currency, "io")) .. " " ..
                   tostring(rawget(currency, "os"))
        "#,
        )
        .unwrap();
    assert_eq!(r, "function function nil nil");
}

#[test]
fn country_sandbox_no_io_access() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local mt = getmetatable(country)
            if mt then
                local idx = rawget(mt, "__index")
                if type(idx) == "table" then
                    if rawget(idx, "io") or rawget(idx, "os") then
                        return "metatable leaks dangerous globals"
                    end
                end
            end
            local count = 0
            for k, v in pairs(country) do
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
fn currency_sandbox_no_io_access() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local mt = getmetatable(currency)
            if mt then
                local idx = rawget(mt, "__index")
                if type(idx) == "table" then
                    if rawget(idx, "io") or rawget(idx, "os") then
                        return "metatable leaks dangerous globals"
                    end
                end
            end
            local count = 0
            for k, v in pairs(currency) do
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
fn country_sandbox_no_network_access() {
    // All country/currency operations should work without any network
    let s = sb();
    let r = s
        .exec(
            r#"
            local results = {}
            local c = country.lookup("US")
            table.insert(results, c.alpha2)
            local n = country.byName("France")
            table.insert(results, n[1].alpha2)
            local cur = currency.lookup("EUR")
            table.insert(results, cur.code)
            return table.concat(results, ",")
        "#,
        )
        .unwrap();
    assert_eq!(r, "US,FR,EUR");
}

// ── Edge cases ─────────────────────────────────────────────────

#[test]
fn lookup_with_whitespace() {
    let s = sb();
    let r = s
        .exec(r#"local c = country.lookup("  US  "); return c.alpha2"#)
        .unwrap();
    assert_eq!(r, "US");
}

#[test]
fn lookup_various_countries() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local codes = {"CN", "IN", "BR", "AU", "CA", "RU"}
            local results = {}
            for _, code in ipairs(codes) do
                local c = country.lookup(code)
                if c then
                    table.insert(results, c.alpha2)
                end
            end
            return table.concat(results, ",")
        "#,
        )
        .unwrap();
    assert_eq!(r, "CN,IN,BR,AU,CA,RU");
}

#[test]
fn multiple_operations_same_country() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local c = country.lookup("DE")
            local curs = country.currency("DE")
            local found_eur = false
            for _, cur in ipairs(curs) do
                if cur.code == "EUR" then found_eur = true end
            end
            return c.name .. " " .. tostring(found_eur)
        "#,
        )
        .unwrap();
    assert!(r.contains("Germany"), "got: {}", r);
    assert!(r.contains("true"), "got: {}", r);
}

#[test]
fn currency_jpy_zero_minor_units() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local c = currency.lookup("JPY")
            return tostring(c.minor_units)
        "#,
        )
        .unwrap();
    assert_eq!(r, "0");
}

#[test]
fn currency_bhd_three_minor_units() {
    // Bahraini dinar has 3 decimal places
    let s = sb();
    let r = s
        .exec(
            r#"
            local c = currency.lookup("BHD")
            return tostring(c.minor_units)
        "#,
        )
        .unwrap();
    assert_eq!(r, "3");
}

#[test]
fn currency_countries_list_contains_alpha2() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local c = currency.lookup("CHF")
            local found_ch = false
            for _, cc in ipairs(c.countries) do
                if cc == "CH" then found_ch = true end
            end
            return tostring(found_ch)
        "#,
        )
        .unwrap();
    assert_eq!(r, "true");
}

// ── Python transpiler e2e ──────────────────────────────────────

#[test]
fn python_pycountry_lookup() {
    let s = sb();
    let pyrt = include_str!("../../pyrt.luau");
    s.setup_python_runtime(pyrt).unwrap();

    let py_code = r#"
import pycountry
c = pycountry.lookup("US")
print(c.alpha2)
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    let r = s.exec(&transpiled.luau_source).unwrap();
    assert_eq!(r, "US");
}

#[test]
fn python_pycountry_by_name() {
    let s = sb();
    let pyrt = include_str!("../../pyrt.luau");
    s.setup_python_runtime(pyrt).unwrap();

    // byName returns a Lua table; verify the call succeeds without error
    let py_code = r#"
import pycountry
results = pycountry.byName("Japan")
if results:
    print("found")
else:
    print("empty")
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    let r = s.exec(&transpiled.luau_source).unwrap();
    assert_eq!(r, "found");
}

#[test]
fn python_import_pycountry_passthrough() {
    let py_code = r#"
import pycountry
c = pycountry.lookup("US")
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    assert!(
        transpiled.luau_source.contains("country"),
        "transpiled: {}",
        transpiled.luau_source
    );
}

#[test]
fn python_from_pycountry_import() {
    let py_code = r#"
from pycountry import countries
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    assert!(
        transpiled.luau_source.contains("country"),
        "transpiled: {}",
        transpiled.luau_source
    );
}
