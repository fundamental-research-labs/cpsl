use libloading::{Library, Symbol};
use serde_json::Value;
use std::env;
use std::ffi::{c_char, CStr, CString};
use std::path::{Path, PathBuf};

type AbiVersion = unsafe extern "C" fn() -> u32;
type MetadataJson = unsafe extern "C" fn() -> *mut c_char;
type SessionNew = unsafe extern "C" fn(*const c_char) -> *mut std::ffi::c_void;
type SessionNewWithWebBrowserCallbacks =
    unsafe extern "C" fn(*const c_char, *const std::ffi::c_void) -> *mut std::ffi::c_void;
type SessionNewWithCallbacks = unsafe extern "C" fn(
    *const c_char,
    *const std::ffi::c_void,
    *const std::ffi::c_void,
) -> *mut std::ffi::c_void;
type SessionNewWithHostCallbacks = unsafe extern "C" fn(
    *const c_char,
    *const std::ffi::c_void,
    *const std::ffi::c_void,
    *const std::ffi::c_void,
) -> *mut std::ffi::c_void;
type SessionNewWithHostCallbacksV2 = unsafe extern "C" fn(
    *const c_char,
    *const std::ffi::c_void,
    *const std::ffi::c_void,
    *const std::ffi::c_void,
    *const std::ffi::c_void,
) -> *mut std::ffi::c_void;
type SessionNewWithHostCallbacksV3 = unsafe extern "C" fn(
    *const c_char,
    *const std::ffi::c_void,
    *const std::ffi::c_void,
    *const std::ffi::c_void,
    *const std::ffi::c_void,
    *const std::ffi::c_void,
) -> *mut std::ffi::c_void;
type VisionRespond = unsafe extern "C" fn(*mut std::ffi::c_void, *const u8, usize, u8);
type SessionFree = unsafe extern "C" fn(*mut std::ffi::c_void);
type Eval = unsafe extern "C" fn(*mut std::ffi::c_void, *const c_char) -> *mut c_char;
type StringFree = unsafe extern "C" fn(*mut c_char);
type LastError = unsafe extern "C" fn() -> *const c_char;

