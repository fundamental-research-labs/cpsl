#![cfg(feature = "all")]

//! Phase 6: Enforcement tests for structured parameter metadata.
//!
//! Every module function must produce clear errors when called with no args or wrong types.
//! Shell-format help must list all params with `--name <type>` syntax.

use cpsl_core::{MountTable, Sandbox};

fn sb() -> Sandbox {
    Sandbox::new().unwrap()
}

fn sb_with_mounts() -> Sandbox {
    let dir = tempfile::TempDir::new().unwrap();
    let mut mt = MountTable::new();
    mt.parse_and_add(&format!("{}:/workspace", dir.path().display()))
        .unwrap();
    // Leak the tempdir so it outlives the sandbox
    std::mem::forget(dir);
    Sandbox::with_mounts(mt).unwrap()
}

/// Helper: exec Luau, expect error, return the error message.
fn exec_err(sb: &Sandbox, code: &str) -> String {
    sb.exec(code).unwrap_err().message
}

// ============================================================
// 6a: No-args errors mention required param names
// ============================================================

// -- xml --

#[test]
fn xml_parse_no_args_mentions_params() {
    let msg = exec_err(&sb(), "xml.parse()");
    assert!(
        msg.contains("text"),
        "xml.parse error should mention 'text', got: {msg}"
    );
}

#[test]
fn xml_parse_file_no_args_mentions_params() {
    let msg = exec_err(&sb(), "xml.parseFile()");
    assert!(
        msg.contains("path"),
        "xml.parseFile error should mention 'path', got: {msg}"
    );
}

#[test]
fn xml_query_no_args_mentions_params() {
    let msg = exec_err(&sb(), "xml.query()");
    assert!(
        msg.contains("doc") && msg.contains("path"),
        "xml.query error should mention 'doc' and 'path', got: {msg}"
    );
}

#[test]
fn xml_text_no_args_mentions_params() {
    let msg = exec_err(&sb(), "xml.text()");
    assert!(
        msg.contains("node"),
        "xml.text error should mention 'node', got: {msg}"
    );
}

#[test]
fn xml_encode_no_args_mentions_params() {
    let msg = exec_err(&sb(), "xml.encode()");
    assert!(
        msg.contains("tree"),
        "xml.encode error should mention 'tree', got: {msg}"
    );
}

// -- json --

#[test]
fn json_decode_no_args_mentions_params() {
    let msg = exec_err(&sb(), "json.decode()");
    assert!(
        msg.contains("text"),
        "json.decode error should mention 'text', got: {msg}"
    );
}

#[test]
fn json_encode_no_args_mentions_params() {
    let msg = exec_err(&sb(), "json.encode()");
    assert!(
        msg.contains("value"),
        "json.encode error should mention 'value', got: {msg}"
    );
}

// -- csv --

#[test]
fn csv_parse_no_args_mentions_params() {
    let msg = exec_err(&sb(), "csv.parse()");
    assert!(
        msg.contains("text"),
        "csv.parse error should mention 'text', got: {msg}"
    );
}

#[test]
fn csv_stringify_no_args_mentions_params() {
    let msg = exec_err(&sb(), "csv.stringify()");
    assert!(
        msg.contains("rows"),
        "csv.stringify error should mention 'rows', got: {msg}"
    );
}

// -- yaml --

#[test]
fn yaml_decode_no_args_mentions_params() {
    let msg = exec_err(&sb(), "yaml.decode()");
    assert!(
        msg.contains("text"),
        "yaml.decode error should mention 'text', got: {msg}"
    );
}

#[test]
fn yaml_encode_no_args_mentions_params() {
    let msg = exec_err(&sb(), "yaml.encode()");
    assert!(
        msg.contains("value"),
        "yaml.encode error should mention 'value', got: {msg}"
    );
}

// -- fs --

#[test]
fn fs_read_no_args_mentions_params() {
    let msg = exec_err(&sb(), "fs.read()");
    assert!(
        msg.contains("path"),
        "fs.read error should mention 'path', got: {msg}"
    );
}

