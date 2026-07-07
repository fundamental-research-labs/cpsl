//! SEC EDGAR module for the Luau sandbox.
//!
//! Exposes `edgar.search`, `edgar.filings`, `edgar.company` as globals.
//! All logic in Rust — constructs SEC EDGAR API URLs, makes HTTP requests
//! via the shared `HttpGateway`, parses JSON responses into Lua tables.
//!
//! SEC EDGAR APIs used (all free, no auth required):
//! - Full-text search: https://efts.sec.gov/LATEST/search-index
//! - Submissions by CIK: https://data.sec.gov/submissions/CIK{cik}.json
//!
//! Rate limit: 10 requests/second (enforced by SEC). User-Agent required.

use crate::sandbox::{
    validate_args, wrap_module_with_help_hints, FnDoc, ModuleDoc, Param, ParamType, ReturnType,
};
use mlua::{Lua, MultiValue, Value};
use native_http::{Headers, HttpGateway, Method, Request};
use std::sync::Arc;

pub(crate) static EDGAR_DOC: ModuleDoc = ModuleDoc {
    name: "edgar",
    summary: "SEC EDGAR filings (full-text search, company filings, company info)",
    functions: &[
        FnDoc {
            name: "search",
            description: "Full-text search of SEC filings. Returns array of {accession, form, filed, company, cik, description, url}.",
            params: &[
                Param { name: "query", short: Some('q'), typ: ParamType::String, required: true, fields: None },
                Param { name: "type", short: Some('t'), typ: ParamType::String, required: false, fields: None },
                Param { name: "start", short: Some('s'), typ: ParamType::String, required: false, fields: None },
                Param { name: "end", short: Some('e'), typ: ParamType::String, required: false, fields: None },
                Param { name: "count", short: Some('n'), typ: ParamType::Number, required: false, fields: None },
            ],
            returns: ReturnType::Table,
            example: Some(r#"edgar.search({query="annual report", type="10-K", start="2024-01-01", count=5})"#),
        },
        FnDoc {
            name: "filings",
            description: "List recent filings for a company by CIK number. Returns array of {accession, form, filed, reportDate, document, description, url}.",
            params: &[
                Param { name: "cik", short: Some('c'), typ: ParamType::String, required: true, fields: None },
                Param { name: "type", short: Some('t'), typ: ParamType::String, required: false, fields: None },
                Param { name: "count", short: Some('n'), typ: ParamType::Number, required: false, fields: None },
            ],
            returns: ReturnType::Table,
            example: Some(r#"edgar.filings({cik="320193", type="10-K", count=5})"#),
        },
        FnDoc {
            name: "company",
            description: "Get company information by CIK number. Returns {cik, name, sic, sicDescription, tickers, exchanges, website, stateOfIncorporation, category, phone}.",
            params: &[
                Param { name: "cik", short: Some('c'), typ: ParamType::String, required: true, fields: None },
            ],
            returns: ReturnType::Table,
            example: Some(r#"edgar.company("320193")"#),
        },
    ],
};

// ── SEC EDGAR API base URLs ──────────────────────────────────────────

const EDGAR_SEARCH_BASE: &str = "https://efts.sec.gov/LATEST/search-index";
const EDGAR_SUBMISSIONS_BASE: &str = "https://data.sec.gov/submissions";

// ── Input validation ─────────────────────────────────────────────────

/// Sanitize and zero-pad a CIK number. Accepts numeric strings (with or without
/// leading zeros) up to 10 digits. Returns 10-digit zero-padded CIK.
fn sanitize_cik(cik: &str) -> Result<String, String> {
    let trimmed = cik.trim();
    if trimmed.is_empty() {
        return Err("edgar: CIK cannot be empty".into());
    }
    // Strip optional "CIK" prefix (case-insensitive)
    let num_part = if trimmed.len() > 3 && trimmed[..3].eq_ignore_ascii_case("cik") {
        trimmed[3..].trim()
    } else {
        trimmed
    };
    // Must be all digits
    if !num_part.chars().all(|c| c.is_ascii_digit()) {
        return Err(format!(
            "edgar: invalid CIK '{}' — must be numeric (e.g., '320193' or 'CIK0000320193')",
            cik
        ));
    }
    if num_part.is_empty() || num_part.len() > 10 {
        return Err(format!("edgar: CIK '{}' out of range (1-10 digits)", cik));
    }
    // Zero-pad to 10 digits
    Ok(format!("{:0>10}", num_part))
}

/// Validate a filing form type (e.g., "10-K", "10-Q", "8-K", "4", "DEF 14A").
fn sanitize_form_type(form: &str) -> Result<String, String> {
    let trimmed = form.trim();
    if trimmed.is_empty() {
        return Err("edgar: form type cannot be empty".into());
    }
    if trimmed.len() > 30 {
        return Err(format!(
            "edgar: form type too long (max 30 chars): '{}'",
            form
        ));
    }
    // Allow alphanumeric, hyphens, slashes, spaces (SEC form types use all of these)
    for c in trimmed.chars() {
        if !c.is_alphanumeric() && c != '-' && c != '/' && c != ' ' {
            return Err(format!(
                "edgar: invalid character '{}' in form type '{}'",
                c, trimmed
            ));
        }
    }
    Ok(trimmed.to_uppercase())
}

/// Validate a date string in YYYY-MM-DD format.
fn validate_date(date: &str) -> Result<&str, String> {
    let parts: Vec<&str> = date.split('-').collect();
    if parts.len() != 3 {
        return Err(format!(
            "edgar: invalid date '{}', expected YYYY-MM-DD",
            date
        ));
    }
    let year: i32 = parts[0]
        .parse()
        .map_err(|_| format!("edgar: invalid year in '{}'", date))?;
    let month: i32 = parts[1]
        .parse()
        .map_err(|_| format!("edgar: invalid month in '{}'", date))?;
    let day: i32 = parts[2]
        .parse()
        .map_err(|_| format!("edgar: invalid day in '{}'", date))?;
    if !(1993..=2100).contains(&year) || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return Err(format!(
            "edgar: date out of range '{}' (EDGAR filings available from 1993)",
            date
        ));
    }
    Ok(date)
}

/// Validate a search query string.
fn validate_query(query: &str) -> Result<&str, String> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Err("edgar.search: query cannot be empty".into());
    }
    if trimmed.len() > 500 {
        return Err("edgar.search: query too long (max 500 chars)".into());
    }
    Ok(trimmed)
}

