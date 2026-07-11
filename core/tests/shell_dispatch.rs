#![cfg(feature = "all")]

//! Phase 6: Shell round-trip tests for all modules.
//!
//! Tests that every registered module works end-to-end from shell syntax via sh.run().

use cpsl_core::{sh_transpile, MountTable, Sandbox};

fn sb() -> Sandbox {
    Sandbox::new().unwrap()
}

fn sb_with_shell() -> Sandbox {
    let s = sb();
    let shrt = include_str!("../../runtime/shrt.luau");
    s.register_module("shrt", shrt).unwrap();
    s
}

fn sb_with_shell_and_mounts(mounts: MountTable) -> Sandbox {
    let s = Sandbox::with_mounts(mounts).unwrap();
    let shrt = include_str!("../../runtime/shrt.luau");
    s.register_module("shrt", shrt).unwrap();
    s
}

// ── 6a: Bare module name → help() ─────────────────────────────

#[test]
fn bare_fs_shows_help() {
    let s = sb_with_shell();
    let luau = sh_transpile::transpile_sh("fs").unwrap().luau_source;
    let r = s.exec(&luau).unwrap();
    assert!(r.contains("fs"), "bare fs should show help, got: {}", r);
}

#[test]
fn bare_json_shows_help() {
    let s = sb_with_shell();
    let luau = sh_transpile::transpile_sh("json").unwrap().luau_source;
    let r = s.exec(&luau).unwrap();
    assert!(r.contains("json"), "bare json should show help, got: {}", r);
}

#[test]
fn bare_csv_shows_help() {
    let s = sb_with_shell();
    let luau = sh_transpile::transpile_sh("csv").unwrap().luau_source;
    let r = s.exec(&luau).unwrap();
    assert!(r.contains("csv"), "bare csv should show help, got: {}", r);
}

#[test]
fn bare_yaml_shows_help() {
    let s = sb_with_shell();
    let luau = sh_transpile::transpile_sh("yaml").unwrap().luau_source;
    let r = s.exec(&luau).unwrap();
    assert!(r.contains("yaml"), "bare yaml should show help, got: {}", r);
}

#[test]
fn bare_xml_shows_help() {
    let s = sb_with_shell();
    let luau = sh_transpile::transpile_sh("xml").unwrap().luau_source;
    let r = s.exec(&luau).unwrap();
    assert!(r.contains("xml"), "bare xml should show help, got: {}", r);
}

#[test]
fn bare_plot_shows_help() {
    let dir = tempfile::TempDir::new().unwrap();
    let mut mt = MountTable::new();
    mt.parse_and_add(&format!("{}:/workspace", dir.path().display()))
        .unwrap();
    let s = sb_with_shell_and_mounts(mt);
    let luau = sh_transpile::transpile_sh("plot").unwrap().luau_source;
    let r = s.exec(&luau).unwrap();
    assert!(r.contains("plot"), "bare plot should show help, got: {}", r);
}

#[test]
fn bare_doc_shows_help() {
    let s = sb_with_shell();
    let luau = sh_transpile::transpile_sh("doc").unwrap().luau_source;
    let r = s.exec(&luau).unwrap();
    assert!(r.contains("doc"), "bare doc should show help, got: {}", r);
}

#[test]
fn bare_compress_shows_help() {
    let s = sb_with_shell();
    let luau = sh_transpile::transpile_sh("compress").unwrap().luau_source;
    let r = s.exec(&luau).unwrap();
    assert!(
        r.contains("compress"),
        "bare compress should show help, got: {}",
        r
    );
}

// ── 6a-flags: --help / -h on modules → help ───────────────────

#[test]
fn fs_dashdash_help_shows_help() {
    let s = sb_with_shell();
    let luau = sh_transpile::transpile_sh("fs --help").unwrap().luau_source;
    let r = s.exec(&luau).unwrap();
    assert!(r.contains("fs"), "fs --help should show help, got: {}", r);
}

#[test]
fn json_dash_h_shows_help() {
    let s = sb_with_shell();
    let luau = sh_transpile::transpile_sh("json -h").unwrap().luau_source;
    let r = s.exec(&luau).unwrap();
    assert!(r.contains("json"), "json -h should show help, got: {}", r);
}