#[test]
fn fs_write_no_args_mentions_params() {
    let msg = exec_err(&sb(), "fs.write()");
    assert!(
        msg.contains("path") && msg.contains("content"),
        "fs.write error should mention 'path' and 'content', got: {msg}"
    );
}

#[test]
fn fs_list_no_args_mentions_params() {
    let msg = exec_err(&sb(), "fs.list()");
    assert!(
        msg.contains("path"),
        "fs.list error should mention 'path', got: {msg}"
    );
}

#[test]
fn fs_mkdir_no_args_mentions_params() {
    let msg = exec_err(&sb(), "fs.mkdir()");
    assert!(
        msg.contains("path"),
        "fs.mkdir error should mention 'path', got: {msg}"
    );
}

#[test]
fn fs_remove_no_args_mentions_params() {
    let msg = exec_err(&sb(), "fs.remove()");
    assert!(
        msg.contains("path"),
        "fs.remove error should mention 'path', got: {msg}"
    );
}

#[test]
fn fs_copy_no_args_mentions_params() {
    let msg = exec_err(&sb(), "fs.copy()");
    assert!(
        msg.contains("src") && msg.contains("dst"),
        "fs.copy error should mention 'src' and 'dst', got: {msg}"
    );
}

// -- http (no test — requires gateway) --
// HTTP functions require an HttpGateway which isn't available in basic sandbox.

// -- compress --

#[test]
fn compress_unzip_no_args_mentions_params() {
    let msg = exec_err(&sb_with_mounts(), "compress.unzip()");
    assert!(
        msg.contains("archive") && msg.contains("dest"),
        "compress.unzip error should mention 'archive' and 'dest', got: {msg}"
    );
}

#[test]
fn compress_zip_no_args_mentions_params() {
    let msg = exec_err(&sb_with_mounts(), "compress.zip()");
    assert!(
        msg.contains("source") && msg.contains("archive"),
        "compress.zip error should mention 'source' and 'archive', got: {msg}"
    );
}

#[test]
fn compress_bzip2_no_args_mentions_params() {
    let msg = exec_err(&sb_with_mounts(), "compress.bzip2()");
    assert!(
        msg.contains("input") && msg.contains("output"),
        "compress.bzip2 error should mention 'input' and 'output', got: {msg}"
    );
}

#[test]
fn compress_xz_no_args_mentions_params() {
    let msg = exec_err(&sb_with_mounts(), "compress.xz()");
    assert!(
        msg.contains("input") && msg.contains("output"),
        "compress.xz error should mention 'input' and 'output', got: {msg}"
    );
}

// -- doc --

#[test]
fn doc_read_no_args_mentions_params() {
    let msg = exec_err(&sb(), "doc.read()");
    assert!(
        msg.contains("path"),
        "doc.read error should mention 'path', got: {msg}"
    );
}

#[test]
fn doc_render_no_args_mentions_params() {
    let msg = exec_err(&sb(), "doc.render()");
    assert!(
        msg.contains("text") && msg.contains("from") && msg.contains("to"),
        "doc.render error should mention 'text', 'from', 'to', got: {msg}"
    );
}

#[test]
fn doc_render_file_no_args_mentions_params() {
    let msg = exec_err(&sb(), "doc.renderFile()");
    assert!(
        msg.contains("source") && msg.contains("target"),
        "doc.renderFile error should mention 'source' and 'target', got: {msg}"
    );
}

// -- plot --

#[test]
fn plot_line_no_args_mentions_params() {
    let msg = exec_err(&sb_with_mounts(), "plot.line()");
    assert!(
        msg.contains("x") && msg.contains("y"),
        "plot.line error should mention 'x' and 'y', got: {msg}"
    );
}

#[test]
fn plot_bar_no_args_mentions_params() {
    let msg = exec_err(&sb_with_mounts(), "plot.bar()");
    assert!(
        msg.contains("labels") && msg.contains("values"),
        "plot.bar error should mention 'labels' and 'values', got: {msg}"
    );
}

