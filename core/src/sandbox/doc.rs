//! Help metadata, signatures, and argument validation for sandbox modules.

use mlua::{MultiValue, Value};

// --- Module documentation convention ---
// Every sandbox module (fs, http, etc.) must have a help() function.
// Argument errors automatically hint at help() for discoverability.

/// Type of a function parameter, used for validation and help rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ParamType {
    String,
    /// A byte sequence accepted as either an immutable Luau string or a native buffer.
    StringOrBuffer,
    Number,
    Table,
    #[allow(dead_code)]
    Boolean,
    /// Any type (for encode-style functions that accept varied input).
    Value,
}

impl ParamType {
    pub fn label(self) -> &'static str {
        match self {
            ParamType::String => "string",
            ParamType::StringOrBuffer => "string | buffer",
            ParamType::Number => "number",
            ParamType::Table => "table",
            ParamType::Boolean => "boolean",
            ParamType::Value => "value",
        }
    }

    /// Shell-friendly label (table → JSON for CLI context).
    pub fn shell_label(self) -> &'static str {
        match self {
            ParamType::Table => "JSON",
            // Shell arguments and output are text-only. Native buffers are an
            // in-sandbox Luau representation, not a shell transport format.
            ParamType::StringOrBuffer => "string",
            _ => self.label(),
        }
    }

    /// Check whether an mlua::Value matches this expected type.
    pub fn matches(self, val: &mlua::Value) -> bool {
        match self {
            ParamType::String => matches!(val, mlua::Value::String(_)),
            ParamType::StringOrBuffer => {
                matches!(val, mlua::Value::String(_) | mlua::Value::Buffer(_))
            }
            ParamType::Number => matches!(val, mlua::Value::Number(_) | mlua::Value::Integer(_)),
            ParamType::Table => matches!(val, mlua::Value::Table(_)),
            ParamType::Boolean => matches!(val, mlua::Value::Boolean(_)),
            ParamType::Value => true, // accepts anything (except nil, checked separately)
        }
    }
}

/// Documents a named field within an opts table parameter.
#[derive(Debug, Clone, Copy)]
pub(crate) struct FieldDoc {
    pub name: &'static str,
    pub typ: &'static str,
    pub required: bool,
    pub description: &'static str,
}

/// Structured parameter metadata for a sandbox function.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Param {
    pub name: &'static str,
    pub short: Option<char>,
    pub typ: ParamType,
    pub required: bool,
    /// Known fields when type is Table or Value (rendered as sub-items in help).
    pub fields: Option<&'static [FieldDoc]>,
}

/// Type of a function's return value, used for help rendering and validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReturnType {
    String,
    /// Mode-dependent bytes returned as either a Luau string or native buffer.
    StringOrBuffer,
    Number,
    Table,
    Boolean,
    /// Any type (e.g. json.decode returns varied types).
    Value,
    /// No return value (e.g. fs.write, fs.mkdir).
    Void,
    /// Userdata object with methods (e.g. DocFuture).
    UserData,
}

impl ReturnType {
    pub fn label(self) -> &'static str {
        match self {
            ReturnType::String => "string",
            ReturnType::StringOrBuffer => "string | buffer",
            ReturnType::Number => "number",
            ReturnType::Table => "table",
            ReturnType::Boolean => "boolean",
            ReturnType::Value => "any",
            ReturnType::Void => "",
            ReturnType::UserData => "userdata",
        }
    }

    /// Shell-friendly label (table → JSON for CLI context).
    pub fn shell_label(self) -> &'static str {
        match self {
            ReturnType::Table => "JSON",
            // Shell output is always textual; native buffers must be converted
            // by selecting a transport-safe mode such as base64.
            ReturnType::StringOrBuffer => "string",
            _ => self.label(),
        }
    }
}

