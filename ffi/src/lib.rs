//! C ABI for loading CPSL as a sandbox library.

#[cfg(feature = "pdfium-render")]
use cpsl_core::PdfiumEngine;
#[cfg(feature = "webbrowser")]
use cpsl_core::WebBrowserGateway;
use cpsl_core::{HttpGateway, MountPermission, MountTable, Sandbox};
use serde::Deserialize;
use serde_json::json;
use std::ffi::{c_char, c_void, CStr, CString};
use std::net::IpAddr;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Component, Path, PathBuf};
use std::ptr;
use std::sync::{Arc, Mutex, OnceLock};
#[cfg(feature = "webbrowser")]
use url::Url;

const CPSL_ABI_VERSION: u32 = 2;
const WORKDIR: &str = "/workdir";
const LANGUAGE_LUAU: &str = "luau";
const LANGUAGE_BASH: &str = "bash";
#[cfg(feature = "pdfium-render")]
const CPSL_LIBRARY_DIR_ENV: &str = "CPSL_LIBRARY_DIR";
#[cfg(feature = "pdfium-render")]
const CPSL_REQUIRE_STAGED_PDFIUM_ENV: &str = "CPSL_REQUIRE_STAGED_PDFIUM";
const SHRT_SOURCE: &str = include_str!("../../runtime/shrt.luau");

static LAST_ERROR: OnceLock<Mutex<CString>> = OnceLock::new();

#[allow(non_camel_case_types)]
#[repr(C)]
pub struct cpsl_session_t {
    _private: [u8; 0],
}

type WebBrowserHandleJsonFn =
    unsafe extern "C" fn(user_data: *mut c_void, request_json: *const c_char) -> *mut c_char;
type WebBrowserStringFreeFn = unsafe extern "C" fn(value: *mut c_char);
type WebBrowserUserDataFreeFn = unsafe extern "C" fn(user_data: *mut c_void);

#[allow(non_camel_case_types)]
#[repr(C)]
pub struct cpsl_webbrowser_callbacks_t {
    user_data: *mut c_void,
    handle_json: Option<WebBrowserHandleJsonFn>,
    string_free: Option<WebBrowserStringFreeFn>,
    user_data_free: Option<WebBrowserUserDataFreeFn>,
}

#[cfg(feature = "webbrowser")]
struct FfiWebBrowserGateway {
    user_data: *mut c_void,
    handle_json: WebBrowserHandleJsonFn,
    string_free: WebBrowserStringFreeFn,
    user_data_free: Option<WebBrowserUserDataFreeFn>,
    callback_lock: Mutex<()>,
}

#[cfg(feature = "webbrowser")]
struct PolicyWebBrowserGateway {
    inner: Arc<dyn WebBrowserGateway>,
    policy: WebBrowserPolicy,
}

#[cfg(feature = "webbrowser")]
#[derive(Clone)]
struct WebBrowserPolicy {
    allow_domains: Vec<String>,
    deny_domains: Vec<String>,
}

#[cfg(feature = "webbrowser")]
unsafe impl Send for FfiWebBrowserGateway {}
#[cfg(feature = "webbrowser")]
unsafe impl Sync for FfiWebBrowserGateway {}

#[cfg(feature = "webbrowser")]
impl WebBrowserGateway for FfiWebBrowserGateway {
    fn handle_json(&self, request_json: &str) -> Result<String, String> {
        let request = CString::new(request_json)
            .map_err(|_| "webbrowser request JSON contained an embedded NUL byte".to_string())?;
        let _guard = self
            .callback_lock
            .lock()
            .map_err(|_| "webbrowser callback lock was poisoned".to_string())?;
        let response = unsafe { (self.handle_json)(self.user_data, request.as_ptr()) };
        if response.is_null() {
            return Err("webbrowser callback returned NULL".to_string());
        }

        let result = unsafe { CStr::from_ptr(response) }
            .to_str()
            .map(str::to_owned)
            .map_err(|_| "webbrowser callback returned non-UTF-8 JSON".to_string());
        unsafe {
            (self.string_free)(response);
        }
        result
    }
}

#[cfg(feature = "webbrowser")]
impl WebBrowserGateway for PolicyWebBrowserGateway {
    fn handle_json(&self, request_json: &str) -> Result<String, String> {
        let mut request: serde_json::Value = serde_json::from_str(request_json)
            .map_err(|error| format!("invalid webbrowser request JSON: {error}"))?;
        let object = request
            .as_object_mut()
            .ok_or_else(|| "webbrowser request JSON must be an object".to_string())?;

        if object.get("command").and_then(serde_json::Value::as_str) == Some("open") {
            let url = object
                .get("url")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| "webbrowser.open: missing URL".to_string())?;
            self.policy.check_url(url)?;
        }

        object.insert(
            "networkPolicy".to_string(),
            json!({
                "allowDomains": &self.policy.allow_domains,
                "denyDomains": &self.policy.deny_domains,
                "unrestricted": self.policy.allow_domains.iter().any(|domain| domain == "*"),
            }),
        );
        self.inner.handle_json(&request.to_string())
    }
}

#[cfg(feature = "webbrowser")]
impl Drop for FfiWebBrowserGateway {
    fn drop(&mut self) {
        if !self.user_data.is_null() {
            if let Some(user_data_free) = self.user_data_free {
                unsafe {
                    user_data_free(self.user_data);
                }
            }
        }
    }
}

struct Session {
    _config: ValidatedSessionConfig,
    sandbox: Sandbox,
}

struct ValidatedSessionConfig {
    mounts: Vec<ValidatedMountConfig>,
    initial_cwd: String,
    allow_domains: Vec<String>,
    deny_domains: Vec<String>,
    #[cfg(feature = "webbrowser")]
    webbrowser_policy: WebBrowserPolicy,
}

