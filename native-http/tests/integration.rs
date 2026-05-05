use native_http::{Headers, HttpGateway, Method, Request};

fn get(url: &str) -> Request {
    Request {
        method: Method::Get,
        url: url.to_string(),
        headers: Headers::new(),
        body: None,
    }
}

fn post(url: &str, body: &str) -> Request {
    let mut headers = Headers::new();
    headers.insert("Content-Type", "application/json");
    Request {
        method: Method::Post,
        url: url.to_string(),
        headers,
        body: Some(body.as_bytes().to_vec()),
    }
}

/// Uses the platform-default backend (NSURLSession on macOS, reqwest elsewhere).
fn gateway_allowing(domains: &[&str]) -> HttpGateway {
    let mut builder = HttpGateway::builder();
    for d in domains {
        builder = builder.allow_domain(*d);
    }
    builder.build()
}

#[test]
fn real_http_get() {
    let gw = gateway_allowing(&["httpbin.org"]);
    let resp = gw.request(get("https://httpbin.org/get")).unwrap();
    assert!(resp.ok(), "status was {}", resp.status);
    assert!(!resp.body.is_empty());
    let body = resp.body_string();
    assert!(body.contains("httpbin.org"), "body: {}", body);
}

#[test]
fn real_http_post() {
    let gw = gateway_allowing(&["httpbin.org"]);
    let resp = gw
        .request(post("https://httpbin.org/post", r#"{"hello":"world"}"#))
        .unwrap();
    assert!(resp.ok(), "status was {}", resp.status);
    let body = resp.body_string();
    assert!(body.contains("hello"), "body: {}", body);
}

#[test]
fn real_http_status_codes() {
    let gw = gateway_allowing(&["httpbin.org"]);
    let resp = gw
        .request(get("https://httpbin.org/status/404"))
        .unwrap();
    assert_eq!(resp.status, 404);
    assert!(!resp.ok());
}

#[test]
fn real_http_response_headers() {
    let gw = gateway_allowing(&["httpbin.org"]);
    let resp = gw
        .request(get("https://httpbin.org/response-headers?X-Test=hello"))
        .unwrap();
    assert!(resp.ok());
    // httpbin returns custom headers
    let header_val = resp.headers.get("X-Test").or(resp.headers.get("x-test"));
    assert_eq!(header_val, Some("hello"));
}

#[test]
fn real_http_custom_request_headers() {
    let gw = gateway_allowing(&["httpbin.org"]);
    let mut req = get("https://httpbin.org/headers");
    req.headers.insert("X-Custom", "test-value");
    let resp = gw.request(req).unwrap();
    assert!(resp.ok());
    let body = resp.body_string();
    assert!(
        body.contains("test-value"),
        "custom header not echoed: {}",
        body
    );
}

#[test]
fn domain_denied_never_makes_network_request() {
    let gw = HttpGateway::builder()
        .deny_domain("httpbin.org")
        .build();
    let err = gw.request(get("https://httpbin.org/get")).unwrap_err();
    assert!(
        err.to_string().contains("denied"),
        "error: {}",
        err
    );
}

#[test]
fn credential_injection_with_real_backend() {
    let gw = HttpGateway::builder()
        .allow_domain("httpbin.org")
        .credentials(
            "httpbin.org",
            vec![("X-Injected".into(), "from-credential-store".into())],
        )
        .build();
    let resp = gw.request(get("https://httpbin.org/headers")).unwrap();
    assert!(resp.ok());
    let body = resp.body_string();
    assert!(
        body.contains("from-credential-store"),
        "injected header not present: {}",
        body
    );
}
