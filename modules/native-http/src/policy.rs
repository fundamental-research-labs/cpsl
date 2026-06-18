//! Domain allow/deny policy and credential storage for HTTP requests.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};

/// Response from a domain prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PromptResponse {
    /// Whether the request should be allowed.
    pub allowed: bool,
    /// Whether the decision should be cached for future requests.
    /// When `false` (allow-once), the domain remains unknown and will
    /// be prompted again on the next request.
    pub remember: bool,
}

impl PromptResponse {
    pub fn allow_once() -> Self {
        Self {
            allowed: true,
            remember: false,
        }
    }

    pub fn always_allow() -> Self {
        Self {
            allowed: true,
            remember: true,
        }
    }

    pub fn deny() -> Self {
        Self {
            allowed: false,
            remember: true,
        }
    }
}

/// Called when a domain is not in allow/deny lists.
/// The host app implements this to prompt the user.
pub trait DomainPrompt: Send + Sync {
    /// Called for unknown domains. May block (e.g. showing a dialog).
    fn prompt_domain(&self, domain: &str) -> PromptResponse;
}

/// Runtime-mutable domain access policy with optional interactive prompting.
pub struct DomainPolicy {
    allowed: RwLock<HashSet<String>>,
    denied: RwLock<HashSet<String>>,
    prompt: Option<Arc<dyn DomainPrompt>>,
}

/// Result of checking a domain against the policy.
pub enum DomainVerdict {
    Allowed,
    Denied,
}

impl DomainPolicy {
    pub fn new(prompt: Option<Arc<dyn DomainPrompt>>) -> Self {
        Self {
            allowed: RwLock::new(HashSet::new()),
            denied: RwLock::new(HashSet::new()),
            prompt,
        }
    }

    /// Check whether a domain is allowed. Entries match the exact host or a
    /// subdomain suffix, so `example.com` matches `api.example.com` but not
    /// `badexample.com`.
    ///
    /// For unknown domains, invokes the prompt callback (if configured) and
    /// caches the result. If no prompt is configured, unknown domains are
    /// denied.
    pub fn check(&self, domain: &str) -> DomainVerdict {
        // Deny list checked first so explicit deny entries override allow entries.
        // On poisoned lock, default to deny (safe fallback).
        let dominated_by_deny = self
            .denied
            .read()
            .map(|set| matches_any(domain, &set))
            .unwrap_or(true);
        if dominated_by_deny {
            return DomainVerdict::Denied;
        }

        let is_allowed = self
            .allowed
            .read()
            .map(|set| matches_any(domain, &set))
            .unwrap_or(false);
        if is_allowed {
            return DomainVerdict::Allowed;
        }

        // Unknown domain — prompt the user or deny
        match &self.prompt {
            Some(prompt) => {
                let response = prompt.prompt_domain(domain);
                if response.allowed {
                    if response.remember {
                        if let Ok(mut set) = self.allowed.write() {
                            set.insert(domain.to_owned());
                        }
                    }
                    DomainVerdict::Allowed
                } else {
                    if response.remember {
                        if let Ok(mut set) = self.denied.write() {
                            set.insert(domain.to_owned());
                        }
                    }
                    DomainVerdict::Denied
                }
            }
            None => DomainVerdict::Denied,
        }
    }

    pub fn allow(&self, domain: &str) {
        if let Ok(mut set) = self.allowed.write() {
            set.insert(domain.to_owned());
        }
    }

    pub fn deny(&self, domain: &str) {
        if let Ok(mut set) = self.denied.write() {
            set.insert(domain.to_owned());
        }
    }

    pub fn is_allowed(&self, domain: &str) -> bool {
        self.allowed
            .read()
            .map(|set| set.contains(domain))
            .unwrap_or(false)
    }

    pub fn is_denied(&self, domain: &str) -> bool {
        self.denied
            .read()
            .map(|set| set.contains(domain))
            .unwrap_or(false)
    }

    /// Remove a domain from both allowed and denied lists.
    pub fn remove(&self, domain: &str) {
        if let Ok(mut set) = self.allowed.write() {
            set.remove(domain);
        }
        if let Ok(mut set) = self.denied.write() {
            set.remove(domain);
        }
    }