#[test]
fn xml_dashdash_help_shows_help() {
    let s = sb_with_shell();
    let luau = sh_transpile::transpile_sh("xml --help")
        .unwrap()
        .luau_source;
    let r = s.exec(&luau).unwrap();
    assert!(r.contains("xml"), "xml --help should show help, got: {}", r);
}

// ── 6b: module method args → correct dispatch ─────────────────

#[test]
fn shell_json_decode_dispatch() {
    let s = sb_with_shell();
    let luau = sh_transpile::transpile_sh(r#"json decode '{"a":1}'"#)
        .unwrap()
        .luau_source;
    let r = s.exec(&luau).unwrap();
    assert!(r.contains("a"), "should display decoded JSON, got: {}", r);
}

#[test]
fn shell_yaml_decode_dispatch() {
    let s = sb_with_shell();
    let luau = sh_transpile::transpile_sh(r#"yaml decode "key: value""#)
        .unwrap()
        .luau_source;
    let r = s.exec(&luau).unwrap();
    assert!(r.contains("key"), "should display decoded YAML, got: {}", r);
}

#[test]
fn shell_xml_parse_dispatch() {
    let s = sb_with_shell();
    let luau = sh_transpile::transpile_sh(r#"xml parse "<root>hi</root>""#)
        .unwrap()
        .luau_source;
    let r = s.exec(&luau).unwrap();
    assert!(r.contains("root"), "should display parsed XML, got: {}", r);
}

#[test]
fn shell_csv_parse_dispatch() {
    let s = sb_with_shell();
    let luau = sh_transpile::transpile_sh(r#"csv parse "a,b""#)
        .unwrap()
        .luau_source;
    let r = s.exec(&luau).unwrap();
    assert!(
        r.contains("a") && r.contains("b"),
        "should display parsed CSV, got: {}",
        r
    );
}

#[test]
fn shell_fs_read_dispatch() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::write(dir.path().join("hello.txt"), "hello world").unwrap();
    let mut mt = MountTable::new();
    mt.parse_and_add(&format!("{}:/workspace", dir.path().display()))
        .unwrap();
    let s = sb_with_shell_and_mounts(mt);
    let luau = sh_transpile::transpile_sh("fs read /workspace/hello.txt")
        .unwrap()
        .luau_source;
    let r = s.exec(&luau).unwrap();
    assert!(
        r.contains("hello world"),
        "should display file content, got: {}",
        r
    );
}

#[test]
fn shell_fs_read_base64_options() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::write(dir.path().join("blob.bin"), [0, 1, 2, 255]).unwrap();
    let mut mt = MountTable::new();
    mt.parse_and_add(&format!("{}:/workspace", dir.path().display()))
        .unwrap();
    let s = sb_with_shell_and_mounts(mt);
    let luau = sh_transpile::transpile_sh(
        "fs read --path /workspace/blob.bin --mode base64 --offset 1 --count 2",
    )
    .unwrap()
    .luau_source;

    let result = s.exec(&luau).unwrap();

    assert_eq!(result, "AQI=");

    let short_luau =
        sh_transpile::transpile_sh("fs read -c 2 -m base64 -p /workspace/blob.bin -o 1")
            .unwrap()
            .luau_source;
    let short_result = s.exec(&short_luau).unwrap();

    assert_eq!(short_result, "AQI=");
}

#[test]
fn shell_fs_read_buffer_rejects_text_output_boundaries() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::write(dir.path().join("blob.bin"), [0, 1, 2, 255]).unwrap();

    for command in [
        "fs read /workspace/blob.bin --mode buffer",
        "fs read /workspace/blob.bin --mode buffer | wc -c",
        "fs read /workspace/blob.bin --mode buffer > /workspace/copy.bin",
    ] {
        let mut mt = MountTable::new();
        mt.parse_and_add(&format!("{}:/workspace", dir.path().display()))
            .unwrap();
        let s = sb_with_shell_and_mounts(mt);
        let luau = sh_transpile::transpile_sh(command).unwrap().luau_source;
        let err = s.exec(&luau).unwrap_err();

        assert!(
            err.message
                .contains("native buffer output cannot cross the shell boundary"),
            "command {command:?}: {}",
            err.message
        );
        assert!(
            err.message.contains("use --mode base64"),
            "command {command:?}: {}",
            err.message
        );
    }
}

#[test]
fn shell_fs_read_invalid_mode_uses_shell_native_usage() {
    let s = sb_with_shell();
    let luau = sh_transpile::transpile_sh("fs read /workspace/blob.bin --mode bogus")
        .unwrap()
        .luau_source;
    let err = s.exec(&luau).unwrap_err();

    assert!(
        err.message
            .contains("fs read: invalid mode 'bogus'; expected text, buffer, or base64"),
        "{err}"
    );
    assert!(
        err.message
            .contains("Usage: fs read -p/--path <string> [-m/--mode <string>]"),
        "{err}"
    );
    assert!(!err.message.contains("fs.read("), "{err}");
    assert!(!err.message.contains("Example:"), "{err}");
    assert!(!err.message.contains("string | buffer"), "{err}");
}

#[test]
fn shell_buffer_errors_restore_output_after_pipe_capture_and_redirects() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::write(dir.path().join("blob.bin"), [0, 1, 2, 255]).unwrap();
    let mut mt = MountTable::new();
    mt.parse_and_add(&format!("{}:/workspace", dir.path().display()))
        .unwrap();
    let s = sb_with_shell_and_mounts(mt);

    for (label, command) in [
        ("pipe", "fs read /workspace/blob.bin --mode buffer | wc -c"),
        (
            "capture",
            "echo $(fs read /workspace/blob.bin --mode buffer)",
        ),
        (
            "redirect-write",
            "fs read /workspace/blob.bin --mode buffer > /workspace/copy.bin",
        ),
        (
            "redirect-append",
            "fs read /workspace/blob.bin --mode buffer >> /workspace/copy.bin",
        ),
    ] {
        let failing = sh_transpile::transpile_sh(command).unwrap().luau_source;
        let err = s.exec(&failing).unwrap_err();
        assert!(
            err.message
                .contains("native buffer output cannot cross the shell boundary"),
            "{label}: {}",
            err.message
        );

        let marker = format!("recovered-{label}");
        let follow_up = sh_transpile::transpile_sh(&format!("echo {marker}"))
            .unwrap()
            .luau_source;
        assert_eq!(s.exec(&follow_up).unwrap(), marker, "{label}");
    }
}

