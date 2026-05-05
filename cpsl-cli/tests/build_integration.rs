//! Integration tests for `cpsl build`.
//!
//! These tests invoke real cargo builds and are slow (~30s+).
//! Run with: cargo test -p cpsl-cli --test build_integration -- --ignored

use std::path::{Path, PathBuf};
use std::process::Command;

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("cpsl-cli must be inside the cpsl workspace")
        .to_path_buf()
}

fn cli_binary() -> PathBuf {
    // The test binary is in target/debug/deps/, the CLI binary is in target/debug/
    let mut path = PathBuf::from(env!("CARGO_BIN_EXE_cpsl-cli"));
    // If that doesn't exist, fall back to building it
    if !path.exists() {
        path = workspace_root().join("target").join("debug").join("cpsl-cli");
    }
    path
}

fn fixture_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

#[test]
#[ignore] // Requires cargo build — slow
fn build_json_only_produces_working_binary() {
    let workspace = workspace_root();
    let config = fixture_path("json-only.toml");

    // Build a json+fs only binary
    let output = Command::new(cli_binary())
        .args(["build", "-t", "test-json-integ", "-f"])
        .arg(&config)
        .arg(".")
        .current_dir(&workspace)
        .output()
        .expect("failed to run cpsl build");

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "cpsl build failed:\nstdout: {}\nstderr: {}",
        stdout,
        stderr
    );

    // The binary should be installed at ~/.cpsl/bin/test-json-integ
    let bin_path = dirs::home_dir()
        .unwrap()
        .join(".cpsl")
        .join("bin")
        .join("test-json-integ");
    assert!(bin_path.exists(), "binary not found at {:?}", bin_path);

    // Test that json module works
    let output = Command::new(&bin_path)
        .args(["--", "print(json.encode({hello = \"world\"}))"])
        .current_dir(&workspace)
        .output()
        .expect("failed to run built binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "json test failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("hello"),
        "expected json output, got: {}",
        stdout
    );

    // Test that csv module is NOT available (should be nil)
    let output = Command::new(&bin_path)
        .args(["--", "print(csv == nil)"])
        .current_dir(&workspace)
        .output()
        .expect("failed to run built binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "csv nil check failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.trim() == "true",
        "csv should be nil but got: {}",
        stdout
    );

    // Cleanup
    let _ = std::fs::remove_file(&bin_path);
    let images_path = dirs::home_dir()
        .unwrap()
        .join(".cpsl")
        .join("images")
        .join("test-json-integ.toml");
    let _ = std::fs::remove_file(&images_path);
}
