//! Yahoo Finance module for the Luau sandbox.
//!
//! Exposes `yfinance.history`, `yfinance.info`, `yfinance.quote` as globals.
//! All logic in Rust — constructs Yahoo Finance API URLs, makes HTTP requests
//! via the shared `HttpGateway`, parses JSON responses into Lua tables.

use crate::sandbox::{
    validate_args, wrap_module_with_help_hints, FnDoc, ModuleDoc, Param, ParamType, ReturnType,
};
use mlua::{Lua, MultiValue, Value};
use native_http::{Headers, HttpGateway, Method, Request};
use std::sync::Arc;

pub(crate) static YFINANCE_DOC: ModuleDoc = ModuleDoc {
    name: "yfinance",
    summary: "Yahoo Finance data (stock quotes, history, company info)",
    functions: &[
        FnDoc {
            name: "history",
            description: "Fetch historical price data for a ticker. Returns array of {date, open, high, low, close, volume}.",
            params: &[
                Param { name: "ticker", short: Some('t'), typ: ParamType::String, required: true, fields: None },
                Param { name: "period", short: Some('p'), typ: ParamType::String, required: false, fields: None },
                Param { name: "interval", short: Some('i'), typ: ParamType::String, required: false, fields: None },
                Param { name: "start", short: Some('s'), typ: ParamType::String, required: false, fields: None },
                Param { name: "end", short: Some('e'), typ: ParamType::String, required: false, fields: None },
            ],
            returns: ReturnType::Table,
            example: Some(r#"yfinance.history({ticker="AAPL", period="3mo", interval="1d"})"#),
        },
        FnDoc {
            name: "quote",
            description: "Get current quote for a ticker. Returns {price, change, changePercent, volume, marketCap, name, exchange, currency}.",
            params: &[
                Param { name: "ticker", short: Some('t'), typ: ParamType::String, required: true, fields: None },
            ],
            returns: ReturnType::Table,
            example: Some(r#"yfinance.quote("AAPL")"#),
        },
        FnDoc {
            name: "info",
            description: "Get company information for a ticker. Returns {name, sector, industry, country, website, employees, description, marketCap, ...}.",
            params: &[
                Param { name: "ticker", short: Some('t'), typ: ParamType::String, required: true, fields: None },
            ],
            returns: ReturnType::Table,
            example: None,
        },
        FnDoc {
            name: "search",
            description: "Search for tickers by name or keyword. Returns array of {symbol, name, type, exchange}.",
            params: &[
                Param { name: "query", short: Some('q'), typ: ParamType::String, required: true, fields: None },
                Param { name: "count", short: Some('n'), typ: ParamType::Number, required: false, fields: None },
            ],
            returns: ReturnType::Table,
            example: Some(r#"yfinance.search({query="Apple", count=5})"#),
        },
    ],
};

// ── Yahoo Finance API base URL ────────────────────────────────────────────

const YF_CHART_BASE: &str = "https://query1.finance.yahoo.com/v8/finance/chart";
const YF_QUOTE_BASE: &str = "https://query1.finance.yahoo.com/v10/finance/quoteSummary";
const YF_SEARCH_BASE: &str = "https://query1.finance.yahoo.com/v1/finance/search";

// ── Period/interval validation ────────────────────────────────────────────

fn validate_period(period: &str) -> Result<&str, String> {
    match period {
        "1d" | "5d" | "1mo" | "3mo" | "6mo" | "1y" | "2y" | "5y" | "10y" | "ytd" | "max" => {
            Ok(period)
        }
        _ => Err(format!(
            "yfinance.history: invalid period '{}'. Valid: 1d, 5d, 1mo, 3mo, 6mo, 1y, 2y, 5y, 10y, ytd, max",
            period
        )),
    }
}

fn validate_interval(interval: &str) -> Result<&str, String> {
    match interval {
        "1m" | "2m" | "5m" | "15m" | "30m" | "60m" | "90m" | "1h" | "1d" | "5d" | "1wk"
        | "1mo" | "3mo" => Ok(interval),
        _ => Err(format!(
            "yfinance.history: invalid interval '{}'. Valid: 1m, 2m, 5m, 15m, 30m, 60m, 90m, 1h, 1d, 5d, 1wk, 1mo, 3mo",
            interval
        )),
    }
}

/// Sanitize ticker symbol — only allow alphanumeric, dots, hyphens, carets, equals.
fn sanitize_ticker(ticker: &str) -> Result<String, String> {
    let trimmed = ticker.trim();
    if trimmed.is_empty() {
        return Err("yfinance: ticker cannot be empty".into());
    }
    if trimmed.len() > 20 {
        return Err("yfinance: ticker too long (max 20 chars)".into());
    }
    for c in trimmed.chars() {
        if !c.is_alphanumeric() && c != '.' && c != '-' && c != '^' && c != '=' {
            return Err(format!(
                "yfinance: invalid character '{}' in ticker '{}'",
                c, trimmed
            ));
        }
    }
    Ok(trimmed.to_uppercase())
}

// ── HTTP request helper ───────────────────────────────────────────────────

fn yf_get(gateway: &HttpGateway, url: &str) -> Result<serde_json::Value, mlua::Error> {
    let mut headers = Headers::new();
    headers.insert(
        "User-Agent".to_string(),
        "Mozilla/5.0 (compatible; sandbox/1.0)".to_string(),
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
            "yfinance: HTTP {} from Yahoo Finance API (url: {})",
            response.status, url
        )));
    }

    let body_str = std::str::from_utf8(&response.body)
        .map_err(|e| mlua::Error::external(format!("yfinance: invalid UTF-8 response: {}", e)))?;

    serde_json::from_str(body_str)
        .map_err(|e| mlua::Error::external(format!("yfinance: invalid JSON response: {}", e)))
}

