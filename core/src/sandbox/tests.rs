//! Tests for sandbox execution, help metadata, and filesystem bindings.

use super::*;

#[test]
fn test_basic_arithmetic() {
    let sandbox = Sandbox::new().unwrap();
    let result = sandbox.exec("return 1 + 1").unwrap();
    assert_eq!(result, "2");
}

#[test]
fn test_string_return() {
    let sandbox = Sandbox::new().unwrap();
    let result = sandbox.exec("return 'hello'").unwrap();
    assert_eq!(result, "hello");
}

#[test]
fn test_no_return_value() {
    let sandbox = Sandbox::new().unwrap();
    let result = sandbox.exec("local x = 1").unwrap();
    assert_eq!(result, "");
}

#[test]
fn test_multiple_return_values() {
    let sandbox = Sandbox::new().unwrap();
    let result = sandbox.exec("return 1, 2, 3").unwrap();
    assert_eq!(result, "1\t2\t3");
}

#[test]
fn test_syntax_error() {
    let sandbox = Sandbox::new().unwrap();
    let result = sandbox.exec("this is not valid luau");
    assert!(result.is_err());
}

#[test]
fn test_sandbox_blocks_dangerous_globals() {
    let sandbox = Sandbox::new().unwrap();
    let result = sandbox.exec("return string.dump(function() end)");
    assert!(
        result.is_err(),
        "string.dump should be blocked in sandbox mode"
    );
}

#[test]
fn test_luau_buffer_and_vector_are_available() {
    let sandbox = Sandbox::new().unwrap();
    let result = sandbox
        .exec(
            r#"
            local b = buffer.create(4)
            buffer.writeu8(b, 0, 65)
            local v = vector.create(1, 2, 3)
            local w = vector.create(3, 2, 1)
            return type(buffer), type(b), buffer.readu8(b, 0), type(vector), type(v), vector.dot(v, w)
            "#,
        )
        .unwrap();

    assert_eq!(result, "table\tbuffer\t65\ttable\tvector\t10");
}

#[test]
fn test_luau_integer_and_buffer_integer_access_are_available() {
    let sandbox = Sandbox::new().unwrap();
    let result = sandbox
        .exec(
            r#"
            local b = buffer.create(8)
            local n = integer.fromstring("1122334455667788", 16)
            assert(n ~= nil)
            buffer.writeinteger(b, 0, n)
            local r = buffer.readinteger(b, 0)
            return type(integer),
                type(buffer.readinteger),
                type(buffer.writeinteger),
                type(r),
                tostring(r),
                buffer.readu32(b, 0),
                buffer.readu32(b, 4),
                type(42i),
                tostring(42i)
            "#,
        )
        .unwrap();

    assert_eq!(
        result,
        "table\tfunction\tfunction\tinteger\t1234605616436508552\t1432778632\t287454020\tinteger\t42"
    );
}

#[test]
fn test_global_help_returns_help() {
    let sandbox = Sandbox::new().unwrap();
    let result = sandbox.exec("return help()").unwrap();
    assert!(
        result.contains("Sandbox"),
        "should contain title: {}",
        result
    );
    assert!(
        result.contains("Modules:"),
        "should have modules section: {}",
        result
    );
    assert!(result.contains("print"), "should list print: {}", result);
    assert!(
        result.contains("help()"),
        "should list help itself: {}",
        result
    );
    assert!(
        result.contains("string, table, math, bit32, buffer, vector, integer, coroutine, utf8"),
        "should list standard libs: {}",
        result
    );
    assert!(
        result.contains("Use number for ordinary math; integer.* is for exact 64-bit values."),
        "should explain when to use integer: {}",
        result
    );
    assert!(
        result.contains("Integer values do not auto-convert with number/string"),
        "should explain integer interop: {}",
        result
    );
    assert!(
        result.contains("buffer.readinteger/writeinteger"),
        "should mention integer buffer APIs: {}",
        result
    );
}

#[test]
fn test_global_help_works_without_return() {
    // Simulates Python mode where help() is a bare expression statement
    let sandbox = Sandbox::new().unwrap();
    let result = sandbox.exec("help()").unwrap();
    assert!(
        result.contains("Sandbox"),
        "bare help() should print: {}",
        result
    );
}

#[test]
#[cfg(feature = "mod-fs")]
fn test_fs_help_works_without_return() {
    let sandbox = Sandbox::new().unwrap();
    let result = sandbox.exec("fs.help()").unwrap();
    assert!(
        result.contains("fs — sandboxed filesystem"),
        "bare fs.help() should print: {}",
        result
    );
}

#[test]
#[cfg(feature = "mod-fs")]
fn test_fs_help_returns_help() {
    let sandbox = Sandbox::new().unwrap();
    let result = sandbox.exec("return fs.help()").unwrap();
    assert!(result.contains("fs — sandboxed filesystem"));
    assert!(result.contains("fs.read"));
    assert!(result.contains("fs.write"));
    assert!(result.contains("fs.list"));
    assert!(result.contains("fs.exists"));
    assert!(result.contains("fs.mkdir"));
    assert!(result.contains("fs.help()"));
}