pub(crate) struct FnDoc {
    pub name: &'static str,
    pub description: &'static str,
    pub params: &'static [Param],
    pub returns: ReturnType,
    /// Optional usage example shown after the description in help output.
    pub example: Option<&'static str>,
}

pub(crate) struct ModuleDoc {
    pub name: &'static str,
    pub summary: &'static str,
    pub functions: &'static [FnDoc],
}

impl ModuleDoc {
    /// Look up the params for a function by name.
    ///
    /// Panics if the name is not found — a wrong name is a bug that will be
    /// caught immediately on first module registration, not at runtime.
    pub fn params(&self, fn_name: &str) -> &'static [Param] {
        self.functions
            .iter()
            .find(|f| f.name == fn_name)
            .unwrap_or_else(|| panic!("no FnDoc named '{}' in module '{}'", fn_name, self.name))
            .params
    }
}

/// Controls whether help output uses Lua call syntax or shell flag syntax.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HelpMode {
    /// `module.fn(arg1, arg2) -> type`
    Lua,
    /// `module fn --arg1 <type> --arg2 <type>`
    Shell,
}

impl FnDoc {
    /// Compact help for error context — signature + example in 2-3 lines.
    /// Shown inline when a usage error occurs so the agent can self-correct
    /// without an extra round-trip to call `module.help()`.
    pub fn format_error_help(&self, module_name: &str) -> String {
        let sig = self.generated_signature(HelpMode::Lua);
        let mut out = format!("  Usage: {}.{}{}", module_name, self.name, sig);
        if let Some(ex) = self.example {
            out.push_str(&format!("\n  Example: {}", ex));
        }
        out
    }

    /// Compact shell-native usage for argument errors raised through `sh.run()`.
    ///
    /// Lua examples are intentionally omitted: shell callers need the flattened
    /// flag signature, and embedding a Luau call in a shell error is misleading.
    pub fn format_shell_error_help(&self, module_name: &str) -> String {
        let sig = self.generated_signature_for_module(HelpMode::Shell, Some(module_name));
        if sig.is_empty() {
            format!("  Usage: {} {}", module_name, self.name)
        } else {
            format!("  Usage: {} {} {}", module_name, self.name, sig)
        }
    }

    /// Generate signature from structured params + returns.
    pub fn generated_signature(&self, mode: HelpMode) -> String {
        self.generated_signature_for_module(mode, None)
    }