// ── HTTP request helper ──────────────────────────────────────────────

fn edgar_get(gateway: &HttpGateway, url: &str) -> Result<serde_json::Value, mlua::Error> {
    let mut headers = Headers::new();
    // SEC EDGAR requires a descriptive User-Agent
    headers.insert(
        "User-Agent".to_string(),
        "sandbox/1.0 (sandbox@example.com)".to_string(),
    );
    headers.insert("Accept".to_string(), "application/json".to_string());

    let request = Request {
        method: Method::Get,
        url: url.to_string(),
        headers,
        body: None,
    };

    let response = gateway.request(request).map_err(mlua::Error::external)?;

    if !response.ok() {
        return Err(mlua::Error::external(format!(
            "edgar: HTTP {} from SEC EDGAR API (url: {})",
            response.status, url
        )));
    }

    let body_str = std::str::from_utf8(&response.body)
        .map_err(|e| mlua::Error::external(format!("edgar: invalid UTF-8 response: {}", e)))?;

    serde_json::from_str(body_str)
        .map_err(|e| mlua::Error::external(format!("edgar: invalid JSON response: {}", e)))
}

// ── Simple URL encoding ──────────────────────────────────────────────

fn url_encode(s: &str) -> String {
    let mut encoded = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(b as char);
            }
            b' ' => encoded.push_str("%20"),
            _ => {
                encoded.push_str(&format!("%{:02X}", b));
            }
        }
    }
    encoded
}

// ── edgar.search implementation ──────────────────────────────────────