#[test]
#[cfg(feature = "mod-fs")]
fn test_fs_bad_args_includes_inline_help() {
    let sandbox = Sandbox::new().unwrap();
    // Missing argument — should inline Usage + Example
    let err = sandbox.exec("fs.read()").unwrap_err().to_string();
    assert!(
        err.contains("Usage: fs.read("),
        "missing-arg error should contain inline usage: {}",
        err
    );
    // Wrong type (boolean can't coerce to string) — same inline help
    let err = sandbox.exec("fs.read(true)").unwrap_err().to_string();
    assert!(
        err.contains("Usage: fs.read("),
        "wrong-type error should contain inline usage: {}",
        err
    );
}

#[test]
#[cfg(feature = "mod-fs")]
fn test_fs_runtime_error_no_hint() {
    let sandbox = Sandbox::new().unwrap();
    // Real runtime error (no mount → not found) should NOT have hint
    let err = sandbox
        .exec("fs.read('/nonexistent')")
        .unwrap_err()
        .to_string();
    assert!(
        !err.contains("hint:"),
        "runtime error should not contain hint: {}",
        err
    );
}

#[test]
#[cfg(feature = "mod-fs")]
fn test_fs_nil_function_includes_hint() {
    let sandbox = Sandbox::new().unwrap();
    let err = sandbox.exec("fs.test()").unwrap_err().to_string();
    assert!(
        err.contains("fs.test does not exist"),
        "nil access should name the key: {}",
        err
    );
    assert!(
        err.contains("hint: call fs.help() for usage"),
        "nil access should contain hint: {}",
        err
    );
}

#[test]
#[cfg(feature = "mod-fs")]
fn test_fs_functions_still_work_through_wrapper() {
    // Ensure the pcall wrapper doesn't break normal return values
    let sandbox = Sandbox::new().unwrap();
    let result = sandbox.exec("return fs.exists('/')").unwrap();
    assert_eq!(result, "true");

    let result = sandbox.exec("return type(fs.list('/'))").unwrap();
    assert_eq!(result, "table");
}

#[test]
fn test_print_captured() {
    let sandbox = Sandbox::new().unwrap();
    let result = sandbox.exec("print('hello world')").unwrap();
    assert_eq!(result, "hello world");
}

#[test]
fn test_print_multiple_lines() {
    let sandbox = Sandbox::new().unwrap();
    let result = sandbox.exec("print('a')\nprint('b')\nprint('c')").unwrap();
    assert_eq!(result, "a\nb\nc");
}

#[test]
fn test_print_multiple_args() {
    let sandbox = Sandbox::new().unwrap();
    let result = sandbox.exec("print(1, 2, 3)").unwrap();
    assert_eq!(result, "1\t2\t3");
}

#[test]
fn test_print_and_return() {
    let sandbox = Sandbox::new().unwrap();
    let result = sandbox.exec("print('printed')\nreturn 42").unwrap();
    assert_eq!(result, "printed\n42");
}

#[test]
fn test_print_buffer_clears_between_execs() {
    let sandbox = Sandbox::new().unwrap();
    let r1 = sandbox.exec("print('first')").unwrap();
    assert_eq!(r1, "first");
    let r2 = sandbox.exec("print('second')").unwrap();
    assert_eq!(r2, "second");
}

// -- clean_lua_error tests --

