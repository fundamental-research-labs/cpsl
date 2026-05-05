#![cfg(feature = "all")]

//! Bash compatibility tests.
//!
//! Discovers `.sh` fixture files under `tests/compat/bash/`,
//! transpiles each to Luau, executes in a sandbox with shrt.luau,
//! and compares stdout against the committed `.expected` baseline.

use cpsl_core::{sh_transpile, MountTable, Sandbox};
use std::path::Path;

/// Expand `{{KEY}}` template patterns in expected output with real host values.
/// Supported keys: ARCH, OS, OS_VERSION, CPU_BRAND
fn expand_templates(s: &str) -> String {
    let arch = std::env::consts::ARCH;
    let os = std::env::consts::OS;
    let os_version = host_os_version();
    let cpu_brand = host_cpu_brand();

    s.replace("{{ARCH}}", arch)
        .replace("{{OS}}", os)
        .replace("{{OS_VERSION}}", &os_version)
        .replace("{{CPU_BRAND}}", &cpu_brand)
}

fn host_cpu_brand() -> String {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("sysctl")
            .args(["-n", "machdep.cpu.brand_string"])
            .output()
            .ok()
            .and_then(|o| {
                if o.status.success() {
                    Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| "Unknown CPU".to_string())
    }
    #[cfg(not(target_os = "macos"))]
    {
        std::fs::read_to_string("/proc/cpuinfo")
            .ok()
            .and_then(|content| {
                content
                    .lines()
                    .find(|l| l.starts_with("model name"))
                    .and_then(|l| l.split(':').nth(1))
                    .map(|s| s.trim().to_string())
            })
            .unwrap_or_else(|| "Unknown CPU".to_string())
    }
}

fn host_os_version() -> String {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("sysctl")
            .args(["-n", "kern.osproductversion"])
            .output()
            .ok()
            .and_then(|o| {
                if o.status.success() {
                    Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
                } else {
                    None
                }
            })
            .unwrap_or_default()
    }
    #[cfg(not(target_os = "macos"))]
    {
        // Linux: read VERSION_ID from /etc/os-release
        std::fs::read_to_string("/etc/os-release")
            .ok()
            .and_then(|content| {
                content
                    .lines()
                    .find(|l| l.starts_with("VERSION_ID="))
                    .map(|l| {
                        l.trim_start_matches("VERSION_ID=")
                            .trim_matches('"')
                            .to_string()
                    })
            })
            .unwrap_or_default()
    }
}

/// Create a sandbox with the shell runtime and a writable temp filesystem.
fn bash_sandbox() -> (Sandbox, tempfile::TempDir) {
    let dir = tempfile::TempDir::new().unwrap();
    let mut mounts = MountTable::new();
    mounts
        .parse_and_add(&format!("{}:/", dir.path().display()))
        .unwrap();
    let s = Sandbox::with_mounts(mounts).unwrap();
    let shrt = include_str!("../../runtime/shrt.luau");
    s.register_module("shrt", shrt).unwrap();
    (s, dir)
}

/// Create a sandbox without mounts (for tests that don't need file I/O).
fn bash_sandbox_no_fs() -> Sandbox {
    let s = Sandbox::new().unwrap();
    let shrt = include_str!("../../runtime/shrt.luau");
    s.register_module("shrt", shrt).unwrap();
    s
}

/// Run all `.sh` fixtures in a directory, comparing against `.expected` files.
/// Returns a vec of (filename, failure_message) for any mismatches.
fn run_bash_category(dir: &Path, needs_fs: bool) -> Vec<(String, String)> {
    let mut failures = Vec::new();

    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("cannot read dir {}: {}", dir.display(), e))
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "sh"))
        .collect();
    entries.sort_by_key(|e| e.file_name());

    if entries.is_empty() {
        return failures;
    }

    for entry in entries {
        let sh_path = entry.path();
        let expected_path = sh_path.with_extension("sh.expected");
        let name = sh_path.file_name().unwrap().to_string_lossy().to_string();

        let source = std::fs::read_to_string(&sh_path)
            .unwrap_or_else(|e| panic!("cannot read {}: {}", sh_path.display(), e));

        // Check for SKIP marker
        if source.starts_with("# SKIP:") {
            continue;
        }

        let expected = match std::fs::read_to_string(&expected_path) {
            Ok(s) => s,
            Err(_) => {
                failures.push((name, "missing .expected file".to_string()));
                continue;
            }
        };

        // Transpile
        let transpiled = match sh_transpile::transpile_sh(&source) {
            Ok(r) => r,
            Err(e) => {
                failures.push((name, format!("transpile error: {}", e)));
                continue;
            }
        };

        // Execute — use fs sandbox for file_ops, no-fs for everything else
        let actual = if needs_fs {
            let (sb, _dir) = bash_sandbox();
            match sb.exec(&transpiled.luau_source) {
                Ok(output) => output,
                Err(e) => {
                    failures.push((name, format!("exec error: {}", e)));
                    continue;
                }
            }
        } else {
            let sb = bash_sandbox_no_fs();
            match sb.exec(&transpiled.luau_source) {
                Ok(output) => output,
                Err(e) => {
                    failures.push((name, format!("exec error: {}", e)));
                    continue;
                }
            }
        };

        // Expand {{TEMPLATE}} patterns in expected output with real host values.
        // This lets expected files reference dynamic values like architecture.
        let expected = expand_templates(&expected);

        // Compare (normalize trailing whitespace)
        let expected_trimmed = expected.trim_end();
        let actual_trimmed = actual.trim_end();

        if actual_trimmed != expected_trimmed {
            let mut diff = String::new();
            diff.push_str(&format!("--- expected\n+++ actual\n"));
            let exp_lines: Vec<&str> = expected_trimmed.lines().collect();
            let act_lines: Vec<&str> = actual_trimmed.lines().collect();
            let max_lines = exp_lines.len().max(act_lines.len());
            for i in 0..max_lines {
                let exp = exp_lines.get(i).unwrap_or(&"<missing>");
                let act = act_lines.get(i).unwrap_or(&"<missing>");
                if exp != act {
                    diff.push_str(&format!(
                        "  line {}: expected {:?}, got {:?}\n",
                        i + 1,
                        exp,
                        act
                    ));
                }
            }
            failures.push((name, diff));
        }
    }

    failures
}

/// Format failures into a readable assertion message.
fn assert_category(category: &str, failures: Vec<(String, String)>) {
    if !failures.is_empty() {
        let mut msg = format!(
            "\n{} compatibility failures in '{}':\n",
            failures.len(),
            category
        );
        for (name, detail) in &failures {
            msg.push_str(&format!("\n  {} ----\n{}\n", name, detail));
        }
        panic!("{}", msg);
    }
}

macro_rules! compat_test {
    ($name:ident, $subdir:expr, $needs_fs:expr) => {
        #[test]
        fn $name() {
            let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/compat/bash")
                .join($subdir);
            if !dir.exists() {
                return; // skip if category dir doesn't exist yet
            }
            let failures = run_bash_category(&dir, $needs_fs);
            assert_category($subdir, failures);
        }
    };
}

compat_test!(compat_bash_basics, "basics", false);
compat_test!(compat_bash_pipelines, "pipelines", false);
compat_test!(compat_bash_variables, "variables", false);
compat_test!(compat_bash_control_flow, "control_flow", false);
compat_test!(compat_bash_file_ops, "file_ops", true);
compat_test!(compat_bash_text_processing, "text_processing", false);
compat_test!(compat_bash_functions, "functions", false);
compat_test!(compat_bash_advanced, "advanced", true);