    /// Return a snapshot of all allowed domains.
    pub fn allowed_domains(&self) -> Vec<String> {
        let Ok(set) = self.allowed.read() else {
            return Vec::new();
        };
        let mut v: Vec<String> = set.iter().cloned().collect();
        v.sort();
        v
    }

    /// Return a snapshot of all denied domains.
    pub fn denied_domains(&self) -> Vec<String> {
        let Ok(set) = self.denied.read() else {
            return Vec::new();
        };
        let mut v: Vec<String> = set.iter().cloned().collect();
        v.sort();
        v
    }
}

/// Check if a domain matches any entry in the set.
/// Supports exact host and subdomain suffix matches only.
fn matches_any(domain: &str, set: &HashSet<String>) -> bool {
    set.iter().any(|entry| domain_matches_entry(domain, entry))
}

fn domain_matches_entry(domain: &str, entry: &str) -> bool {
    if entry.is_empty() {
        return false;
    }
    if entry == "*" {
        return true;
    }
    if domain == entry {
        return true;
    }
    if domain.len() <= entry.len() || !domain.ends_with(entry) {
        return false;
    }
    let boundary = domain.len() - entry.len() - 1;
    domain.as_bytes().get(boundary) == Some(&b'.')
}

/// Runtime-mutable per-domain credential (header) store.
pub struct CredentialStore {
    /// domain → headers to inject
    credentials: RwLock<HashMap<String, Vec<(String, String)>>>,
}

impl CredentialStore {
    pub fn new() -> Self {
        Self {
            credentials: RwLock::new(HashMap::new()),
        }
    }

    pub fn set(&self, domain: &str, headers: Vec<(String, String)>) {
        if let Ok(mut creds) = self.credentials.write() {
            creds.insert(domain.to_owned(), headers);
        }
    }

    pub fn remove(&self, domain: &str) {
        if let Ok(mut creds) = self.credentials.write() {
            creds.remove(domain);
        }
    }

    /// Get credentials for a domain. Checks exact match first, then wildcards.
    /// Returns empty vec if none configured.
    pub fn get(&self, domain: &str) -> Vec<(String, String)> {
        let Ok(creds) = self.credentials.read() else {
            return Vec::new();
        };
        // Exact match first
        if let Some(h) = creds.get(domain) {
            return h.clone();
        }
        // Wildcard: * matches all
        if let Some(h) = creds.get("*") {
            return h.clone();
        }
        // Subdomain wildcards: *.example.com
        for (pattern, headers) in creds.iter() {
            if let Some(suffix) = pattern.strip_prefix("*.") {
                if domain.ends_with(suffix) && domain.len() > suffix.len() {
                    let prefix_len = domain.len() - suffix.len();
                    if prefix_len > 0 && domain.as_bytes()[prefix_len - 1] == b'.' {
                        return headers.clone();
                    }
                }
            }
        }
        Vec::new()
    }
}