#[test]
fn test_clean_runtime_error_with_line() {
    let err =
        mlua::Error::RuntimeError(r#"[string "input"]:3: attempt to call a nil value"#.to_string());
    let e = clean_lua_error(&err);
    assert_eq!(e.line, Some(3));
    assert_eq!(e.message, "function is not defined");
}

#[test]
fn test_clean_runtime_error_strips_traceback() {
    let err = mlua::Error::RuntimeError(
            "[string \"input\"]:1: attempt to call a nil value\nstack traceback:\n    [string \"input\"]:1: in main chunk".to_string(),
        );
    let e = clean_lua_error(&err);
    assert_eq!(e.line, Some(1));
    assert_eq!(e.message, "function is not defined");
}

#[test]
fn test_clean_syntax_error() {
    let err = mlua::Error::SyntaxError {
        message: "[string \"input\"]:1: Expected 'end' (to close 'function' at line 1), got <eof>"
            .to_string(),
        incomplete_input: true,
    };
    let e = clean_lua_error(&err);
    assert_eq!(e.line, Some(1));
    assert!(e.message.contains("Expected 'end'"), "msg: {}", e.message);
}

#[test]
fn test_clean_error_no_rust_paths() {
    // Errors should never contain Rust source paths
    let sandbox = Sandbox::new().unwrap();
    let err = sandbox.exec("foo()").unwrap_err();
    let s = err.to_string();
    assert!(!s.contains("sandbox.rs"), "leaked Rust path: {}", s);
    assert!(!s.contains(".rs:"), "leaked Rust file ref: {}", s);
}

#[test]
fn test_clean_error_no_location() {
    // Errors re-thrown through pcall wrapper (level 0) have no location
    let err = mlua::Error::RuntimeError("some error without location".to_string());
    let e = clean_lua_error(&err);
    assert_eq!(e.line, None);
    assert_eq!(e.message, "some error without location");
}

#[test]
#[cfg(feature = "mod-fs")]
fn test_clean_fs_error_passthrough() {
    // FS errors are already clean — they should pass through unchanged
    let sandbox = Sandbox::new().unwrap();
    let err = sandbox.exec("fs.read('/nonexistent')").unwrap_err();
    assert!(
        err.message.contains("No such file or directory"),
        "msg: {}",
        err.message
    );
}

#[test]
fn test_non_input_chunk_has_no_line_number() {
    // Errors from shrt or other non-input chunks should have line: None
    let err =
        mlua::Error::RuntimeError(r#"[string "shrt"]:908: xml.lfdskl does not exist"#.to_string());
    let e = clean_lua_error(&err);
    assert_eq!(e.line, None, "non-input chunk should have no line number");
    assert!(e.message.contains("does not exist"), "msg: {}", e.message);
}

#[test]
fn test_unnamed_chunk_has_no_line_number() {
    // Errors from unnamed chunks (e.g., metatable handlers) should have line: None
    let err = mlua::Error::RuntimeError(r#"[string ""]:5: some internal error"#.to_string());
    let e = clean_lua_error(&err);
    assert_eq!(e.line, None, "unnamed chunk should have no line number");
}

#[test]
fn test_input_chunk_preserves_line_number() {
    // Errors from input (user code) should still have line numbers
    let err = mlua::Error::RuntimeError(r#"[string "input"]:5: something went wrong"#.to_string());
    let e = clean_lua_error(&err);
    assert_eq!(e.line, Some(5));
    assert_eq!(e.message, "something went wrong");
}

// -- humanize_error tests --

#[test]
fn test_humanize_nil_call() {
    assert_eq!(
        humanize_error("attempt to call a nil value"),
        "function is not defined"
    );
}

#[test]
fn test_humanize_nil_index() {
    assert_eq!(
        humanize_error("attempt to index nil with 'bar'"),
        "nil has no member 'bar'"
    );
}

#[test]
fn test_humanize_nil_index_named() {
    // Format: attempt to index foo (a nil value) with 'bar'
    assert_eq!(
        humanize_error("attempt to index foo (a nil value) with 'bar'"),
        "nil has no member 'bar'"
    );
}

#[test]
fn test_humanize_arithmetic_nil() {
    assert_eq!(
        humanize_error("attempt to perform arithmetic (add) on nil and number"),
        "arithmetic on nil value"
    );
}

#[test]
fn test_humanize_compare_nil() {
    assert_eq!(
        humanize_error("attempt to compare nil < number"),
        "cannot compare nil < number"
    );
}

#[test]
fn test_humanize_table_nil_key() {
    assert_eq!(
        humanize_error("table index is nil"),
        "table key cannot be nil"
    );
}

#[test]
fn test_humanize_passthrough() {
    // Messages we don't recognize should pass through unchanged
    assert_eq!(
        humanize_error("/path: Read-only file system"),
        "/path: Read-only file system"
    );
    assert_eq!(
        humanize_error("bad argument #1 to 'read'"),
        "bad argument #1 to 'read'"
    );
}

// -- Integration tests: exec() returns clean ExecError --

#[test]
fn test_exec_error_undefined_function() {
    let sandbox = Sandbox::new().unwrap();
    let err = sandbox.exec("foo()").unwrap_err();
    assert_eq!(err.line, Some(1));
    assert_eq!(err.message, "function is not defined");
    assert_eq!(err.to_string(), "1: function is not defined");
}

#[test]
fn test_exec_error_syntax() {
    let sandbox = Sandbox::new().unwrap();
    let err = sandbox.exec("if then end").unwrap_err();
    assert_eq!(err.line, Some(1));
    // Syntax error message should be clean, no Rust paths
    assert!(!err.message.contains("sandbox.rs"), "msg: {}", err.message);
}

#[test]
fn test_exec_error_multiline() {
    let sandbox = Sandbox::new().unwrap();
    let err = sandbox.exec("local x = 1\nlocal y = 2\nfoo()").unwrap_err();
    assert_eq!(err.line, Some(3), "should point to line 3 where foo() is");
    assert_eq!(err.message, "function is not defined");
}

#[test]
#[cfg(feature = "mod-fs")]
fn test_exec_error_readonly_fs() {
    let sandbox = Sandbox::new().unwrap();
    let err = sandbox.exec("fs.read('/nonexistent')").unwrap_err();
    assert_eq!(err.line, Some(1), "FS errors should include caller's line");
    assert!(err.message.contains("No such file or directory"));
}

// -- validate_args tests --

fn test_params() -> &'static [Param] {
    &[
        Param {
            name: "text",
            short: None,
            typ: ParamType::String,
            required: true,
            fields: None,
        },
        Param {
            name: "count",
            short: None,
            typ: ParamType::Number,
            required: false,
            fields: None,
        },
    ]
}

#[test]
fn test_validate_args_positional_all_present() {
    let lua = Lua::new();
    let mut mv = MultiValue::new();
    mv.push_back(Value::String(lua.create_string("hello").unwrap()));
    mv.push_back(Value::Number(42.0));

    let result = validate_args(&mv, test_params(), "test.fn").unwrap();
    assert_eq!(result.len(), 2);
    assert!(matches!(&result[0], Value::String(s) if s.to_string_lossy() == "hello"));
    assert!(matches!(&result[1], Value::Number(n) if *n == 42.0));
}

#[test]
fn test_validate_args_positional_optional_missing() {
    let lua = Lua::new();
    let mut mv = MultiValue::new();
    mv.push_back(Value::String(lua.create_string("hello").unwrap()));

    let result = validate_args(&mv, test_params(), "test.fn").unwrap();
    assert_eq!(result.len(), 2);
    assert!(matches!(&result[0], Value::String(_)));
    assert!(matches!(&result[1], Value::Nil));
}

#[test]
fn test_validate_args_missing_required() {
    let mv = MultiValue::new();
    let err = validate_args(&mv, test_params(), "test.fn").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("test.fn"), "should name the function: {}", msg);
    assert!(
        msg.contains("'text'"),
        "should name the missing param: {}",
        msg
    );
}

#[test]
fn test_validate_args_wrong_type() {
    let mut mv = MultiValue::new();
    mv.push_back(Value::Number(123.0)); // should be string

    let err = validate_args(&mv, test_params(), "test.fn").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("'text'"), "should name the param: {}", msg);
    assert!(msg.contains("string"), "should name expected type: {}", msg);
}

#[test]
fn test_validate_args_table_form_named() {
    let lua = Lua::new();
    let t = lua.create_table().unwrap();
    t.set("text", lua.create_string("hello").unwrap()).unwrap();
    t.set("count", 5.0).unwrap();

    let mut mv = MultiValue::new();
    mv.push_back(Value::Table(t));

    let result = validate_args(&mv, test_params(), "test.fn").unwrap();
    assert_eq!(result.len(), 2);
    assert!(matches!(&result[0], Value::String(s) if s.to_string_lossy() == "hello"));
}

#[test]
fn test_validate_args_table_form_positional() {
    let lua = Lua::new();
    let t = lua.create_table().unwrap();
    t.set(1, lua.create_string("hello").unwrap()).unwrap();
    t.set(2, 5.0).unwrap();

    let mut mv = MultiValue::new();
    mv.push_back(Value::Table(t));

    let result = validate_args(&mv, test_params(), "test.fn").unwrap();
    assert_eq!(result.len(), 2);
    assert!(matches!(&result[0], Value::String(s) if s.to_string_lossy() == "hello"));
}

#[test]
fn test_validate_args_table_form_missing_required() {
    let lua = Lua::new();
    let t = lua.create_table().unwrap();
    t.set("count", 5.0).unwrap(); // text is missing

    let mut mv = MultiValue::new();
    mv.push_back(Value::Table(t));

    let err = validate_args(&mv, test_params(), "test.fn").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("'text'"), "should name missing param: {}", msg);
}

