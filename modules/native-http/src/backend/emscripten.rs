//! Emscripten HTTP backend that delegates requests to browser fetch.

use super::HttpBackend;
use crate::types::{Headers, HttpError, Request, Response};
use serde::Deserialize;
use serde_json::json;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

extern "C" {
    fn emscripten_run_script_string(script: *const c_char) -> *const c_char;
}

pub struct EmscriptenBackend;

impl EmscriptenBackend {
    pub fn new() -> Self {
        Self
    }
}

impl HttpBackend for EmscriptenBackend {
    fn send(&self, request: &Request) -> Result<Response, HttpError> {
        let script = CString::new(browser_fetch_script(request))
            .map_err(|_| HttpError::RequestFailed("request contained a NUL byte".to_string()))?;
        let response_ptr = unsafe { emscripten_run_script_string(script.as_ptr()) };
        if response_ptr.is_null() {
            return Err(HttpError::RequestFailed(
                "browser HTTP bridge returned no response".to_string(),
            ));
        }

        let response_json = unsafe { CStr::from_ptr(response_ptr) }
            .to_string_lossy()
            .into_owned();
        let bridge_response: BrowserResponse =
            serde_json::from_str(&response_json).map_err(|error| {
                HttpError::RequestFailed(format!(
                    "browser HTTP bridge returned invalid JSON: {error}"
                ))
            })?;

        if !bridge_response.ok {
            return Err(HttpError::RequestFailed(
                bridge_response
                    .error
                    .unwrap_or_else(|| "browser request failed".to_string()),
            ));
        }

        Ok(Response {
            status: bridge_response.status.unwrap_or(0),
            headers: parse_raw_headers(bridge_response.headers.as_deref().unwrap_or("")),
            body: bridge_response.body.unwrap_or_default().into_bytes(),
        })
    }
}

#[derive(Deserialize)]
struct BrowserResponse {
    ok: bool,
    status: Option<u16>,
    headers: Option<String>,
    body: Option<String>,
    error: Option<String>,
}

fn browser_fetch_script(request: &Request) -> String {
    let headers = request
        .headers
        .iter()
        .map(|(key, value)| (key.to_string(), json!(value)))
        .collect::<serde_json::Map<_, _>>();
    let body = request
        .body
        .as_ref()
        .map(|bytes| String::from_utf8_lossy(bytes).into_owned());
    let payload = json!({
        "method": request.method.as_str(),
        "url": request.url,
        "headers": headers,
        "body": body,
    });

    format!(
        r#"(function() {{
  var req = {payload};
  try {{
    var xhr = new XMLHttpRequest();
    xhr.open(req.method, req.url, false);
    for (var key in req.headers) {{
      if (Object.prototype.hasOwnProperty.call(req.headers, key)) {{
        xhr.setRequestHeader(key, req.headers[key]);
      }}
    }}
    xhr.send(req.body === null ? null : req.body);
    if (xhr.status === 0) {{
      return JSON.stringify({{ok:false,error:"browser request failed or was blocked by CORS"}});
    }}
    return JSON.stringify({{
      ok: true,
      status: xhr.status,
      headers: xhr.getAllResponseHeaders() || "",
      body: xhr.responseText || ""
    }});
  }} catch (error) {{
    return JSON.stringify({{
      ok: false,
      error: String(error && (error.message || error))
    }});
  }}
}})()"#
    )
}

fn parse_raw_headers(raw: &str) -> Headers {
    let mut headers = Headers::new();
    for line in raw.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        headers.insert(key.trim(), value.trim());
    }
    headers
}