fn do_search(
    lua: &Lua,
    gateway: &HttpGateway,
    query: &str,
    form_type: Option<&str>,
    start_date: Option<&str>,
    end_date: Option<&str>,
    count: Option<i32>,
) -> Result<mlua::Table, mlua::Error> {
    let query = validate_query(query).map_err(mlua::Error::external)?;
    let count = count.unwrap_or(10).min(100).max(1);

    let mut url = format!(
        "{}?q={}&from=0&size={}",
        EDGAR_SEARCH_BASE,
        url_encode(query),
        count
    );

    if let Some(form) = form_type {
        let form = sanitize_form_type(form).map_err(mlua::Error::external)?;
        url.push_str(&format!("&forms={}", url_encode(&form)));
    }

    if start_date.is_some() || end_date.is_some() {
        url.push_str("&dateRange=custom");
        if let Some(start) = start_date {
            validate_date(start).map_err(mlua::Error::external)?;
            url.push_str(&format!("&startdt={}", start));
        }
        if let Some(end) = end_date {
            validate_date(end).map_err(mlua::Error::external)?;
            url.push_str(&format!("&enddt={}", end));
        }
    }

    let json = edgar_get(gateway, &url)?;

    // Parse Elasticsearch-style response from efts.sec.gov
    let hits = json
        .get("hits")
        .and_then(|h| h.get("hits"))
        .and_then(|h| h.as_array())
        .ok_or_else(|| {
            mlua::Error::external("edgar: unexpected search response format (no hits.hits)")
        })?;

    let results = lua.create_table()?;

    for (i, hit) in hits.iter().enumerate() {
        let source = match hit.get("_source") {
            Some(s) => s,
            None => continue,
        };

        let row = lua.create_table()?;

        // accession number
        if let Some(adsh) = source.get("adsh").and_then(|v| v.as_str()) {
            row.set("accession", lua.create_string(adsh)?)?;

            // Build document URL from accession number
            // Format: https://www.sec.gov/Archives/edgar/data/{cik}/{accession-no-dashes}/{filename}
            if let Some(ciks) = source.get("ciks").and_then(|v| v.as_array()) {
                if let Some(cik) = ciks.first().and_then(|v| v.as_str()) {
                    let adsh_clean = adsh.replace('-', "");
                    let cik_trimmed = cik.trim_start_matches('0');
                    let url = format!(
                        "https://www.sec.gov/Archives/edgar/data/{}/{}/{}-index.htm",
                        if cik_trimmed.is_empty() {
                            "0"
                        } else {
                            cik_trimmed
                        },
                        adsh_clean,
                        adsh
                    );
                    row.set("url", lua.create_string(&url)?)?;
                }
            }
        }

        // form type
        if let Some(form) = source.get("form").and_then(|v| v.as_str()) {
            row.set("form", lua.create_string(form)?)?;
        }

        // filing date
        if let Some(date) = source.get("file_date").and_then(|v| v.as_str()) {
            row.set("filed", lua.create_string(date)?)?;
        }

        // company display name
        if let Some(names) = source.get("display_names").and_then(|v| v.as_array()) {
            if let Some(name) = names.first().and_then(|v| v.as_str()) {
                row.set("company", lua.create_string(name)?)?;
            }
        }

        // CIK (first one)
        if let Some(ciks) = source.get("ciks").and_then(|v| v.as_array()) {
            if let Some(cik) = ciks.first().and_then(|v| v.as_str()) {
                row.set("cik", lua.create_string(cik)?)?;
            }
        }

        // file description
        if let Some(desc) = source.get("file_description").and_then(|v| v.as_str()) {
            row.set("description", lua.create_string(desc)?)?;
        }

        // period ending
        if let Some(period) = source.get("period_ending").and_then(|v| v.as_str()) {
            row.set("periodEnding", lua.create_string(period)?)?;
        }

        results.set(i + 1, row)?;
    }

    Ok(results)
}

// ── edgar.filings implementation ─────────────────────────────────────