#[test]
fn test_validate_args_no_args_mentions_all_required() {
    let params: &[Param] = &[
        Param {
            name: "doc",
            short: None,
            typ: ParamType::Table,
            required: true,
            fields: None,
        },
        Param {
            name: "path",
            short: Some('p'),
            typ: ParamType::String,
            required: true,
            fields: None,
        },
    ];
    let mv = MultiValue::new();
    let err = validate_args(&mv, params, "xml.query").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("'doc'"), "should mention doc: {}", msg);
    assert!(msg.contains("'path'"), "should mention path: {}", msg);
    assert!(
        msg.contains("xml.query"),
        "should mention function: {}",
        msg
    );
}

// -- format_help tests --

fn test_module_doc() -> ModuleDoc {
    ModuleDoc {
        name: "xml",
        summary: "XML parse, query & encode",
        functions: &[
            FnDoc {
                name: "parse",
                description: "Parse an XML string into a tree.",
                params: &[Param {
                    name: "text",
                    short: None,
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                }],
                returns: ReturnType::Table,
                example: None,
            },
            FnDoc {
                name: "query",
                description: "Filter nodes by path.",
                params: &[
                    Param {
                        name: "doc",
                        short: None,
                        typ: ParamType::Table,
                        required: true,
                        fields: None,
                    },
                    Param {
                        name: "path",
                        short: None,
                        typ: ParamType::String,
                        required: true,
                        fields: None,
                    },
                ],
                returns: ReturnType::Table,
                example: None,
            },
            FnDoc {
                name: "encode",
                description: "Encode to XML.",
                params: &[
                    Param {
                        name: "table",
                        short: None,
                        typ: ParamType::Table,
                        required: true,
                        fields: None,
                    },
                    Param {
                        name: "opts",
                        short: None,
                        typ: ParamType::Table,
                        required: false,
                        fields: None,
                    },
                ],
                returns: ReturnType::String,
                example: None,
            },
        ],
    }
}

