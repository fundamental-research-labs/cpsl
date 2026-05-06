//! SFAE credential and proxy integration exposed to the Lua sandbox.

use crate::lua_util::register_help_functions;
use crate::sandbox::{
    arg_error, wrap_module_with_help_hints, FieldDoc, FnDoc, ModuleDoc, Param, ParamType,
    ReturnType,
};
use mlua::{Lua, MultiValue};
use sfae_core::credential::{self, CredentialType};
use sfae_core::proxy::{self, ProxyRequest};
use sfae_core::store::{self, SecretStore};
use sfae_core::SfaeError;
use std::sync::{Arc, Mutex};

/// Callback for prompting the user to enter a credential.
/// Implementations should block until the user submits or cancels.
pub trait CredentialPrompt: Send + Sync {
    /// Prompt the user for a credential.
    /// Returns `Ok(secret)` on submission, `Err(msg)` on cancel or error.
    fn prompt_credential(
        &self,
        domain: &str,
        credential_type: &str,
        url: Option<&str>,
    ) -> Result<String, String>;
}

/// Opens a URL in the system browser.
/// Desktop implements via Tauri shell opener, CLI could use Command::new("open").
pub trait BrowserOpener: Send + Sync {
    fn open_url(&self, url: &str) -> Result<(), String>;
}

const SFAE_PROMPT_OPTS_FIELDS: &[FieldDoc] = &[FieldDoc {
    name: "url",
    typ: "string",
    required: false,
    description: "URL shown as a helpful link for obtaining the credential",
}];

const SFAE_OAUTH_OPTS_FIELDS: &[FieldDoc] = &[
    FieldDoc {
        name: "scope",
        typ: "string",
        required: false,
        description: "OAuth scope(s), space-separated (e.g. \"https://www.googleapis.com/auth/gmail.readonly\")",
    },
    FieldDoc {
        name: "client_id",
        typ: "string",
        required: false,
        description: "OAuth client ID (provided by preset for known providers like googleapis.com)",
    },
    FieldDoc {
        name: "auth_url",
        typ: "string",
        required: false,
        description: "Authorization endpoint URL (provided by preset for known providers)",
    },
    FieldDoc {
        name: "token_url",
        typ: "string",
        required: false,
        description: "Token endpoint URL (provided by preset for known providers)",
    },
    FieldDoc {
        name: "client_secret",
        typ: "string",
        required: false,
        description: "OAuth client secret — omit for public clients",
    },
];

const SFAE_REQUEST_OPTS_FIELDS: &[FieldDoc] = &[
    FieldDoc {
        name: "headers",
        typ: "table",
        required: false,
        description: "Request headers {[name] = value} — use -ACCESS_TOKEN-, -API_KEY-, -PASSWORD-, -REFRESH_TOKEN- placeholders for credential injection",
    },
    FieldDoc {
        name: "body",
        typ: "string",
        required: false,
        description: "Request body (placeholders are resolved here too)",
    },
];