#[test]
fn shell_fs_write_missing_content_uses_the_shell_string_type() {
    let s = sb_with_shell();
    let luau = sh_transpile::transpile_sh("fs write --path /workspace/out.bin")
        .unwrap()
        .luau_source;
    let err = s.exec(&luau).unwrap_err();
    assert!(err.message.contains("--content <string>"), "{err}");
    assert!(
        err.message
            .contains("Usage: fs write -p/--path <string> -c/--content <string>"),
        "{err}"
    );
    assert!(!err.message.contains("string | buffer"), "{err}");
    assert!(!err.message.contains("fs.write("), "{err}");
    assert!(!err.message.contains("Example:"), "{err}");
}

#[test]
fn shell_fs_write_type_error_uses_shell_native_usage() {
    let s = sb_with_shell();
    let luau = sh_transpile::transpile_sh("fs write --path /workspace/out.bin --content")
        .unwrap()
        .luau_source;
    let err = s.exec(&luau).unwrap_err();

    assert!(
        err.message.contains("expected string, got boolean"),
        "{err}"
    );
    assert!(
        err.message
            .contains("Usage: fs write -p/--path <string> -c/--content <string>"),
        "{err}"
    );
    assert!(!err.message.contains("string | buffer"), "{err}");
    assert!(!err.message.contains("fs.write("), "{err}");
}