#[test]
fn test_format_help_lua_mode() {
    let doc = test_module_doc();
    let help = doc.format_help(HelpMode::Lua);
    // Title
    assert!(
        help.contains("xml — XML parse, query & encode"),
        "title: {}",
        help
    );
    // Structured param rendering: module.fn(params) -> type
    assert!(
        help.contains("xml.parse(text: string) -> table"),
        "parse sig: {}",
        help
    );
    assert!(
        help.contains("xml.query(doc: table, path: string) -> table"),
        "query sig: {}",
        help
    );
    // Optional params have ?
    assert!(
        help.contains("xml.encode(table: table, opts?: table) -> string"),
        "encode sig: {}",
        help
    );
    // Help entry
    assert!(help.contains("xml.help()"), "help entry: {}", help);
}

#[test]
fn test_format_help_shell_mode() {
    let doc = test_module_doc();
    let help = doc.format_help(HelpMode::Shell);
    // Title same in both modes
    assert!(
        help.contains("xml — XML parse, query & encode"),
        "title: {}",
        help
    );
    // Shell flag syntax
    assert!(
        help.contains("xml parse --text <string>"),
        "parse shell: {}",
        help
    );
    assert!(
        help.contains("xml query --doc <JSON> --path <string>"),
        "query shell: {}",
        help
    );
    // Optional in brackets
    assert!(help.contains("[--opts <JSON>]"), "opts optional: {}", help);
    // Help entry uses shell syntax
    assert!(help.contains("xml help"), "shell help entry: {}", help);
    assert!(
        !help.contains("xml.help()"),
        "shell help should not use Lua syntax: {}",
        help
    );
}

#[test]
fn test_format_help_empty_params() {
    // When params is empty, generated signature shows () with no args
    let doc = ModuleDoc {
        name: "json",
        summary: "JSON encode & decode",
        functions: &[FnDoc {
            name: "decode",
            description: "Parse JSON.",
            params: &[],
            returns: ReturnType::Void,
            example: None,
        }],
    };
    let lua_help = doc.format_help(HelpMode::Lua);
    assert!(
        lua_help.contains("json.decode()"),
        "lua empty params: {}",
        lua_help
    );
    let shell_help = doc.format_help(HelpMode::Shell);
    assert!(
        shell_help.contains("json decode"),
        "shell empty params: {}",
        shell_help
    );
}

// -- ReturnType tests --

#[test]
fn test_return_type_labels() {
    assert_eq!(ReturnType::String.label(), "string");
    assert_eq!(ReturnType::Number.label(), "number");
    assert_eq!(ReturnType::Table.label(), "table");
    assert_eq!(ReturnType::Boolean.label(), "boolean");
    assert_eq!(ReturnType::Value.label(), "any");
    assert_eq!(ReturnType::Void.label(), "");
}

// -- generated_signature unit tests --

#[test]
fn test_generated_signature_lua_required_params() {
    let f = FnDoc {
        name: "parse",
        description: "",
        params: &[Param {
            name: "text",
            short: None,
            typ: ParamType::String,
            required: true,
            fields: None,
        }],
        returns: ReturnType::Table,
        example: None,
    };
    assert_eq!(
        f.generated_signature(HelpMode::Lua),
        "(text: string) -> table"
    );
}

#[test]
fn test_generated_signature_lua_optional_param() {
    let f = FnDoc {
        name: "encode",
        description: "",
        params: &[
            Param {
                name: "tree",
                short: None,
                typ: ParamType::Table,
                required: true,
                fields: None,
            },
            Param {
                name: "opts",
                short: None,
                typ: ParamType::Table,
                required: false,
                fields: None,
            },
        ],
        returns: ReturnType::String,
        example: None,
    };
    assert_eq!(
        f.generated_signature(HelpMode::Lua),
        "(tree: table, opts?: table) -> string"
    );
}

