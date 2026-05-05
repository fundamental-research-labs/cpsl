use cpsl_core::{sh_transpile, transpile, Sandbox};
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;
use std::sync::Mutex;

const MODE_BASH: u32 = 0;
const MODE_PYTHON: u32 = 1;
const MODE_LUA: u32 = 2;

const PYRT: &str = include_str!("../../../runtime/pyrt.luau");
const SHRT: &str = include_str!("../../../runtime/shrt.luau");

static LAST_ERROR: Mutex<Option<CString>> = Mutex::new(None);

fn main() {}

pub struct Session {
    sandbox: Sandbox,
    mode: Mode,
    lua_buffer: String,
    lua_multiline: bool,
    python_buffer: String,
    python_multiline: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Bash,
    Python,
    Lua,
}

impl Mode {
    fn from_u32(value: u32) -> Result<Self, String> {
        match value {
            MODE_BASH => Ok(Self::Bash),
            MODE_PYTHON => Ok(Self::Python),
            MODE_LUA => Ok(Self::Lua),
            _ => Err(format!("unknown CPSL mode: {value}")),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Bash => "shell",
            Self::Python => "python",
            Self::Lua => "luau",
        }
    }

    fn prompt(self, incomplete: bool) -> &'static str {
        match (self, incomplete) {
            (Self::Bash, _) => "$",
            (Self::Python, true) => "...",
            (Self::Python, false) => ">>>",
            (Self::Lua, true) => ">>",
            (Self::Lua, false) => ">",
        }
    }
}

impl Session {
    fn new() -> Result<Self, String> {
        let sandbox = Sandbox::new().map_err(|e| e.to_string())?;
        sandbox
            .setup_shell_runtime(SHRT)
            .map_err(|e| format!("failed to load shell runtime: {e}"))?;
        sandbox
            .setup_python_runtime(PYRT)
            .map_err(|e| format!("failed to load python runtime: {e}"))?;

        Ok(Self {
            sandbox,
            mode: Mode::Bash,
            lua_buffer: String::new(),
            lua_multiline: false,
            python_buffer: String::new(),
            python_multiline: false,
        })
    }

    fn eval(&mut self, mode: Mode, input: &str) -> EvalResponse {
        if self.mode != mode {
            self.reset_buffers();
            self.mode = mode;
        }

        match mode {
            Mode::Bash => self.eval_bash(input),
            Mode::Python => self.eval_python(input),
            Mode::Lua => self.eval_lua(input),
        }
    }

    fn reset_buffers(&mut self) {
        self.lua_buffer.clear();
        self.lua_multiline = false;
        self.python_buffer.clear();
        self.python_multiline = false;
    }

    fn eval_bash(&self, input: &str) -> EvalResponse {
        if input.trim().is_empty() {
            return EvalResponse::ok(Mode::Bash, false, String::new(), Vec::new());
        }

        match sh_transpile::transpile_sh(input) {
            Ok(result) => {
                let warnings = result.warnings.into_iter().map(|w| w.to_string()).collect();
                match self.sandbox.exec(&result.luau_source) {
                    Ok(output) => EvalResponse::ok(Mode::Bash, false, output, warnings),
                    Err(e) => EvalResponse::err(Mode::Bash, false, e.to_string(), warnings),
                }
            }
            Err(e) => EvalResponse::err(Mode::Bash, false, e.to_string(), Vec::new()),
        }
    }

    fn eval_python(&mut self, input: &str) -> EvalResponse {
        let trimmed = input.trim();

        if self.python_multiline {
            if trimmed.is_empty() {
                let source = std::mem::take(&mut self.python_buffer);
                self.python_multiline = false;
                return self.run_python_source(&source);
            }

            self.python_buffer.push('\n');
            self.python_buffer.push_str(input);
            return EvalResponse::ok(Mode::Python, true, String::new(), Vec::new());
        }

        if trimmed.is_empty() {
            return EvalResponse::ok(Mode::Python, false, String::new(), Vec::new());
        }

        if starts_python_block(trimmed) {
            self.python_buffer.clear();
            self.python_buffer.push_str(input);
            self.python_multiline = true;
            return EvalResponse::ok(Mode::Python, true, String::new(), Vec::new());
        }

        self.run_python_source(input)
    }

    fn run_python_source(&self, source: &str) -> EvalResponse {
        match transpile::transpile(source) {
            Ok(result) => {
                let warnings = result.warnings.iter().map(|w| w.to_string()).collect();
                match self.sandbox.exec(&result.luau_source) {
                    Ok(output) => EvalResponse::ok(Mode::Python, false, output, warnings),
                    Err(e) => {
                        let translated = transpile::translate_error(&e, &result.source_map, source);
                        EvalResponse::err(Mode::Python, false, translated, warnings)
                    }
                }
            }
            Err(e) => EvalResponse::err(Mode::Python, false, e.to_string(), Vec::new()),
        }
    }