#[test]
fn shell_fs_read_legacy_short_aliases_in_any_order() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::write(dir.path().join("lines.txt"), "one\ntwo\nthree\nfour\n").unwrap();
    let mut mt = MountTable::new();
    mt.parse_and_add(&format!("{}:/workspace", dir.path().display()))
        .unwrap();
    let s = sb_with_shell_and_mounts(mt);
    let luau = sh_transpile::transpile_sh("fs read -l 2 -p /workspace/lines.txt -o 2")
        .unwrap()
        .luau_source;

    let result = s.exec(&luau).unwrap();

    assert_eq!(result, "two\nthree");
}

#[test]
fn shell_fs_read_mixes_positional_path_and_named_ranges() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::write(dir.path().join("lines.txt"), "one\ntwo\nthree\nfour\n").unwrap();
    let mut mt = MountTable::new();
    mt.parse_and_add(&format!("{}:/workspace", dir.path().display()))
        .unwrap();
    let s = sb_with_shell_and_mounts(mt);
    let luau = sh_transpile::transpile_sh("fs read --limit 1 /workspace/lines.txt -o 3")
        .unwrap()
        .luau_source;

    let result = s.exec(&luau).unwrap();

    assert_eq!(result, "three");
}

#[test]
fn shell_boolean_option_accepts_explicit_false_and_presence_true() {
    let s = sb_with_shell();
    let token = s
        .exec(r#"return crypto.jwt_encode({sub="shell-user"}, "right-secret")"#)
        .unwrap();

    let false_command = format!(
        "crypto jwt_decode --token '{}' --secret wrong-secret --validate false",
        token
    );
    let false_luau = sh_transpile::transpile_sh(&false_command)
        .unwrap()
        .luau_source;
    let decoded = s.exec(&false_luau).unwrap();
    assert!(
        decoded.contains("shell-user"),
        "explicit false should disable JWT validation, got: {decoded}"
    );

    let explicit_true_command = format!(
        "crypto jwt_decode --token '{}' --secret wrong-secret --validate true",
        token
    );
    let explicit_true_luau = sh_transpile::transpile_sh(&explicit_true_command)
        .unwrap()
        .luau_source;
    assert!(
        s.exec(&explicit_true_luau).is_err(),
        "explicit true should enable JWT validation"
    );

    let true_command = format!(
        "crypto jwt_decode --token '{}' --secret wrong-secret --validate",
        token
    );
    let true_luau = sh_transpile::transpile_sh(&true_command)
        .unwrap()
        .luau_source;
    assert!(
        s.exec(&true_luau).is_err(),
        "a presence-only boolean flag must remain true"
    );

    let string_false_command = format!(
        "crypto jwt_decode --token '{}' --secret right-secret --algorithm false",
        token
    );
    let string_false_luau = sh_transpile::transpile_sh(&string_false_command)
        .unwrap()
        .luau_source;
    assert!(
        s.exec(&string_false_luau).is_err(),
        "the literal 'false' must remain a string for string-typed options"
    );
}

#[test]
fn image_resize_shell_help_marks_required_options_group() {
    let s = sb_with_shell();
    let help = s.exec(r#"image.help("shell")"#).unwrap();
    let resize_line = help
        .lines()
        .find(|line| line.contains("image resize"))
        .expect("image resize help line");

    assert!(
        resize_line.contains(
            "[--width <number>] [--height <number>] [--filter <string>] (at least one option required)"
        ),
        "required flattened opts should retain their parent requirement: {resize_line}"
    );
    assert!(
        help.contains("At least one of width or height is required"),
        "image.resize help should state its concrete size requirement: {help}"
    );
}

#[test]
fn shell_xml_parse_file_dispatch() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::write(dir.path().join("data.xml"), "<item>test</item>").unwrap();
    let mut mt = MountTable::new();
    mt.parse_and_add(&format!("{}:/workspace", dir.path().display()))
        .unwrap();
    let s = sb_with_shell_and_mounts(mt);
    let luau = sh_transpile::transpile_sh("xml parseFile /workspace/data.xml")
        .unwrap()
        .luau_source;
    let r = s.exec(&luau).unwrap();
    assert!(
        r.contains("test"),
        "should display parsed XML file, got: {}",
        r
    );
}

// ── 6c: module method --flag value → correct named params ─────

#[test]
fn shell_csv_parse_with_flags() {
    // csv parse "data" --delimiter ";" --header → flags become named params
    let luau = sh_transpile::transpile_sh(r#"csv parse "a;b" --delimiter ";""#)
        .unwrap()
        .luau_source;
    assert!(
        luau.contains("delimiter"),
        "should have delimiter param, got: {}",
        luau
    );
}

#[test]
fn shell_json_decode_flag_form() {
    // json decode has no flags, but verify basic dispatch transpile is correct
    let luau = sh_transpile::transpile_sh(r#"json decode '{"x":1}'"#)
        .unwrap()
        .luau_source;
    assert!(
        luau.contains("sh.run(\"json\", \"decode\""),
        "should use sh.run dispatch, got: {}",
        luau
    );
}

#[test]
fn unknown_command_errors() {
    let s = sb_with_shell();
    let luau = sh_transpile::transpile_sh("nonexistent_cmd arg1")
        .unwrap()
        .luau_source;
    let err = s.exec(&luau).unwrap_err();
    assert!(
        err.message.contains("not found") || err.message.contains("unknown"),
        "should error on unknown command, got: {}",
        err.message
    );
}

// ── 6d: Shell error formatting for missing args ──────────────

#[test]
fn shell_json_decode_no_args_shows_flag_error() {
    let s = sb_with_shell();
    let luau = sh_transpile::transpile_sh("json decode")
        .unwrap()
        .luau_source;
    let err = s.exec(&luau).unwrap_err();
    // Should contain --text flag syntax, not 'text' (string) positional syntax
    assert!(
        err.message.contains("--text"),
        "shell missing-arg error should use --flag format, got: {}",
        err.message
    );
}

#[test]
fn shell_xml_parse_no_args_shows_flag_error() {
    let s = sb_with_shell();
    let luau = sh_transpile::transpile_sh("xml parse").unwrap().luau_source;
    let err = s.exec(&luau).unwrap_err();
    assert!(
        err.message.contains("--text"),
        "shell missing-arg error should use --flag format, got: {}",
        err.message
    );
}

#[test]
fn shell_fs_read_no_args_shows_flag_error() {
    let s = sb_with_shell();
    let luau = sh_transpile::transpile_sh("fs read").unwrap().luau_source;
    let err = s.exec(&luau).unwrap_err();
    assert!(
        err.message.contains("--path"),
        "shell missing-arg error should use --flag format, got: {}",
        err.message
    );
}

#[test]
fn shell_compress_unzip_no_args_shows_flag_error() {
    let s = sb_with_shell();
    let luau = sh_transpile::transpile_sh("compress unzip")
        .unwrap()
        .luau_source;
    let err = s.exec(&luau).unwrap_err();
    assert!(
        err.message.contains("--archive") && err.message.contains("--dest"),
        "shell missing-arg error should use --flag format, got: {}",
        err.message
    );
}

#[test]
fn shell_unknown_method_has_no_line_number() {
    let s = sb_with_shell();
    let luau = sh_transpile::transpile_sh("xml lfdskl")
        .unwrap()
        .luau_source;
    let err = s.exec(&luau).unwrap_err();
    // Error from shrt/metatable handler should not have a line number
    assert_eq!(
        err.line, None,
        "shell unknown method error should have no line number, got: {:?}",
        err.line
    );
    assert!(
        err.message.contains("unknown method") || err.message.contains("does not exist"),
        "should say unknown method, got: {}",
        err.message
    );
}

#[test]
fn shell_error_uses_space_not_dot_syntax() {
    let s = sb_with_shell();
    let luau = sh_transpile::transpile_sh("json decode")
        .unwrap()
        .luau_source;
    let err = s.exec(&luau).unwrap_err();
    // Should say "json decode:" not "json.decode:"
    assert!(
        err.message.contains("json decode:"),
        "shell error should use 'module method:' not 'module.method:', got: {}",
        err.message
    );
}

// ── 6e: numx shell dispatch (named args → positional unpacking) ──

#[test]
fn shell_numx_add_scalars() {
    // `numx add --a 1 --b 2` → sh.run("numx", "add", {a="1", b="2"})
    // sh.run should unpack using __params → numx.add(1, 2)
    let s = sb_with_shell();
    let luau = sh_transpile::transpile_sh("numx add --a 1 --b 2")
        .unwrap()
        .luau_source;
    let r = s.exec(&luau).unwrap();
    assert!(
        r.contains("3"),
        "numx add --a 1 --b 2 should output 3, got: {}",
        r
    );
}

#[test]
fn shell_numx_add_positional() {
    // `numx add 1 2` → sh.run("numx", "add", {[1]="1", [2]="2"})
    // Should also work via positional args
    let s = sb_with_shell();
    let luau = sh_transpile::transpile_sh("numx add 1 2")
        .unwrap()
        .luau_source;
    let r = s.exec(&luau).unwrap();
    assert!(r.contains("3"), "numx add 1 2 should output 3, got: {}", r);
}

#[test]
fn shell_numx_zeros() {
    let s = sb_with_shell();
    let luau = sh_transpile::transpile_sh("numx zeros 3")
        .unwrap()
        .luau_source;
    let r = s.exec(&luau).unwrap();
    assert!(
        r.contains("0"),
        "numx zeros 3 should output array of zeros, got: {}",
        r
    );
}

// ── 6f: ls on files (not directories) ─────────────────────────

#[test]
fn ls_file_shows_filename() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::write(dir.path().join("hello.txt"), "hello world").unwrap();
    let mut mt = MountTable::new();
    mt.parse_and_add(&format!("{}:/workspace", dir.path().display()))
        .unwrap();
    let s = sb_with_shell_and_mounts(mt);
    let luau = sh_transpile::transpile_sh("ls /workspace/hello.txt")
        .unwrap()
        .luau_source;
    let r = s.exec(&luau).unwrap();
    assert!(
        r.contains("hello.txt"),
        "ls on a file should output the filename, got: {}",
        r
    );
}

#[test]
fn ls_file_long_format() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::write(dir.path().join("hello.txt"), "hello world").unwrap();
    let mut mt = MountTable::new();
    mt.parse_and_add(&format!("{}:/workspace", dir.path().display()))
        .unwrap();
    let s = sb_with_shell_and_mounts(mt);
    let luau = sh_transpile::transpile_sh("ls -l /workspace/hello.txt")
        .unwrap()
        .luau_source;
    let r = s.exec(&luau).unwrap();
    assert!(
        r.contains("file") && r.contains("hello.txt"),
        "ls -l on a file should show type and name, got: {}",
        r
    );
}

#[test]
fn ls_file_with_spaces() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::write(dir.path().join("p p"), "content").unwrap();
    let mut mt = MountTable::new();
    mt.parse_and_add(&format!("{}:/workspace", dir.path().display()))
        .unwrap();
    let s = sb_with_shell_and_mounts(mt);

    // Backslash-escaped form
    let luau = sh_transpile::transpile_sh(r#"ls /workspace/p\ p"#)
        .unwrap()
        .luau_source;
    let r = s.exec(&luau).unwrap();
    assert!(
        r.contains("p p"),
        "ls on a file with spaces (escaped) should output filename, got: {}",
        r
    );

    // Double-quoted form
    let luau2 = sh_transpile::transpile_sh(r#"ls "/workspace/p p""#)
        .unwrap()
        .luau_source;
    let r2 = s.exec(&luau2).unwrap();
    assert!(
        r2.contains("p p"),
        "ls on a file with spaces (quoted) should output filename, got: {}",
        r2
    );
}

// ── Regression: ls / with root mount must not produce leading empty line ──

#[test]
fn ls_root_no_leading_empty_line() {
    // When "/" is mounted (ephemeral root), `ls /` must not produce a
    // leading empty line caused by the root mount key appearing as an
    // empty-string entry in fs.list("/").
    let root_dir = tempfile::TempDir::new().unwrap();
    let ws_dir = tempfile::TempDir::new().unwrap();
    std::fs::create_dir(root_dir.path().join("tmp")).unwrap();

    let mut mounts = MountTable::new();
    mounts
        .parse_and_add(&format!("{}:/", root_dir.path().display()))
        .unwrap();
    mounts
        .parse_and_add(&format!("{}:/workspace", ws_dir.path().display()))
        .unwrap();

    let s = Sandbox::with_mounts(mounts).unwrap();
    let shrt = include_str!("../../runtime/shrt.luau");
    s.register_module("shrt", shrt).unwrap();

    let luau = sh_transpile::transpile_sh("ls /").unwrap().luau_source;
    let output = s.exec(&luau).unwrap();

    assert!(
        !output.starts_with('\n'),
        "ls / must not start with an empty line, got: {:?}",
        output
    );
    assert!(
        output.contains("workspace"),
        "ls / should list workspace, got: {:?}",
        output
    );
    assert!(
        output.contains("tmp"),
        "ls / should list tmp from root mount, got: {:?}",
        output
    );
}

// ── 6g: base64 shell dispatch ──────────────────────────────────

#[test]
fn bare_base64_shows_help() {
    let s = sb_with_shell();
    let luau = sh_transpile::transpile_sh("base64").unwrap().luau_source;
    let r = s.exec(&luau).unwrap();
    assert!(
        r.contains("base64"),
        "bare base64 should show help, got: {}",
        r
    );
}

#[test]
fn shell_base64_encode() {
    let s = sb_with_shell();
    let luau = sh_transpile::transpile_sh(r#"echo -n "Hello" | base64"#)
        .unwrap()
        .luau_source;
    let r = s.exec(&luau).unwrap();
    assert!(
        r.contains("SGVsbG8="),
        "base64 encode should output SGVsbG8=, got: {}",
        r
    );
}

#[test]
fn shell_base64_decode() {
    let s = sb_with_shell();
    let luau = sh_transpile::transpile_sh(r#"echo -n "SGVsbG8=" | base64 -d"#)
        .unwrap()
        .luau_source;
    let r = s.exec(&luau).unwrap();
    assert!(
        r.contains("Hello"),
        "base64 -d should output Hello, got: {}",
        r
    );
}

#[test]
fn shell_base64_roundtrip_pipe() {
    let s = sb_with_shell();
    let luau = sh_transpile::transpile_sh(r#"echo -n "test data" | base64 | base64 -d"#)
        .unwrap()
        .luau_source;
    let r = s.exec(&luau).unwrap();
    assert!(
        r.contains("test data"),
        "base64 roundtrip should preserve data, got: {}",
        r
    );
}

#[test]
fn shell_base64_encode_dispatch() {
    let s = sb_with_shell();
    let luau = sh_transpile::transpile_sh(r#"base64 encode --data "hello""#)
        .unwrap()
        .luau_source;
    let r = s.exec(&luau).unwrap();
    assert!(
        r.contains("aGVsbG8="),
        "base64 encode should output aGVsbG8=, got: {}",
        r
    );
}

#[test]
fn shell_base64_decode_dispatch() {
    let s = sb_with_shell();
    let luau = sh_transpile::transpile_sh(r#"base64 decode --text "aGVsbG8=""#)
        .unwrap()
        .luau_source;
    let r = s.exec(&luau).unwrap();
    assert!(
        r.contains("hello"),
        "base64 decode should output hello, got: {}",
        r
    );
}

#[test]
fn shell_base64_encode_direct() {
    // `base64 "text"` → encode (Linux-style, no subcommand)
    let s = sb_with_shell();
    let luau = sh_transpile::transpile_sh(r#"base64 "hello""#)
        .unwrap()
        .luau_source;
    let r = s.exec(&luau).unwrap();
    assert_eq!(r, "aGVsbG8=", "base64 \"hello\" should encode, got: {}", r);
}

#[test]
fn shell_base64_decode_flag_direct() {
    // `base64 -d "text"` → decode (Linux-style flag)
    let s = sb_with_shell();
    let luau = sh_transpile::transpile_sh(r#"base64 -d "aGVsbG8=""#)
        .unwrap()
        .luau_source;
    let r = s.exec(&luau).unwrap();
    assert_eq!(r, "hello", "base64 -d should decode, got: {}", r);
}