struct ValidatedMountConfig {
    host: PathBuf,
    virtual_path: String,
    permission: MountPermission,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SessionConfig {
    mounts: Vec<MountConfig>,
    initial_cwd: String,
    language: String,
    http: HttpConfig,
    #[cfg_attr(not(feature = "webbrowser"), allow(dead_code))]
    #[serde(default)]
    webbrowser: Option<WebBrowserConfig>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MountConfig {
    host: String,
    #[serde(rename = "virtual")]
    virtual_path: String,
    mode: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HttpConfig {
    mode: String,
    allow_domains: Vec<String>,
    deny_domains: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(not(feature = "webbrowser"), allow(dead_code))]
struct WebBrowserConfig {
    mode: String,
    allow_domains: Vec<String>,
    deny_domains: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EvalRequest {
    language: String,
    input: String,
    timeout_ms: u64,
}

#[no_mangle]
pub extern "C" fn cpsl_abi_version() -> u32 {
    clear_last_error();
    CPSL_ABI_VERSION
}

#[no_mangle]
pub extern "C" fn cpsl_backend_metadata_json() -> *mut c_char {
    ffi_result(|| {
        let metadata = json!({
            "name": "cpsl",
            "abi_version": CPSL_ABI_VERSION,
            "version": env!("CARGO_PKG_VERSION"),
            "languages": [LANGUAGE_LUAU, LANGUAGE_BASH],
            "capabilities": {
                "mounts": true,
                "network_policy": true,
                "webbrowser_callbacks": cfg!(feature = "webbrowser")
            }
        });
        owned_c_string(metadata.to_string())
    })
}

#[no_mangle]
pub extern "C" fn cpsl_session_new(config_json: *const c_char) -> *mut cpsl_session_t {
    session_new_inner(config_json, None)
}

#[cfg(feature = "webbrowser")]
#[no_mangle]
pub extern "C" fn cpsl_session_new_with_webbrowser_callbacks(
    config_json: *const c_char,
    callbacks: *const cpsl_webbrowser_callbacks_t,
) -> *mut cpsl_session_t {
    let gateway = match validate_webbrowser_callbacks(callbacks) {
        Ok(gateway) => gateway,
        Err(error) => {
            set_last_error(&error);
            return ptr::null_mut();
        }
    };
    session_new_inner(config_json, gateway)
}

#[cfg(not(feature = "webbrowser"))]
#[no_mangle]
pub extern "C" fn cpsl_session_new_with_webbrowser_callbacks(
    _config_json: *const c_char,
    callbacks: *const cpsl_webbrowser_callbacks_t,
) -> *mut cpsl_session_t {
    free_webbrowser_callback_context(callbacks);
    set_last_error("CPSL was built without mod-webbrowser");
    ptr::null_mut()
}

fn session_new_inner(
    config_json: *const c_char,
    #[cfg(feature = "webbrowser")] webbrowser_gateway: Option<Arc<dyn WebBrowserGateway>>,
    #[cfg(not(feature = "webbrowser"))] _webbrowser_gateway: Option<()>,
) -> *mut cpsl_session_t {
    ffi_result(|| {
        let config_json = c_str_arg(config_json, "config_json")?;
        let config = validate_session_config(&config_json)?;
        let sandbox = create_runtime_sandbox(
            &config,
            #[cfg(feature = "webbrowser")]
            webbrowser_gateway,
        )?;
        let session = Box::new(Session {
            sandbox,
            _config: config,
        });
        Ok(Box::into_raw(session) as *mut cpsl_session_t)
    })
}

#[no_mangle]
pub extern "C" fn cpsl_session_free(session: *mut cpsl_session_t) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        clear_last_error();
        if !session.is_null() {
            unsafe {
                drop(Box::from_raw(session as *mut Session));
            }
        }
    }));
}

#[no_mangle]
pub extern "C" fn cpsl_eval(
    session: *mut cpsl_session_t,
    request_json: *const c_char,
) -> *mut c_char {
    ffi_result(|| {
        if session.is_null() {
            return Err("session must not be NULL".to_string());
        }

        let request_json = c_str_arg(request_json, "request_json")?;
        let request: EvalRequest = serde_json::from_str(&request_json)
            .map_err(|error| format!("invalid eval request: {error}"))?;
        let session = unsafe { &*(session as *const Session) };

        let response = match request.language.as_str() {
            LANGUAGE_LUAU => eval_luau(session, &request),
            LANGUAGE_BASH => eval_bash(session, &request),
            _ => eval_error_json(
                "unsupported_language",
                "Supported CPSL languages are luau and bash",
                &shell_cwd(session),
            ),
        };

        owned_c_string(response.to_string())
    })
}

#[no_mangle]
pub extern "C" fn cpsl_string_free(value: *mut c_char) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        clear_last_error();
        if !value.is_null() {
            unsafe {
                drop(CString::from_raw(value));
            }
        }
    }));
}

#[no_mangle]
pub extern "C" fn cpsl_last_error() -> *const c_char {
    with_last_error(|error| error.as_ptr())
}

fn ffi_result<T, F>(f: F) -> *mut T
where
    F: FnOnce() -> Result<*mut T, String>,
{
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(Ok(value)) => {
            clear_last_error();
            value
        }
        Ok(Err(error)) => {
            set_last_error(&error);
            ptr::null_mut()
        }
        Err(_) => {
            set_last_error("panic across CPSL FFI boundary");
            ptr::null_mut()
        }
    }
}

#[cfg(feature = "webbrowser")]
fn validate_webbrowser_callbacks(
    callbacks: *const cpsl_webbrowser_callbacks_t,
) -> Result<Option<Arc<dyn WebBrowserGateway>>, String> {
    if callbacks.is_null() {
        return Ok(None);
    }

    let callbacks = unsafe { &*callbacks };
    let handle_json = callbacks
        .handle_json
        .ok_or_else(|| "webbrowser callbacks.handle_json must not be NULL".to_string())?;
    let string_free = callbacks
        .string_free
        .ok_or_else(|| "webbrowser callbacks.string_free must not be NULL".to_string())?;

    Ok(Some(Arc::new(FfiWebBrowserGateway {
        user_data: callbacks.user_data,
        handle_json,
        string_free,
        user_data_free: callbacks.user_data_free,
        callback_lock: Mutex::new(()),
    })))
}

#[cfg(not(feature = "webbrowser"))]
fn free_webbrowser_callback_context(callbacks: *const cpsl_webbrowser_callbacks_t) {
    if callbacks.is_null() {
        return;
    }
    let callbacks = unsafe { &*callbacks };
    if !callbacks.user_data.is_null() {
        if let Some(user_data_free) = callbacks.user_data_free {
            unsafe {
                user_data_free(callbacks.user_data);
            }
        }
    }
}

fn validate_session_config(config_json: &str) -> Result<ValidatedSessionConfig, String> {
    let config: SessionConfig = serde_json::from_str(config_json)
        .map_err(|error| format!("invalid session config: {error}"))?;

    if config.language != LANGUAGE_LUAU && config.language != LANGUAGE_BASH {
        return Err("session language must be luau or bash".to_string());
    }

    if config.mounts.is_empty() {
        return Err("session config must contain at least one mount".to_string());
    }

    if config.http.mode != "policy" {
        return Err("session http mode must be policy".to_string());
    }
    let allow_domains = validate_domain_list(config.http.allow_domains, "http", "allow_domains")?;
    let deny_domains = validate_domain_list(config.http.deny_domains, "http", "deny_domains")?;
    #[cfg(feature = "webbrowser")]
    let webbrowser_policy =
        validate_webbrowser_policy(config.webbrowser, &allow_domains, &deny_domains)?;

    let mut mount_table = MountTable::new();
    let mut mounts = Vec::with_capacity(config.mounts.len());
    for mount in &config.mounts {
        let validated = validate_mount_config(mount)?;
        mount_table
            .add_mount(
                validated.host.clone(),
                &validated.virtual_path,
                validated.permission,
            )
            .map_err(|error| {
                format!(
                    "failed to configure mount {}: {error}",
                    validated.virtual_path
                )
            })?;
        mounts.push(validated);
    }
    validate_initial_cwd(&config.initial_cwd, &mount_table)?;

    Ok(ValidatedSessionConfig {
        mounts,
        initial_cwd: config.initial_cwd,
        allow_domains,
        deny_domains,
        #[cfg(feature = "webbrowser")]
        webbrowser_policy,
    })
}