    /// Generate a signature with module context for module-specific shell
    /// compatibility aliases. Keeping the context here prevents aliases for
    /// `fs.read` from leaking into unrelated functions such as `doc.read`.
    pub(crate) fn generated_signature_for_module(
        &self,
        mode: HelpMode,
        module_name: Option<&str>,
    ) -> String {
        match mode {
            HelpMode::Lua => {
                let ret = if self.returns == ReturnType::Void {
                    String::new()
                } else {
                    format!(" -> {}", self.returns.label())
                };
                let params_str: Vec<String> = self
                    .params
                    .iter()
                    .map(|p| {
                        if p.required {
                            format!("{}: {}", p.name, p.typ.label())
                        } else {
                            format!("{}?: {}", p.name, p.typ.label())
                        }
                    })
                    .collect();
                format!("({}){}", params_str.join(", "), ret)
            }
            HelpMode::Shell => {
                let ret = if self.returns == ReturnType::Void {
                    String::new()
                } else {
                    format!(" -> {}", self.returns.shell_label())
                };
                let flags: Vec<String> = self
                    .params
                    .iter()
                    .flat_map(|p| {
                        if p.name == "opts" {
                            if let Some(fields) = p.fields.filter(|fields| {
                                fields.iter().all(|field| {
                                    matches!(field.typ, "string" | "number" | "boolean")
                                })
                            }) {
                                let mut flattened = fields
                                    .iter()
                                    .map(|field| {
                                        let long = format!("--{}", field.name.replace('_', "-"));
                                        let flag = match (module_name, self.name, field.name) {
                                            // Compatibility aliases from fs.read's historical
                                            // positional range parameters plus the canonical
                                            // short forms for its new mode/count options.
                                            // FieldDoc does not otherwise declare short aliases.
                                            (Some("fs"), "read", "mode") => format!("-m/{long}"),
                                            (Some("fs"), "read", "offset") => format!("-o/{long}"),
                                            (Some("fs"), "read", "limit") => format!("-l/{long}"),
                                            (Some("fs"), "read", "count") => format!("-c/{long}"),
                                            _ => long,
                                        };
                                        if field.typ == "boolean" && field.required {
                                            flag
                                        } else if field.typ == "boolean" {
                                            format!("[{}]", flag)
                                        } else if field.required {
                                            format!("{} <{}>", flag, field.typ)
                                        } else {
                                            format!("[{} <{}>]", flag, field.typ)
                                        }
                                    })
                                    .collect::<Vec<_>>();
                                // A required opts table cannot be omitted in shell mode. When
                                // none of its fields is individually required, keep each field
                                // optional (and composable) while making the parent requirement
                                // explicit.
                                if p.required && fields.iter().all(|field| !field.required) {
                                    flattened.push("(at least one option required)".to_string());
                                }
                                return flattened;
                            }
                        }
                        let flag = match p.short {
                            Some(c) => format!("-{}/--{}", c, p.name),
                            None => format!("--{}", p.name),
                        };
                        vec![if p.required {
                            format!("{} <{}>", flag, p.typ.shell_label())
                        } else {
                            format!("[{} <{}>]", flag, p.typ.shell_label())
                        }]
                    })
                    .collect();
                format!("{}{}", flags.join(" "), ret)
            }
        }
    }
}

impl ModuleDoc {
    /// Render help in the given mode.
    pub fn format_help(&self, mode: HelpMode) -> String {
        let mut out = format!("{} — {}\n", self.name, self.summary);
        let mut sorted_fns: Vec<&FnDoc> = self.functions.iter().collect();
        sorted_fns.sort_by_key(|f| f.name);
        for f in sorted_fns {
            let sig = f.generated_signature_for_module(mode, Some(self.name));
            match mode {
                HelpMode::Lua => {
                    out.push_str(&format!(
                        "\n  {}.{}{}\n    {}\n",
                        self.name, f.name, sig, f.description
                    ));
                }
                HelpMode::Shell => {
                    out.push_str(&format!(
                        "\n  {} {} {}\n    {}\n",
                        self.name, f.name, sig, f.description
                    ));
                }
            }

            // Render opts table field docs (Lua mode only — shell uses flags)
            if mode == HelpMode::Lua {
                for p in f.params {
                    if let Some(fields) = p.fields {
                        out.push_str(&format!("    {} fields:\n", p.name));
                        for fd in fields {
                            let req = if fd.required { "" } else { "?" };
                            out.push_str(&format!(
                                "      {}{}: {} — {}\n",
                                fd.name, req, fd.typ, fd.description
                            ));
                        }
                    }
                }
            }

            // Render example if present (Lua mode only — examples are Luau code)
            if mode == HelpMode::Lua {
                if let Some(ex) = f.example {
                    out.push_str(&format!("    Example: {}\n", ex));
                }
            }
        }
        match mode {
            HelpMode::Lua => {
                out.push_str(&format!(
                    "\n  {}.help() -> string\n    Show this help message.\n",
                    self.name
                ));
            }
            HelpMode::Shell => {
                out.push_str(&format!(
                    "\n  {} help\n    Show this help message.\n",
                    self.name
                ));
            }
        }
        out
    }
}