fn do_filings(
    lua: &Lua,
    gateway: &HttpGateway,
    cik: &str,
    form_type: Option<&str>,
    count: Option<i32>,
) -> Result<mlua::Table, mlua::Error> {
    let cik_padded = sanitize_cik(cik).map_err(mlua::Error::external)?;
    let count = count.unwrap_or(20).min(1000).max(1) as usize;

    let form_filter = match form_type {
        Some(f) => Some(sanitize_form_type(f).map_err(mlua::Error::external)?),
        None => None,
    };

    let url = format!("{}/CIK{}.json", EDGAR_SUBMISSIONS_BASE, cik_padded);

    let json = edgar_get(gateway, &url)?;

    // Parse submissions response — columnar format
    let recent = json
        .get("filings")
        .and_then(|f| f.get("recent"))
        .ok_or_else(|| {
            mlua::Error::external(
                "edgar: unexpected submissions response format (no filings.recent)",
            )
        })?;

    let accessions = recent.get("accessionNumber").and_then(|v| v.as_array());
    let forms = recent.get("form").and_then(|v| v.as_array());
    let filing_dates = recent.get("filingDate").and_then(|v| v.as_array());
    let report_dates = recent.get("reportDate").and_then(|v| v.as_array());
    let primary_docs = recent.get("primaryDocument").and_then(|v| v.as_array());
    let descriptions = recent
        .get("primaryDocDescription")
        .and_then(|v| v.as_array());

    let total = accessions.map(|a| a.len()).unwrap_or(0);
    let results = lua.create_table()?;
    let mut row_idx = 0;

    // Strip leading zeros for URL construction
    let cik_trimmed = cik_padded.trim_start_matches('0');
    let cik_for_url = if cik_trimmed.is_empty() {
        "0"
    } else {
        cik_trimmed
    };

    for i in 0..total {
        if row_idx >= count {
            break;
        }

        // Apply form type filter
        if let Some(ref filter) = form_filter {
            if let Some(form_arr) = forms {
                if let Some(form_val) = form_arr.get(i).and_then(|v| v.as_str()) {
                    if !form_val.eq_ignore_ascii_case(filter) {
                        continue;
                    }
                }
            }
        }

        let row = lua.create_table()?;

        if let Some(arr) = accessions {
            if let Some(v) = arr.get(i).and_then(|v| v.as_str()) {
                row.set("accession", lua.create_string(v)?)?;

                // Build document URL
                if let Some(doc_arr) = primary_docs {
                    if let Some(doc) = doc_arr.get(i).and_then(|v| v.as_str()) {
                        if !doc.is_empty() {
                            let adsh_clean = v.replace('-', "");
                            let url = format!(
                                "https://www.sec.gov/Archives/edgar/data/{}/{}/{}",
                                cik_for_url, adsh_clean, doc
                            );
                            row.set("url", lua.create_string(&url)?)?;
                        }
                    }
                }
            }
        }

        if let Some(arr) = forms {
            if let Some(v) = arr.get(i).and_then(|v| v.as_str()) {
                row.set("form", lua.create_string(v)?)?;
            }
        }

        if let Some(arr) = filing_dates {
            if let Some(v) = arr.get(i).and_then(|v| v.as_str()) {
                if !v.is_empty() {
                    row.set("filed", lua.create_string(v)?)?;
                }
            }
        }

        if let Some(arr) = report_dates {
            if let Some(v) = arr.get(i).and_then(|v| v.as_str()) {
                if !v.is_empty() {
                    row.set("reportDate", lua.create_string(v)?)?;
                }
            }
        }

        if let Some(arr) = primary_docs {
            if let Some(v) = arr.get(i).and_then(|v| v.as_str()) {
                if !v.is_empty() {
                    row.set("document", lua.create_string(v)?)?;
                }
            }
        }

        if let Some(arr) = descriptions {
            if let Some(v) = arr.get(i).and_then(|v| v.as_str()) {
                if !v.is_empty() {
                    row.set("description", lua.create_string(v)?)?;
                }
            }
        }

        row_idx += 1;
        results.set(row_idx, row)?;
    }

    Ok(results)
}

// ── edgar.company implementation ─────────────────────────────────────

