//! Policy-aware HTTP gateway that validates requests before dispatch.

use crate::backend::HttpBackend;
use crate::policy::{CredentialStore, DomainPolicy, DomainPrompt, DomainVerdict};
use crate::types::{HttpError, Limits, Request, Response};
use std::sync::Arc;
use url::Url;

/// Orchestrates policy checks, credential injection, and backend dispatch.
pub struct HttpGateway {
    backend: Box<dyn HttpBackend>,
    policy: DomainPolicy,
    credentials: CredentialStore,
    limits: Limits,
}

impl HttpGateway {
    pub fn builder() -> HttpGatewayBuilder {
        HttpGatewayBuilder::default()
    }

    /// Execute an HTTP request through the gateway.
    ///
    /// 1. Parse URL and extract domain
    /// 2. Check domain policy (may prompt user)
    /// 3. Inject per-domain credentials
    /// 4. Delegate to platform backend
    pub fn request(&self, mut req: Request) -> Result<Response, HttpError> {
        let domain = extract_domain(&req.url)?;

        // Policy check
        match self.policy.check(&domain) {
            DomainVerdict::Allowed => {}
            DomainVerdict::Denied => return Err(HttpError::DomainDenied(domain)),
        }

        // Inject credentials
        for (key, value) in self.credentials.get(&domain) {
            req.headers.insert(key, value);
        }

        // Delegate to backend
        self.backend.send(&req)
    }

    // -- Runtime policy mutation --

    pub fn allow_domain(&self, domain: &str) {
        self.policy.allow(domain);
    }

    pub fn deny_domain(&self, domain: &str) {
        self.policy.deny(domain);
    }

    pub fn remove_domain(&self, domain: &str) {
        self.policy.remove(domain);
    }

    pub fn set_credentials(&self, domain: &str, headers: Vec<(String, String)>) {
        self.credentials.set(domain, headers);
    }

    pub fn remove_credentials(&self, domain: &str) {
        self.credentials.remove(domain);
    }

    pub fn limits(&self) -> &Limits {
        &self.limits
    }

    /// Snapshot of currently allowed domains.
    pub fn allowed_domains(&self) -> Vec<String> {
        self.policy.allowed_domains()
    }

    /// Snapshot of currently denied domains.
    pub fn denied_domains(&self) -> Vec<String> {
        self.policy.denied_domains()
    }
}

fn extract_domain(url: &str) -> Result<String, HttpError> {
    let parsed = Url::parse(url).map_err(|e| HttpError::InvalidUrl(e.to_string()))?;
    parsed
        .host_str()
        .map(|h| h.to_owned())
        .ok_or_else(|| HttpError::InvalidUrl("no host in URL".to_string()))
}

/// Builder for constructing an `HttpGateway` with the desired configuration.
pub struct HttpGatewayBuilder {
    backend: Option<Box<dyn HttpBackend>>,
    prompt: Option<Arc<dyn DomainPrompt>>,
    allowed_domains: Vec<String>,
    denied_domains: Vec<String>,
    credentials: Vec<(String, Vec<(String, String)>)>,
    limits: Limits,
}

impl Default for HttpGatewayBuilder {
    fn default() -> Self {
        Self {
            backend: None,
            prompt: None,
            allowed_domains: Vec::new(),
            denied_domains: Vec::new(),
            credentials: Vec::new(),
            limits: Limits::default(),
        }
    }
}

impl HttpGatewayBuilder {
    pub fn backend(mut self, backend: Box<dyn HttpBackend>) -> Self {
        self.backend = Some(backend);
        self
    }

    pub fn prompt(mut self, prompt: Arc<dyn DomainPrompt>) -> Self {
        self.prompt = Some(prompt);
        self
    }

    pub fn allow_domain(mut self, domain: impl Into<String>) -> Self {
        self.allowed_domains.push(domain.into());
        self
    }

    pub fn deny_domain(mut self, domain: impl Into<String>) -> Self {
        self.denied_domains.push(domain.into());
        self
    }

    pub fn credentials(
        mut self,
        domain: impl Into<String>,
        headers: Vec<(String, String)>,
    ) -> Self {
        self.credentials.push((domain.into(), headers));
        self
    }

    pub fn limits(mut self, limits: Limits) -> Self {
        self.limits = limits;
        self
    }