fn validate_mount_config(mount: &MountConfig) -> Result<ValidatedMountConfig, String> {
    let host = validate_host_path(&mount.host)?;
    validate_absolute_virtual_path(&mount.virtual_path, "session mount virtual path")?;

    let permission = match mount.mode.as_str() {
        "ro" => MountPermission::ReadOnly,
        "rw" => MountPermission::ReadWrite,
        _ => return Err("session mount mode must be rw or ro".to_string()),
    };

    Ok(ValidatedMountConfig {
        host,
        virtual_path: mount.virtual_path.clone(),
        permission,
    })
}

fn create_runtime_sandbox(
    config: &ValidatedSessionConfig,
    #[cfg(feature = "webbrowser")] webbrowser_gateway: Option<Arc<dyn WebBrowserGateway>>,
) -> Result<Sandbox, String> {
    let mut mounts = MountTable::new();
    for mount in &config.mounts {
        mounts
            .add_mount(mount.host.clone(), &mount.virtual_path, mount.permission)
            .map_err(|error| {
                format!("failed to configure mount {}: {error}", mount.virtual_path)
            })?;
    }

    let builder = Sandbox::builder()
        .mounts(mounts)
        .http_gateway(Arc::new(create_http_gateway(config)))
        .auto_tmp(false);
    #[cfg(feature = "webbrowser")]
    let builder = if let Some(gateway) = webbrowser_gateway {
        builder.webbrowser_gateway(Arc::new(PolicyWebBrowserGateway {
            inner: gateway,
            policy: config.webbrowser_policy.clone(),
        }))
    } else {
        builder
    };
    #[cfg(feature = "pdfium-render")]
    let builder = if let Some(engine) = discover_pdfium_engine() {
        builder.pdfium_engine(Arc::new(engine))
    } else {
        builder
    };

    let sandbox = builder
        .build()
        .map_err(|error| format!("failed to create CPSL sandbox: {error}"))?;
    sandbox
        .setup_shell_runtime(SHRT_SOURCE)
        .map_err(|error| format!("failed to load CPSL shell runtime: {error}"))?;
    sandbox
        .exec(&format!(
            "local sh = require(\"shrt\"); sh.set_root(\"{}\"); sh.cd(\"{}\")",
            escape_luau_string(shell_root_for(config)),
            escape_luau_string(&config.initial_cwd)
        ))
        .map_err(|error| format!("failed to initialize CPSL shell: {error}"))?;
    Ok(sandbox)
}

fn shell_root_for(config: &ValidatedSessionConfig) -> &str {
    if config.mounts.len() == 1 && config.mounts[0].virtual_path == WORKDIR {
        WORKDIR
    } else {
        "/"
    }
}

fn create_http_gateway(config: &ValidatedSessionConfig) -> HttpGateway {
    let mut builder = HttpGateway::builder();
    for domain in &config.allow_domains {
        builder = builder.allow_domain(domain.clone());
    }
    for domain in &config.deny_domains {
        builder = builder.deny_domain(domain.clone());
    }
    builder.build()
}

#[cfg(feature = "webbrowser")]
fn validate_webbrowser_policy(
    config: Option<WebBrowserConfig>,
    http_allow_domains: &[String],
    http_deny_domains: &[String],
) -> Result<WebBrowserPolicy, String> {
    match config {
        Some(config) => {
            if config.mode != "policy" {
                return Err("session webbrowser mode must be policy".to_string());
            }
            Ok(WebBrowserPolicy {
                allow_domains: validate_domain_list(
                    config.allow_domains,
                    "webbrowser",
                    "allow_domains",
                )?,
                deny_domains: validate_domain_list(
                    config.deny_domains,
                    "webbrowser",
                    "deny_domains",
                )?,
            })
        }
        None => Ok(WebBrowserPolicy {
            allow_domains: http_allow_domains.to_vec(),
            deny_domains: http_deny_domains.to_vec(),
        }),
    }
}

#[cfg(feature = "webbrowser")]
impl WebBrowserPolicy {
    fn check_url(&self, url: &str) -> Result<(), String> {
        let parsed =
            Url::parse(url).map_err(|error| format!("webbrowser: invalid URL: {error}"))?;
        match parsed.scheme() {
            "http" | "https" => {}
            scheme => {
                return Err(format!(
                    "webbrowser: URL scheme '{scheme}' is not allowed by policy"
                ))
            }
        }
        let domain = parsed
            .host_str()
            .ok_or_else(|| "webbrowser: URL must include a host".to_string())?
            .to_ascii_lowercase();
        if self.is_allowed(&domain) {
            Ok(())
        } else {
            Err(format!("http: access to '{domain}' was denied"))
        }
    }

    fn is_allowed(&self, domain: &str) -> bool {
        !matches_any_policy_domain(domain, &self.deny_domains)
            && matches_any_policy_domain(domain, &self.allow_domains)
    }
}

#[cfg(feature = "webbrowser")]
fn matches_any_policy_domain(domain: &str, entries: &[String]) -> bool {
    entries
        .iter()
        .any(|entry| entry == "*" || domain == entry || domain.ends_with(&format!(".{entry}")))
}

#[cfg(feature = "pdfium-render")]
fn discover_pdfium_engine() -> Option<PdfiumEngine> {
    if std::env::var_os(CPSL_REQUIRE_STAGED_PDFIUM_ENV).is_some() {
        return std::env::var_os(CPSL_LIBRARY_DIR_ENV)
            .and_then(|path| discover_pdfium_from_base(&PathBuf::from(path)));
    }

    if std::env::var_os("PDFIUM_DYNAMIC_LIB_PATH").is_some() {
        if let Ok(engine) = PdfiumEngine::discover(None) {
            return Some(engine);
        }
    }

    for base in pdfium_base_dirs() {
        if let Some(engine) = discover_pdfium_from_base(&base) {
            return Some(engine);
        }
    }

    PdfiumEngine::discover(None).ok()
}

#[cfg(feature = "pdfium-render")]
fn pdfium_base_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(path) = std::env::var_os(CPSL_LIBRARY_DIR_ENV) {
        push_unique_path(&mut dirs, PathBuf::from(path));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            push_unique_path(&mut dirs, parent.to_path_buf());
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        push_unique_path(&mut dirs, cwd);
    }
    dirs
}