    fn eval_lua(&mut self, input: &str) -> EvalResponse {
        if input.trim().is_empty() && !self.lua_multiline {
            return EvalResponse::ok(Mode::Lua, false, String::new(), Vec::new());
        }

        if self.lua_multiline {
            self.lua_buffer.push('\n');
            self.lua_buffer.push_str(input);
        } else {
            self.lua_buffer.clear();
            self.lua_buffer.push_str(input);
        }

        match self.sandbox.exec(&self.lua_buffer) {
            Ok(output) => {
                self.lua_buffer.clear();
                self.lua_multiline = false;
                EvalResponse::ok(Mode::Lua, false, output, Vec::new())
            }
            Err(e) => {
                let err = e.to_string();
                if is_incomplete_lua(&err) {
                    self.lua_multiline = true;
                    EvalResponse::ok(Mode::Lua, true, String::new(), Vec::new())
                } else {
                    self.lua_buffer.clear();
                    self.lua_multiline = false;
                    EvalResponse::err(Mode::Lua, false, err, Vec::new())
                }
            }
        }
    }
}

fn starts_python_block(input: &str) -> bool {
    input.ends_with(':')
        || input.starts_with("def ")
        || input.starts_with("if ")
        || input.starts_with("for ")
        || input.starts_with("while ")
        || input.starts_with("try:")
        || input.starts_with("elif ")
        || input.starts_with("else:")
        || input.starts_with("except")
        || input.starts_with("finally:")
}

fn is_incomplete_lua(err: &str) -> bool {
    err.contains("Expected") && (err.contains("to close") || err.contains("Expected 'end'"))
}

struct EvalResponse {
    ok: bool,
    mode: Mode,
    incomplete: bool,
    output: String,
    error: Option<String>,
    warnings: Vec<String>,
}

impl EvalResponse {
    fn ok(mode: Mode, incomplete: bool, output: String, warnings: Vec<String>) -> Self {
        Self {
            ok: true,
            mode,
            incomplete,
            output,
            error: None,
            warnings,
        }
    }

    fn err(mode: Mode, incomplete: bool, error: String, warnings: Vec<String>) -> Self {
        Self {
            ok: false,
            mode,
            incomplete,
            output: String::new(),
            error: Some(error),
            warnings,
        }
    }

    fn to_json(&self) -> String {
        let warnings = self
            .warnings
            .iter()
            .map(|w| format!("\"{}\"", json_escape(w)))
            .collect::<Vec<_>>()
            .join(",");
        let error = self
            .error
            .as_ref()
            .map(|e| format!("\"{}\"", json_escape(e)))
            .unwrap_or_else(|| "null".to_string());

        format!(
            "{{\"ok\":{},\"mode\":\"{}\",\"prompt\":\"{}\",\"incomplete\":{},\"output\":\"{}\",\"error\":{},\"warnings\":[{}]}}",
            self.ok,
            self.mode.name(),
            self.mode.prompt(self.incomplete),
            self.incomplete,
            json_escape(&self.output),
            error,
            warnings
        )
    }
}

fn json_escape(input: &str) -> String {
    let mut escaped = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            c if c.is_control() => {
                use std::fmt::Write;
                let _ = write!(escaped, "\\u{:04x}", c as u32);
            }
            c => escaped.push(c),
        }
    }
    escaped
}

fn set_last_error(message: impl Into<String>) {
    let message = message.into().replace('\0', "\\0");
    if let Ok(mut slot) = LAST_ERROR.lock() {
        *slot = CString::new(message).ok();
    }
}

fn clear_last_error() {
    if let Ok(mut slot) = LAST_ERROR.lock() {
        *slot = None;
    }
}

fn into_c_string(value: String) -> *mut c_char {
    let sanitized = value.replace('\0', "\\0");
    CString::new(sanitized)
        .unwrap_or_else(|_| CString::new("{\"ok\":false,\"error\":\"invalid response\"}").unwrap())
        .into_raw()
}

#[no_mangle]
pub extern "C" fn cpsl_session_new() -> *mut Session {
    match Session::new() {
        Ok(session) => {
            clear_last_error();
            Box::into_raw(Box::new(session))
        }
        Err(e) => {
            set_last_error(e);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn cpsl_session_free(session: *mut Session) {
    if !session.is_null() {
        drop(Box::from_raw(session));
    }
}

#[no_mangle]
pub unsafe extern "C" fn cpsl_eval(
    session: *mut Session,
    mode: u32,
    input: *const c_char,
) -> *mut c_char {
    if session.is_null() {
        return into_c_string(
            EvalResponse::err(
                Mode::Bash,
                false,
                "CPSL session is not initialized".to_string(),
                Vec::new(),
            )
            .to_json(),
        );
    }

    let Ok(mode) = Mode::from_u32(mode) else {
        return into_c_string(
            EvalResponse::err(
                Mode::Bash,
                false,
                "unknown CPSL mode".to_string(),
                Vec::new(),
            )
            .to_json(),
        );
    };

    if input.is_null() {
        return into_c_string(
            EvalResponse::err(mode, false, "missing input".to_string(), Vec::new()).to_json(),
        );
    }

    let source = CStr::from_ptr(input).to_string_lossy();
    let response = (&mut *session).eval(mode, source.as_ref());
    into_c_string(response.to_json())
}

#[no_mangle]
pub unsafe extern "C" fn cpsl_string_free(value: *mut c_char) {
    if !value.is_null() {
        drop(CString::from_raw(value));
    }
}

#[no_mangle]
pub extern "C" fn cpsl_last_error() -> *const c_char {
    if let Ok(slot) = LAST_ERROR.lock() {
        if let Some(ref value) = *slot {
            return value.as_ptr();
        }
    }
    ptr::null()
}