fn do_company(lua: &Lua, gateway: &HttpGateway, cik: &str) -> Result<mlua::Table, mlua::Error> {
    let cik_padded = sanitize_cik(cik).map_err(mlua::Error::external)?;

    let url = format!("{}/CIK{}.json", EDGAR_SUBMISSIONS_BASE, cik_padded);

    let json = edgar_get(gateway, &url)?;

    let result = lua.create_table()?;

    // Scalar fields
    set_str(lua, &result, &json, "cik", "cik")?;
    set_str(lua, &result, &json, "name", "name")?;
    set_str(lua, &result, &json, "sic", "sic")?;
    set_str(lua, &result, &json, "sicDescription", "sicDescription")?;
    set_str(lua, &result, &json, "entityType", "entityType")?;
    set_str(lua, &result, &json, "website", "website")?;
    set_str(
        lua,
        &result,
        &json,
        "stateOfIncorporation",
        "stateOfIncorporation",
    )?;
    set_str(lua, &result, &json, "category", "category")?;
    set_str(lua, &result, &json, "phone", "phone")?;
    set_str(lua, &result, &json, "ein", "ein")?;
    set_str(lua, &result, &json, "fiscalYearEnd", "fiscalYearEnd")?;
    set_str(lua, &result, &json, "description", "description")?;

    // Array fields → Lua tables
    if let Some(tickers) = json.get("tickers").and_then(|v| v.as_array()) {
        let t = lua.create_table()?;
        for (i, ticker) in tickers.iter().enumerate() {
            if let Some(s) = ticker.as_str() {
                t.set(i + 1, lua.create_string(s)?)?;
            }
        }
        result.set("tickers", t)?;
    }

    if let Some(exchanges) = json.get("exchanges").and_then(|v| v.as_array()) {
        let t = lua.create_table()?;
        for (i, ex) in exchanges.iter().enumerate() {
            if let Some(s) = ex.as_str() {
                t.set(i + 1, lua.create_string(s)?)?;
            }
        }
        result.set("exchanges", t)?;
    }

    // Addresses
    if let Some(addrs) = json.get("addresses") {
        if let Some(biz) = addrs.get("business") {
            let addr = lua.create_table()?;
            set_str(lua, &addr, biz, "street1", "street1")?;
            set_str(lua, &addr, biz, "street2", "street2")?;
            set_str(lua, &addr, biz, "city", "city")?;
            set_str(lua, &addr, biz, "stateOrCountry", "stateOrCountry")?;
            set_str(lua, &addr, biz, "zipCode", "zipCode")?;
            result.set("address", addr)?;
        }
    }

    Ok(result)
}

fn set_str(
    lua: &Lua,
    table: &mlua::Table,
    parent: &serde_json::Value,
    json_key: &str,
    lua_key: &str,
) -> Result<(), mlua::Error> {
    if let Some(s) = parent.get(json_key).and_then(|v| v.as_str()) {
        if !s.is_empty() {
            table.set(lua_key, lua.create_string(s)?)?;
        }
    }
    Ok(())
}

// ── Register Luau globals ────────────────────────────────────────────

pub(crate) fn register_edgar_globals(
    lua: &Lua,
    gateway: Arc<HttpGateway>,
) -> Result<(), mlua::Error> {
    let edgar_table = lua.create_table()?;

    // edgar.search(query, opts?)
    {
        let gw = gateway.clone();
        edgar_table.set(
            "search",
            lua.create_function(move |lua, args: MultiValue| {
                let validated = validate_args(&args, EDGAR_DOC.params("search"), "edgar.search")?;
                let query = match &validated[0] {
                    Value::String(s) => s.to_string_lossy().to_string(),
                    _ => {
                        return Err(mlua::Error::external(
                            "edgar.search: 'query' must be a string",
                        ))
                    }
                };
                let form_type = match validated.get(1) {
                    Some(Value::String(s)) => Some(s.to_string_lossy().to_string()),
                    _ => None,
                };
                let start = match validated.get(2) {
                    Some(Value::String(s)) => Some(s.to_string_lossy().to_string()),
                    _ => None,
                };
                let end_date = match validated.get(3) {
                    Some(Value::String(s)) => Some(s.to_string_lossy().to_string()),
                    _ => None,
                };
                let count = match validated.get(4) {
                    Some(Value::Integer(n)) => Some(*n as i32),
                    Some(Value::Number(n)) => Some(*n as i32),
                    _ => None,
                };
                let result = do_search(
                    lua,
                    &gw,
                    &query,
                    form_type.as_deref(),
                    start.as_deref(),
                    end_date.as_deref(),
                    count,
                )?;
                Ok(Value::Table(result))
            })?,
        )?;
    }

    // edgar.filings(cik, opts?)
    {
        let gw = gateway.clone();
        edgar_table.set(
            "filings",
            lua.create_function(move |lua, args: MultiValue| {
                let validated = validate_args(&args, EDGAR_DOC.params("filings"), "edgar.filings")?;
                let cik = match &validated[0] {
                    Value::String(s) => s.to_string_lossy().to_string(),
                    _ => {
                        return Err(mlua::Error::external(
                            "edgar.filings: 'cik' must be a string",
                        ))
                    }
                };
                let form_type = match validated.get(1) {
                    Some(Value::String(s)) => Some(s.to_string_lossy().to_string()),
                    _ => None,
                };
                let count = match validated.get(2) {
                    Some(Value::Integer(n)) => Some(*n as i32),
                    Some(Value::Number(n)) => Some(*n as i32),
                    _ => None,
                };
                let result = do_filings(lua, &gw, &cik, form_type.as_deref(), count)?;
                Ok(Value::Table(result))
            })?,
        )?;
    }

    // edgar.company(cik)
    {
        let gw = gateway.clone();
        edgar_table.set(
            "company",
            lua.create_function(move |lua, args: MultiValue| {
                let validated = validate_args(&args, EDGAR_DOC.params("company"), "edgar.company")?;
                let cik = match &validated[0] {
                    Value::String(s) => s.to_string_lossy().to_string(),
                    _ => {
                        return Err(mlua::Error::external(
                            "edgar.company: 'cik' must be a string",
                        ))
                    }
                };
                let result = do_company(lua, &gw, &cik)?;
                Ok(Value::Table(result))
            })?,
        )?;
    }

    crate::lua_util::register_help_functions(lua, &edgar_table, &EDGAR_DOC)?;

    lua.globals().set("edgar", edgar_table)?;
    wrap_module_with_help_hints(lua, "edgar")?;

    Ok(())
}

