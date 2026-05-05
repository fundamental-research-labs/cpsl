//! Lua error cleanup and structured sandbox error types.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum SandboxError {
    #[error("Lua error: {0}")]
    Lua(#[from] mlua::Error),
}

/// Structured error from sandbox execution.
///
/// Contains the clean error message and optional source location,
/// with all mlua/Luau noise stripped.
#[derive(Debug, Clone)]
pub struct ExecError {
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub message: String,
}

impl std::fmt::Display for ExecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (self.line, self.column) {
            (Some(line), Some(col)) => write!(f, "{}:{}: {}", line, col, self.message),
            (Some(line), None) => write!(f, "{}: {}", line, self.message),
            _ => write!(f, "{}", self.message),
        }
    }
}

impl std::error::Error for ExecError {}

/// Extract a clean, structured error from an `mlua::Error`.
///
/// Strips all mlua/Luau noise:
/// - The `[string "..."]:<line>:<col>:` chunk location prefix (extracting line/col)
/// - `\nstack traceback: ...` suffix
///
/// Returns an [`ExecError`] with optional line/column and a clean message.
pub fn clean_lua_error(err: &mlua::Error) -> ExecError {
    match err {
        mlua::Error::SyntaxError { message, .. } => parse_chunk_location(message),
        mlua::Error::RuntimeError(msg) => {
            // Strip stack traceback
            let without_trace = match msg.find("\nstack traceback:") {
                Some(pos) => &msg[..pos],
                None => msg.as_str(),
            };
            parse_chunk_location(without_trace.trim())
        }
        mlua::Error::CallbackError { cause, .. } => clean_lua_error(cause),
        other => ExecError {
            line: None,
            column: None,
            message: other.to_string(),
        },
    }
}

/// Parse `[string "..."]:<line>: <message>` (Luau error location format).
///
/// Handles both `[string "name"]:LINE: message` and `[string "name"]:LINE:COL: message`.
/// Only reports line/column when the chunk name is "input" (the user's code).
/// Errors from other chunks (shrt, pyrt, metatable handlers) get `line: None`.
fn parse_chunk_location(msg: &str) -> ExecError {
    if let Some(bracket_end) = msg.find("]:") {
        // Extract chunk name from [string "NAME"]
        let is_user_input = msg.starts_with("[string \"input\"]");

        let after = &msg[bracket_end + 2..];
        if let Some(colon) = after.find(':') {
            if let Ok(line) = after[..colon].parse::<usize>() {
                let rest = &after[colon + 1..];
                // Check for column number: <digits>: <message>
                if let Some(colon2) = rest.find(':') {
                    if let Ok(col) = rest[..colon2].trim().parse::<usize>() {
                        return ExecError {
                            line: if is_user_input { Some(line) } else { None },
                            column: if is_user_input { Some(col) } else { None },
                            message: humanize_error(rest[colon2 + 1..].trim()),
                        };
                    }
                }
                return ExecError {
                    line: if is_user_input { Some(line) } else { None },
                    column: None,
                    message: humanize_error(rest.trim()),
                };
            }
        }
    }
    ExecError {
        line: None,
        column: None,
        message: humanize_error(msg.trim()),
    }
}

/// Translate Luau's internal error jargon into human-friendly messages.
///
/// Patterns that are already clear (FS errors, bad argument errors) pass through unchanged.
pub fn humanize_error(msg: &str) -> String {
    // "attempt to call a nil value" → "'X' is not defined" or "function is not defined"
    if msg == "attempt to call a nil value" {
        return "function is not defined".to_string();
    }

    // "attempt to index nil with 'Y'" → "nil has no member 'Y'"
    if let Some(rest) = msg.strip_prefix("attempt to index nil with '") {
        if let Some(member) = rest.strip_suffix('\'') {
            return format!("nil has no member '{}'", member);
        }
    }

    // "attempt to index ? (a nil value) with 'Y'" → "nil has no member 'Y'"
    if msg.starts_with("attempt to index") && msg.contains("(a nil value)") {
        if let Some(with_pos) = msg.find("with '") {
            let after = &msg[with_pos + 6..];
            if let Some(member) = after.strip_suffix('\'') {
                return format!("nil has no member '{}'", member);
            }
        }
    }

    // "attempt to perform arithmetic (add) on nil and number" → "arithmetic on nil value"
    if msg.starts_with("attempt to perform arithmetic") {
        return "arithmetic on nil value".to_string();
    }

    // "attempt to concatenate string and nil" → hint about nil values in concatenation
    if msg.starts_with("attempt to concatenate") {
        return "string concatenation (..) with nil value — check that both sides are strings or numbers".to_string();
    }

    // "attempt to compare nil < number" → "cannot compare nil with number"
    if msg.starts_with("attempt to compare") {
        return msg.replacen("attempt to compare", "cannot compare", 1);
    }

    // "table index is nil" → "table key cannot be nil"
    if msg == "table index is nil" {
        return "table key cannot be nil".to_string();
    }

    // "attempt to modify a readonly table" → hint at `local` declaration
    if msg.contains("attempt to modify a readonly table")
        || msg.contains("cannot modify a readonly table")
    {
        return "cannot assign to undeclared variable — use 'local' to declare variables (e.g., local x = 5)".to_string();
    }

    // Everything else passes through unchanged
    msg.to_string()
}