#[test]
fn plot_histogram_no_args_mentions_params() {
    let msg = exec_err(&sb_with_mounts(), "plot.histogram()");
    assert!(
        msg.contains("data"),
        "plot.histogram error should mention 'data', got: {msg}"
    );
}

#[test]
fn plot_figure_no_args_mentions_params() {
    let msg = exec_err(&sb_with_mounts(), "plot.figure()");
    assert!(
        msg.contains("opts"),
        "plot.figure error should mention 'opts', got: {msg}"
    );
}

// ============================================================
// 6c: Shell-format help lists params with --name <type> syntax
// ============================================================

fn sb_with_shell() -> Sandbox {
    let s = sb();
    let shrt = include_str!("../../runtime/shrt.luau");
    s.register_module("shrt", shrt).unwrap();
    s
}

fn sb_with_shell_and_mounts() -> Sandbox {
    let s = sb_with_mounts();
    let shrt = include_str!("../../runtime/shrt.luau");
    s.register_module("shrt", shrt).unwrap();
    s
}

#[test]
fn xml_shell_help_lists_flag_params() {
    let s = sb_with_shell();
    let out = s.exec(r#"xml.help("shell")"#).unwrap();
    assert!(
        out.contains("--text"),
        "xml help(shell) should list --text, got: {out}"
    );
    assert!(
        out.contains("--path"),
        "xml help(shell) should list --path, got: {out}"
    );
    assert!(
        out.contains("--doc"),
        "xml help(shell) should list --doc, got: {out}"
    );
}

#[test]
fn json_shell_help_lists_flag_params() {
    let s = sb_with_shell();
    let out = s.exec(r#"json.help("shell")"#).unwrap();
    assert!(
        out.contains("--text"),
        "json help(shell) should list --text, got: {out}"
    );
    assert!(
        out.contains("--value"),
        "json help(shell) should list --value, got: {out}"
    );
}

#[test]
fn fs_shell_help_lists_flag_params() {
    let s = sb_with_shell();
    let out = s.exec(r#"fs.help("shell")"#).unwrap();
    assert!(
        out.contains("--path"),
        "fs help(shell) should list --path, got: {out}"
    );
}

#[test]
fn compress_shell_help_lists_flag_params() {
    let s = sb_with_shell_and_mounts();
    let out = s.exec(r#"compress.help("shell")"#).unwrap();
    assert!(
        out.contains("--source"),
        "compress help(shell) should list --source, got: {out}"
    );
    assert!(
        out.contains("--archive"),
        "compress help(shell) should list --archive, got: {out}"
    );
}

#[test]
fn doc_shell_help_lists_flag_params() {
    let s = sb_with_shell();
    let out = s.exec(r#"doc.help("shell")"#).unwrap();
    assert!(
        out.contains("--path"),
        "doc help(shell) should list --path, got: {out}"
    );
    assert!(
        out.contains("--source"),
        "doc help(shell) should list --source, got: {out}"
    );
}

#[test]
fn plot_shell_help_lists_flag_params() {
    let s = sb_with_shell_and_mounts();
    let out = s.exec(r#"plot.help("shell")"#).unwrap();
    assert!(
        out.contains("--x"),
        "plot help(shell) should list --x, got: {out}"
    );
    assert!(
        out.contains("--y"),
        "plot help(shell) should list --y, got: {out}"
    );
}

// ============================================================
// 6d: Every module's help("shell") has --flag entries (proves params are structured)
// ============================================================

/// Verify that every module's help("shell") has at least one `--` flag entry,
/// proving that structured params exist (not empty freeform-only signatures).
#[test]
fn all_modules_shell_help_has_flag_syntax() {
    let s = sb_with_shell_and_mounts();

    let modules = [
        "xml", "json", "csv", "yaml", "fs", "compress", "doc", "plot",
    ];
    for module in modules {
        let code = format!(r#"{module}.help("shell")"#);
        let out = s.exec(&code).unwrap();
        assert!(
            out.contains("--"),
            r#"{module}.help("shell") should contain '--' flag syntax, got: {out}"#
        );
    }
}