// ── Unit tests for pure functions ────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_cik_valid() {
        assert_eq!(sanitize_cik("320193").unwrap(), "0000320193");
        assert_eq!(sanitize_cik("0000320193").unwrap(), "0000320193");
        assert_eq!(sanitize_cik("CIK0000320193").unwrap(), "0000320193");
        assert_eq!(sanitize_cik("cik320193").unwrap(), "0000320193");
        assert_eq!(sanitize_cik("  320193  ").unwrap(), "0000320193");
        assert_eq!(sanitize_cik("1").unwrap(), "0000000001");
    }

    #[test]
    fn test_sanitize_cik_invalid() {
        assert!(sanitize_cik("").is_err());
        assert!(sanitize_cik("   ").is_err());
        assert!(sanitize_cik("abc").is_err());
        assert!(sanitize_cik("320193abc").is_err());
        assert!(sanitize_cik("12345678901").is_err()); // 11 digits
        assert!(sanitize_cik("../../etc/passwd").is_err());
        assert!(sanitize_cik("CIK").is_err()); // just prefix, no number
    }

    #[test]
    fn test_sanitize_form_type_valid() {
        assert_eq!(sanitize_form_type("10-K").unwrap(), "10-K");
        assert_eq!(sanitize_form_type("10-Q").unwrap(), "10-Q");
        assert_eq!(sanitize_form_type("8-K").unwrap(), "8-K");
        assert_eq!(sanitize_form_type("4").unwrap(), "4");
        assert_eq!(sanitize_form_type("DEF 14A").unwrap(), "DEF 14A");
        assert_eq!(sanitize_form_type("S-1/A").unwrap(), "S-1/A");
        assert_eq!(sanitize_form_type("10-k").unwrap(), "10-K"); // uppercased
    }

    #[test]
    fn test_sanitize_form_type_invalid() {
        assert!(sanitize_form_type("").is_err());
        assert!(sanitize_form_type("10-K; DROP TABLE").is_err());
        assert!(sanitize_form_type("a".repeat(31).as_str()).is_err());
    }

    #[test]
    fn test_validate_date_valid() {
        assert!(validate_date("2024-01-15").is_ok());
        assert!(validate_date("1993-01-01").is_ok());
        assert!(validate_date("2025-12-31").is_ok());
    }

    #[test]
    fn test_validate_date_invalid() {
        assert!(validate_date("not-a-date").is_err());
        assert!(validate_date("2024/01/01").is_err());
        assert!(validate_date("1992-01-01").is_err()); // before EDGAR
        assert!(validate_date("2024-13-01").is_err());
        assert!(validate_date("2024-01-32").is_err());
    }

    #[test]
    fn test_validate_query() {
        assert!(validate_query("apple").is_ok());
        assert!(validate_query("  apple  ").is_ok());
        assert!(validate_query("").is_err());
        assert!(validate_query("   ").is_err());
        assert!(validate_query(&"x".repeat(501)).is_err());
    }

    #[test]
    fn test_url_encode() {
        assert_eq!(url_encode("apple"), "apple");
        assert_eq!(url_encode("hello world"), "hello%20world");
        assert_eq!(url_encode("10-K"), "10-K");
        assert_eq!(url_encode("a&b=c"), "a%26b%3Dc");
    }
}