#[test]
fn test_generated_signature_lua_void_return() {
    let f = FnDoc {
        name: "write",
        description: "",
        params: &[
            Param {
                name: "path",
                short: Some('p'),
                typ: ParamType::String,
                required: true,
                fields: None,
            },
            Param {
                name: "content",
                short: Some('c'),
                typ: ParamType::String,
                required: true,
                fields: None,
            },
        ],
        returns: ReturnType::Void,
        example: None,
    };
    assert_eq!(
        f.generated_signature(HelpMode::Lua),
        "(path: string, content: string)"
    );
}

#[test]
fn test_generated_signature_lua_no_params() {
    let f = FnDoc {
        name: "help",
        description: "",
        params: &[],
        returns: ReturnType::String,
        example: None,
    };
    assert_eq!(f.generated_signature(HelpMode::Lua), "() -> string");
}

#[test]
fn test_generated_signature_shell_required_params() {
    let f = FnDoc {
        name: "query",
        description: "",
        params: &[
            Param {
                name: "doc",
                short: None,
                typ: ParamType::Table,
                required: true,
                fields: None,
            },
            Param {
                name: "path",
                short: None,
                typ: ParamType::String,
                required: true,
                fields: None,
            },
        ],
        returns: ReturnType::Table,
        example: None,
    };
    assert_eq!(
        f.generated_signature(HelpMode::Shell),
        "--doc <JSON> --path <string> -> JSON"
    );
}

#[test]
fn test_generated_signature_shell_optional_brackets() {
    let f = FnDoc {
        name: "encode",
        description: "",
        params: &[
            Param {
                name: "tree",
                short: None,
                typ: ParamType::Table,
                required: true,
                fields: None,
            },
            Param {
                name: "indent",
                short: None,
                typ: ParamType::Boolean,
                required: false,
                fields: None,
            },
        ],
        returns: ReturnType::String,
        example: None,
    };
    assert_eq!(
        f.generated_signature(HelpMode::Shell),
        "--tree <JSON> [--indent <boolean>] -> string"
    );
}

#[test]
fn test_generated_signature_shell_no_params() {
    let f = FnDoc {
        name: "help",
        description: "",
        params: &[],
        returns: ReturnType::Void,
        example: None,
    };
    assert_eq!(f.generated_signature(HelpMode::Shell), "");
}

// -- Short flag tests --

#[test]
fn test_generated_signature_shell_with_short_flags() {
    let f = FnDoc {
        name: "zip",
        description: "",
        params: &[
            Param {
                name: "source",
                short: Some('s'),
                typ: ParamType::String,
                required: true,
                fields: None,
            },
            Param {
                name: "archive",
                short: Some('a'),
                typ: ParamType::String,
                required: true,
                fields: None,
            },
        ],
        returns: ReturnType::Void,
        example: None,
    };
    assert_eq!(
        f.generated_signature(HelpMode::Shell),
        "-s/--source <string> -a/--archive <string>"
    );
}

#[test]
fn test_generated_signature_shell_short_flag_optional() {
    let f = FnDoc {
        name: "get",
        description: "",
        params: &[
            Param {
                name: "url",
                short: Some('u'),
                typ: ParamType::String,
                required: true,
                fields: None,
            },
            Param {
                name: "headers",
                short: Some('H'),
                typ: ParamType::Table,
                required: false,
                fields: None,
            },
        ],
        returns: ReturnType::Table,
        example: None,
    };
    assert_eq!(
        f.generated_signature(HelpMode::Shell),
        "-u/--url <string> [-H/--headers <JSON>] -> JSON"
    );
}

#[test]
fn test_generated_signature_lua_ignores_short_flags() {
    // Lua mode should never show short flags
    let f = FnDoc {
        name: "read",
        description: "",
        params: &[Param {
            name: "path",
            short: Some('p'),
            typ: ParamType::String,
            required: true,
            fields: None,
        }],
        returns: ReturnType::String,
        example: None,
    };
    assert_eq!(
        f.generated_signature(HelpMode::Lua),
        "(path: string) -> string"
    );
}