/// Extract and validate arguments from a `MultiValue` against structured `Param` metadata.
///
/// Supports two calling conventions:
/// - **Positional**: `fn(arg1, arg2, arg3)` — values matched in order to params
/// - **Table form**: `fn({name1=val1, name2=val2})` — single table with named keys
///
/// Returns a `Vec<Value>` aligned with `params` (one entry per param, `Nil` for missing optional).
/// Errors are human-readable: `"module.fn: missing required argument 'name' (type)"`.
pub(crate) fn validate_args(
    args: &MultiValue,
    params: &[Param],
    fn_name: &str,
) -> Result<Vec<Value>, mlua::Error> {
    let vals = args.iter().collect::<Vec<_>>();

    // Detect table form: exactly one table argument.
    //
    // Ambiguity: when a function expects a single Table param, the table could be
    // the argument itself (positional: `xml.encode(my_table)`) or a wrapper with
    // named keys (table-form from shell: `xml.encode({tree=my_table})`).
    // Resolution: try table-form first; if it fails, fall back to positional.
    if vals.len() == 1 {
        if let Value::Table(t) = &vals[0] {
            if params.len() == 1 && params[0].typ == ParamType::Table {
                // Ambiguous single-table case: try table-form, fall back to positional
                match validate_table_form(t, params, fn_name) {
                    Ok(result) => return Ok(result),
                    Err(_) => return Ok(vec![vals[0].clone()]),
                }
            }
            return validate_table_form(t, params, fn_name);
        }
    }

    // Positional form
    let mut result = Vec::with_capacity(params.len());
    for (i, param) in params.iter().enumerate() {
        let val = vals.get(i).copied().unwrap_or(&Value::Nil);
        if matches!(val, Value::Nil) {
            if param.required {
                return Err(arg_error(fn_name, params));
            }
            result.push(Value::Nil);
        } else if !param.typ.matches(val) {
            return Err(mlua::Error::external(format!(
                "{}: argument '{}' expected {}, got {}",
                fn_name,
                param.name,
                param.typ.label(),
                val.type_name()
            )));
        } else {
            result.push(val.clone());
        }
    }
    Ok(result)
}

/// Validate arguments passed as a single table with named keys.
fn validate_table_form(
    t: &mlua::Table,
    params: &[Param],
    fn_name: &str,
) -> Result<Vec<Value>, mlua::Error> {
    let mut result = Vec::with_capacity(params.len());
    for (i, param) in params.iter().enumerate() {
        // Try named key first, then positional index (1-based)
        let val: Value = t
            .get::<Value>(param.name)
            .ok()
            .filter(|v| !matches!(v, Value::Nil))
            .or_else(|| {
                t.get::<Value>(i + 1)
                    .ok()
                    .filter(|v| !matches!(v, Value::Nil))
            })
            .unwrap_or(Value::Nil);

        if matches!(val, Value::Nil) {
            if param.required {
                return Err(arg_error(fn_name, params));
            }
            result.push(Value::Nil);
        } else if !param.typ.matches(&val) {
            return Err(mlua::Error::external(format!(
                "{}: argument '{}' expected {}, got {}",
                fn_name,
                param.name,
                param.typ.label(),
                val.type_name()
            )));
        } else {
            result.push(val);
        }
    }
    Ok(result)
}

/// Build a clear error listing all missing required arguments.
pub(crate) fn arg_error(fn_name: &str, params: &[Param]) -> mlua::Error {
    let required: Vec<String> = params
        .iter()
        .filter(|p| p.required)
        .map(|p| format!("'{}' ({})", p.name, p.typ.label()))
        .collect();
    let list = match required.len() {
        0 => return mlua::Error::external(format!("{}: unknown argument error", fn_name)),
        1 => format!("missing required argument {}", required[0]),
        _ => format!("missing required arguments {}", required.join(" and ")),
    };
    mlua::Error::external(format!("{}: {}", fn_name, list))
}

#[cfg(feature = "mod-fs")]
#[cfg(any(
    feature = "mod-ripgrep",
    all(feature = "mod-fff", not(feature = "mod-ripgrep"))
))]
const FS_GREP_DESCRIPTION: &str = "Search file contents by regex or plain pattern. Searches recursively in directories (respects .gitignore). Returns table of matches.";

