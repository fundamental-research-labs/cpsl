//! Focused request-shape tests for the Lua-facing webbrowser module.

use super::*;
use crate::{MountPermission, Sandbox};
use std::sync::Mutex;

#[derive(Default)]
struct RecordingGateway {
    requests: Mutex<Vec<serde_json::Value>>,
}

impl WebBrowserGateway for RecordingGateway {
    fn handle_json(&self, request_json: &str) -> Result<String, String> {
        let value: serde_json::Value = serde_json::from_str(request_json).unwrap();
        self.requests.lock().unwrap().push(value);
        Ok(
            r#"{"ok":true,"browser":"abc123ef","result":{"ok":true,"browser":"abc123ef"}}"#
                .to_string(),
        )
    }
}

fn sandbox(gateway: Arc<RecordingGateway>) -> Sandbox {
    Sandbox::builder()
        .webbrowser_gateway(gateway)
        .build()
        .unwrap()
}

#[test]
fn open_defaults_to_lean_resource_mode() {
    let gateway = Arc::new(RecordingGateway::default());
    let sandbox = sandbox(gateway.clone());

    let result = sandbox
        .exec(r#"local r = webbrowser.open("https://example.com"); return r.browser"#)
        .unwrap();

    assert_eq!(result, "abc123ef");
    let requests = gateway.requests.lock().unwrap();
    assert_eq!(requests[0]["command"], "open");
    assert_eq!(requests[0]["url"], "https://example.com");
    assert_eq!(requests[0]["resourceMode"], "lean");
}

#[test]
fn help_documents_eval_value_and_open_without_resource_wait() {
    let gateway = Arc::new(RecordingGateway::default());
    let sandbox = sandbox(gateway);

    let help = sandbox.exec("return webbrowser.help()").unwrap();

    assert!(
        help.contains("read the JavaScript result from the value field"),
        "eval help should document result.value: {help}"
    );
    assert!(
        help.contains(r#"local title = webbrowser.eval(browser, "return document.title", {function_body=true}).value"#),
        "eval example should read .value: {help}"
    );
    assert!(
        help.contains(r#"local browser = webbrowser.open("https://example.com").browser"#),
        "open example should avoid initial resource wait: {help}"
    );
}

#[test]
fn screenshot_maps_virtual_destination_to_host_path() {
    let tempdir = tempfile::tempdir().unwrap();
    let mut mounts = MountTable::new();
    mounts
        .add_mount(
            tempdir.path().to_path_buf(),
            "/tmp",
            MountPermission::ReadWrite,
        )
        .unwrap();
    let gateway = Arc::new(RecordingGateway::default());
    let sandbox = Sandbox::builder()
        .mounts(mounts)
        .auto_tmp(false)
        .webbrowser_gateway(gateway.clone())
        .build()
        .unwrap();

    sandbox
        .exec(r#"webbrowser.screenshot("abc123ef", "/tmp/nested/page.png")"#)
        .unwrap();

    let requests = gateway.requests.lock().unwrap();
    let destination = requests[0]["destinationPath"].as_str().unwrap();
    assert!(destination.starts_with(tempdir.path().to_str().unwrap()));
    assert!(destination.ends_with("nested/page.png"));
    assert_eq!(
        requests[0]["virtualDestinationPath"],
        "/tmp/nested/page.png"
    );
    assert_eq!(requests[0]["waitForResources"], true);
}

#[test]
fn show_dispatches_handoff_request() {
    let gateway = Arc::new(RecordingGateway::default());
    let sandbox = sandbox(gateway.clone());

    sandbox.exec(r#"webbrowser.show("abc123ef")"#).unwrap();

    let requests = gateway.requests.lock().unwrap();
    assert_eq!(requests[0]["command"], "browserShow");
    assert_eq!(requests[0]["browser"], "abc123ef");
}

#[test]
fn type_forwards_natural_typing_options() {
    let gateway = Arc::new(RecordingGateway::default());
    let sandbox = sandbox(gateway.clone());

    sandbox
        .exec(
            r#"
            webbrowser.type("abc123ef", "a2", "hello", {
                backend = "native",
                rhythm = "natural",
                speed = 4.5,
                delay_min = 0.01,
                delay_max = 0.2,
            })
            "#,
        )
        .unwrap();

    let requests = gateway.requests.lock().unwrap();
    assert_eq!(requests[0]["command"], "type");
    assert_eq!(requests[0]["browser"], "abc123ef");
    assert_eq!(requests[0]["action"], "a2");
    assert_eq!(requests[0]["value"], "hello");
    assert_eq!(requests[0]["typingBackend"], "native");
    assert_eq!(requests[0]["typingRhythm"], "natural");
    assert_eq!(requests[0]["typingSpeed"].as_f64(), Some(4.5));
    assert_eq!(requests[0]["typingDelayMin"].as_f64(), Some(0.01));
    assert_eq!(requests[0]["typingDelayMax"].as_f64(), Some(0.2));
}

#[test]
fn key_press_accepts_documented_control_keys() {
    let gateway = Arc::new(RecordingGateway::default());
    let sandbox = sandbox(gateway.clone());

    sandbox
        .exec(r#"webbrowser.key_press("abc123ef", "a2", "Enter")"#)
        .unwrap();

    let requests = gateway.requests.lock().unwrap();
    assert_eq!(requests[0]["command"], "keyPress");
    assert_eq!(requests[0]["value"], "Enter");
}

#[test]
fn key_press_rejects_modifier_combinations() {
    let gateway = Arc::new(RecordingGateway::default());
    let sandbox = sandbox(gateway.clone());

    let error = sandbox
        .exec(r#"webbrowser.key_press("abc123ef", "a2", "Control+Enter")"#)
        .unwrap_err()
        .to_string();

    assert!(error.contains("unsupported control key"), "{error}");
    assert!(gateway.requests.lock().unwrap().is_empty());
}