// ── yfinance.history implementation ───────────────────────────────────────

fn do_history(
    lua: &Lua,
    gateway: &HttpGateway,
    ticker: &str,
    period: Option<&str>,
    interval: Option<&str>,
    start: Option<&str>,
    end_date: Option<&str>,
) -> Result<mlua::Table, mlua::Error> {
    let ticker = sanitize_ticker(ticker).map_err(mlua::Error::external)?;

    let interval = interval.unwrap_or("1d");
    validate_interval(interval).map_err(mlua::Error::external)?;

    // Build URL
    let mut url = format!("{}/{}?interval={}", YF_CHART_BASE, ticker, interval);

    if let (Some(start), Some(end)) = (start, end_date) {
        // Use start/end timestamps
        let start_ts = parse_date_to_unix(start).map_err(mlua::Error::external)?;
        let end_ts = parse_date_to_unix(end).map_err(mlua::Error::external)?;
        url.push_str(&format!("&period1={}&period2={}", start_ts, end_ts));
    } else {
        let period = period.unwrap_or("1mo");
        validate_period(period).map_err(mlua::Error::external)?;
        url.push_str(&format!("&range={}", period));
    }

    let json = yf_get(gateway, &url)?;

    // Parse chart response into array of {date, open, high, low, close, volume}
    parse_chart_response(lua, &json)
}