#[cfg(feature = "pdfium-render")]
fn push_unique_path(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if !paths.iter().any(|existing| existing == &path) {
        paths.push(path);
    }
}

#[cfg(feature = "pdfium-render")]
fn discover_pdfium_from_base(base: &Path) -> Option<PdfiumEngine> {
    for candidate in [
        base.join("libs").join("pdfium").join("lib"),
        base.to_path_buf(),
        base.join("Frameworks"),
    ] {
        if candidate.exists() {
            if let Ok(engine) = PdfiumEngine::from_path(&candidate) {
                return Some(engine);
            }
        }
    }
    None
}

fn eval_bash(session: &Session, request: &EvalRequest) -> serde_json::Value {
    let _timeout_ms = request.timeout_ms;
    let transpiled = match cpsl_core::sh_transpile::transpile_sh(&request.input) {
        Ok(transpiled) => transpiled,
        Err(error) => {
            return eval_error_json("invalid_request", &error, &shell_cwd(session));
        }
    };

    match session.sandbox.exec_stdout(&transpiled.luau_source) {
        Ok(stdout) => eval_success_json(
            stdout,
            shell_exit_code(session),
            transpiled.warnings,
            &shell_cwd(session),
        ),
        Err(error) => match shell_exit_code_from_error(&error.message) {
            Some(exit_code) => eval_success_json(
                String::new(),
                exit_code,
                transpiled.warnings,
                &shell_cwd(session),
            ),
            None => eval_exec_error_json(&error.message, &shell_cwd(session)),
        },
    }
}

fn eval_luau(session: &Session, request: &EvalRequest) -> serde_json::Value {
    let _timeout_ms = request.timeout_ms;
    match session.sandbox.exec_stdout(&request.input) {
        Ok(stdout) => eval_success_json(stdout, 0, Vec::new(), &shell_cwd(session)),
        Err(error) => eval_exec_error_json(&error.message, &shell_cwd(session)),
    }
}

fn eval_exec_error_json(message: &str, cwd: &str) -> serde_json::Value {
    if is_network_policy_denial(message) {
        eval_error_json("sandbox_denied", "Network access is denied by policy", cwd)
    } else {
        eval_error_json("runtime_error", message, cwd)
    }
}

fn is_network_policy_denial(message: &str) -> bool {
    message.starts_with("http: access to '") && message.ends_with("' was denied")
}

fn eval_success_json(
    stdout: String,
    exit_code: i64,
    warnings: Vec<String>,
    cwd: &str,
) -> serde_json::Value {
    json!({
        "ok": true,
        "stdout": stdout,
        "stderr": "",
        "exit_code": exit_code,
        "error": null,
        "warnings": warnings,
        "cwd": cwd
    })
}

fn shell_exit_code(session: &Session) -> i64 {
    session
        .sandbox
        .exec("local sh = require(\"shrt\"); return sh.last_exit_code")
        .ok()
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(1)
}

fn shell_cwd(session: &Session) -> String {
    session
        .sandbox
        .exec("local sh = require(\"shrt\"); return sh.cwd")
        .unwrap_or_else(|_| WORKDIR.to_string())
}

fn shell_exit_code_from_error(message: &str) -> Option<i64> {
    message
        .strip_prefix("exit:")
        .and_then(|code| code.parse::<i64>().ok())
}

fn validate_host_path(host: &str) -> Result<PathBuf, String> {
    let host_path = Path::new(host);
    if !host_path.is_absolute() {
        return Err("session mount host path must be absolute".to_string());
    }

    let canonical = host_path
        .canonicalize()
        .map_err(|error| format!("session mount host path must be canonical: {error}"))?;
    if !canonical.is_dir() {
        return Err("session mount host path must be a directory".to_string());
    }

    Ok(canonical)
}

fn validate_absolute_virtual_path(path: &str, field: &str) -> Result<(), String> {
    if !path.starts_with('/') {
        return Err(format!("{field} must be absolute"));
    }
    for component in Path::new(path).components() {
        match component {
            Component::RootDir | Component::Normal(_) => {}
            Component::CurDir | Component::ParentDir | Component::Prefix(_) => {
                return Err(format!("{field} must not contain relative components"));
            }
        }
    }
    Ok(())
}

fn validate_initial_cwd(path: &str, mounts: &MountTable) -> Result<(), String> {
    validate_absolute_virtual_path(path, "session initial_cwd")?;
    if mounts.mount_key_for(path).is_none() {
        return Err("session initial_cwd must be covered by a mount".to_string());
    }
    Ok(())
}

fn validate_domain_list(
    domains: Vec<String>,
    policy_name: &str,
    field: &str,
) -> Result<Vec<String>, String> {
    for domain in &domains {
        validate_policy_domain(domain).map_err(|error| {
            format!("session {policy_name} {field} entry {domain:?} is invalid: {error}")
        })?;
    }
    Ok(domains)
}

fn validate_policy_domain(domain: &str) -> Result<(), &'static str> {
    if domain.is_empty() {
        return Err("domain must not be empty");
    }
    if domain == "*" {
        return Ok(());
    }
    if domain.len() > 253 {
        return Err("domain is too long");
    }
    if domain != domain.to_ascii_lowercase() {
        return Err("domain must be lowercase");
    }
    if domain.contains('*') {
        return Err("wildcards are only supported as \"*\"");
    }
    if domain.contains("://") || domain.contains('/') || domain.contains(':') {
        return Err("domain must not include a scheme, port, or path");
    }
    if domain.starts_with('.') || domain.ends_with('.') || domain.contains("..") {
        return Err("domain labels must be non-empty");
    }
    if domain.parse::<IpAddr>().is_ok() {
        return Err("IP addresses are not supported");
    }

    for label in domain.split('.') {
        if label.is_empty() || label.len() > 63 {
            return Err("domain labels must be between 1 and 63 bytes");
        }
        if label.starts_with('-') || label.ends_with('-') {
            return Err("domain labels must not start or end with '-'");
        }
        if !label
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        {
            return Err("domain labels must contain only lowercase letters, digits, and '-'");
        }
    }

    Ok(())
}