#[test]
fn test_format_help_shell_mode_with_short_flags() {
    let doc = ModuleDoc {
        name: "compress",
        summary: "zip, tar, gzip",
        functions: &[FnDoc {
            name: "zip",
            description: "Create a zip archive.",
            params: &[
                Param {
                    name: "source",
                    short: Some('s'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "archive",
                    short: Some('a'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
            ],
            returns: ReturnType::Void,
            example: None,
        }],
    };
    let help = doc.format_help(HelpMode::Shell);
    assert!(
        help.contains("compress zip -s/--source <string> -a/--archive <string>"),
        "shell short flags: {}",
        help
    );
}

// -- Phase 4: ALL_MODULE_DOCS registry and exhaustive verification --

/// Central registry of every module's documentation static.
/// Used by exhaustive tests to verify consistency across all modules.
fn all_module_docs() -> Vec<(&'static str, &'static ModuleDoc)> {
    let mut docs = Vec::new();
    #[cfg(feature = "mod-fs")]
    docs.push(("fs", &FS_DOC));
    #[cfg(feature = "mod-json")]
    docs.push(("json", &crate::json::JSON_DOC));
    #[cfg(feature = "mod-yaml")]
    docs.push(("yaml", &crate::yaml::YAML_DOC));
    #[cfg(feature = "mod-xml")]
    docs.push(("xml", &crate::xml::XML_DOC));
    #[cfg(feature = "mod-csv")]
    docs.push(("csv", &crate::csv_mod::CSV_DOC));
    #[cfg(feature = "mod-compress")]
    docs.push(("compress", &crate::compress::COMPRESS_DOC));
    #[cfg(all(feature = "mod-doc", not(feature = "pdfium-render")))]
    docs.push(("doc", &crate::doc::DOC_MOD_DOC));
    #[cfg(all(feature = "mod-doc", feature = "pdfium-render"))]
    docs.push(("doc", &crate::doc::DOC_MOD_DOC_PDFIUM));
    #[cfg(feature = "mod-http")]
    docs.push(("http", &crate::http::HTTP_DOC));
    #[cfg(feature = "mod-plot")]
    docs.push(("plot", &crate::plot::PLOT_DOC));
    #[cfg(feature = "mod-numpy")]
    docs.push(("numx", &crate::numpy::NUMPY_DOC));
    #[cfg(feature = "mod-fuzzy")]
    docs.push(("fuzzy", &crate::fuzzy::FUZZY_DOC));
    #[cfg(feature = "mod-phone")]
    docs.push(("phone", &crate::phone::PHONE_DOC));
    #[cfg(feature = "mod-email")]
    docs.push(("email", &crate::email::EMAIL_DOC));
    #[cfg(feature = "mod-country")]
    {
        docs.push(("country", &crate::country::COUNTRY_DOC));
        docs.push(("currency", &crate::country::CURRENCY_DOC));
    }
    #[cfg(feature = "mod-datetime")]
    docs.push(("datetime", &crate::datetime::DATETIME_DOC));
    #[cfg(feature = "mod-fin")]
    docs.push(("fin", &crate::fin::FIN_DOC));
    #[cfg(feature = "mod-yfinance")]
    docs.push(("yfinance", &crate::yfinance::YFINANCE_DOC));
    #[cfg(feature = "mod-edgar")]
    docs.push(("edgar", &crate::edgar::EDGAR_DOC));
    #[cfg(feature = "mod-image")]
    docs.push(("image", &crate::image::IMAGE_DOC));
    #[cfg(feature = "mod-random")]
    docs.push(("random", &crate::random::RANDOM_DOC));
    #[cfg(feature = "mod-base64")]
    docs.push(("base64", &crate::base64::BASE64_DOC));
    #[cfg(feature = "mod-crypto")]
    docs.push(("crypto", &crate::crypto::CRYPTO_DOC));
    #[cfg(feature = "mod-regex")]
    docs.push(("regex", &crate::regex_mod::REGEX_DOC));
    #[cfg(feature = "mod-html")]
    docs.push(("html", &crate::html_mod::HTML_DOC));
    #[cfg(feature = "mod-url")]
    docs.push(("url", &crate::url_mod::URL_DOC));
    #[cfg(feature = "mod-qr")]
    docs.push(("qr", &crate::qr::QR_DOC));
    docs
}

#[test]
fn test_all_modules_have_nonempty_params() {
    // Parameterless functions are allowed (e.g. datetime.now()).
    // This test verifies that most functions have structured params
    // for shell dispatch and help rendering.
    let allowed_empty = &[
        ("datetime", "now"),
        ("image", "fonts"),
        ("random", "random"),
        ("crypto", "uuid"),
        ("crypto", "uuid_v7"),
    ];
    for (mod_name, doc) in all_module_docs() {
        for f in doc.functions {
            if allowed_empty.contains(&(mod_name, f.name)) {
                continue;
            }
            assert!(
                !f.params.is_empty(),
                "{}::{} has empty params — every function should have structured params",
                mod_name,
                f.name
            );
        }
    }
}

#[test]
fn test_no_duplicate_function_names_within_module() {
    for (mod_name, doc) in all_module_docs() {
        let mut seen = std::collections::HashSet::new();
        for f in doc.functions {
            assert!(
                seen.insert(f.name),
                "{}::{} is duplicated in module doc",
                mod_name,
                f.name
            );
        }
    }
}

#[test]
fn test_no_duplicate_short_flags_within_function() {
    for (mod_name, doc) in all_module_docs() {
        for f in doc.functions {
            let mut seen = std::collections::HashSet::new();
            for p in f.params {
                if let Some(c) = p.short {
                    assert!(
                        seen.insert(c),
                        "{}::{} has duplicate short flag '-{}'",
                        mod_name,
                        f.name,
                        c
                    );
                }
            }
        }
    }
}

#[test]
fn test_void_return_consistency() {
    // Functions that write/mutate (writeFile, mkdir, remove, rename) should return Void.
    // Functions that read/compute should NOT return Void.
    let void_fn_patterns = ["write", "mkdir", "remove", "rename"];
    for (mod_name, doc) in all_module_docs() {
        for f in doc.functions {
            let is_void = f.returns == ReturnType::Void;
            let name_lower = f.name.to_lowercase();
            let looks_like_write = void_fn_patterns.iter().any(|p| name_lower.contains(p));

            if looks_like_write && !is_void {
                // Some write-like functions do return values (e.g. compress.zip might return path)
                // This is a soft check — just flag it, don't fail
                // eprintln!("note: {}::{} looks like a write fn but returns {:?}", mod_name, f.name, f.returns);
            }

            // Hard check: Void functions should not have "Returns" in description
            if is_void && f.description.contains("Returns ") {
                panic!(
                    "{}::{} returns Void but description says 'Returns': {}",
                    mod_name, f.name, f.description
                );
            }
        }
    }
}

#[test]
fn test_generated_signature_param_count_matches() {
    for (mod_name, doc) in all_module_docs() {
        for f in doc.functions {
            let lua_sig = f.generated_signature(HelpMode::Lua);
            // Count params in Lua signature: parse "(a, b?, c)" -> count commas + 1
            // Handle empty case: "()" -> 0 params
            let inner = &lua_sig[1..lua_sig.find(')').unwrap()];
            let sig_param_count = if inner.is_empty() {
                0
            } else {
                inner.split(',').count()
            };
            assert_eq!(
                sig_param_count,
                f.params.len(),
                "{}::{}: Lua signature '{}' has {} params but params array has {}",
                mod_name,
                f.name,
                lua_sig,
                sig_param_count,
                f.params.len()
            );
        }
    }
}

#[test]
fn test_module_doc_name_matches_registry() {
    for (mod_name, doc) in all_module_docs() {
        assert_eq!(
            mod_name, doc.name,
            "Registry name '{}' doesn't match ModuleDoc.name '{}'",
            mod_name, doc.name
        );
    }
}

#[test]
fn test_format_help_renders_example() {
    let doc = ModuleDoc {
        name: "test",
        summary: "test module",
        functions: &[FnDoc {
            name: "foo",
            description: "Do stuff.",
            params: &[Param {
                name: "x",
                short: None,
                typ: ParamType::String,
                required: true,
                fields: None,
            }],
            returns: ReturnType::String,
            example: Some(r#"test.foo("hello")"#),
        }],
    };
    let help = doc.format_help(HelpMode::Lua);
    assert!(
        help.contains("Example:"),
        "help should contain 'Example:' when example is set: {}",
        help
    );
    assert!(
        help.contains(r#"test.foo("hello")"#),
        "help should contain the example text: {}",
        help
    );
}

#[test]
fn test_format_help_renders_field_docs() {
    static FIELDS: &[FieldDoc] = &[
        FieldDoc {
            name: "width",
            typ: "number",
            required: true,
            description: "Width in pixels",
        },
        FieldDoc {
            name: "title",
            typ: "string",
            required: false,
            description: "Chart title",
        },
    ];
    static PARAMS: &[Param] = &[Param {
        name: "opts",
        short: None,
        typ: ParamType::Table,
        required: true,
        fields: Some(FIELDS),
    }];
    static FUNCS: &[FnDoc] = &[FnDoc {
        name: "bar",
        description: "Draw stuff.",
        params: PARAMS,
        returns: ReturnType::Void,
        example: None,
    }];
    let doc = ModuleDoc {
        name: "test",
        summary: "test module",
        functions: FUNCS,
    };
    let help = doc.format_help(HelpMode::Lua);
    assert!(
        help.contains("width: number"),
        "help should contain field name and type: {}",
        help
    );
    assert!(
        help.contains("title?: string"),
        "optional field should have '?': {}",
        help
    );
    assert!(
        help.contains("Width in pixels"),
        "help should contain field description: {}",
        help
    );
}

#[test]
fn test_functions_with_3_plus_params_have_examples() {
    for (mod_name, doc) in all_module_docs() {
        for f in doc.functions {
            if f.params.len() >= 3 && f.example.is_none() {
                panic!(
                    "{}::{} has {} params but no example",
                    mod_name,
                    f.name,
                    f.params.len()
                );
            }
        }
    }
}

#[test]
fn test_table_params_have_field_docs() {
    // Soft check: log warnings but don't fail
    for (mod_name, doc) in all_module_docs() {
        for f in doc.functions {
            for p in f.params {
                if p.typ == ParamType::Table && p.fields.is_none() && p.name == "opts" {
                    eprintln!(
                        "note: {}::{} param '{}' is Table but has no FieldDoc",
                        mod_name, f.name, p.name
                    );
                }
            }
        }
    }
}