#[test]
#[ignore = "requires `cargo build -p cpsl-ffi --release` first"]
fn probe_release_library_exports_contract_symbols() {
    let library_path = library_path();
    assert!(
        library_path.is_file(),
        "CPSL FFI library not found at {}. Run `cargo build -p cpsl-ffi --release` or set CPSL_FFI_LIB.",
        library_path.display()
    );

    let temp_dir = tempfile::tempdir().unwrap();
    let host = temp_dir.path().canonicalize().unwrap();
    let host = host.to_str().unwrap();

    unsafe {
        let library = Library::new(&library_path).unwrap();
        let abi_version: Symbol<AbiVersion> = library.get(b"cpsl_abi_version").unwrap();
        let metadata_json: Symbol<MetadataJson> =
            library.get(b"cpsl_backend_metadata_json").unwrap();
        let session_new: Symbol<SessionNew> = library.get(b"cpsl_session_new").unwrap();
        let _session_new_with_webbrowser_callbacks: Symbol<SessionNewWithWebBrowserCallbacks> =
            library
                .get(b"cpsl_session_new_with_webbrowser_callbacks")
                .unwrap();
        let _session_new_with_callbacks: Symbol<SessionNewWithCallbacks> =
            library.get(b"cpsl_session_new_with_callbacks").unwrap();
        let _session_new_with_host_callbacks: Symbol<SessionNewWithHostCallbacks> = library
            .get(b"cpsl_session_new_with_host_callbacks")
            .unwrap();
        let _session_new_with_host_callbacks_v2: Symbol<SessionNewWithHostCallbacksV2> = library
            .get(b"cpsl_session_new_with_host_callbacks_v2")
            .unwrap();
        let _session_new_with_host_callbacks_v3: Symbol<SessionNewWithHostCallbacksV3> = library
            .get(b"cpsl_session_new_with_host_callbacks_v3")
            .unwrap();
        let _vision_respond: Symbol<VisionRespond> = library.get(b"cpsl_vision_respond").unwrap();
        let session_free: Symbol<SessionFree> = library.get(b"cpsl_session_free").unwrap();
        let eval: Symbol<Eval> = library.get(b"cpsl_eval").unwrap();
        let string_free: Symbol<StringFree> = library.get(b"cpsl_string_free").unwrap();
        let last_error: Symbol<LastError> = library.get(b"cpsl_last_error").unwrap();

        assert_eq!(abi_version(), 2);
        assert_eq!(borrowed_c_string(last_error()), "");

        let metadata = owned_c_string(metadata_json(), *string_free);
        let metadata: Value = serde_json::from_str(&metadata).unwrap();
        assert_eq!(metadata["name"], "cpsl");
        assert_eq!(metadata["abi_version"], 2);
        assert!(metadata["languages"]
            .as_array()
            .unwrap()
            .iter()
            .any(|language| language == "luau"));
        assert!(metadata["languages"]
            .as_array()
            .unwrap()
            .iter()
            .any(|language| language == "bash"));
        assert_eq!(metadata["capabilities"]["mounts"], true);
        assert_eq!(metadata["capabilities"]["network_policy"], true);
        assert_eq!(metadata["capabilities"]["file_activity_callbacks"], true);
        assert_eq!(
            metadata["capabilities"]["vision_callbacks"],
            cfg!(feature = "doc")
        );

        string_free(std::ptr::null_mut());
        session_free(std::ptr::null_mut());

        assert!(session_new(std::ptr::null()).is_null());
        assert_ne!(borrowed_c_string(last_error()), "");

        let config = CString::new(session_config(host).to_string()).unwrap();
        let session = session_new(config.as_ptr());
        assert!(
            !session.is_null(),
            "session_new failed: {}",
            borrowed_c_string(last_error())
        );
        assert_eq!(borrowed_c_string(last_error()), "");

        let valid_request = CString::new(
            serde_json::json!({
                "language": "bash",
                "input": "pwd",
                "timeout_ms": 120000
            })
            .to_string(),
        )
        .unwrap();
        assert!(eval(std::ptr::null_mut(), valid_request.as_ptr()).is_null());
        assert_ne!(borrowed_c_string(last_error()), "");

        assert!(eval(session, std::ptr::null()).is_null());
        assert_ne!(borrowed_c_string(last_error()), "");

        let response = owned_c_string(eval(session, valid_request.as_ptr()), *string_free);
        let response: Value = serde_json::from_str(&response).unwrap();
        assert_eq!(response["ok"], true);
        assert_eq!(response["stdout"], "/workdir\n");
        assert_eq!(response["stderr"], "");
        assert_eq!(response["exit_code"], 0);
        assert_eq!(response["error"], Value::Null);
        assert_eq!(response["cwd"], "/workdir");
        assert_eq!(borrowed_c_string(last_error()), "");

        let luau_request = CString::new(
            serde_json::json!({
                "language": "luau",
                "input": "print('luau ok')",
                "timeout_ms": 120000
            })
            .to_string(),
        )
        .unwrap();
        let response = owned_c_string(eval(session, luau_request.as_ptr()), *string_free);
        let response: Value = serde_json::from_str(&response).unwrap();
        assert_eq!(response["ok"], true);
        assert_eq!(response["stdout"], "luau ok\n");
        assert_eq!(response["exit_code"], 0);
        assert_eq!(response["error"], Value::Null);

        let denied_request = CString::new(
            serde_json::json!({
                "language": "bash",
                "input": "http get https://example.com/",
                "timeout_ms": 120000
            })
            .to_string(),
        )
        .unwrap();
        let response = owned_c_string(eval(session, denied_request.as_ptr()), *string_free);
        let response: Value = serde_json::from_str(&response).unwrap();
        assert_eq!(response["ok"], false);
        assert_eq!(response["error"]["code"], "sandbox_denied");

        session_free(session);

        let config = CString::new(
            session_config_with_policy(host, &["example.com"], &["api.example.com"]).to_string(),
        )
        .unwrap();
        let session = session_new(config.as_ptr());
        assert!(
            !session.is_null(),
            "session_new failed: {}",
            borrowed_c_string(last_error())
        );
        let denied_subdomain_request = CString::new(
            serde_json::json!({
                "language": "bash",
                "input": "http get https://api.example.com/",
                "timeout_ms": 120000
            })
            .to_string(),
        )
        .unwrap();
        let response = owned_c_string(
            eval(session, denied_subdomain_request.as_ptr()),
            *string_free,
        );
        let response: Value = serde_json::from_str(&response).unwrap();
        assert_eq!(response["ok"], false);
        assert_eq!(response["error"]["code"], "sandbox_denied");
        session_free(session);
    }
}

fn library_path() -> PathBuf {
    if let Ok(path) = env::var("CPSL_FFI_LIB") {
        return PathBuf::from(path);
    }

    let target_dir = env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("target")
        });
    target_dir.join("release").join(format!(
        "{}cpsl{}",
        env::consts::DLL_PREFIX,
        env::consts::DLL_SUFFIX
    ))
}

unsafe fn borrowed_c_string(value: *const c_char) -> String {
    assert!(!value.is_null());
    CStr::from_ptr(value).to_str().unwrap().to_owned()
}

unsafe fn owned_c_string(value: *mut c_char, string_free: StringFree) -> String {
    assert!(!value.is_null());
    let text = CStr::from_ptr(value).to_str().unwrap().to_owned();
    string_free(value);
    text
}

fn session_config(host: &str) -> serde_json::Value {
    session_config_with_policy(host, &[], &[])
}

fn session_config_with_policy(
    host: &str,
    allow_domains: &[&str],
    deny_domains: &[&str],
) -> serde_json::Value {
    serde_json::json!({
        "mounts": [
            {"host": host, "virtual": "/workdir", "mode": "rw"}
        ],
        "initial_cwd": "/workdir",
        "language": "luau",
        "http": {
            "mode": "policy",
            "allow_domains": allow_domains,
            "deny_domains": deny_domains
        }
    })
}
