//! Phase 2 C ABI skeleton for loading CPSL as a sandbox library.

use serde::Deserialize;
use serde_json::json;
use std::ffi::{c_char, CStr, CString};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Component, Path, PathBuf};
use std::ptr;
use std::sync::{Mutex, OnceLock};

const CPSL_ABI_VERSION: u32 = 1;
const WORKDIR: &str = "/workdir";

static LAST_ERROR: OnceLock<Mutex<CString>> = OnceLock::new();

#[allow(non_camel_case_types)]
#[repr(C)]
pub struct cpsl_session_t {
    _private: [u8; 0],
}

struct Session {
    _config: ValidatedSessionConfig,
    cwd: String,
}

struct ValidatedSessionConfig {
    _host: PathBuf,
    initial_cwd: String,
    _allow_domains: Vec<String>,
    _deny_domains: Vec<String>,
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
            "languages": ["bash"],
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
        let session = Box::new(Session {
            cwd: config.initial_cwd.clone(),
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

        let response = if request.language == "bash" {
            let message = format!(
                "bash eval is not implemented in the CPSL FFI skeleton ({} byte input, {} ms timeout)",
                request.input.len(),
                request.timeout_ms
            );
            eval_error_json("runtime_error", &message, &session.cwd)
        } else {
            eval_error_json(
                "unsupported_language",
                "Only bash is supported by the CPSL FFI skeleton",
                &session.cwd,
            )
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

    if config.language != "bash" {
        return Err("session language must be bash".to_string());
    }

    if config.mounts.len() != 1 {
        return Err("session config must contain exactly one mount".to_string());
    }

    if config.http.mode != "policy" {
        return Err("session http mode must be policy".to_string());
    }

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
        _host: host,
        initial_cwd: config.initial_cwd,
        _allow_domains: config.http.allow_domains,
        _deny_domains: config.http.deny_domains,
    })
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
    use tempfile::TempDir;

    #[test]
    fn validates_minimal_session_config() {
        let dir = TempDir::new().unwrap();
        let config = session_config(dir.path().canonicalize().unwrap().to_str().unwrap());

        let validated = validate_session_config(&config.to_string()).unwrap();

        assert_eq!(validated.initial_cwd, WORKDIR);
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

        let mut outside_cwd = session_config(host);
        outside_cwd["initial_cwd"] = json!("/tmp");
        assert!(validate_session_config(&outside_cwd.to_string()).is_err());

        let mut relative_host = session_config(host);
        relative_host["mounts"][0]["host"] = json!("relative");
        assert!(validate_session_config(&relative_host.to_string()).is_err());
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
}
