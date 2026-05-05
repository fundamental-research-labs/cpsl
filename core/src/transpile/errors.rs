//! Error translation from generated Luau locations back to Python source.

use std::collections::HashMap;

pub fn translate_error(
    error: &crate::ExecError,
    source_map: &HashMap<usize, usize>,
    python_source: &str,
) -> String {
    let py_lines: Vec<&str> = python_source.lines().collect();
    let message = pythonize_error(&error.message);

    // Map Luau line → Python line via source map
    let py_line = error
        .line
        .and_then(|luau_line| source_map.get(&luau_line).copied());

    if let Some(py_line) = py_line {
        let context = py_lines
            .get(py_line.wrapping_sub(1))
            .map(|l| format!("\n    {}", l.trim()))
            .unwrap_or_default();
        format!("{}: {}{}", py_line, message, context)
    } else {
        message
    }
}

/// Translate humanized error messages to Python-style error names.
fn pythonize_error(msg: &str) -> String {
    // "function is not defined" / "'X' is not defined" → NameError
    if msg.contains("is not defined") {
        return format!("NameError: {}", msg);
    }

    // "nil has no member 'X'" → AttributeError
    if msg.contains("has no member") {
        return format!(
            "AttributeError: 'NoneType' {}",
            msg.strip_prefix("nil ").unwrap_or(msg)
        );
    }

    // "arithmetic on nil value" → TypeError
    if msg.starts_with("arithmetic on") {
        return format!("TypeError: unsupported operand type(s): {}", msg);
    }

    // "cannot compare ..." → TypeError
    if msg.starts_with("cannot compare") {
        return format!("TypeError: {}", msg);
    }

    // "table key cannot be nil" → TypeError
    if msg == "table key cannot be nil" {
        return format!("TypeError: {}", msg);
    }

    msg.to_string()
}
