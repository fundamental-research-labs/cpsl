//! Integration tests for `cpsl run`.
//!
//! These tests require a prior `cpsl build` and are slow.
//! Run with: cargo test -p cpsl-cli --test run_integration -- --ignored

use std::path::{Path, PathBuf};
use std::process::Command;

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("cpsl-cli must be inside the cpsl workspace")
        .to_path_buf()
}

fn cli_binary() -> PathBuf {
    let path = PathBuf::from(env!("CARGO_BIN_EXE_cpsl-cli"));
    if !path.exists() {
        workspace_root()
            .join("target")
            .join("debug")
            .join("cpsl-cli")
    } else {
        path
    }
}

fn fixture_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

const SANDBOX_NAME: &str = "test-run-integ";

fn ensure_sandbox_built() {
    let bin_path = dirs::home_dir()
        .unwrap()
        .join(".cpsl")
        .join("bin")
        .join(SANDBOX_NAME);

    if bin_path.exists() {
        return;
    }

    let config = fixture_path("json-only.toml");
    let output = Command::new(cli_binary())
        .args(["build", "-t", SANDBOX_NAME, "-f"])
        .arg(&config)
        .arg(".")
        .current_dir(workspace_root())
        .output()
        .expect("failed to run cpsl build");

    assert!(
        output.status.success(),
        "cpsl build failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn cleanup() {
    let home = dirs::home_dir().unwrap();
    let _ = std::fs::remove_file(home.join(".cpsl").join("bin").join(SANDBOX_NAME));
    let _ = std::fs::remove_file(
        home.join(".cpsl")
            .join("images")
            .join(format!("{}.toml", SANDBOX_NAME)),
    );
}

#[test]
#[ignore] // Requires cargo build — slow
fn run_inline_json_encode() {
    ensure_sandbox_built();

    let output = Command::new(cli_binary())
        .args([
            "run",
            SANDBOX_NAME,
            "--lua",
            "--",
            "print(json.encode({a = 1}))",
        ])
        .current_dir(workspace_root())
        .output()
        .expect("failed to run cpsl run");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "cpsl run failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("\"a\"") && stdout.contains("1"),
        "expected json output with 'a' key, got: {}",
        stdout
    );

    cleanup();
}

#[test]
#[ignore] // Requires cargo build — slow
fn run_csv_is_nil_in_json_only_sandbox() {
    ensure_sandbox_built();

    let output = Command::new(cli_binary())
        .args(["run", SANDBOX_NAME, "--lua", "--", "print(csv == nil)"])
        .current_dir(workspace_root())
        .output()
        .expect("failed to run cpsl run");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "cpsl run csv nil check failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.trim() == "true",
        "csv should be nil in json-only sandbox, got: {}",
        stdout
    );

    cleanup();
}

#[test]
#[ignore] // Requires cargo build — slow
fn run_nonexistent_sandbox_shows_error() {
    let output = Command::new(cli_binary())
        .args(["run", "nonexistent-sandbox-xyz"])
        .current_dir(workspace_root())
        .output()
        .expect("failed to run cpsl run");

    assert!(
        !output.status.success(),
        "expected failure for nonexistent sandbox"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("not found"),
        "expected 'not found' error, got: {}",
        stderr
    );
    assert!(
        stderr.contains("cpsl build"),
        "expected hint about 'cpsl build', got: {}",
        stderr
    );
}