pub(crate) static SFAE_DOC: ModuleDoc = ModuleDoc {
    name: "sfae",
    summary: "Secure credential storage and authenticated HTTP requests (OS keychain)",
    functions: &[
        FnDoc {
            name: "credentials",
            description: "List credential types stored for a domain. Returns a table of strings like {\"ACCESS_TOKEN\", \"API_KEY\"}.",
            params: &[Param {
                name: "domain",
                short: Some('d'),
                typ: ParamType::String,
                required: true,
                fields: None,
            }],
            returns: ReturnType::Table,
            example: Some(r#"local creds = sfae.credentials("github.com")"#),
        },
        FnDoc {
            name: "prompt",
            description: "Prompt the user to enter a credential. Blocks until the user submits or cancels. Stores the credential in the OS keychain on success.",
            params: &[
                Param {
                    name: "domain",
                    short: Some('d'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "credential_type",
                    short: Some('t'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "opts",
                    short: None,
                    typ: ParamType::Table,
                    required: false,
                    fields: Some(SFAE_PROMPT_OPTS_FIELDS),
                },
            ],
            returns: ReturnType::Boolean,
            example: Some(
                r#"sfae.prompt("github.com", "ACCESS_TOKEN", {url = "https://github.com/settings/tokens"})"#,
            ),
        },
        FnDoc {
            name: "oauth",
            description: "Initiate an OAuth2 flow: opens the system browser for consent, exchanges the code for tokens, and stores them in the OS keychain. For known providers (googleapis.com), only scope is needed. Returns true on success.",
            params: &[
                Param {
                    name: "domain",
                    short: Some('d'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "opts",
                    short: None,
                    typ: ParamType::Table,
                    required: true,
                    fields: Some(SFAE_OAUTH_OPTS_FIELDS),
                },
            ],
            returns: ReturnType::Boolean,
            example: Some(
                r#"sfae.oauth("googleapis.com", {scope = "https://www.googleapis.com/auth/gmail.readonly"})"#,
            ),
        },
        FnDoc {
            name: "request",
            description: "Make an HTTP request with credential auto-injection. Placeholders like -ACCESS_TOKEN- in headers/body/URL are resolved from the OS keychain. Domain is extracted from the URL automatically. Automatically refreshes OAuth tokens on 401 responses.",
            params: &[
                Param {
                    name: "method",
                    short: Some('m'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "url",
                    short: Some('u'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "opts",
                    short: None,
                    typ: ParamType::Table,
                    required: false,
                    fields: Some(SFAE_REQUEST_OPTS_FIELDS),
                },
            ],
            returns: ReturnType::Table,
            example: Some(
                r#"local resp = sfae.request("GET", "https://api.github.com/user", {headers = {Authorization = "Bearer -ACCESS_TOKEN-"}})"#,
            ),
        },
    ],
};

/// Check whether any part of the request contains an `-ACCESS_TOKEN-` placeholder.
fn request_has_access_token_placeholder(request: &ProxyRequest) -> bool {
    let check = |text: &str| proxy::find_placeholders(text).contains(&CredentialType::AccessToken);
    if check(&request.url) {
        return true;
    }
    for (_, v) in &request.headers {
        if check(v) {
            return true;
        }
    }
    if let Some(b) = &request.body {
        if check(b) {
            return true;
        }
    }
    false
}

/// Attempt to refresh the OAuth access token and retry the request.
///
/// Returns `None` if any precondition is missing or the refresh fails,
/// signalling the caller to use the original 401 response instead.
fn try_refresh_and_retry(
    store: &Arc<Mutex<dyn SecretStore + Send>>,
    request: &ProxyRequest,
    domain: &str,
) -> Option<proxy::ProxyResponse> {
    // Look up OAuth metadata for this domain.
    let metadata = sfae_core::oauth::get_oauth_metadata(domain, None).ok()??;

    let guard = store.lock().ok()?;

    // Look up refresh token.
    let refresh_token = match proxy::get_credential_with_fallback(
        &*guard,
        domain,
        None,
        CredentialType::RefreshToken,
    ) {
        Ok(t) => t,
        Err(SfaeError::CredentialNotFound(_)) => return None,
        Err(_) => return None,
    };

    // Look up client secret (may be absent for public clients).
    let client_secret = match proxy::get_credential_with_fallback(
        &*guard,
        domain,
        None,
        CredentialType::ClientSecret,
    ) {
        Ok(s) => Some(s),
        Err(_) => None,
    };

    drop(guard);

    // Attempt the refresh.
    let token_response = sfae_core::oauth::refresh_access_token(
        &metadata.token_url,
        &refresh_token,
        &metadata.client_id,
        client_secret.as_deref(),
    )
    .ok()?;

    // Update the access token in the store.
    let mut guard = store.lock().ok()?;
    let access_key = credential::credential_key(domain, None, CredentialType::AccessToken);
    guard.set(&access_key, &token_response.access_token).ok()?;

    // If the provider rotated the refresh token, update it too.
    if let Some(ref new_refresh) = token_response.refresh_token {
        let refresh_key = credential::credential_key(domain, None, CredentialType::RefreshToken);
        guard.set(&refresh_key, new_refresh).ok()?;
    }

    // Retry the request once.
    proxy::execute(request, &*guard, domain, None).ok()
}

pub(crate) fn register_sfae_globals(
    lua: &Lua,
    store: Arc<Mutex<dyn SecretStore + Send>>,
    prompt_cb: Arc<dyn CredentialPrompt>,
    browser_opener: Option<Arc<dyn BrowserOpener>>,
) -> Result<(), mlua::Error> {
    let sfae = lua.create_table()?;

    // sfae.credentials(domain) -> table of credential type strings
    {
        let store = store.clone();
        sfae.set(
            "credentials",
            lua.create_function(move |lua, args: MultiValue| {
                if args.is_empty() {
                    return Err(arg_error(
                        "sfae.credentials",
                        SFAE_DOC.params("credentials"),
                    ));
                }
                let domain = match &args[0] {
                    mlua::Value::String(s) => s.to_string_lossy().to_string(),
                    _ => {
                        return Err(mlua::Error::external(format!(
                            "sfae.credentials: argument 'domain' expected string, got {}",
                            args[0].type_name()
                        )))
                    }
                };
                let guard = store.lock().map_err(|e| {
                    mlua::Error::external(format!("sfae: store lock poisoned: {e}"))
                })?;
                let types = store::list_credential_types(&*guard, &domain, None)
                    .map_err(|e| mlua::Error::external(format!("sfae.credentials: {e}")))?;
                let result = lua.create_table()?;
                for (i, ct) in types.iter().enumerate() {
                    result.set(i + 1, ct.as_str())?;
                }
                Ok(mlua::Value::Table(result))
            })?,
        )?;
    }

    // sfae.prompt(domain, credential_type, opts?) -> true
    {
        let store = store.clone();
        let prompt = prompt_cb.clone();
        sfae.set(
            "prompt",
            lua.create_function(move |_lua, args: MultiValue| {
                if args.len() < 2 {
                    return Err(arg_error("sfae.prompt", SFAE_DOC.params("prompt")));
                }
                let domain = match &args[0] {
                    mlua::Value::String(s) => s.to_string_lossy().to_string(),
                    _ => {
                        return Err(mlua::Error::external(
                            "sfae.prompt: argument 'domain' expected string",
                        ))
                    }
                };
                let cred_type_str = match &args[1] {
                    mlua::Value::String(s) => s.to_string_lossy().to_string(),
                    _ => {
                        return Err(mlua::Error::external(
                            "sfae.prompt: argument 'credential_type' expected string",
                        ))
                    }
                };

                // Validate credential type
                let cred_type: CredentialType = cred_type_str
                    .parse()
                    .map_err(|e: String| mlua::Error::external(format!("sfae.prompt: {e}")))?;

                // Extract optional URL from opts table
                let url: Option<String> = if let Some(mlua::Value::Table(opts)) = args.get(2) {
                    opts.get::<Option<String>>("url")
                        .map_err(|e| mlua::Error::external(format!("sfae.prompt: {e}")))?
                } else {
                    None
                };

                // Call the prompt callback (blocks until user responds)
                let secret = prompt
                    .prompt_credential(&domain, &cred_type_str, url.as_deref())
                    .map_err(|e| mlua::Error::external(e))?;

                // Store the credential
                let key = sfae_core::credential::credential_key(&domain, None, cred_type);
                let mut guard = store.lock().map_err(|e| {
                    mlua::Error::external(format!("sfae: store lock poisoned: {e}"))
                })?;
                guard
                    .set(&key, &secret)
                    .map_err(|e| mlua::Error::external(format!("sfae.prompt: {e}")))?;

                Ok(mlua::Value::Boolean(true))
            })?,
        )?;
    }

    // sfae.request(method, url, opts?) -> {status, body, headers, ok}
    {
        let store = store.clone();
        sfae.set(
            "request",
            lua.create_function(move |lua, args: MultiValue| {
                if args.len() < 2 {
                    return Err(arg_error("sfae.request", SFAE_DOC.params("request")));
                }
                let method = match &args[0] {
                    mlua::Value::String(s) => s.to_string_lossy().to_string(),
                    _ => {
                        return Err(mlua::Error::external(
                            "sfae.request: argument 'method' expected string",
                        ))
                    }
                };
                let url = match &args[1] {
                    mlua::Value::String(s) => s.to_string_lossy().to_string(),
                    _ => {
                        return Err(mlua::Error::external(
                            "sfae.request: argument 'url' expected string",
                        ))
                    }
                };

                let domain = proxy::extract_host(&url).ok_or_else(|| {
                    mlua::Error::external(format!(
                        "sfae.request: cannot extract domain from URL: {url}"
                    ))
                })?;

                let mut headers = Vec::new();
                let mut body: Option<String> = None;

                if let Some(mlua::Value::Table(opts)) = args.get(2) {
                    if let Some(h) = opts
                        .get::<Option<mlua::Table>>("headers")
                        .map_err(|e| mlua::Error::external(format!("sfae.request: {e}")))?
                    {
                        for pair in h.pairs::<String, String>() {
                            let (k, v) = pair?;
                            headers.push((k, v));
                        }
                    }
                    body = opts
                        .get::<Option<String>>("body")
                        .map_err(|e| mlua::Error::external(format!("sfae.request: {e}")))?;
                }

                let proxy_req = ProxyRequest {
                    method: method.to_uppercase(),
                    url,
                    headers,
                    body,
                };

                let guard = store.lock().map_err(|e| {
                    mlua::Error::external(format!("sfae: store lock poisoned: {e}"))
                })?;
                let proxy_resp = proxy::execute(&proxy_req, &*guard, &domain, None)
                    .map_err(|e| mlua::Error::external(format!("sfae.request: {e}")))?;
                drop(guard);

                // 401 auto-refresh: if the response is 401 and the request uses
                // an ACCESS_TOKEN placeholder, try to refresh and retry once.
                let proxy_resp = if proxy_resp.status == 401
                    && request_has_access_token_placeholder(&proxy_req)
                {
                    try_refresh_and_retry(&store, &proxy_req, &domain).unwrap_or(proxy_resp)
                } else {
                    proxy_resp
                };

                let resp_table = lua.create_table()?;
                resp_table.set("status", proxy_resp.status)?;
                resp_table.set("body", lua.create_string(&proxy_resp.body)?)?;
                resp_table.set("ok", (200..300).contains(&proxy_resp.status))?;

                let headers_table = lua.create_table()?;
                for (key, value) in &proxy_resp.headers {
                    headers_table.set(key.as_str(), value.as_str())?;
                }
                resp_table.set("headers", headers_table)?;

                Ok(resp_table)
            })?,
        )?;
    }

    // sfae.oauth(domain, opts) -> true
    {
        let store = store.clone();
        let opener = browser_opener.clone();
        sfae.set(
            "oauth",
            lua.create_function(move |_lua, args: MultiValue| {
                if args.len() < 2 {
                    return Err(arg_error("sfae.oauth", SFAE_DOC.params("oauth")));
                }
                let domain = match &args[0] {
                    mlua::Value::String(s) => s.to_string_lossy().to_string(),
                    _ => {
                        return Err(mlua::Error::external(
                            "sfae.oauth: argument 'domain' expected string",
                        ))
                    }
                };
                let opts = match &args[1] {
                    mlua::Value::Table(t) => t.clone(),
                    _ => {
                        return Err(mlua::Error::external(
                            "sfae.oauth: argument 'opts' expected table",
                        ))
                    }
                };

                let opener = opener.as_ref().ok_or_else(|| {
                    mlua::Error::external("sfae.oauth: browser opener not available")
                })?;

                // Extract opts
                let scope: Option<String> = opts.get("scope")
                    .map_err(|e| mlua::Error::external(format!("sfae.oauth: {e}")))?;
                let mut client_id: Option<String> = opts.get("client_id")
                    .map_err(|e| mlua::Error::external(format!("sfae.oauth: {e}")))?;
                let mut auth_url: Option<String> = opts.get("auth_url")
                    .map_err(|e| mlua::Error::external(format!("sfae.oauth: {e}")))?;
                let mut token_url: Option<String> = opts.get("token_url")
                    .map_err(|e| mlua::Error::external(format!("sfae.oauth: {e}")))?;
                let mut client_secret: Option<String> = opts.get("client_secret")
                    .map_err(|e| mlua::Error::external(format!("sfae.oauth: {e}")))?;
                let mut revocation_url: Option<String> = None;

                // Resolve provider preset and merge (preset fills gaps, user params override)
                if let Some(preset) = sfae_core::oauth::get_provider_preset(&domain) {
                    if client_id.is_none() {
                        client_id = Some(preset.client_id.to_string());
                    }
                    if auth_url.is_none() {
                        auth_url = Some(preset.auth_url.to_string());
                    }
                    if token_url.is_none() {
                        token_url = Some(preset.token_url.to_string());
                    }
                    if client_secret.is_none() {
                        client_secret = preset.client_secret.map(|s| s.to_string());
                    }
                    revocation_url = preset.revocation_url.map(|s| s.to_string());
                }

                // Validate required params
                let mut missing = Vec::new();
                if client_id.is_none() { missing.push("client_id"); }
                if auth_url.is_none() { missing.push("auth_url"); }
                if token_url.is_none() { missing.push("token_url"); }
                if !missing.is_empty() {
                    return Err(mlua::Error::external(format!(
                        "sfae.oauth: missing required parameters for '{}': {}. Provide them in the opts table or use a known provider domain.",
                        domain,
                        missing.join(", ")
                    )));
                }

                let client_id = client_id.unwrap();
                let auth_url = auth_url.unwrap();
                let token_url = token_url.unwrap();

                // Revoke existing access token if revocation URL is available
                if let Some(ref rev_url) = revocation_url {
                    let guard = store.lock().map_err(|e| {
                        mlua::Error::external(format!("sfae: store lock poisoned: {e}"))
                    })?;
                    if let Ok(old_token) = proxy::get_credential_with_fallback(
                        &*guard, &domain, None, CredentialType::AccessToken,
                    ) {
                        drop(guard);
                        // Best-effort revocation — ignore errors
                        let _ = sfae_core::oauth::revoke_token(rev_url, &old_token);
                    }
                }

                // Generate PKCE verifier + challenge + state
                let verifier = sfae_core::oauth::generate_code_verifier();
                let challenge = sfae_core::oauth::compute_code_challenge(&verifier);
                let state = sfae_core::oauth::generate_state();

                // Create local server for OAuth callback
                let server = sfae_core::browser::LocalServer::new().map_err(|e| {
                    mlua::Error::external(format!("sfae.oauth: failed to bind local server: {e}"))
                })?;
                let redirect_uri = format!("http://127.0.0.1:{}/callback", server.port());

                // Build authorization URL
                let auth_url_full = sfae_core::oauth::build_authorization_url(
                    &auth_url,
                    &client_id,
                    &redirect_uri,
                    &challenge,
                    scope.as_deref(),
                    &state,
                );

                // Open browser
                opener.open_url(&auth_url_full).map_err(|e| {
                    mlua::Error::external(format!("sfae.oauth: {e}"))
                })?;

                // Wait for OAuth callback
                let (code, callback_state) = sfae_core::browser::oauth_callback(&server).map_err(|e| {
                    match e {
                        sfae_core::SfaeError::Cancelled => mlua::Error::external(
                            "sfae.oauth: OAuth timed out — no authorization received",
                        ),
                        other => mlua::Error::external(format!("sfae.oauth: {other}")),
                    }
                })?;

                // Verify state matches
                if callback_state != state {
                    return Err(mlua::Error::external("sfae.oauth: OAuth state mismatch"));
                }

                // Exchange code for tokens
                let token_response = sfae_core::oauth::exchange_code(
                    &token_url,
                    &code,
                    &redirect_uri,
                    &client_id,
                    client_secret.as_deref(),
                    &verifier,
                )
                .map_err(|e| mlua::Error::external(format!("sfae.oauth: {e}")))?;

                // Store tokens
                {
                    let mut guard = store.lock().map_err(|e| {
                        mlua::Error::external(format!("sfae: store lock poisoned: {e}"))
                    })?;
                    let access_key = credential::credential_key(&domain, None, CredentialType::AccessToken);
                    guard.set(&access_key, &token_response.access_token).map_err(|e| {
                        mlua::Error::external(format!("sfae.oauth: failed to store access token: {e}"))
                    })?;
                    if let Some(ref refresh_token) = token_response.refresh_token {
                        let refresh_key = credential::credential_key(&domain, None, CredentialType::RefreshToken);
                        guard.set(&refresh_key, refresh_token).map_err(|e| {
                            mlua::Error::external(format!("sfae.oauth: failed to store refresh token: {e}"))
                        })?;
                    }
                    if let Some(ref secret) = client_secret {
                        let secret_key = credential::credential_key(&domain, None, CredentialType::ClientSecret);
                        guard.set(&secret_key, secret).map_err(|e| {
                            mlua::Error::external(format!("sfae.oauth: failed to store client secret: {e}"))
                        })?;
                    }
                }

                // Save OAuth metadata to disk
                sfae_core::oauth::save_oauth_metadata(
                    &domain,
                    None,
                    sfae_core::oauth::OAuthMetadata {
                        token_url: token_url.clone(),
                        client_id: client_id.clone(),
                        revocation_url,
                    },
                )
                .map_err(|e| mlua::Error::external(format!("sfae.oauth: failed to save OAuth metadata: {e}")))?;

                Ok(mlua::Value::Boolean(true))
            })?,
        )?;
    }

    register_help_functions(lua, &sfae, &SFAE_DOC)?;
    lua.globals().set("sfae", sfae)?;
    wrap_module_with_help_hints(lua, "sfae")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sfae_core::store::InMemoryStore;
    use std::sync::atomic::{AtomicBool, Ordering};

    /// Mock credential prompt for testing.
    struct MockPrompt {
        secret: Mutex<Option<String>>,
        cancelled: AtomicBool,
    }

    impl MockPrompt {
        fn new(secret: &str) -> Self {
            Self {
                secret: Mutex::new(Some(secret.to_string())),
                cancelled: AtomicBool::new(false),
            }
        }

        fn cancelled() -> Self {
            Self {
                secret: Mutex::new(None),
                cancelled: AtomicBool::new(true),
            }
        }
    }

    impl CredentialPrompt for MockPrompt {
        fn prompt_credential(
            &self,
            _domain: &str,
            _credential_type: &str,
            _url: Option<&str>,
        ) -> Result<String, String> {
            if self.cancelled.load(Ordering::Relaxed) {
                return Err("credential prompt cancelled".to_string());
            }
            let guard = self.secret.lock().unwrap();
            match &*guard {
                Some(s) => Ok(s.clone()),
                None => Err("credential prompt cancelled".to_string()),
            }
        }
    }

    fn setup_lua(
        store: Arc<Mutex<dyn SecretStore + Send>>,
        prompt: Arc<dyn CredentialPrompt>,
    ) -> Lua {
        let lua = Lua::new();
        register_sfae_globals(&lua, store, prompt, None).unwrap();
        lua
    }

    #[test]
    fn credentials_returns_stored_types() {
        let store = Arc::new(Mutex::new(InMemoryStore::new()));
        {
            let mut guard = store.lock().unwrap();
            guard.set("github.com_ACCESS_TOKEN", "tok").unwrap();
            guard.set("github.com_API_KEY", "key").unwrap();
        }
        let prompt = Arc::new(MockPrompt::new("unused"));
        let lua = setup_lua(store, prompt);

        let result: Vec<String> = lua
            .load(r#"sfae.credentials("github.com")"#)
            .eval()
            .unwrap();
        assert!(result.contains(&"ACCESS_TOKEN".to_string()));
        assert!(result.contains(&"API_KEY".to_string()));
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn credentials_returns_empty_for_unknown_domain() {
        let store = Arc::new(Mutex::new(InMemoryStore::new()));
        let prompt = Arc::new(MockPrompt::new("unused"));
        let lua = setup_lua(store, prompt);

        let result: Vec<String> = lua
            .load(r#"sfae.credentials("unknown.com")"#)
            .eval()
            .unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn prompt_stores_credential_on_success() {
        let store = Arc::new(Mutex::new(InMemoryStore::new()));
        let prompt = Arc::new(MockPrompt::new("my-secret-token"));
        let lua = setup_lua(store.clone(), prompt);

        let result: bool = lua
            .load(r#"sfae.prompt("github.com", "ACCESS_TOKEN", {url = "https://github.com/settings/tokens"})"#)
            .eval()
            .unwrap();
        assert!(result);

        // Verify credential was stored
        let guard = store.lock().unwrap();
        assert_eq!(
            guard.get("github.com_ACCESS_TOKEN").unwrap(),
            "my-secret-token"
        );
    }

    #[test]
    fn prompt_returns_error_on_cancel() {
        let store = Arc::new(Mutex::new(InMemoryStore::new()));
        let prompt = Arc::new(MockPrompt::cancelled());
        let lua = setup_lua(store, prompt);

        let result = lua
            .load(r#"sfae.prompt("github.com", "ACCESS_TOKEN")"#)
            .eval::<bool>();
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("cancelled"),
            "expected 'cancelled' in error: {err_msg}"
        );
    }

    #[test]
    fn request_resolves_placeholders() {
        let store = Arc::new(Mutex::new(InMemoryStore::new()));
        {
            let mut guard = store.lock().unwrap();
            guard.set("httpbin.org_ACCESS_TOKEN", "test-token").unwrap();
        }
        let prompt = Arc::new(MockPrompt::new("unused"));
        let lua = setup_lua(store, prompt);

        // We can't easily test real HTTP here without a server, but we can verify
        // that the proxy machinery is invoked by checking that a missing credential
        // produces the expected error (see next test). For placeholder resolution,
        // we'd need a mock HTTP server which is out of scope for unit tests.
        // Instead, we verify the function exists and accepts the right arguments.
        // Integration tests with a real HTTP server belong in a separate test suite.

        // Verify that sfae.request is callable (it will fail with a network error
        // since httpbin.org might not be reachable, but the point is it gets past
        // argument parsing and placeholder resolution)
        let result = lua
            .load(r#"sfae.request("GET", "https://httpbin.org/get", {headers = {Authorization = "Bearer -ACCESS_TOKEN-"}})"#)
            .eval::<mlua::Table>();
        // Either succeeds (if network available) or fails with HTTP error (not credential error)
        match result {
            Ok(table) => {
                // If network was available, verify response shape
                assert!(table.get::<u16>("status").is_ok());
                assert!(table.get::<String>("body").is_ok());
                assert!(table.get::<bool>("ok").is_ok());
            }
            Err(e) => {
                let msg = e.to_string();
                // Should NOT be a credential error since we stored the token
                assert!(
                    !msg.contains("credential not found"),
                    "unexpected credential error: {msg}"
                );
            }
        }
    }

    #[test]
    fn request_errors_on_missing_credential() {
        let store = Arc::new(Mutex::new(InMemoryStore::new()));
        let prompt = Arc::new(MockPrompt::new("unused"));
        let lua = setup_lua(store, prompt);

        let result = lua
            .load(r#"sfae.request("GET", "https://api.github.com/user", {headers = {Authorization = "Bearer -ACCESS_TOKEN-"}})"#)
            .eval::<mlua::Table>();
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("credential not found"),
            "expected 'credential not found' in error: {err_msg}"
        );
    }

    #[test]
    fn help_prints_nonempty_output() {
        let store = Arc::new(Mutex::new(InMemoryStore::new()));
        let prompt = Arc::new(MockPrompt::new("unused"));
        let lua = setup_lua(store, prompt);

        // help() prints to stdout and returns nil — verify it doesn't error
        lua.load(r#"sfae.help()"#).exec().unwrap();

        // Verify help text is accessible via __summary
        let sfae_table: mlua::Table = lua.globals().get("sfae").unwrap();
        let summary: String = sfae_table.get("__summary").unwrap();
        assert!(!summary.is_empty());
        assert!(summary.contains("credential"));
    }

    // === OAuth + 401 refresh test infrastructure ===

    /// Serialize tests that modify the on-disk `oauth.json` file.
    fn oauth_metadata_test_lock() -> &'static Mutex<()> {
        static LOCK: std::sync::OnceLock<Mutex<()>> = std::sync::OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    /// Mock browser opener that records URLs without performing any action.
    struct MockBrowserOpener {
        urls: Mutex<Vec<String>>,
    }

    impl MockBrowserOpener {
        fn new() -> Self {
            Self {
                urls: Mutex::new(Vec::new()),
            }
        }
    }

    impl BrowserOpener for MockBrowserOpener {
        fn open_url(&self, url: &str) -> Result<(), String> {
            self.urls.lock().unwrap().push(url.to_string());
            Ok(())
        }
    }

    /// Mock browser opener that records the URL AND simulates an OAuth provider
    /// redirecting back to the local callback server with a fake auth code.
    struct CallbackSimulatingOpener {
        urls: Mutex<Vec<String>>,
    }

    impl CallbackSimulatingOpener {
        fn new() -> Self {
            Self {
                urls: Mutex::new(Vec::new()),
            }
        }

        fn recorded_urls(&self) -> Vec<String> {
            self.urls.lock().unwrap().clone()
        }
    }

    impl BrowserOpener for CallbackSimulatingOpener {
        fn open_url(&self, url: &str) -> Result<(), String> {
            self.urls.lock().unwrap().push(url.to_string());

            // Parse redirect_uri and state from the authorization URL query params.
            let query = url.split('?').nth(1).ok_or("no query string in auth URL")?;
            let redirect_uri = test_extract_query_param(query, "redirect_uri")
                .ok_or("missing redirect_uri in auth URL")?;
            let state =
                test_extract_query_param(query, "state").ok_or("missing state in auth URL")?;

            // Decode the percent-encoded redirect_uri.
            let redirect_uri = test_percent_decode(&redirect_uri);

            // Extract host:port from http://127.0.0.1:{port}/callback
            let without_scheme = redirect_uri
                .strip_prefix("http://")
                .ok_or("redirect_uri missing http:// scheme")?;
            let host_port = without_scheme
                .split('/')
                .next()
                .ok_or("redirect_uri missing host")?
                .to_string();

            // Spawn a thread to simulate the provider redirect after a small delay.
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(50));
                if let Ok(mut stream) = std::net::TcpStream::connect(&host_port) {
                    use std::io::Write;
                    let _ = write!(
                        stream,
                        "GET /callback?code=fake_auth_code&state={state} HTTP/1.1\r\n\
                         Host: 127.0.0.1\r\nConnection: close\r\n\r\n"
                    );
                }
            });

            Ok(())
        }
    }

    fn setup_lua_with_opener(
        store: Arc<Mutex<dyn SecretStore + Send>>,
        prompt: Arc<dyn CredentialPrompt>,
        opener: Arc<dyn BrowserOpener>,
    ) -> Lua {
        let lua = Lua::new();
        register_sfae_globals(&lua, store, prompt, Some(opener)).unwrap();
        lua
    }

    /// Extract a query parameter value from a query string.
    fn test_extract_query_param(query: &str, key: &str) -> Option<String> {
        let prefix = format!("{key}=");
        for pair in query.split('&') {
            if let Some(value) = pair.strip_prefix(&prefix) {
                return Some(value.to_string());
            }
        }
        None
    }

    /// Minimal percent-decoding for test URLs.
    fn test_percent_decode(s: &str) -> String {
        let mut result = Vec::new();
        let bytes = s.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'%' && i + 2 < bytes.len() {
                if let Ok(byte) =
                    u8::from_str_radix(&String::from_utf8_lossy(&bytes[i + 1..i + 3]), 16)
                {
                    result.push(byte);
                    i += 3;
                    continue;
                }
            }
            result.push(bytes[i]);
            i += 1;
        }
        String::from_utf8_lossy(&result).into_owned()
    }

    /// Accept one HTTP request on a listener and respond with the given status and body.
    fn mock_http_accept(listener: &std::net::TcpListener, status: u16, body: &str) {
        use std::io::{BufRead, BufReader, Read, Write};

        let (stream, _) = listener.accept().unwrap();
        let read_stream = stream.try_clone().unwrap();
        let mut reader = BufReader::new(read_stream);

        // Read request line + headers, tracking content-length.
        let mut content_length: usize = 0;
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).unwrap() == 0 {
                break;
            }
            if line.trim().is_empty() {
                break;
            }
            let lower = line.to_ascii_lowercase();
            if let Some(val) = lower.strip_prefix("content-length:") {
                if let Ok(len) = val.trim().parse::<usize>() {
                    content_length = len;
                }
            }
        }

        // Consume request body if present.
        if content_length > 0 {
            let mut buf = vec![0u8; content_length];
            let _ = reader.read_exact(&mut buf);
        }

        let status_text = match status {
            200 => "OK",
            401 => "Unauthorized",
            _ => "Unknown",
        };
        let response = format!(
            "HTTP/1.1 {status} {status_text}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len(),
        );
        let mut write_stream = stream;
        write_stream.write_all(response.as_bytes()).unwrap();
        write_stream.flush().unwrap();
    }

    /// RAII guard that removes OAuth metadata for a test domain on drop.
    struct OAuthMetadataCleanup {
        domain: String,
    }

    impl Drop for OAuthMetadataCleanup {
        fn drop(&mut self) {
            let _ = sfae_core::oauth::remove_oauth_metadata(&self.domain, None);
        }
    }

    // === 4a: sfae.oauth() parameter validation ===

    #[test]
    fn oauth_errors_without_browser_opener() {
        let store = Arc::new(Mutex::new(InMemoryStore::new()));
        let prompt = Arc::new(MockPrompt::new("unused"));
        let lua = setup_lua(store, prompt); // passes None for opener

        let result = lua
            .load(r#"sfae.oauth("googleapis.com", {scope = "test"})"#)
            .eval::<bool>();
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("browser opener not available"),
            "expected 'browser opener not available' in: {err}"
        );
    }

    #[test]
    fn oauth_errors_missing_all_required_params() {
        let store = Arc::new(Mutex::new(InMemoryStore::new()));
        let prompt = Arc::new(MockPrompt::new("unused"));
        let opener = Arc::new(MockBrowserOpener::new());
        let lua = setup_lua_with_opener(store, prompt, opener);

        let result = lua
            .load(r#"sfae.oauth("unknown-provider.example.com", {scope = "read"})"#)
            .eval::<bool>();
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("missing required parameters"),
            "expected 'missing required parameters' in: {err}"
        );
        assert!(err.contains("client_id"), "expected 'client_id' in: {err}");
        assert!(err.contains("auth_url"), "expected 'auth_url' in: {err}");
        assert!(err.contains("token_url"), "expected 'token_url' in: {err}");
    }

    #[test]
    fn oauth_errors_missing_partial_params() {
        let store = Arc::new(Mutex::new(InMemoryStore::new()));
        let prompt = Arc::new(MockPrompt::new("unused"));
        let opener = Arc::new(MockBrowserOpener::new());
        let lua = setup_lua_with_opener(store, prompt, opener);

        let result = lua
            .load(
                r#"sfae.oauth("unknown-provider.example.com", {client_id = "my-id", scope = "read"})"#,
            )
            .eval::<bool>();
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("missing required parameters"),
            "expected 'missing required parameters' in: {err}"
        );
        assert!(
            !err.contains("client_id"),
            "should NOT list client_id as missing: {err}"
        );
        assert!(err.contains("auth_url"), "expected 'auth_url' in: {err}");
        assert!(err.contains("token_url"), "expected 'token_url' in: {err}");
    }

    #[test]
    fn oauth_preset_resolves_googleapis() {
        // googleapis.com preset fills client_id, auth_url, token_url automatically.
        // We override token_url to a mock server so the test is fast and offline.
        let mock_token = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let mock_token_port = mock_token.local_addr().unwrap().port();
        let handle = std::thread::spawn(move || {
            // Return 200 without access_token — exchange_code will error, that's expected.
            mock_http_accept(&mock_token, 200, r#"{"error": "test_fail"}"#);
        });

        let store = Arc::new(Mutex::new(InMemoryStore::new()));
        let prompt = Arc::new(MockPrompt::new("unused"));
        let opener = Arc::new(CallbackSimulatingOpener::new());
        let lua = setup_lua_with_opener(store, prompt, opener.clone());

        let lua_code = format!(
            r#"sfae.oauth("googleapis.com", {{
                scope = "https://www.googleapis.com/auth/gmail.readonly",
                token_url = "http://127.0.0.1:{mock_token_port}/token"
            }})"#,
        );
        let result = lua.load(&lua_code).eval::<bool>();

        // exchange_code fails — expected. Key assertion: NOT a "missing params" error.
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            !err.contains("missing required parameters"),
            "preset should fill required params: {err}"
        );

        // Verify the opener was called with Google's auth URL and preset client_id.
        let urls = opener.recorded_urls();
        assert_eq!(urls.len(), 1, "opener should be called once");
        let auth_url = &urls[0];
        assert!(
            auth_url.contains("accounts.google.com"),
            "should use Google's auth URL: {auth_url}"
        );
        assert!(
            auth_url.contains(".apps.googleusercontent.com"),
            "should contain preset client_id: {auth_url}"
        );
        assert!(
            auth_url.contains("gmail.readonly"),
            "should contain the requested scope: {auth_url}"
        );

        handle.join().unwrap();
    }

    #[test]
    fn oauth_user_params_override_preset() {
        let mock_token = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let mock_token_port = mock_token.local_addr().unwrap().port();
        let handle = std::thread::spawn(move || {
            mock_http_accept(&mock_token, 200, r#"{"error": "test_fail"}"#);
        });

        let store = Arc::new(Mutex::new(InMemoryStore::new()));
        let prompt = Arc::new(MockPrompt::new("unused"));
        let opener = Arc::new(CallbackSimulatingOpener::new());
        let lua = setup_lua_with_opener(store, prompt, opener.clone());

        let lua_code = format!(
            r#"sfae.oauth("googleapis.com", {{
                client_id = "my-custom-client-id",
                scope = "https://www.googleapis.com/auth/gmail.readonly",
                token_url = "http://127.0.0.1:{mock_token_port}/token"
            }})"#,
        );
        let result = lua.load(&lua_code).eval::<bool>();

        assert!(result.is_err()); // exchange_code fails, expected
        let err = result.unwrap_err().to_string();
        assert!(
            !err.contains("missing required parameters"),
            "should not have missing params: {err}"
        );

        let urls = opener.recorded_urls();
        assert_eq!(urls.len(), 1);
        let auth_url = &urls[0];
        assert!(
            auth_url.contains("client_id=my-custom-client-id"),
            "should use custom client_id: {auth_url}"
        );
        assert!(
            !auth_url.contains(".apps.googleusercontent.com"),
            "should NOT use preset client_id: {auth_url}"
        );

        handle.join().unwrap();
    }

    // === 4b: OAuth integration test ===

    #[test]
    fn oauth_integration_stores_tokens() {
        let _lock = oauth_metadata_test_lock().lock().unwrap();

        // Mock token server returns fake tokens on exchange.
        let token_listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let token_port = token_listener.local_addr().unwrap().port();
        let token_url = format!("http://127.0.0.1:{token_port}/token");

        let token_handle = std::thread::spawn(move || {
            mock_http_accept(
                &token_listener,
                200,
                r#"{"access_token": "test-access-tok", "refresh_token": "test-refresh-tok"}"#,
            );
        });

        let store = Arc::new(Mutex::new(InMemoryStore::new()));
        let prompt = Arc::new(MockPrompt::new("unused"));
        let opener = Arc::new(CallbackSimulatingOpener::new());
        let test_domain = "test-integration-4b.example.com";
        let _cleanup = OAuthMetadataCleanup {
            domain: test_domain.to_string(),
        };

        let lua = setup_lua_with_opener(store.clone(), prompt, opener);

        let lua_code = format!(
            r#"sfae.oauth("{test_domain}", {{
                client_id = "test-client-id",
                auth_url = "http://127.0.0.1:{token_port}/auth",
                token_url = "{token_url}",
                scope = "test-scope"
            }})"#,
        );

        let result: bool = lua.load(&lua_code).eval().unwrap();
        assert!(result);

        // Verify tokens were stored.
        let guard = store.lock().unwrap();
        assert_eq!(
            guard.get(&format!("{test_domain}_ACCESS_TOKEN")).unwrap(),
            "test-access-tok"
        );
        assert_eq!(
            guard.get(&format!("{test_domain}_REFRESH_TOKEN")).unwrap(),
            "test-refresh-tok"
        );

        token_handle.join().unwrap();
    }

    // === 4c: 401 auto-refresh tests ===

    #[test]
    fn access_token_placeholder_in_header() {
        let req = ProxyRequest {
            method: "GET".into(),
            url: "https://api.example.com/data".into(),
            headers: vec![("Authorization".into(), "Bearer -ACCESS_TOKEN-".into())],
            body: None,
        };
        assert!(request_has_access_token_placeholder(&req));
    }

    #[test]
    fn access_token_placeholder_in_url() {
        let req = ProxyRequest {
            method: "GET".into(),
            url: "https://api.example.com/data?token=-ACCESS_TOKEN-".into(),
            headers: vec![],
            body: None,
        };
        assert!(request_has_access_token_placeholder(&req));
    }

    #[test]
    fn access_token_placeholder_in_body() {
        let req = ProxyRequest {
            method: "POST".into(),
            url: "https://api.example.com/data".into(),
            headers: vec![],
            body: Some("token=-ACCESS_TOKEN-".into()),
        };
        assert!(request_has_access_token_placeholder(&req));
    }

    #[test]
    fn no_access_token_placeholder() {
        let req = ProxyRequest {
            method: "GET".into(),
            url: "https://api.example.com/data".into(),
            headers: vec![("Authorization".into(), "Bearer -API_KEY-".into())],
            body: None,
        };
        assert!(!request_has_access_token_placeholder(&req));
    }

    #[test]
    fn try_refresh_returns_none_without_metadata() {
        // Domain with no OAuth metadata on disk → returns None immediately.
        let store: Arc<Mutex<dyn SecretStore + Send>> = Arc::new(Mutex::new(InMemoryStore::new()));
        let req = ProxyRequest {
            method: "GET".into(),
            url: "https://no-metadata-domain.example.com/api".into(),
            headers: vec![],
            body: None,
        };
        let result = try_refresh_and_retry(&store, &req, "no-metadata-domain.example.com");
        assert!(result.is_none());
    }

    #[test]
    fn try_refresh_returns_none_without_refresh_token() {
        let _lock = oauth_metadata_test_lock().lock().unwrap();
        let test_domain = "test-no-refresh-4c.example.com";
        let _cleanup = OAuthMetadataCleanup {
            domain: test_domain.to_string(),
        };

        // Write OAuth metadata but don't store a refresh token.
        sfae_core::oauth::save_oauth_metadata(
            test_domain,
            None,
            sfae_core::oauth::OAuthMetadata {
                token_url: "http://localhost:99999/token".into(),
                client_id: "test".into(),
                revocation_url: None,
            },
        )
        .unwrap();

        let store: Arc<Mutex<dyn SecretStore + Send>> = Arc::new(Mutex::new(InMemoryStore::new()));
        let req = ProxyRequest {
            method: "GET".into(),
            url: format!("https://{test_domain}/api"),
            headers: vec![],
            body: None,
        };
        let result = try_refresh_and_retry(&store, &req, test_domain);
        assert!(result.is_none());
    }

    #[test]
    fn request_401_with_refresh_retries_successfully() {
        let _lock = oauth_metadata_test_lock().lock().unwrap();

        // API server: first request → 401, second → 200.
        let api_listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let api_port = api_listener.local_addr().unwrap().port();

        let api_handle = std::thread::spawn(move || {
            mock_http_accept(&api_listener, 401, r#"{"error": "unauthorized"}"#);
            mock_http_accept(&api_listener, 200, r#"{"data": "success"}"#);
        });

        // Token server: returns refreshed tokens.
        let token_listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let token_port = token_listener.local_addr().unwrap().port();

        let token_handle = std::thread::spawn(move || {
            mock_http_accept(
                &token_listener,
                200,
                r#"{"access_token": "refreshed-tok", "refresh_token": "new-refresh-tok"}"#,
            );
        });

        let test_domain = "127.0.0.1"; // extracted from the URL by proxy::extract_host
        let _cleanup = OAuthMetadataCleanup {
            domain: test_domain.to_string(),
        };

        // Save OAuth metadata pointing to the mock token server.
        sfae_core::oauth::save_oauth_metadata(
            test_domain,
            None,
            sfae_core::oauth::OAuthMetadata {
                token_url: format!("http://127.0.0.1:{token_port}/token"),
                client_id: "test-client".into(),
                revocation_url: None,
            },
        )
        .unwrap();

        let store = Arc::new(Mutex::new(InMemoryStore::new()));
        {
            let mut guard = store.lock().unwrap();
            guard.set("127.0.0.1_ACCESS_TOKEN", "expired-tok").unwrap();
            guard
                .set("127.0.0.1_REFRESH_TOKEN", "old-refresh-tok")
                .unwrap();
        }

        let prompt = Arc::new(MockPrompt::new("unused"));
        let lua = setup_lua(store.clone(), prompt);

        let lua_code = format!(
            r#"sfae.request("GET", "http://127.0.0.1:{api_port}/api", {{headers = {{Authorization = "Bearer -ACCESS_TOKEN-"}}}})"#,
        );

        let result: mlua::Table = lua.load(&lua_code).eval().unwrap();
        assert_eq!(result.get::<u16>("status").unwrap(), 200);

        // Verify tokens were updated in the store.
        let guard = store.lock().unwrap();
        assert_eq!(
            guard.get("127.0.0.1_ACCESS_TOKEN").unwrap(),
            "refreshed-tok"
        );
        assert_eq!(
            guard.get("127.0.0.1_REFRESH_TOKEN").unwrap(),
            "new-refresh-tok"
        );

        api_handle.join().unwrap();
        token_handle.join().unwrap();
    }
}