impl Default for CredentialStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    struct AlwaysAllow;
    impl DomainPrompt for AlwaysAllow {
        fn prompt_domain(&self, _domain: &str) -> PromptResponse {
            PromptResponse::always_allow()
        }
    }

    struct AlwaysDeny;
    impl DomainPrompt for AlwaysDeny {
        fn prompt_domain(&self, _domain: &str) -> PromptResponse {
            PromptResponse::deny()
        }
    }

    // -- DomainPolicy tests --

    #[test]
    fn pre_allowed_domain_passes() {
        let policy = DomainPolicy::new(None);
        policy.allow("example.com");
        assert!(matches!(
            policy.check("example.com"),
            DomainVerdict::Allowed
        ));
    }

    #[test]
    fn pre_denied_domain_fails() {
        let policy = DomainPolicy::new(Some(Arc::new(AlwaysAllow)));
        policy.deny("evil.com");
        // Even with AlwaysAllow prompt, explicit deny wins
        assert!(matches!(policy.check("evil.com"), DomainVerdict::Denied));
    }

    #[test]
    fn unknown_domain_denied_without_prompt() {
        let policy = DomainPolicy::new(None);
        assert!(matches!(policy.check("unknown.com"), DomainVerdict::Denied));
    }

    #[test]
    fn unknown_domain_prompts_and_allows() {
        let policy = DomainPolicy::new(Some(Arc::new(AlwaysAllow)));
        assert!(matches!(
            policy.check("newsite.com"),
            DomainVerdict::Allowed
        ));
        // Should be cached as allowed now
        assert!(policy.is_allowed("newsite.com"));
    }

    #[test]
    fn unknown_domain_prompts_and_denies() {
        let policy = DomainPolicy::new(Some(Arc::new(AlwaysDeny)));
        assert!(matches!(policy.check("newsite.com"), DomainVerdict::Denied));
        // Should be cached as denied now
        assert!(policy.is_denied("newsite.com"));
    }

    #[test]
    fn runtime_mutation_allow_does_not_override_deny() {
        let policy = DomainPolicy::new(None);
        policy.deny("example.com");
        assert!(matches!(policy.check("example.com"), DomainVerdict::Denied));
        policy.allow("example.com");
        assert!(matches!(policy.check("example.com"), DomainVerdict::Denied));
        assert!(policy.is_allowed("example.com"));
        assert!(policy.is_denied("example.com"));
    }

    #[test]
    fn runtime_mutation_deny_overrides_allow() {
        let policy = DomainPolicy::new(None);
        policy.allow("example.com");
        policy.deny("example.com");
        assert!(matches!(policy.check("example.com"), DomainVerdict::Denied));
        assert!(policy.is_allowed("example.com"));
        assert!(policy.is_denied("example.com"));
    }

    #[test]
    fn concurrent_access() {
        let policy = Arc::new(DomainPolicy::new(Some(Arc::new(AlwaysAllow))));
        let handles: Vec<_> = (0..10)
            .map(|i| {
                let p = policy.clone();
                std::thread::spawn(move || {
                    let domain = format!("domain{}.com", i);
                    p.check(&domain);
                    assert!(p.is_allowed(&domain));
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
    }

    #[test]
    fn prompt_only_called_once_per_domain_when_remembered() {
        let called = Arc::new(AtomicBool::new(false));
        struct OncePrompt(Arc<AtomicBool>);
        impl DomainPrompt for OncePrompt {
            fn prompt_domain(&self, _domain: &str) -> PromptResponse {
                assert!(
                    !self.0.swap(true, Ordering::SeqCst),
                    "prompt called more than once"
                );
                PromptResponse::always_allow()
            }
        }
        let policy = DomainPolicy::new(Some(Arc::new(OncePrompt(called))));
        policy.check("test.com");
        policy.check("test.com"); // second call should hit cache, not prompt
    }

    #[test]
    fn allow_once_does_not_cache() {
        use std::sync::atomic::AtomicUsize;
        let count = Arc::new(AtomicUsize::new(0));
        struct AllowOncePrompt(Arc<AtomicUsize>);
        impl DomainPrompt for AllowOncePrompt {
            fn prompt_domain(&self, _domain: &str) -> PromptResponse {
                self.0.fetch_add(1, Ordering::SeqCst);
                PromptResponse::allow_once()
            }
        }
        let policy = DomainPolicy::new(Some(Arc::new(AllowOncePrompt(count.clone()))));
        // First call — prompted, allowed
        assert!(matches!(policy.check("test.com"), DomainVerdict::Allowed));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        // Second call — should prompt again (not cached)
        assert!(matches!(policy.check("test.com"), DomainVerdict::Allowed));
        assert_eq!(count.load(Ordering::SeqCst), 2);
        // Domain should NOT appear in allowed list
        assert!(!policy.is_allowed("test.com"));
    }

    // -- Suffix matching tests --

    #[test]
    fn allowed_domain_matches_subdomains() {
        let policy = DomainPolicy::new(None);
        policy.allow("example.com");
        assert!(matches!(
            policy.check("example.com"),
            DomainVerdict::Allowed
        ));
        assert!(matches!(
            policy.check("api.example.com"),
            DomainVerdict::Allowed
        ));
        assert!(matches!(
            policy.check("foo.bar.example.com"),
            DomainVerdict::Allowed
        ));
        assert!(matches!(
            policy.check("badexample.com"),
            DomainVerdict::Denied
        ));
    }

    #[test]
    fn wildcard_allow_matches_any_domain() {
        let policy = DomainPolicy::new(None);
        policy.allow("*");
        assert!(matches!(
            policy.check("example.com"),
            DomainVerdict::Allowed
        ));
        assert!(matches!(
            policy.check("api.other.test"),
            DomainVerdict::Allowed
        ));
    }

    #[test]
    fn deny_overrides_wildcard_allow() {
        let policy = DomainPolicy::new(None);
        policy.allow("*");
        policy.deny("blocked.example.com");
        assert!(matches!(
            policy.check("api.blocked.example.com"),
            DomainVerdict::Denied
        ));
        assert!(matches!(
            policy.check("api.example.com"),
            DomainVerdict::Allowed
        ));
    }

    #[test]
    fn denied_domain_matches_subdomains() {
        let policy = DomainPolicy::new(None);
        policy.allow("example.com");
        policy.deny("evil.example.com");
        assert!(matches!(
            policy.check("api.evil.example.com"),
            DomainVerdict::Denied
        ));
        assert!(matches!(
            policy.check("good.example.com"),
            DomainVerdict::Allowed
        ));
    }

    #[test]
    fn parent_deny_overrides_child_allow() {
        let policy = DomainPolicy::new(None);
        policy.allow("api.example.com");
        policy.deny("example.com");
        assert!(matches!(
            policy.check("api.example.com"),
            DomainVerdict::Denied
        ));
    }

    #[test]
    fn child_deny_overrides_parent_allow() {
        let policy = DomainPolicy::new(None);
        policy.allow("example.com");
        policy.deny("api.example.com");
        assert!(matches!(
            policy.check("api.example.com"),
            DomainVerdict::Denied
        ));
        assert!(matches!(
            policy.check("docs.example.com"),
            DomainVerdict::Allowed
        ));
    }

    #[test]
    fn wildcard_credentials_star() {
        let store = CredentialStore::new();
        store.set("*", vec![("X-Global".into(), "val".into())]);
        let creds = store.get("any-domain.com");
        assert_eq!(creds.len(), 1);
        assert_eq!(creds[0].0, "X-Global");
    }

    #[test]
    fn wildcard_credentials_subdomain() {
        let store = CredentialStore::new();
        store.set(
            "*.api.com",
            vec![("Authorization".into(), "Bearer x".into())],
        );
        let creds = store.get("v1.api.com");
        assert_eq!(creds.len(), 1);
        // Exact domain doesn't match wildcard
        assert!(store.get("api.com").is_empty());
        // Unrelated domain doesn't match
        assert!(store.get("other.com").is_empty());
    }

    #[test]
    fn exact_credentials_take_priority_over_wildcard() {
        let store = CredentialStore::new();
        store.set("*", vec![("X-Global".into(), "global".into())]);
        store.set("special.com", vec![("X-Special".into(), "specific".into())]);
        let creds = store.get("special.com");
        assert_eq!(creds[0].0, "X-Special");
        let creds = store.get("other.com");
        assert_eq!(creds[0].0, "X-Global");
    }

    // -- CredentialStore tests --

    #[test]
    fn empty_store_returns_empty() {
        let store = CredentialStore::new();
        assert!(store.get("anything.com").is_empty());
    }

    #[test]
    fn set_and_get_credentials() {
        let store = CredentialStore::new();
        store.set(
            "api.example.com",
            vec![("Authorization".into(), "Bearer tok".into())],
        );
        let creds = store.get("api.example.com");
        assert_eq!(creds.len(), 1);
        assert_eq!(creds[0].0, "Authorization");
        assert_eq!(creds[0].1, "Bearer tok");
    }

    #[test]
    fn remove_credentials() {
        let store = CredentialStore::new();
        store.set("api.example.com", vec![("X-Key".into(), "123".into())]);
        store.remove("api.example.com");
        assert!(store.get("api.example.com").is_empty());
    }

    #[test]
    fn overwrite_credentials() {
        let store = CredentialStore::new();
        store.set("api.example.com", vec![("X-V".into(), "1".into())]);
        store.set("api.example.com", vec![("X-V".into(), "2".into())]);
        let creds = store.get("api.example.com");
        assert_eq!(creds[0].1, "2");
    }
}