#[cfg(feature = "mod-fs")]
#[cfg(any(
    feature = "mod-ripgrep",
    all(feature = "mod-fff", not(feature = "mod-ripgrep"))
))]
const FS_GREP_OPTS_FIELDS: &[FieldDoc] = &[
    FieldDoc {
        name: "pattern",
        typ: "string",
        required: true,
        description: "Pattern to search for",
    },
    FieldDoc {
        name: "path",
        typ: "string",
        required: true,
        description: "File or directory to search",
    },
    FieldDoc {
        name: "mode",
        typ: "string",
        required: false,
        description: "Search mode: \"regex\" (default) or \"plain\"",
    },
    FieldDoc {
        name: "glob",
        typ: "string",
        required: false,
        description: "Glob filter for file names (e.g. \"*.rs\")",
    },
    FieldDoc {
        name: "max_count",
        typ: "number",
        required: false,
        description: "Maximum number of matches to return",
    },
    FieldDoc {
        name: "files_only",
        typ: "boolean",
        required: false,
        description: "Return only unique file paths (like rg -l)",
    },
];

#[cfg(feature = "mod-fs")]
const FS_READ_OPTS_FIELDS: &[FieldDoc] = &[
    FieldDoc {
        name: "mode",
        typ: "string",
        required: false,
        description: "Output mode: \"text\" (default), \"buffer\", or \"base64\"",
    },
    FieldDoc {
        name: "offset",
        typ: "number",
        required: false,
        description:
            "Text: 1-based first line (0 also means first); buffer/base64: 0-based byte offset",
    },
    FieldDoc {
        name: "limit",
        typ: "number",
        required: false,
        description: "Maximum number of lines in text mode",
    },
    FieldDoc {
        name: "count",
        typ: "number",
        required: false,
        description: "Maximum bytes to read in buffer or base64 mode",
    },
];

