//! Integration tests for `cpsl sandboxes` and `cpsl rm`.
//!
//! These tests require a prior `cpsl build` and are slow.
//! Run with: cargo test -p cpsl-cli --test sandboxes_rm_integration -- --ignored

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

const SANDBOX_NAME: &str = "test-ls-rm-integ";

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
fn sandboxes_lists_built_sandbox() {
    cleanup();

    // Build a sandbox
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
        "cpsl build failed:\nstderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // List sandboxes
    let output = Command::new(cli_binary())
        .args(["sandboxes"])
        .current_dir(workspace_root())
        .output()
        .expect("failed to run cpsl sandboxes");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "cpsl sandboxes failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains(SANDBOX_NAME),
        "expected sandbox '{}' in output:\n{}",
        SANDBOX_NAME,
        stdout
    );
    // Should show module names
    assert!(
        stdout.contains("json"),
        "expected 'json' module in output:\n{}",
        stdout
    );

    // Also test the `ls` alias
    let output = Command::new(cli_binary())
        .args(["ls"])
        .current_dir(workspace_root())
        .output()
        .expect("failed to run cpsl ls");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "cpsl ls failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains(SANDBOX_NAME),
        "ls alias should list sandbox '{}', got:\n{}",
        SANDBOX_NAME,
        stdout
    );

    cleanup();
}

#[test]
#[ignore] // Requires cargo build — slow
fn rm_removes_sandbox() {
    cleanup();

    // Build a sandbox
    let config = fixture_path("json-only.toml");
    let output = Command::new(cli_binary())
        .args(["build", "-t", SANDBOX_NAME, "-f"])
        .arg(&config)
        .arg(".")
        .current_dir(workspace_root())
        .output()
        .expect("failed to run cpsl build");

    assert!(output.status.success(), "cpsl build failed");

    // Verify it exists
    let home = dirs::home_dir().unwrap();
    let bin_path = home.join(".cpsl").join("bin").join(SANDBOX_NAME);
    let config_path = home
        .join(".cpsl")
        .join("images")
        .join(format!("{}.toml", SANDBOX_NAME));
    assert!(bin_path.exists(), "binary should exist before rm");
    assert!(config_path.exists(), "config should exist before rm");

    // Remove with --force (no confirmation prompt)
    let output = Command::new(cli_binary())
        .args(["rm", "--force", SANDBOX_NAME])
        .current_dir(workspace_root())
        .output()
        .expect("failed to run cpsl rm");

    assert!(
        output.status.success(),
        "cpsl rm failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Verify both files are gone
    assert!(!bin_path.exists(), "binary should be removed after rm");
    assert!(!config_path.exists(), "config should be removed after rm");

    // Verify sandboxes no longer lists it
    let output = Command::new(cli_binary())
        .args(["sandboxes"])
        .current_dir(workspace_root())
        .output()
        .expect("failed to run cpsl sandboxes");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains(SANDBOX_NAME),
        "sandbox should not appear after rm:\n{}",
        stdout
    );
}

#[test]
#[ignore] // No build needed, but grouped with integration tests
fn rm_nonexistent_sandbox_errors() {
    let output = Command::new(cli_binary())
        .args(["rm", "--force", "nonexistent-sandbox-xyz-rm"])
        .current_dir(workspace_root())
        .output()
        .expect("failed to run cpsl rm");

    assert!(
        !output.status.success(),
        "expected failure for nonexistent sandbox"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("does not exist"),
        "expected 'does not exist' error, got: {}",
        stderr
    );
}
