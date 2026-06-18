#![cfg(feature = "mod-http")]

use cpsl_core::Sandbox;
use native_http::HttpGateway;
use std::sync::Arc;

fn sandbox_with_http(allowed_domains: &[&str]) -> Sandbox {
    sandbox_with_http_policy(allowed_domains, &[])
}

fn sandbox_with_http_policy(allowed_domains: &[&str], denied_domains: &[&str]) -> Sandbox {
    let mut builder = HttpGateway::builder();
    for d in allowed_domains {
        builder = builder.allow_domain(*d);
    }
    for d in denied_domains {
        builder = builder.deny_domain(*d);
    }
    let gw = Arc::new(builder.build());
    Sandbox::builder().http_gateway(gw).build().unwrap()
}

fn sandbox_with_http_and_creds(domain: &str, header_key: &str, header_val: &str) -> Sandbox {
    let gw = Arc::new(
        HttpGateway::builder()
            .allow_domain(domain)
            .credentials(domain, vec![(header_key.into(), header_val.into())])
            .build(),
    );
    Sandbox::builder().http_gateway(gw).build().unwrap()
}

#[test]
fn http_get_returns_response_table() {
    let sb = sandbox_with_http(&["httpbin.org"]);
    let result = sb
        .exec(
            r#"
        local resp = http.get("https://httpbin.org/get")
        return resp.status
        "#,
        )
        .unwrap();
    assert_eq!(result, "200");
}

#[test]
fn http_get_body_is_string() {
    let sb = sandbox_with_http(&["httpbin.org"]);
    let result = sb
        .exec(
            r#"
        local resp = http.get("https://httpbin.org/get")
        return type(resp.body)
        "#,
        )
        .unwrap();
    assert_eq!(result, "string");
}

#[test]
fn http_get_ok_field() {
    let sb = sandbox_with_http(&["httpbin.org"]);
    let result = sb
        .exec(
            r#"
        local resp = http.get("https://httpbin.org/get")
        return tostring(resp.ok)
        "#,
        )
        .unwrap();
    assert_eq!(result, "true");
}

#[test]
fn http_get_headers_is_table() {
    let sb = sandbox_with_http(&["httpbin.org"]);
    let result = sb
        .exec(
            r#"
        local resp = http.get("https://httpbin.org/get")
        return type(resp.headers)
        "#,
        )
        .unwrap();
    assert_eq!(result, "table");
}

#[test]
fn http_post_with_body() {
    let sb = sandbox_with_http(&["httpbin.org"]);
    let result = sb
        .exec(
            r#"
        local resp = http.post("https://httpbin.org/post", {
            body = '{"key":"value"}',
            headers = { ["Content-Type"] = "application/json" }
        })
        return resp.status
        "#,
        )
        .unwrap();
    assert_eq!(result, "200");
}

#[test]
fn http_get_with_custom_headers() {
    let sb = sandbox_with_http(&["httpbin.org"]);
    let result = sb
        .exec(
            r#"
        local resp = http.get("https://httpbin.org/headers", {
            headers = { ["X-Custom"] = "lua-test" }
        })
        -- httpbin echoes headers back in JSON body
        local has_header = string.find(resp.body, "lua-test", 1, true)
        return has_header ~= nil
        "#,
        )
        .unwrap();
    assert_eq!(result, "true");
}

#[test]
fn http_domain_denied_gives_lua_error() {
    let sb = sandbox_with_http(&[]); // no domains allowed
    let err = sb
        .exec(r#"return http.get("https://httpbin.org/get")"#)
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("denied"),
        "expected denial error, got: {}",
        err
    );
}

#[test]
fn http_credentials_injected_invisibly() {
    let sb = sandbox_with_http_and_creds("httpbin.org", "X-Secret", "injected-by-host");
    let result = sb
        .exec(
            r#"
        -- Lua code does NOT set X-Secret — the gateway injects it
        local resp = http.get("https://httpbin.org/headers")
        local has_secret = string.find(resp.body, "injected-by-host", 1, true)
        return has_secret ~= nil
        "#,
        )
        .unwrap();
    assert_eq!(result, "true");
}

#[test]
fn http_request_method_works() {
    let sb = sandbox_with_http(&["httpbin.org"]);
    let result = sb
        .exec(
            r#"
        local resp = http.request("DELETE", "https://httpbin.org/delete")
        return resp.status
        "#,
        )
        .unwrap();
    assert_eq!(result, "200");
}

#[test]
fn http_help_returns_help() {
    let sb = sandbox_with_http(&[]);
    let result = sb.exec("return http.help()").unwrap();
    assert!(result.contains("http — HTTP requests"), "got: {}", result);
    assert!(result.contains("http.get"), "got: {}", result);
    assert!(result.contains("http.post"), "got: {}", result);
    assert!(result.contains("http.policy"), "got: {}", result);
    assert!(result.contains("http.help()"), "got: {}", result);
}

#[test]
fn http_policy_reports_current_domain_policy() {
    let sb = sandbox_with_http_policy(&["example.com", "*"], &["blocked.example.com"]);
    let result = sb
        .exec(
            r#"
        local policy = http.policy()
        return table.concat(policy.allowed_domains, ",")
            .. "|"
            .. table.concat(policy.denied_domains, ",")
            .. "|"
            .. tostring(policy.unrestricted)
        "#,
        )
        .unwrap();
    assert_eq!(result, "*,example.com|blocked.example.com|true");
}

#[test]
fn http_wildcard_allow_still_honors_denies() {
    let sb = sandbox_with_http_policy(&["*"], &["httpbin.org"]);
    let err = sb
        .exec(r#"return http.get("https://httpbin.org/get")"#)
        .unwrap_err()
        .to_string();
    assert!(err.contains("denied"), "error: {}", err);
}

#[test]
fn http_help_works_bare_call() {
    let sb = sandbox_with_http(&[]);
    let result = sb.exec("http.help()").unwrap();
    assert!(result.contains("http — HTTP requests"), "got: {}", result);
}

#[test]
fn global_help_includes_http_when_enabled() {
    let sb = sandbox_with_http(&[]);
    let result = sb.exec("return help()").unwrap();
    assert!(
        result.contains("http"),
        "global help should mention http: {}",
        result
    );
}

#[test]
fn global_help_excludes_http_when_disabled() {
    let sb = Sandbox::new().unwrap();
    let result = sb.exec("return help()").unwrap();
    // Should NOT contain http module line
    assert!(
        !result.contains("http         HTTP"),
        "global help should not mention http: {}",
        result
    );
}

#[test]
fn http_bad_args_includes_hint() {
    let sb = sandbox_with_http(&[]);
    let err = sb.exec("http.get()").unwrap_err().to_string();
    assert!(
        err.contains("Usage: http.get("),
        "bad args should include inline usage: {}",
        err
    );
}

#[test]
fn http_nonexistent_fn_includes_hint() {
    let sb = sandbox_with_http(&[]);
    let err = sb.exec("http.foo()").unwrap_err().to_string();
    assert!(
        err.contains("http.foo does not exist"),
        "should name missing key: {}",
        err
    );
    assert!(
        err.contains("hint: call http.help() for usage"),
        "should hint: {}",
        err
    );
}

#[test]
fn sandbox_without_http_has_no_http_global() {
    let sb = Sandbox::new().unwrap();
    let result = sb.exec("return type(http)").unwrap();
    assert_eq!(result, "nil");
}