#[cfg(feature = "mod-fs")]
pub(crate) static FS_DOC: ModuleDoc = ModuleDoc {
    name: "fs",
    summary: "sandboxed filesystem (read, write, list, mkdir, copy, grep, tree, ...)",
    functions: &[
        FnDoc {
            name: "read",
            description: "Read file contents. Text mode returns a UTF-8 string and uses line offset/limit. Buffer mode returns a native Luau buffer; buffer and base64 modes use byte offset/count. Shell callers must use base64 for byte output. Shell aliases: -p/--path, -m/--mode, -o/--offset, -l/--limit, and -c/--count.",
            params: &[
                Param { name: "path", short: Some('p'), typ: ParamType::String, required: true, fields: None },
                Param { name: "opts", short: None, typ: ParamType::Table, required: false, fields: Some(FS_READ_OPTS_FIELDS) },
            ],
            returns: ReturnType::StringOrBuffer,
            example: Some(r#"local bytes = fs.read("/workspace/audio.wav", {mode="buffer", count=4096}); print(buffer.readu8(bytes, 0))"#),
        },
        FnDoc {
            name: "write",
            description: "Replace a file with bytes supplied as a string or native Luau buffer.",
            params: &[
                Param { name: "path", short: Some('p'), typ: ParamType::String, required: true, fields: None },
                Param { name: "content", short: Some('c'), typ: ParamType::StringOrBuffer, required: true, fields: None },
            ],
            returns: ReturnType::Void,
            example: Some(r#"fs.write("/artifacts/out.txt", "hello")"#),
        },
        FnDoc {
            name: "list",
            description: "List entries in a directory. Returns an array of entry name strings, not records.",
            params: &[Param { name: "path", short: Some('p'), typ: ParamType::String, required: true, fields: None }],
            returns: ReturnType::Table,
            example: Some(r#"for _, name in ipairs(fs.list("/workspace")) do print(name) end"#),
        },
        FnDoc {
            name: "exists",
            description: "Check if a path exists (file or directory).",
            params: &[Param { name: "path", short: Some('p'), typ: ParamType::String, required: true, fields: None }],
            returns: ReturnType::Boolean,
            example: None,
        },
        FnDoc {
            name: "writable",
            description: "Returns true if the path can be written to.",
            params: &[Param { name: "path", short: Some('p'), typ: ParamType::String, required: true, fields: None }],
            returns: ReturnType::Boolean,
            example: None,
        },
        FnDoc {
            name: "mkdir",
            description: "Create a directory and parents.",
            params: &[Param { name: "path", short: Some('p'), typ: ParamType::String, required: true, fields: None }],
            returns: ReturnType::Void,
            example: None,
        },
        FnDoc {
            name: "rename",
            description: "Rename/move a file or directory.",
            params: &[
                Param { name: "src", short: Some('s'), typ: ParamType::String, required: true, fields: None },
                Param { name: "dst", short: Some('d'), typ: ParamType::String, required: true, fields: None },
            ],
            returns: ReturnType::Void,
            example: None,
        },
        FnDoc {
            name: "remove",
            description: "Remove a file or directory (recursive).",
            params: &[Param { name: "path", short: Some('p'), typ: ParamType::String, required: true, fields: None }],
            returns: ReturnType::Void,
            example: None,
        },
        FnDoc {
            name: "isdir",
            description: "Check if a path is a directory.",
            params: &[Param { name: "path", short: Some('p'), typ: ParamType::String, required: true, fields: None }],
            returns: ReturnType::Boolean,
            example: None,
        },
        FnDoc {
            name: "isfile",
            description: "Check if a path is a file.",
            params: &[Param { name: "path", short: Some('p'), typ: ParamType::String, required: true, fields: None }],
            returns: ReturnType::Boolean,
            example: None,
        },
        FnDoc {
            name: "size",
            description: "Get the size of a file in bytes (without reading it).",
            params: &[Param { name: "path", short: Some('p'), typ: ParamType::String, required: true, fields: None }],
            returns: ReturnType::Number,
            example: None,
        },
        FnDoc {
            name: "copy",
            description: "Copy a file.",
            params: &[
                Param { name: "src", short: Some('s'), typ: ParamType::String, required: true, fields: None },
                Param { name: "dst", short: Some('d'), typ: ParamType::String, required: true, fields: None },
            ],
            returns: ReturnType::Void,
            example: Some(r#"fs.copy("/workspace/data.txt", "/artifacts/data.txt")"#),
        },
        #[cfg(any(feature = "mod-ripgrep", all(feature = "mod-fff", not(feature = "mod-ripgrep"))))]
        FnDoc {
            name: "grep",
            description: FS_GREP_DESCRIPTION,
            params: &[Param {
                name: "opts",
                short: None,
                typ: ParamType::Table,
                required: true,
                fields: Some(FS_GREP_OPTS_FIELDS),
            }],
            returns: ReturnType::Table,
            example: Some(r#"fs.grep({pattern="TODO", path="/workspace", glob="*.rs", max_count=20})"#),
        },
        #[cfg(feature = "mod-ripgrep")]
        FnDoc {
            name: "tree",
            description: "Display a directory tree.",
            params: &[Param {
                name: "opts",
                short: None,
                typ: ParamType::Table,
                required: true,
                fields: Some(&[
                    FieldDoc { name: "path", typ: "string", required: true, description: "Root directory path" },
                    FieldDoc { name: "depth", typ: "number", required: false, description: "Max depth (default 3)" },
                    FieldDoc { name: "dirs_only", typ: "boolean", required: false, description: "Show only directories" },
                    FieldDoc { name: "glob", typ: "string", required: false, description: "Only show files matching glob pattern (e.g. \"*.rs\")" },
                ]),
            }],
            returns: ReturnType::String,
            example: Some(r#"print(fs.tree({path="/workspace", depth=5, glob="*.rs"}))"#),
        },
    ],
};