    pub fn build(self) -> HttpGateway {
        let backend = self
            .backend
            .unwrap_or_else(|| crate::backend::platform_default());

        let policy = DomainPolicy::new(self.prompt);
        for d in &self.allowed_domains {
            policy.allow(d);
        }
        for d in &self.denied_domains {
            policy.deny(d);
        }

        let cred_store = CredentialStore::new();
        for (domain, headers) in self.credentials {
            cred_store.set(&domain, headers);
        }

        HttpGateway {
            backend,
            policy,
            credentials: cred_store,
            limits: self.limits,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Headers, Method, Response};

    /// A mock backend that always returns a fixed response.
    struct MockBackend {
        status: u16,
        body: Vec<u8>,
    }

    impl MockBackend {
        fn ok(body: &str) -> Box<Self> {
            Box::new(Self {
                status: 200,
                body: body.as_bytes().to_vec(),
            })
        }
    }

    impl HttpBackend for MockBackend {
        fn send(&self, _req: &Request) -> Result<Response, HttpError> {
            Ok(Response {
                status: self.status,
                headers: Headers::new(),
                body: self.body.clone(),
            })
        }
    }

    /// A mock backend that captures the request it receives.
    struct CapturingBackend {
        captured: std::sync::Mutex<Option<Request>>,
    }

    impl CapturingBackend {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                captured: std::sync::Mutex::new(None),
            })
        }
    }

    impl HttpBackend for Arc<CapturingBackend> {
        fn send(&self, req: &Request) -> Result<Response, HttpError> {
            *self.captured.lock().unwrap() = Some(req.clone());
            Ok(Response {
                status: 200,
                headers: Headers::new(),
                body: Vec::new(),
            })
        }
    }

    fn get_request(url: &str) -> Request {
        Request {
            method: Method::Get,
            url: url.to_string(),
            headers: Headers::new(),
            body: None,
        }
    }

    #[test]
    fn allowed_domain_succeeds() {
        let gw = HttpGateway::builder()
            .backend(MockBackend::ok("ok"))
            .allow_domain("example.com")
            .build();

        let resp = gw.request(get_request("https://example.com/path")).unwrap();
        assert_eq!(resp.status, 200);
        assert_eq!(resp.body_string(), "ok");
    }

    #[test]
    fn denied_domain_fails() {
        let gw = HttpGateway::builder()
            .backend(MockBackend::ok("ok"))
            .deny_domain("evil.com")
            .build();

        let err = gw
            .request(get_request("https://evil.com/path"))
            .unwrap_err();
        assert!(matches!(err, HttpError::DomainDenied(d) if d == "evil.com"));
    }

    #[test]
    fn unknown_domain_denied_without_prompt() {
        let gw = HttpGateway::builder()
            .backend(MockBackend::ok("ok"))
            .build();

        let err = gw
            .request(get_request("https://unknown.com/path"))
            .unwrap_err();
        assert!(matches!(err, HttpError::DomainDenied(_)));
    }

    #[test]
    fn invalid_url_returns_error() {
        let gw = HttpGateway::builder()
            .backend(MockBackend::ok("ok"))
            .build();

        let err = gw.request(get_request("not-a-url")).unwrap_err();
        assert!(matches!(err, HttpError::InvalidUrl(_)));
    }

    #[test]
    fn credentials_injected_into_request() {
        let backend = CapturingBackend::new();
        let gw = HttpGateway::builder()
            .backend(Box::new(backend.clone()))
            .allow_domain("api.example.com")
            .credentials(
                "api.example.com",
                vec![("Authorization".into(), "Bearer tok123".into())],
            )
            .build();

        gw.request(get_request("https://api.example.com/data"))
            .unwrap();

        let captured = backend.captured.lock().unwrap();
        let req = captured.as_ref().unwrap();
        assert_eq!(req.headers.get("Authorization"), Some("Bearer tok123"));
    }

    #[test]
    fn credentials_not_injected_for_other_domains() {
        let backend = CapturingBackend::new();
        let gw = HttpGateway::builder()
            .backend(Box::new(backend.clone()))
            .allow_domain("other.com")
            .credentials(
                "api.example.com",
                vec![("Authorization".into(), "Bearer tok123".into())],
            )
            .build();

        gw.request(get_request("https://other.com/data")).unwrap();

        let captured = backend.captured.lock().unwrap();
        let req = captured.as_ref().unwrap();
        assert!(req.headers.get("Authorization").is_none());
    }

    #[test]
    fn runtime_allow_then_request() {
        let gw = HttpGateway::builder()
            .backend(MockBackend::ok("ok"))
            .build();

        // Initially denied
        assert!(gw.request(get_request("https://new.com/")).is_err());

        // Allow at runtime
        gw.allow_domain("new.com");
        let resp = gw.request(get_request("https://new.com/")).unwrap();
        assert_eq!(resp.status, 200);
    }

    #[test]
    fn runtime_credential_update() {
        let backend = CapturingBackend::new();
        let gw = HttpGateway::builder()
            .backend(Box::new(backend.clone()))
            .allow_domain("api.com")
            .build();

        // No credentials initially
        gw.request(get_request("https://api.com/")).unwrap();
        {
            let captured = backend.captured.lock().unwrap();
            assert!(captured.as_ref().unwrap().headers.get("X-Key").is_none());
        }

        // Add credentials at runtime
        gw.set_credentials("api.com", vec![("X-Key".into(), "secret".into())]);
        gw.request(get_request("https://api.com/")).unwrap();
        {
            let captured = backend.captured.lock().unwrap();
            assert_eq!(
                captured.as_ref().unwrap().headers.get("X-Key"),
                Some("secret")
            );
        }

        // Remove credentials
        gw.remove_credentials("api.com");
        gw.request(get_request("https://api.com/")).unwrap();
        {
            let captured = backend.captured.lock().unwrap();
            assert!(captured.as_ref().unwrap().headers.get("X-Key").is_none());
        }
    }
}
