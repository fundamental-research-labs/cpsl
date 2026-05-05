#![cfg(feature = "all")]

//! Python compatibility tests.
//!
//! Discovers `.py` fixture files under `tests/compat/python/`,
//! transpiles each to Luau, executes in a sandbox with pyrt.luau,
//! and compares stdout against the committed `.expected` baseline.

use std::path::Path;
use cpsl_core::{transpile, Sandbox};

/// Create a sandbox with the Python runtime loaded.
fn python_sandbox() -> Sandbox {
    let s = Sandbox::new().unwrap();
    let pyrt = include_str!("../../pyrt.luau");
    s.setup_python_runtime(pyrt).unwrap();
    s
}

/// Run all `.py` fixtures in a directory, comparing against `.expected` files.
/// Returns a vec of (filename, failure_message) for any mismatches.
fn run_python_category(dir: &Path) -> Vec<(String, String)> {
    let mut failures = Vec::new();

    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("cannot read dir {}: {}", dir.display(), e))
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map_or(false, |ext| ext == "py")
        })
        .collect();
    entries.sort_by_key(|e| e.file_name());

    if entries.is_empty() {
        return failures;
    }

    for entry in entries {
        let py_path = entry.path();
        let expected_path = py_path.with_extension("py.expected");
        let name = py_path.file_name().unwrap().to_string_lossy().to_string();

        let source = std::fs::read_to_string(&py_path)
            .unwrap_or_else(|e| panic!("cannot read {}: {}", py_path.display(), e));

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
        let transpiled = match transpile::transpile(&source) {
            Ok(r) => r,
            Err(e) => {
                failures.push((name, format!("transpile error: {}", e)));
                continue;
            }
        };

        // Execute
        let sb = python_sandbox();
        let actual = match sb.exec(&transpiled.luau_source) {
            Ok(output) => output,
            Err(e) => {
                failures.push((name, format!("exec error: {}", e)));
                continue;
            }
        };

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
                    diff.push_str(&format!("  line {}: expected {:?}, got {:?}\n", i + 1, exp, act));
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
        let mut msg = format!("\n{} compatibility failures in '{}':\n", failures.len(), category);
        for (name, detail) in &failures {
            msg.push_str(&format!("\n  {} ----\n{}\n", name, detail));
        }
        panic!("{}", msg);
    }
}

macro_rules! compat_test {
    ($name:ident, $subdir:expr) => {
        #[test]
        fn $name() {
            let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/compat/python")
                .join($subdir);
            if !dir.exists() {
                return; // skip if category dir doesn't exist yet
            }
            let failures = run_python_category(&dir);
            assert_category($subdir, failures);
        }
    };
}

compat_test!(compat_python_basics, "basics");
compat_test!(compat_python_collections, "collections");
compat_test!(compat_python_strings, "strings");
compat_test!(compat_python_control_flow, "control_flow");
compat_test!(compat_python_functions, "functions");
compat_test!(compat_python_errors, "errors");
compat_test!(compat_python_comprehensions, "comprehensions");
compat_test!(compat_python_builtins, "builtins");
compat_test!(compat_python_edge_cases, "edge_cases");