fn escape_luau_string(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

fn eval_error_json(code: &str, message: &str, cwd: &str) -> serde_json::Value {
    json!({
        "ok": false,
        "stdout": "",
        "stderr": "",
        "exit_code": null,
        "error": {
            "code": code,
            "message": message
        },
        "warnings": [],
        "cwd": cwd
    })
}

fn c_str_arg(value: *const c_char, name: &str) -> Result<String, String> {
    if value.is_null() {
        return Err(format!("{name} must not be NULL"));
    }

    unsafe { CStr::from_ptr(value) }
        .to_str()
        .map(str::to_owned)
        .map_err(|_| format!("{name} must be UTF-8"))
}

fn owned_c_string(value: String) -> Result<*mut c_char, String> {
    CString::new(value)
        .map(CString::into_raw)
        .map_err(|_| "FFI string contained an embedded NUL byte".to_string())
}

fn set_last_error(message: &str) {
    let sanitized = message.replace('\0', "\\0");
    with_last_error(|error| {
        *error = CString::new(sanitized).unwrap_or_else(|_| empty_c_string());
    });
}

fn clear_last_error() {
    with_last_error(|error| {
        *error = empty_c_string();
    });
}

fn with_last_error<R>(f: impl FnOnce(&mut CString) -> R) -> R {
    let mutex = LAST_ERROR.get_or_init(|| Mutex::new(empty_c_string()));
    let mut guard = match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    f(&mut guard)
}

fn empty_c_string() -> CString {
    CString::new("").unwrap_or_else(|_| unreachable!("empty CString is always valid"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::{c_char, CStr, CString};
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    #[test]
    fn validates_minimal_session_config() {
        let dir = TempDir::new().unwrap();
        let config = session_config(dir.path().canonicalize().unwrap().to_str().unwrap());

        let validated = validate_session_config(&config.to_string()).unwrap();

        assert_eq!(validated.initial_cwd, WORKDIR);
        assert_eq!(validated.mounts.len(), 1);
        assert_eq!(validated.mounts[0].host, dir.path().canonicalize().unwrap());
        assert_eq!(validated.mounts[0].virtual_path, WORKDIR);
        assert_eq!(validated.mounts[0].permission, MountPermission::ReadWrite);
        assert_eq!(validated.allow_domains, Vec::<String>::new());
        assert_eq!(validated.deny_domains, Vec::<String>::new());
        #[cfg(feature = "webbrowser")]
        {
            assert_eq!(
                validated.webbrowser_policy.allow_domains,
                Vec::<String>::new()
            );
            assert_eq!(
                validated.webbrowser_policy.deny_domains,
                Vec::<String>::new()
            );
        }
    }

    #[test]
    fn validates_composed_root_and_workdir_mounts() {
        let root = TempDir::new().unwrap();
        let workdir = root.path().join("workdir");
        fs::create_dir(&workdir).unwrap();
        let config = json!({
            "mounts": [
                {
                    "host": root.path().canonicalize().unwrap(),
                    "virtual": "/",
                    "mode": "rw"
                },
                {
                    "host": workdir.canonicalize().unwrap(),
                    "virtual": WORKDIR,
                    "mode": "rw"
                }
            ],
            "initial_cwd": WORKDIR,
            "language": "bash",
            "http": {
                "mode": "policy",
                "allow_domains": [],
                "deny_domains": []
            }
        });

        let validated = validate_session_config(&config.to_string()).unwrap();

        assert_eq!(validated.mounts.len(), 2);
        assert_eq!(validated.mounts[0].virtual_path, "/");
        assert_eq!(validated.mounts[1].virtual_path, WORKDIR);
        assert_eq!(validated.initial_cwd, WORKDIR);
    }

    #[test]
    fn composed_root_mount_is_visible_to_bash() {
        let root = TempDir::new().unwrap();
        let workdir = root.path().join("workdir");
        fs::create_dir(&workdir).unwrap();
        fs::write(root.path().join("top.txt"), "root file\n").unwrap();
        fs::write(workdir.join("notes.txt"), "workdir file\n").unwrap();
        let config = json!({
            "mounts": [
                {
                    "host": root.path().canonicalize().unwrap(),
                    "virtual": "/",
                    "mode": "rw"
                },
                {
                    "host": workdir.canonicalize().unwrap(),
                    "virtual": WORKDIR,
                    "mode": "rw"
                }
            ],
            "initial_cwd": WORKDIR,
            "language": "bash",
            "http": {
                "mode": "policy",
                "allow_domains": [],
                "deny_domains": []
            }
        });
        let config = CString::new(config.to_string()).unwrap();
        let session = cpsl_session_new(config.as_ptr());
        assert!(!session.is_null(), "session_new failed: {}", unsafe {
            borrowed_ffi_string(cpsl_last_error())
        });

        let root_ls = eval(session, "ls /");
        assert_success(&root_ls, 0);
        assert!(root_ls["stdout"].as_str().unwrap().contains("workdir"));
        assert!(root_ls["stdout"].as_str().unwrap().contains("top.txt"));

        let workdir_ls = eval(session, "ls /workdir");
        assert_success(&workdir_ls, 0);
        assert!(workdir_ls["stdout"].as_str().unwrap().contains("notes.txt"));

        let cd_root = eval(session, "cd /");
        assert_eq!(cd_root["ok"], true);
        assert_eq!(cd_root["exit_code"], 0);
        assert!(cd_root["error"].is_null(), "{cd_root}");
        assert_eq!(cd_root["cwd"], "/");

        cpsl_session_free(session);
    }

    #[test]
    fn metadata_advertises_native_luau_and_bash() {
        let metadata = unsafe { owned_ffi_string(cpsl_backend_metadata_json()) };
        let metadata: serde_json::Value = serde_json::from_str(&metadata).unwrap();
        let languages = metadata["languages"].as_array().unwrap();

        assert!(languages.iter().any(|language| language == LANGUAGE_LUAU));
        assert!(languages.iter().any(|language| language == LANGUAGE_BASH));
    }

    #[test]
    fn metadata_advertises_webbrowser_callback_feature_flag() {
        let metadata = unsafe { owned_ffi_string(cpsl_backend_metadata_json()) };
        let metadata: serde_json::Value = serde_json::from_str(&metadata).unwrap();

        assert_eq!(
            metadata["capabilities"]["webbrowser_callbacks"],
            cfg!(feature = "webbrowser")
        );
    }

    #[cfg(feature = "webbrowser")]
    #[test]
    fn webbrowser_callbacks_enable_luau_module() {
        let dir = TempDir::new().unwrap();
        let session = new_session_with_webbrowser_callbacks(dir.path());

        let response = eval_luau(
            session,
            r#"
                local result = webbrowser.open("https://example.com")
                print(result.browser)
                print(result.page.title)
            "#,
        );

        assert_success(&response, 0);
        assert_eq!(response["stdout"], "test-browser\nExample Domain\n");

        cpsl_session_free(session);
    }

    #[cfg(feature = "webbrowser")]
    #[test]
    fn webbrowser_callbacks_follow_default_network_policy() {
        let dir = TempDir::new().unwrap();
        let session = new_session_with_webbrowser_callbacks_and_policy(dir.path(), None);

        let response = eval_luau(
            session,
            r#"return webbrowser.open("https://example.com").browser"#,
        );

        assert_eval_error(&response, "sandbox_denied");
        assert_eq!(
            response["error"]["message"],
            "Network access is denied by policy"
        );

        cpsl_session_free(session);
    }

    #[cfg(feature = "pdfium-render")]
    #[test]
    fn runtime_sandbox_creation_does_not_require_pdfium_library() {
        let dir = TempDir::new().unwrap();
        let config = session_config(dir.path().canonicalize().unwrap().to_str().unwrap());
        let validated = validate_session_config(&config.to_string()).unwrap();

        let sandbox = create_runtime_sandbox(
            &validated,
            #[cfg(feature = "webbrowser")]
            None,
        )
        .unwrap();

        assert_eq!(
            sandbox.exec("return type(doc.pdfInfo)").unwrap(),
            "function"
        );
    }

    #[test]
    fn validates_network_policy_domains() {
        let dir = TempDir::new().unwrap();
        let mut config = session_config(dir.path().canonicalize().unwrap().to_str().unwrap());
        config["http"]["allow_domains"] = json!(["example.com", "api.example.com", "*"]);
        config["http"]["deny_domains"] = json!(["blocked.example.com", "blocked.example.com"]);

        let validated = validate_session_config(&config.to_string()).unwrap();

        assert_eq!(
            validated.allow_domains,
            ["example.com", "api.example.com", "*"]
        );
        assert_eq!(
            validated.deny_domains,
            ["blocked.example.com", "blocked.example.com"]
        );
    }

    #[cfg(feature = "webbrowser")]
    #[test]
    fn validates_webbrowser_policy_domains() {
        let dir = TempDir::new().unwrap();
        let mut config = session_config(dir.path().canonicalize().unwrap().to_str().unwrap());
        config["webbrowser"] = json!({
            "mode": "policy",
            "allow_domains": ["*"],
            "deny_domains": ["blocked.example.com"]
        });

        let validated = validate_session_config(&config.to_string()).unwrap();

        assert_eq!(validated.webbrowser_policy.allow_domains, ["*"]);
        assert_eq!(
            validated.webbrowser_policy.deny_domains,
            ["blocked.example.com"]
        );
    }

    #[test]
    fn rejects_invalid_session_configs() {
        assert!(validate_session_config("not-json").is_err());

        let dir = TempDir::new().unwrap();
        let host = dir.path().canonicalize().unwrap();
        let host = host.to_str().unwrap();

        let mut wrong_mode = session_config(host);
        wrong_mode["mounts"][0]["mode"] = json!("bad");
        assert!(validate_session_config(&wrong_mode.to_string()).is_err());

        let mut wrong_language = session_config(host);
        wrong_language["language"] = json!("python");
        assert!(validate_session_config(&wrong_language.to_string()).is_err());

        let mut luau_language = session_config(host);
        luau_language["language"] = json!(LANGUAGE_LUAU);
        assert!(validate_session_config(&luau_language.to_string()).is_ok());

        let mut outside_cwd = session_config(host);
        outside_cwd["initial_cwd"] = json!("/tmp");
        assert!(validate_session_config(&outside_cwd.to_string()).is_err());

        let mut relative_host = session_config(host);
        relative_host["mounts"][0]["host"] = json!("relative");
        assert!(validate_session_config(&relative_host.to_string()).is_err());
    }

    #[test]
    fn rejects_invalid_network_policy_domains() {
        let dir = TempDir::new().unwrap();
        let host = dir.path().canonicalize().unwrap();
        let host = host.to_str().unwrap();

        for domain in [
            "",
            "Example.com",
            "*.example.com",
            "https://example.com",
            "example.com:443",
            "example.com/path",
            ".example.com",
            "example.com.",
            "bad..example.com",
            "-example.com",
            "example-.com",
            "bad_example.com",
            "127.0.0.1",
        ] {
            let mut config = session_config(host);
            config["http"]["allow_domains"] = json!([domain]);
            assert!(
                validate_session_config(&config.to_string()).is_err(),
                "domain {domain:?} should be rejected"
            );
        }
    }

    #[test]
    fn rejects_http_callback_and_credential_fields() {
        let dir = TempDir::new().unwrap();
        let host = dir.path().canonicalize().unwrap();
        let host = host.to_str().unwrap();
        for field in ["credentials", "callback", "prompt"] {
            let mut config = session_config(host);
            config["http"][field] = json!(true);
            assert!(
                validate_session_config(&config.to_string()).is_err(),
                "http field {field:?} should be rejected"
            );
        }
    }

    #[test]
    fn evaluates_bash_file_and_data_workflows() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("notes.txt"), "alpha\nbeta\n").unwrap();
        fs::write(dir.path().join("data.json"), r#"{"name":"Ada","count":2}"#).unwrap();
        fs::write(dir.path().join("data.csv"), "name,count\nAda,2\n").unwrap();

        let session = new_session(dir.path());

        let pwd = eval(session, "pwd");
        assert_success(&pwd, 0);
        assert_eq!(pwd["stdout"], "/workdir\n");
        assert_eq!(pwd["cwd"], WORKDIR);

        let ls = eval(session, "ls");
        assert_success(&ls, 0);
        assert!(ls["stdout"].as_str().unwrap().contains("notes.txt"));

        let cat = eval(session, "cat notes.txt");
        assert_success(&cat, 0);
        assert_eq!(cat["stdout"], "alpha\nbeta\n");

        let grep = eval(session, "grep beta notes.txt");
        assert_success(&grep, 0);
        assert_eq!(grep["stdout"], "beta\n");

        let redirected = eval(
            session,
            "echo '# Report' > report.md\necho 'total,2' >> report.md",
        );
        assert_success(&redirected, 0);
        assert_eq!(redirected["stdout"], "");
        assert_eq!(
            fs::read_to_string(dir.path().join("report.md")).unwrap(),
            "# Report\ntotal,2\n"
        );

        let json = eval(session, r#"json decode "$(cat data.json)""#);
        assert_success(&json, 0);
        assert!(json["stdout"].as_str().unwrap().contains("Ada"));

        let csv = eval(session, "csv parseFile /workdir/data.csv");
        assert_success(&csv, 0);
        assert!(csv["stdout"].as_str().unwrap().contains("Ada"));

        let markdown = eval(session, "cat report.md | grep Report");
        assert_success(&markdown, 0);
        assert_eq!(markdown["stdout"], "# Report\n");

        cpsl_session_free(session);
    }

    #[test]
    fn evaluates_native_luau_file_and_data_workflows() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("notes.txt"), "alpha\nbeta\n").unwrap();
        fs::write(dir.path().join("data.json"), r#"{"name":"Ada","count":2}"#).unwrap();

        let session = new_session_with_language(dir.path(), LANGUAGE_LUAU);
        let response = eval_luau(
            session,
            r#"
                local text = fs.read("/workdir/notes.txt")
                local data = json.decode(fs.read("/workdir/data.json"))
                fs.write("/workdir/report.txt", string.upper(text) .. data.name)
                print(text)
                print(data.name)
            "#,
        );

        assert_success(&response, 0);
        let stdout = response["stdout"].as_str().unwrap();
        assert!(stdout.contains("alpha\nbeta"), "{stdout}");
        assert!(stdout.contains("Ada"), "{stdout}");
        assert_eq!(
            fs::read_to_string(dir.path().join("report.txt")).unwrap(),
            "ALPHA\nBETA\nAda"
        );

        let bash = eval_bash(session, "cat report.txt");
        assert_success(&bash, 0);
        assert_eq!(bash["stdout"], "ALPHA\nBETA\nAda\n");

        let grep = eval_luau(
            session,
            r#"
                local matches = fs.grep({pattern="beta", path="/workdir/notes.txt", max_count=1})
                print(#matches)
                print(matches[1].line)
            "#,
        );
        assert_success(&grep, 0);
        assert_eq!(grep["stdout"], "1\nbeta\n");

        cpsl_session_free(session);
    }

    #[test]
    fn reports_nonzero_shell_exits_without_ffi_failure() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("notes.txt"), "alpha\nbeta\n").unwrap();
        let session = new_session(dir.path());

        let false_result = eval(session, "false");
        assert_success(&false_result, 1);
        assert_eq!(false_result["stdout"], "");
        assert!(false_result["error"].is_null());

        let grep = eval(session, "grep missing notes.txt");
        assert_success(&grep, 1);
        assert_eq!(grep["stdout"], "");

        let exited = eval(session, "exit 7");
        assert_success(&exited, 7);

        cpsl_session_free(session);
    }

    #[test]
    fn question_mark_expands_to_previous_exit_code() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("notes.txt"), "alpha\nbeta\n").unwrap();
        let session = new_session(dir.path());

        assert_success(&eval(session, "false"), 1);
        let false_code = eval(session, "echo $?");
        assert_success(&false_code, 0);
        assert_eq!(false_code["stdout"], "1\n");

        assert_success(&eval(session, "grep missing notes.txt"), 1);
        let grep_code = eval(session, "echo $?");
        assert_success(&grep_code, 0);
        assert_eq!(grep_code["stdout"], "1\n");

        assert_success(&eval(session, "exit 7"), 7);
        let exit_code = eval(session, "echo $?");
        assert_success(&exit_code, 0);
        assert_eq!(exit_code["stdout"], "7\n");

        cpsl_session_free(session);
    }

    #[test]
    fn unsupported_development_commands_return_shell_feedback() {
        let dir = TempDir::new().unwrap();
        let session = new_session(dir.path());

        let npm = eval(session, "npm install left-pad");
        assert_success(&npm, 1);
        assert!(npm["stdout"]
            .as_str()
            .unwrap()
            .contains("no package management in sandbox"));

        let git = eval(session, "git status");
        assert_success(&git, 1);
        assert!(git["stdout"]
            .as_str()
            .unwrap()
            .contains("not available in CPSL sandbox"));

        cpsl_session_free(session);
    }

    #[test]
    fn http_module_is_loaded_for_policy_mode() {
        let dir = TempDir::new().unwrap();
        let session = new_session(dir.path());

        let response = eval(session, "http help");

        assert_success(&response, 0);
        assert!(response["stdout"]
            .as_str()
            .unwrap()
            .contains("HTTP requests"));

        cpsl_session_free(session);
    }

    #[test]
    fn network_access_is_denied_by_default() {
        let dir = TempDir::new().unwrap();
        let session = new_session(dir.path());

        let response = eval(session, "http get https://example.com/");

        assert_eval_error(&response, "sandbox_denied");
        assert_eq!(
            response["error"]["message"],
            "Network access is denied by policy"
        );

        cpsl_session_free(session);
    }

    #[test]
    fn native_luau_network_denial_is_structured() {
        let dir = TempDir::new().unwrap();
        let session = new_session_with_language(dir.path(), LANGUAGE_LUAU);

        let response = eval_luau(session, r#"return http.get("https://example.com/")"#);

        assert_eval_error(&response, "sandbox_denied");
        assert_eq!(
            response["error"]["message"],
            "Network access is denied by policy"
        );

        cpsl_session_free(session);
    }

    #[test]
    fn network_deny_wins_over_allow() {
        let dir = TempDir::new().unwrap();
        let session = new_session_with_policy(dir.path(), &["example.com"], &["api.example.com"]);
        let response = eval(session, "http get https://api.example.com/");
        assert_eval_error(&response, "sandbox_denied");
        cpsl_session_free(session);

        let session = new_session_with_policy(dir.path(), &["api.example.com"], &["example.com"]);
        let response = eval(session, "http get https://api.example.com/");
        assert_eval_error(&response, "sandbox_denied");
        cpsl_session_free(session);
    }

    #[test]
    fn mounted_workdir_cannot_be_escaped() {
        let dir = TempDir::new().unwrap();
        let session = new_session(dir.path());

        for command in ["cat ../secret", "cat /workdir/../secret"] {
            let response = eval(session, command);
            assert_success(&response, 1);
            let stdout = response["stdout"].as_str().unwrap();
            assert!(
                stdout.contains("No such file or directory")
                    || stdout.contains("Path traversal denied"),
                "command {command:?} returned {response}"
            );
            assert_eq!(response["cwd"], WORKDIR);
        }

        let etc_hostname = eval(session, "cat /etc/hostname");
        assert_success(&etc_hostname, 1);
        assert!(etc_hostname["stdout"]
            .as_str()
            .unwrap()
            .contains("Path traversal denied"));

        let cd_root = eval(session, "cd /");
        assert_success(&cd_root, 1);
        assert!(cd_root["stdout"]
            .as_str()
            .unwrap()
            .contains("Path traversal denied"));
        assert_eq!(cd_root["cwd"], WORKDIR);

        cpsl_session_free(session);
    }

    #[cfg(unix)]
    #[test]
    fn mounted_workdir_symlinks_cannot_escape() {
        let dir = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        fs::write(outside.path().join("secret.txt"), "outside secret").unwrap();
        std::os::unix::fs::symlink(
            outside.path().join("secret.txt"),
            dir.path().join("link.txt"),
        )
        .unwrap();

        let session = new_session(dir.path());
        let response = eval(session, "cat link.txt");

        assert_success(&response, 1);
        assert!(!response["stdout"]
            .as_str()
            .unwrap()
            .contains("outside secret"));
        assert!(response["stdout"]
            .as_str()
            .unwrap()
            .contains("Path traversal denied"));

        cpsl_session_free(session);
    }

    #[test]
    fn unsupported_language_returns_structured_eval_error() {
        let dir = TempDir::new().unwrap();
        let session = new_session(dir.path());
        let request = CString::new(
            json!({
                "language": "python",
                "input": "print('hi')",
                "timeout_ms": 120000
            })
            .to_string(),
        )
        .unwrap();

        let response = unsafe { owned_ffi_string(cpsl_eval(session, request.as_ptr())) };
        let response: serde_json::Value = serde_json::from_str(&response).unwrap();

        assert_eq!(response["ok"], false);
        assert_eq!(response["error"]["code"], "unsupported_language");
        assert_eq!(response["cwd"], WORKDIR);

        cpsl_session_free(session);
    }

    fn session_config(host: &str) -> serde_json::Value {
        json!({
            "mounts": [
                {"host": host, "virtual": WORKDIR, "mode": "rw"}
            ],
            "initial_cwd": WORKDIR,
            "language": "bash",
            "http": {
                "mode": "policy",
                "allow_domains": [],
                "deny_domains": []
            }
        })
    }

    fn new_session(host: &Path) -> *mut cpsl_session_t {
        new_session_with_language(host, LANGUAGE_BASH)
    }

    fn new_session_with_language(host: &Path, language: &str) -> *mut cpsl_session_t {
        new_session_with_policy_and_language(host, language, &[], &[])
    }

    fn new_session_with_policy(
        host: &Path,
        allow_domains: &[&str],
        deny_domains: &[&str],
    ) -> *mut cpsl_session_t {
        new_session_with_policy_and_language(host, LANGUAGE_BASH, allow_domains, deny_domains)
    }

    fn new_session_with_policy_and_language(
        host: &Path,
        language: &str,
        allow_domains: &[&str],
        deny_domains: &[&str],
    ) -> *mut cpsl_session_t {
        let host = host.canonicalize().unwrap();
        let mut config = session_config(host.to_str().unwrap());
        config["language"] = json!(language);
        config["http"]["allow_domains"] = json!(allow_domains);
        config["http"]["deny_domains"] = json!(deny_domains);
        let config = CString::new(config.to_string()).unwrap();
        let session = cpsl_session_new(config.as_ptr());
        assert!(!session.is_null(), "session_new failed: {}", unsafe {
            borrowed_ffi_string(cpsl_last_error())
        });
        session
    }

    #[cfg(feature = "webbrowser")]
    fn new_session_with_webbrowser_callbacks(host: &Path) -> *mut cpsl_session_t {
        new_session_with_webbrowser_callbacks_and_policy(
            host,
            Some(json!({
                "mode": "policy",
                "allow_domains": ["example.com"],
                "deny_domains": []
            })),
        )
    }

    #[cfg(feature = "webbrowser")]
    fn new_session_with_webbrowser_callbacks_and_policy(
        host: &Path,
        webbrowser_policy: Option<serde_json::Value>,
    ) -> *mut cpsl_session_t {
        let host = host.canonicalize().unwrap();
        let mut config = session_config(host.to_str().unwrap());
        config["language"] = json!(LANGUAGE_LUAU);
        if let Some(webbrowser_policy) = webbrowser_policy {
            config["webbrowser"] = webbrowser_policy;
        }
        let config = CString::new(config.to_string()).unwrap();
        let callbacks = cpsl_webbrowser_callbacks_t {
            user_data: ptr::null_mut(),
            handle_json: Some(test_webbrowser_handle_json),
            string_free: Some(test_webbrowser_string_free),
            user_data_free: None,
        };
        let session = cpsl_session_new_with_webbrowser_callbacks(config.as_ptr(), &callbacks);
        assert!(
            !session.is_null(),
            "session_new_with_webbrowser_callbacks failed: {}",
            unsafe { borrowed_ffi_string(cpsl_last_error()) }
        );
        session
    }

    #[cfg(feature = "webbrowser")]
    unsafe extern "C" fn test_webbrowser_handle_json(
        _user_data: *mut std::ffi::c_void,
        request_json: *const c_char,
    ) -> *mut c_char {
        let request = CStr::from_ptr(request_json).to_str().unwrap();
        let request: serde_json::Value = serde_json::from_str(request).unwrap();
        assert_eq!(request["command"], "open");
        assert_eq!(request["resourceMode"], "lean");
        assert_eq!(request["networkPolicy"]["allowDomains"][0], "example.com");
        CString::new(
            json!({
                "ok": true,
                "browser": "test-browser",
                "resourceMode": "lean",
                "url": request["url"],
                "page": {
                    "browser": "test-browser",
                    "title": "Example Domain",
                    "url": request["url"],
                    "text": "example text",
                    "actions": []
                }
            })
            .to_string(),
        )
        .unwrap()
        .into_raw()
    }

    #[cfg(feature = "webbrowser")]
    unsafe extern "C" fn test_webbrowser_string_free(value: *mut c_char) {
        if !value.is_null() {
            drop(CString::from_raw(value));
        }
    }

    fn eval(session: *mut cpsl_session_t, input: &str) -> serde_json::Value {
        eval_bash(session, input)
    }

    fn eval_bash(session: *mut cpsl_session_t, input: &str) -> serde_json::Value {
        eval_lang(session, LANGUAGE_BASH, input)
    }

    fn eval_luau(session: *mut cpsl_session_t, input: &str) -> serde_json::Value {
        eval_lang(session, LANGUAGE_LUAU, input)
    }

    fn eval_lang(session: *mut cpsl_session_t, language: &str, input: &str) -> serde_json::Value {
        let request = CString::new(
            json!({
                "language": language,
                "input": input,
                "timeout_ms": 120000
            })
            .to_string(),
        )
        .unwrap();
        let response = cpsl_eval(session, request.as_ptr());
        assert!(!response.is_null(), "cpsl_eval failed: {}", unsafe {
            borrowed_ffi_string(cpsl_last_error())
        });
        let response = unsafe { owned_ffi_string(response) };
        serde_json::from_str(&response).unwrap()
    }

    fn assert_eval_error(response: &serde_json::Value, code: &str) {
        assert_eq!(response["ok"], false, "{response}");
        assert_eq!(response["stderr"], "");
        assert!(response["exit_code"].is_null(), "{response}");
        assert_eq!(response["error"]["code"], code, "{response}");
        assert!(response["warnings"].as_array().is_some(), "{response}");
        assert_eq!(response["cwd"], WORKDIR);
    }

    fn assert_success(response: &serde_json::Value, exit_code: i64) {
        assert_eq!(response["ok"], true, "{response}");
        assert_eq!(response["stderr"], "");
        assert_eq!(response["exit_code"], exit_code);
        assert!(response["error"].is_null(), "{response}");
        assert!(response["warnings"].as_array().is_some(), "{response}");
        assert_eq!(response["cwd"], WORKDIR);
    }

    unsafe fn borrowed_ffi_string(value: *const c_char) -> String {
        assert!(!value.is_null());
        CStr::from_ptr(value).to_str().unwrap().to_owned()
    }

    unsafe fn owned_ffi_string(value: *mut c_char) -> String {
        assert!(!value.is_null());
        let text = CStr::from_ptr(value).to_str().unwrap().to_owned();
        cpsl_string_free(value);
        text
    }
}