/// Parse a date string (YYYY-MM-DD) to Unix timestamp.
fn parse_date_to_unix(date: &str) -> Result<i64, String> {
    // Simple YYYY-MM-DD parser — avoid pulling in chrono just for this
    let parts: Vec<&str> = date.split('-').collect();
    if parts.len() != 3 {
        return Err(format!(
            "yfinance: invalid date '{}', expected YYYY-MM-DD",
            date
        ));
    }
    let year: i64 = parts[0]
        .parse()
        .map_err(|_| format!("yfinance: invalid year in '{}'", date))?;
    let month: i64 = parts[1]
        .parse()
        .map_err(|_| format!("yfinance: invalid month in '{}'", date))?;
    let day: i64 = parts[2]
        .parse()
        .map_err(|_| format!("yfinance: invalid day in '{}'", date))?;

    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || year < 1970 {
        return Err(format!(
            "yfinance: date out of range '{}' (must be >= 1970-01-01)",
            date
        ));
    }

    // Days from epoch using a simplified calculation
    // (matches standard Unix timestamp calculation for dates)
    let mut total_days: i64 = 0;
    for y in 1970..year {
        total_days += if is_leap_year(y) { 366 } else { 365 };
    }
    let days_in_month = [0, 31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    for m in 1..month {
        total_days += days_in_month[m as usize] as i64;
        if m == 2 && is_leap_year(year) {
            total_days += 1;
        }
    }
    total_days += day - 1;

    Ok(total_days * 86400)
}

fn is_leap_year(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

/// Parse Yahoo Finance v8 chart response into a Lua table of price bars.
fn parse_chart_response(lua: &Lua, json: &serde_json::Value) -> Result<mlua::Table, mlua::Error> {
    let result = json
        .get("chart")
        .and_then(|c| c.get("result"))
        .and_then(|r| r.get(0))
        .ok_or_else(|| {
            mlua::Error::external("yfinance: unexpected API response format (no chart.result[0])")
        })?;

    let timestamps = result
        .get("timestamp")
        .and_then(|t| t.as_array())
        .ok_or_else(|| mlua::Error::external("yfinance: no timestamp data in response"))?;

    let indicators = result
        .get("indicators")
        .and_then(|i| i.get("quote"))
        .and_then(|q| q.get(0))
        .ok_or_else(|| mlua::Error::external("yfinance: no quote data in response"))?;

    let opens = indicators.get("open").and_then(|v| v.as_array());
    let highs = indicators.get("high").and_then(|v| v.as_array());
    let lows = indicators.get("low").and_then(|v| v.as_array());
    let closes = indicators.get("close").and_then(|v| v.as_array());
    let volumes = indicators.get("volume").and_then(|v| v.as_array());

    // Also try to get adjusted close
    let adj_close = result
        .get("indicators")
        .and_then(|i| i.get("adjclose"))
        .and_then(|a| a.get(0))
        .and_then(|a| a.get("adjclose"))
        .and_then(|v| v.as_array());

    let rows = lua.create_table()?;

    for (i, ts) in timestamps.iter().enumerate() {
        let row = lua.create_table()?;

        // Convert Unix timestamp to YYYY-MM-DD string
        if let Some(ts_val) = ts.as_i64() {
            row.set("date", lua.create_string(&unix_to_date_string(ts_val))?)?;
            row.set("timestamp", ts_val as f64)?;
        }

        if let Some(arr) = opens {
            if let Some(v) = arr.get(i).and_then(|v| v.as_f64()) {
                row.set("open", v)?;
            }
        }
        if let Some(arr) = highs {
            if let Some(v) = arr.get(i).and_then(|v| v.as_f64()) {
                row.set("high", v)?;
            }
        }
        if let Some(arr) = lows {
            if let Some(v) = arr.get(i).and_then(|v| v.as_f64()) {
                row.set("low", v)?;
            }
        }
        if let Some(arr) = closes {
            if let Some(v) = arr.get(i).and_then(|v| v.as_f64()) {
                row.set("close", v)?;
            }
        }
        if let Some(arr) = volumes {
            if let Some(v) = arr.get(i).and_then(|v| v.as_f64()) {
                row.set("volume", v)?;
            }
        }
        if let Some(arr) = adj_close {
            if let Some(v) = arr.get(i).and_then(|v| v.as_f64()) {
                row.set("adjclose", v)?;
            }
        }

        rows.set(i + 1, row)?;
    }

    Ok(rows)
}

/// Convert Unix timestamp to YYYY-MM-DD string.
fn unix_to_date_string(ts: i64) -> String {
    let mut days = ts / 86400;
    let mut year = 1970i64;

    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if days < days_in_year {
            break;
        }
        days -= days_in_year;
        year += 1;
    }

    let leap = is_leap_year(year);
    let days_in_months = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 0usize;
    while month < 12 && days >= days_in_months[month] {
        days -= days_in_months[month];
        month += 1;
    }

    format!("{:04}-{:02}-{:02}", year, month + 1, days + 1)
}

// ── yfinance.quote implementation ─────────────────────────────────────────

fn do_quote(lua: &Lua, gateway: &HttpGateway, ticker: &str) -> Result<mlua::Table, mlua::Error> {
    let ticker = sanitize_ticker(ticker).map_err(mlua::Error::external)?;

    let url = format!("{}/{}?modules=price", YF_QUOTE_BASE, ticker);

    let json = yf_get(gateway, &url)?;

    let price = json
        .get("quoteSummary")
        .and_then(|q| q.get("result"))
        .and_then(|r| r.get(0))
        .and_then(|r| r.get("price"))
        .ok_or_else(|| {
            mlua::Error::external(
                "yfinance: unexpected API response format (no quoteSummary.result[0].price)",
            )
        })?;

    let result = lua.create_table()?;

    // Extract common price fields with the raw/fmt pattern Yahoo uses
    set_yf_raw(lua, &result, price, "regularMarketPrice", "price")?;
    set_yf_raw(lua, &result, price, "regularMarketChange", "change")?;
    set_yf_raw(
        lua,
        &result,
        price,
        "regularMarketChangePercent",
        "changePercent",
    )?;
    set_yf_raw(lua, &result, price, "regularMarketVolume", "volume")?;
    set_yf_raw(lua, &result, price, "marketCap", "marketCap")?;
    set_yf_raw(lua, &result, price, "regularMarketDayHigh", "dayHigh")?;
    set_yf_raw(lua, &result, price, "regularMarketDayLow", "dayLow")?;
    set_yf_raw(lua, &result, price, "regularMarketOpen", "open")?;
    set_yf_raw(
        lua,
        &result,
        price,
        "regularMarketPreviousClose",
        "previousClose",
    )?;

    // String fields
    if let Some(s) = price.get("shortName").and_then(|v| v.as_str()) {
        result.set("name", lua.create_string(s)?)?;
    }
    if let Some(s) = price.get("exchangeName").and_then(|v| v.as_str()) {
        result.set("exchange", lua.create_string(s)?)?;
    }
    if let Some(s) = price.get("currency").and_then(|v| v.as_str()) {
        result.set("currency", lua.create_string(s)?)?;
    }
    if let Some(s) = price.get("symbol").and_then(|v| v.as_str()) {
        result.set("symbol", lua.create_string(s)?)?;
    }

    Ok(result)
}

/// Extract a Yahoo Finance "raw" numeric value from a {raw, fmt} object.
fn set_yf_raw(
    lua: &Lua,
    table: &mlua::Table,
    parent: &serde_json::Value,
    yf_key: &str,
    lua_key: &str,
) -> Result<(), mlua::Error> {
    if let Some(obj) = parent.get(yf_key) {
        if let Some(raw) = obj.get("raw").and_then(|v| v.as_f64()) {
            table.set(lua_key, raw)?;
        } else if let Some(raw) = obj.as_f64() {
            // Sometimes the value is directly a number, not a {raw, fmt} object
            table.set(lua_key, raw)?;
        } else if let Some(s) = obj.as_str() {
            table.set(lua_key, lua.create_string(s)?)?;
        }
    }
    Ok(())
}

// ── yfinance.info implementation ──────────────────────────────────────────

fn do_info(lua: &Lua, gateway: &HttpGateway, ticker: &str) -> Result<mlua::Table, mlua::Error> {
    let ticker = sanitize_ticker(ticker).map_err(mlua::Error::external)?;

    let url = format!(
        "{}/{}?modules=assetProfile,price,summaryDetail,defaultKeyStatistics",
        YF_QUOTE_BASE, ticker
    );

    let json = yf_get(gateway, &url)?;

    let modules = json
        .get("quoteSummary")
        .and_then(|q| q.get("result"))
        .and_then(|r| r.get(0))
        .ok_or_else(|| {
            mlua::Error::external(
                "yfinance: unexpected API response format (no quoteSummary.result[0])",
            )
        })?;

    let result = lua.create_table()?;

    // From assetProfile
    if let Some(profile) = modules.get("assetProfile") {
        set_str(lua, &result, profile, "sector", "sector")?;
        set_str(lua, &result, profile, "industry", "industry")?;
        set_str(lua, &result, profile, "country", "country")?;
        set_str(lua, &result, profile, "website", "website")?;
        set_str(lua, &result, profile, "longBusinessSummary", "description")?;
        set_str(lua, &result, profile, "city", "city")?;
        set_str(lua, &result, profile, "state", "state")?;
        set_str(lua, &result, profile, "zip", "zip")?;
        set_str(lua, &result, profile, "phone", "phone")?;
        if let Some(emp) = profile.get("fullTimeEmployees").and_then(|v| v.as_i64()) {
            result.set("employees", emp as f64)?;
        }
    }

    // From price
    if let Some(price) = modules.get("price") {
        set_str(lua, &result, price, "shortName", "name")?;
        set_str(lua, &result, price, "longName", "longName")?;
        set_str(lua, &result, price, "exchangeName", "exchange")?;
        set_str(lua, &result, price, "currency", "currency")?;
        set_str(lua, &result, price, "symbol", "symbol")?;
        set_str(lua, &result, price, "quoteType", "quoteType")?;
        set_yf_raw(lua, &result, price, "marketCap", "marketCap")?;
        set_yf_raw(lua, &result, price, "regularMarketPrice", "price")?;
    }

    // From summaryDetail
    if let Some(summary) = modules.get("summaryDetail") {
        set_yf_raw(lua, &result, summary, "dividendYield", "dividendYield")?;
        set_yf_raw(lua, &result, summary, "trailingPE", "pe")?;
        set_yf_raw(lua, &result, summary, "forwardPE", "forwardPE")?;
        set_yf_raw(
            lua,
            &result,
            summary,
            "fiftyTwoWeekHigh",
            "fiftyTwoWeekHigh",
        )?;
        set_yf_raw(lua, &result, summary, "fiftyTwoWeekLow", "fiftyTwoWeekLow")?;
        set_yf_raw(lua, &result, summary, "beta", "beta")?;
        set_yf_raw(lua, &result, summary, "averageVolume", "avgVolume")?;
    }

    // From defaultKeyStatistics
    if let Some(stats) = modules.get("defaultKeyStatistics") {
        set_yf_raw(lua, &result, stats, "enterpriseValue", "enterpriseValue")?;
        set_yf_raw(lua, &result, stats, "profitMargins", "profitMargins")?;
        set_yf_raw(
            lua,
            &result,
            stats,
            "earningsQuarterlyGrowth",
            "earningsGrowth",
        )?;
        set_yf_raw(
            lua,
            &result,
            stats,
            "revenueQuarterlyGrowth",
            "revenueGrowth",
        )?;
        set_yf_raw(lua, &result, stats, "returnOnEquity", "roe")?;
        set_yf_raw(lua, &result, stats, "returnOnAssets", "roa")?;
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

// ── yfinance.search implementation ────────────────────────────────────────

fn do_search(
    lua: &Lua,
    gateway: &HttpGateway,
    query: &str,
    count: Option<i32>,
) -> Result<mlua::Table, mlua::Error> {
    let query_trimmed = query.trim();
    if query_trimmed.is_empty() {
        return Err(mlua::Error::external(
            "yfinance.search: query cannot be empty",
        ));
    }

    let count = count.unwrap_or(10).min(50).max(1);
    let encoded_query = url_encode(query_trimmed);
    let url = format!(
        "{}?q={}&quotesCount={}&newsCount=0",
        YF_SEARCH_BASE, encoded_query, count
    );

    let json = yf_get(gateway, &url)?;

    let quotes = json
        .get("quotes")
        .and_then(|q| q.as_array())
        .ok_or_else(|| mlua::Error::external("yfinance: unexpected search response format"))?;

    let results = lua.create_table()?;

    for (i, quote) in quotes.iter().enumerate() {
        let row = lua.create_table()?;
        set_str(lua, &row, quote, "symbol", "symbol")?;
        set_str(lua, &row, quote, "shortname", "name")?;
        // Yahoo uses "shortname" (lowercase) in search results
        if row.get::<Option<String>>("name")?.is_none() {
            set_str(lua, &row, quote, "longname", "name")?;
        }
        set_str(lua, &row, quote, "quoteType", "type")?;
        set_str(lua, &row, quote, "exchDisp", "exchange")?;
        results.set(i + 1, row)?;
    }

    Ok(results)
}

/// Simple URL encoding for query strings.
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

// ── Register Luau globals ─────────────────────────────────────────────────

pub(crate) fn register_yfinance_globals(
    lua: &Lua,
    gateway: Arc<HttpGateway>,
) -> Result<(), mlua::Error> {
    let yf_table = lua.create_table()?;

    // yfinance.history(ticker, opts?)
    {
        let gw = gateway.clone();
        yf_table.set(
            "history",
            lua.create_function(move |lua, args: MultiValue| {
                let validated =
                    validate_args(&args, YFINANCE_DOC.params("history"), "yfinance.history")?;
                let ticker = match &validated[0] {
                    Value::String(s) => s.to_string_lossy().to_string(),
                    _ => {
                        return Err(mlua::Error::external(
                            "yfinance.history: 'ticker' must be a string",
                        ))
                    }
                };
                let period = match validated.get(1) {
                    Some(Value::String(s)) => Some(s.to_string_lossy().to_string()),
                    _ => None,
                };
                let interval = match validated.get(2) {
                    Some(Value::String(s)) => Some(s.to_string_lossy().to_string()),
                    _ => None,
                };
                let start = match validated.get(3) {
                    Some(Value::String(s)) => Some(s.to_string_lossy().to_string()),
                    _ => None,
                };
                let end_date = match validated.get(4) {
                    Some(Value::String(s)) => Some(s.to_string_lossy().to_string()),
                    _ => None,
                };
                let result = do_history(
                    lua,
                    &gw,
                    &ticker,
                    period.as_deref(),
                    interval.as_deref(),
                    start.as_deref(),
                    end_date.as_deref(),
                )?;
                Ok(Value::Table(result))
            })?,
        )?;
    }

    // yfinance.quote(ticker)
    {
        let gw = gateway.clone();
        yf_table.set(
            "quote",
            lua.create_function(move |lua, args: MultiValue| {
                let validated =
                    validate_args(&args, YFINANCE_DOC.params("quote"), "yfinance.quote")?;
                let ticker = match &validated[0] {
                    Value::String(s) => s.to_string_lossy().to_string(),
                    _ => {
                        return Err(mlua::Error::external(
                            "yfinance.quote: 'ticker' must be a string",
                        ))
                    }
                };
                let result = do_quote(lua, &gw, &ticker)?;
                Ok(Value::Table(result))
            })?,
        )?;
    }

    // yfinance.info(ticker)
    {
        let gw = gateway.clone();
        yf_table.set(
            "info",
            lua.create_function(move |lua, args: MultiValue| {
                let validated = validate_args(&args, YFINANCE_DOC.params("info"), "yfinance.info")?;
                let ticker = match &validated[0] {
                    Value::String(s) => s.to_string_lossy().to_string(),
                    _ => {
                        return Err(mlua::Error::external(
                            "yfinance.info: 'ticker' must be a string",
                        ))
                    }
                };
                let result = do_info(lua, &gw, &ticker)?;
                Ok(Value::Table(result))
            })?,
        )?;
    }

    // yfinance.search(query, count?)
    {
        let gw = gateway.clone();
        yf_table.set(
            "search",
            lua.create_function(move |lua, args: MultiValue| {
                let validated =
                    validate_args(&args, YFINANCE_DOC.params("search"), "yfinance.search")?;
                let query = match &validated[0] {
                    Value::String(s) => s.to_string_lossy().to_string(),
                    _ => {
                        return Err(mlua::Error::external(
                            "yfinance.search: 'query' must be a string",
                        ))
                    }
                };
                let count = match validated.get(1) {
                    Some(Value::Integer(n)) => Some(*n as i32),
                    Some(Value::Number(n)) => Some(*n as i32),
                    _ => None,
                };
                let result = do_search(lua, &gw, &query, count)?;
                Ok(Value::Table(result))
            })?,
        )?;
    }

    crate::lua_util::register_help_functions(lua, &yf_table, &YFINANCE_DOC)?;

    lua.globals().set("yfinance", yf_table)?;
    wrap_module_with_help_hints(lua, "yfinance")?;

    Ok(())
}

// ── Unit tests for pure functions ─────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_ticker_valid() {
        assert_eq!(sanitize_ticker("AAPL").unwrap(), "AAPL");
        assert_eq!(sanitize_ticker("msft").unwrap(), "MSFT");
        assert_eq!(sanitize_ticker("BRK.B").unwrap(), "BRK.B");
        assert_eq!(sanitize_ticker("^GSPC").unwrap(), "^GSPC");
        assert_eq!(sanitize_ticker("BTC-USD").unwrap(), "BTC-USD");
    }

    #[test]
    fn test_sanitize_ticker_invalid() {
        assert!(sanitize_ticker("").is_err());
        assert!(sanitize_ticker("   ").is_err());
        assert!(sanitize_ticker("AAPL; DROP TABLE").is_err());
        assert!(sanitize_ticker("../../etc/passwd").is_err());
        assert!(sanitize_ticker("A".repeat(21).as_str()).is_err());
    }

    #[test]
    fn test_validate_period() {
        assert!(validate_period("1d").is_ok());
        assert!(validate_period("1mo").is_ok());
        assert!(validate_period("max").is_ok());
        assert!(validate_period("invalid").is_err());
        assert!(validate_period("").is_err());
    }

    #[test]
    fn test_validate_interval() {
        assert!(validate_interval("1d").is_ok());
        assert!(validate_interval("1h").is_ok());
        assert!(validate_interval("1m").is_ok());
        assert!(validate_interval("invalid").is_err());
    }

    #[test]
    fn test_parse_date_to_unix() {
        // 1970-01-01 = 0
        assert_eq!(parse_date_to_unix("1970-01-01").unwrap(), 0);
        // 1970-01-02 = 86400
        assert_eq!(parse_date_to_unix("1970-01-02").unwrap(), 86400);
        // 2024-01-01 — just check it's reasonable
        let ts = parse_date_to_unix("2024-01-01").unwrap();
        assert!(ts > 1_700_000_000 && ts < 1_710_000_000);
    }

    #[test]
    fn test_parse_date_to_unix_invalid() {
        assert!(parse_date_to_unix("not-a-date").is_err());
        assert!(parse_date_to_unix("2024/01/01").is_err());
        assert!(parse_date_to_unix("1969-01-01").is_err());
        assert!(parse_date_to_unix("2024-13-01").is_err());
        assert!(parse_date_to_unix("2024-01-32").is_err());
    }

    #[test]
    fn test_unix_to_date_string() {
        assert_eq!(unix_to_date_string(0), "1970-01-01");
        assert_eq!(unix_to_date_string(86400), "1970-01-02");
        // 2024-01-01 00:00:00 UTC
        let ts = parse_date_to_unix("2024-01-01").unwrap();
        assert_eq!(unix_to_date_string(ts), "2024-01-01");
    }

    #[test]
    fn test_url_encode() {
        assert_eq!(url_encode("AAPL"), "AAPL");
        assert_eq!(url_encode("hello world"), "hello%20world");
        assert_eq!(url_encode("a&b=c"), "a%26b%3Dc");
    }

    #[test]
    fn test_parse_chart_response() {
        let lua = Lua::new();
        let json: serde_json::Value = serde_json::from_str(
            r#"{
            "chart": {
                "result": [{
                    "timestamp": [1704067200, 1704153600],
                    "indicators": {
                        "quote": [{
                            "open": [100.0, 101.0],
                            "high": [105.0, 106.0],
                            "low": [99.0, 100.0],
                            "close": [103.0, 104.0],
                            "volume": [1000000, 1100000]
                        }]
                    }
                }]
            }
        }"#,
        )
        .unwrap();

        let result = parse_chart_response(&lua, &json).unwrap();
        // Should have 2 rows
        assert_eq!(result.len().unwrap(), 2);

        let row1: mlua::Table = result.get(1).unwrap();
        let open: f64 = row1.get("open").unwrap();
        assert!((open - 100.0).abs() < 0.01);
        let close: f64 = row1.get("close").unwrap();
        assert!((close - 103.0).abs() < 0.01);
    }
}
