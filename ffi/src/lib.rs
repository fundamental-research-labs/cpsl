//! C ABI for loading CPSL as a sandbox library.

#[cfg(feature = "pdfium-render")]
use cpsl_core::PdfiumEngine;
use cpsl_core::{HttpGateway, MountPermission, MountTable, Sandbox};
use serde::Deserialize;
use serde_json::json;
use std::ffi::{c_char, CStr, CString};
use std::net::IpAddr;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Component, Path, PathBuf};
use std::ptr;
use std::sync::{Arc, Mutex, OnceLock};

const CPSL_ABI_VERSION: u32 = 1;
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

struct Session {
    _config: ValidatedSessionConfig,
    sandbox: Sandbox,
}

struct ValidatedSessionConfig {
    host: PathBuf,
    initial_cwd: String,
    allow_domains: Vec<String>,
    deny_domains: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SessionConfig {
    mounts: Vec<MountConfig>,
    initial_cwd: String,
    language: String,
    http: HttpConfig,
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
                "network_policy": true
            }
        });
        owned_c_string(metadata.to_string())
    })
}

#[no_mangle]
pub extern "C" fn cpsl_session_new(config_json: *const c_char) -> *mut cpsl_session_t {
    ffi_result(|| {
        let config_json = c_str_arg(config_json, "config_json")?;
        let config = validate_session_config(&config_json)?;
        let sandbox = create_runtime_sandbox(&config)?;
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

fn validate_session_config(config_json: &str) -> Result<ValidatedSessionConfig, String> {
    let config: SessionConfig = serde_json::from_str(config_json)
        .map_err(|error| format!("invalid session config: {error}"))?;

    if config.language != LANGUAGE_LUAU && config.language != LANGUAGE_BASH {
        return Err("session language must be luau or bash".to_string());
    }

    if config.mounts.len() != 1 {
        return Err("session config must contain exactly one mount".to_string());
    }

    if config.http.mode != "policy" {
        return Err("session http mode must be policy".to_string());
    }
    let allow_domains = validate_domain_list(config.http.allow_domains, "allow_domains")?;
    let deny_domains = validate_domain_list(config.http.deny_domains, "deny_domains")?;

    validate_initial_cwd(&config.initial_cwd)?;
    let mount = &config.mounts[0];
    let host = validate_host_path(&mount.host)?;

    if mount.virtual_path != WORKDIR {
        return Err("session mount virtual path must be /workdir".to_string());
    }

    if mount.mode != "rw" {
        return Err("session mount mode must be rw".to_string());
    }

    Ok(ValidatedSessionConfig {
        host,
        initial_cwd: config.initial_cwd,
        allow_domains,
        deny_domains,
    })
}

fn create_runtime_sandbox(config: &ValidatedSessionConfig) -> Result<Sandbox, String> {
    let mut mounts = MountTable::new();
    mounts
        .add_mount(config.host.clone(), WORKDIR, MountPermission::ReadWrite)
        .map_err(|error| format!("failed to configure workdir mount: {error}"))?;

    let builder = Sandbox::builder()
        .mounts(mounts)
        .http_gateway(Arc::new(create_http_gateway(config)))
        .auto_tmp(false);
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
            escape_luau_string(WORKDIR),
            escape_luau_string(&config.initial_cwd)
        ))
        .map_err(|error| format!("failed to initialize CPSL shell: {error}"))?;
    Ok(sandbox)
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
    let lib_dir = base.join("libs").join("pdfium").join("lib");
    if lib_dir.is_dir() {
        PdfiumEngine::from_path(lib_dir).ok()
    } else {
        None
    }
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

fn validate_initial_cwd(path: &str) -> Result<(), String> {
    if !path.starts_with('/') {
        return Err("session initial_cwd must be absolute".to_string());
    }

    for component in Path::new(path).components() {
        match component {
            Component::RootDir | Component::Normal(_) => {}
            Component::CurDir | Component::ParentDir | Component::Prefix(_) => {
                return Err("session initial_cwd must stay under /workdir".to_string());
            }
        }
    }

    if path == WORKDIR || path.starts_with("/workdir/") {
        Ok(())
    } else {
        Err("session initial_cwd must stay under /workdir".to_string())
    }
}

fn validate_domain_list(domains: Vec<String>, field: &str) -> Result<Vec<String>, String> {
    for domain in &domains {
        validate_policy_domain(domain).map_err(|error| {
            format!("session http {field} entry {domain:?} is invalid: {error}")
        })?;
    }
    Ok(domains)
}

fn validate_policy_domain(domain: &str) -> Result<(), &'static str> {
    if domain.is_empty() {
        return Err("domain must not be empty");
    }
    if domain.len() > 253 {
        return Err("domain is too long");
    }
    if domain != domain.to_ascii_lowercase() {
        return Err("domain must be lowercase");
    }
    if domain.contains('*') {
        return Err("wildcards are not supported");
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
        assert_eq!(validated.host, dir.path().canonicalize().unwrap());
        assert_eq!(validated.allow_domains, Vec::<String>::new());
        assert_eq!(validated.deny_domains, Vec::<String>::new());
    }

    #[test]
    fn metadata_advertises_native_luau_and_bash() {
        let metadata = unsafe { owned_ffi_string(cpsl_backend_metadata_json()) };
        let metadata: serde_json::Value = serde_json::from_str(&metadata).unwrap();
        let languages = metadata["languages"].as_array().unwrap();

        assert!(languages.iter().any(|language| language == LANGUAGE_LUAU));
        assert!(languages.iter().any(|language| language == LANGUAGE_BASH));
    }

    #[cfg(feature = "pdfium-render")]
    #[test]
    fn runtime_sandbox_creation_does_not_require_pdfium_library() {
        let dir = TempDir::new().unwrap();
        let config = session_config(dir.path().canonicalize().unwrap().to_str().unwrap());
        let validated = validate_session_config(&config.to_string()).unwrap();

        let sandbox = create_runtime_sandbox(&validated).unwrap();

        assert_eq!(
            sandbox.exec("return type(doc.pdfInfo)").unwrap(),
            "function"
        );
    }

    #[test]
    fn validates_network_policy_domains() {
        let dir = TempDir::new().unwrap();
        let mut config = session_config(dir.path().canonicalize().unwrap().to_str().unwrap());
        config["http"]["allow_domains"] = json!(["example.com", "api.example.com"]);
        config["http"]["deny_domains"] = json!(["blocked.example.com", "blocked.example.com"]);

        let validated = validate_session_config(&config.to_string()).unwrap();

        assert_eq!(validated.allow_domains, ["example.com", "api.example.com"]);
        assert_eq!(
            validated.deny_domains,
            ["blocked.example.com", "blocked.example.com"]
        );
    }

    #[test]
    fn rejects_invalid_session_configs() {
        assert!(validate_session_config("not-json").is_err());

        let dir = TempDir::new().unwrap();
        let host = dir.path().canonicalize().unwrap();
        let host = host.to_str().unwrap();

        let mut wrong_mode = session_config(host);
        wrong_mode["mounts"][0]["mode"] = json!("ro");
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

        for command in [
            "cat ../secret",
            "cat /workdir/../secret",
            "cat /etc/hostname",
        ] {
            let response = eval(session, command);
            assert_success(&response, 1);
            assert!(
                response["stdout"]
                    .as_str()
                    .unwrap()
                    .contains("Path traversal denied"),
                "command {command:?} returned {response}"
            );
            assert_eq!(response["cwd"], WORKDIR);
        }

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
