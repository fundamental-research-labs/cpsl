#![cfg(feature = "mod-qr")]

use tempfile::TempDir;
use cpsl_core::{MountTable, Sandbox, transpile};

fn sb() -> Sandbox {
    Sandbox::new().unwrap()
}

fn sb_with_workspace() -> (Sandbox, TempDir) {
    let dir = TempDir::new().unwrap();
    let mut mt = MountTable::new();
    mt.parse_and_add(&format!("{}:/workspace", dir.path().display()))
        .unwrap();
    let sb = Sandbox::with_mounts(mt).unwrap();
    (sb, dir)
}

// ── 1. qr.to_string basic tests ────────────────────────────────

#[test]
fn to_string_returns_non_empty() {
    let s = sb();
    let r = s
        .exec(r#"return qr.to_string("hello")"#)
        .unwrap();
    assert!(!r.is_empty(), "to_string should return non-empty string");
}

#[test]
fn to_string_ascii_default_has_hash_patterns() {
    let s = sb();
    let r = s
        .exec(r#"return qr.to_string("hello")"#)
        .unwrap();
    assert!(
        r.contains("##"),
        "ASCII output should contain ## patterns, got: {}",
        r
    );
}

#[test]
fn to_string_ascii_explicit() {
    let s = sb();
    let r = s
        .exec(r#"return qr.to_string("hello", {style="ascii"})"#)
        .unwrap();
    assert!(
        r.contains("##"),
        "ASCII output should contain ## patterns, got: {}",
        r
    );
}

#[test]
fn to_string_unicode_has_block_chars() {
    let s = sb();
    let r = s
        .exec(r#"return qr.to_string("hello", {style="unicode"})"#)
        .unwrap();
    // Unicode style should contain block characters (▀, ▄, █, or space)
    let has_blocks = r.contains('\u{2580}')
        || r.contains('\u{2584}')
        || r.contains('\u{2588}');
    assert!(
        has_blocks,
        "Unicode output should contain block characters, got: {}",
        r
    );
}

#[test]
fn to_string_different_data_produces_different_output() {
    let s = sb();
    let r1 = s
        .exec(r#"return qr.to_string("hello")"#)
        .unwrap();
    let r2 = s
        .exec(r#"return qr.to_string("world")"#)
        .unwrap();
    assert_ne!(r1, r2, "Different data should produce different QR codes");
}

// ── 2. qr.generate tests ───────────────────────────────────────

#[test]
fn generate_png_writes_file() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(r#"return qr.generate("hello", "/workspace/test.png")"#)
        .unwrap();
    assert_eq!(r, "/workspace/test.png");

    let path = dir.path().join("test.png");
    assert!(path.exists(), "PNG file should exist at {:?}", path);
    let contents = std::fs::read(&path).unwrap();
    assert!(contents.len() > 100, "PNG file should have content");
    // PNG magic bytes
    assert_eq!(&contents[0..4], &[0x89, 0x50, 0x4E, 0x47], "File should start with PNG magic");
}

#[test]
fn generate_svg_writes_file() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(r#"return qr.generate("hello", "/workspace/test.svg", {format="svg"})"#)
        .unwrap();
    assert_eq!(r, "/workspace/test.svg");

    let path = dir.path().join("test.svg");
    assert!(path.exists(), "SVG file should exist at {:?}", path);
    let contents = std::fs::read_to_string(&path).unwrap();
    assert!(contents.contains("<svg"), "File should contain SVG content, got: {}", &contents[..100.min(contents.len())]);
    assert!(contents.contains("rect"), "SVG should contain rect elements");
}

#[test]
fn generate_with_custom_options() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(r#"return qr.generate("hello", "/workspace/custom.png", {size=20, margin=2})"#)
        .unwrap();
    assert_eq!(r, "/workspace/custom.png");

    let path = dir.path().join("custom.png");
    assert!(path.exists(), "PNG file should exist");
    let contents = std::fs::read(&path).unwrap();
    assert!(contents.len() > 100, "PNG file should have content");
}

#[test]
fn generate_svg_with_custom_colors() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(r##"return qr.generate("hello", "/workspace/color.svg", {format="svg", color="#FF0000", bg="#00FF00"})"##)
        .unwrap();
    assert_eq!(r, "/workspace/color.svg");

    let path = dir.path().join("color.svg");
    let contents = std::fs::read_to_string(&path).unwrap();
    assert!(contents.contains("rgb(255,0,0)"), "SVG should contain red color");
    assert!(contents.contains("rgb(0,255,0)"), "SVG should contain green background");
}

#[test]
fn generate_short_aliases() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(r##"return qr.generate("hello", "/workspace/short.svg", {f="svg", s=5, m=1, c="#0000FF"})"##)
        .unwrap();
    assert_eq!(r, "/workspace/short.svg");

    let path = dir.path().join("short.svg");
    assert!(path.exists(), "SVG file should exist with short aliases");
    let contents = std::fs::read_to_string(&path).unwrap();
    assert!(contents.contains("rgb(0,0,255)"), "SVG should contain blue color from short alias");
}

#[test]
fn generate_creates_subdirectories() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(r#"return qr.generate("hello", "/workspace/subdir/deep/test.png")"#)
        .unwrap();
    assert_eq!(r, "/workspace/subdir/deep/test.png");

    let path = dir.path().join("subdir/deep/test.png");
    assert!(path.exists(), "PNG file should be created in nested dirs");
}

// ── 3. Table-form calling ───────────────────────────────────────

#[test]
fn to_string_table_form_positional() {
    let s = sb();
    let r = s
        .exec(r#"return qr.to_string({[1]="hello"})"#)
        .unwrap();
    assert!(!r.is_empty(), "Table-form to_string should return non-empty string");
    assert!(r.contains("##"), "Should contain ASCII patterns");
}

#[test]
fn generate_table_form_positional() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(r#"return qr.generate({[1]="hello", [2]="/workspace/table.png"})"#)
        .unwrap();
    assert_eq!(r, "/workspace/table.png");
    let path = dir.path().join("table.png");
    assert!(path.exists(), "PNG file should be created via table form");
}

// ── 4. Error handling ───────────────────────────────────────────

#[test]
fn to_string_no_args_errors() {
    let s = sb();
    let err = s.exec("qr.to_string()").unwrap_err();
    assert!(
        err.message.contains("missing") || err.message.contains("bad argument"),
        "msg: {}",
        err.message
    );
}

#[test]
fn generate_no_args_errors() {
    let s = sb();
    let err = s.exec("qr.generate()").unwrap_err();
    assert!(
        err.message.contains("missing") || err.message.contains("bad argument"),
        "msg: {}",
        err.message
    );
}

#[test]
fn generate_missing_output_errors() {
    let s = sb();
    let err = s.exec(r#"qr.generate("hello")"#).unwrap_err();
    assert!(
        err.message.contains("missing") || err.message.contains("output"),
        "msg: {}",
        err.message
    );
}

#[test]
fn to_string_invalid_style_errors() {
    let s = sb();
    let err = s
        .exec(r#"qr.to_string("hello", {style="bad"})"#)
        .unwrap_err();
    assert!(
        err.message.contains("unsupported style"),
        "msg: {}",
        err.message
    );
}

#[test]
fn generate_invalid_format_errors() {
    let (s, _dir) = sb_with_workspace();
    let err = s
        .exec(r#"qr.generate("hello", "/workspace/test.bmp", {format="bmp"})"#)
        .unwrap_err();
    assert!(
        err.message.contains("unsupported format"),
        "msg: {}",
        err.message
    );
}

// ── 5. Help tests ───────────────────────────────────────────────

#[test]
fn qr_help_returns_help_text() {
    let s = sb();
    let r = s.exec("return qr.help()").unwrap();
    assert!(r.contains("qr"), "help: {}", r);
    assert!(r.contains("qr.generate"), "help: {}", r);
    assert!(r.contains("qr.to_string"), "help: {}", r);
}

#[test]
fn qr_nonexistent_fn_hint() {
    let s = sb();
    let err = s.exec("qr.foo()").unwrap_err();
    assert!(
        err.message.contains("qr.foo does not exist"),
        "msg: {}",
        err.message
    );
    assert!(
        err.message.contains("hint: call qr.help() for usage"),
        "msg: {}",
        err.message
    );
}

#[test]
fn global_help_mentions_qr() {
    let s = sb();
    let r = s.exec("return help()").unwrap();
    assert!(
        r.contains("qr"),
        "global help should list qr: {}",
        r
    );
}

// ── 6. Python transpiler tests ──────────────────────────────────

#[test]
fn python_import_qrcode_maps_to_qr() {
    let py_code = r#"
import qrcode
result = qr.to_string("hello")
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    // "import qrcode" should map to "local qrcode = qr" (sandbox global, not require)
    assert!(
        transpiled.luau_source.contains("local qrcode = qr"),
        "transpiled should map qrcode to qr global: {}",
        transpiled.luau_source
    );
    assert!(
        !transpiled.luau_source.contains("require(\"qrcode\")"),
        "transpiled should not require(\"qrcode\"): {}",
        transpiled.luau_source
    );
}

#[test]
fn python_qr_to_string_e2e() {
    let s = sb();
    let pyrt = include_str!("../../pyrt.luau");
    s.setup_python_runtime(pyrt).unwrap();

    let py_code = r#"
import qrcode
result = qr.to_string("hello")
print(result)
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    let r = s.exec(&transpiled.luau_source).unwrap();
    assert!(
        r.contains("##"),
        "Python QR to_string should produce ASCII output: {}",
        r
    );
}

#[test]
fn python_qr_generate_e2e() {
    let (s, dir) = sb_with_workspace();
    let pyrt = include_str!("../../pyrt.luau");
    s.setup_python_runtime(pyrt).unwrap();

    let py_code = r#"
import qrcode
result = qr.generate("hello", "/workspace/py_test.svg", format="svg")
print(result)
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    let r = s.exec(&transpiled.luau_source).unwrap();
    assert!(
        r.contains("/workspace/py_test.svg"),
        "Should return output path: {}",
        r
    );
    let path = dir.path().join("py_test.svg");
    assert!(path.exists(), "SVG file should be written");
}

// ── 7. Shell dispatch tests ─────────────────────────────────────

#[test]
fn shell_qr_to_string() {
    let s = sb();
    let shrt = include_str!("../../shrt.luau");
    s.register_module("shrt", shrt).unwrap();
    let result = cpsl_core::sh_transpile::transpile_sh(r#"qr to_string "hello""#);
    assert!(result.is_ok(), "transpile err: {:?}", result.err());
    let luau = result.unwrap().luau_source;
    let r = s.exec(&luau).unwrap();
    assert!(
        r.contains("##"),
        "shell qr to_string should produce ASCII output: {}",
        r
    );
}

#[test]
fn shell_qr_help() {
    let s = sb();
    let shrt = include_str!("../../shrt.luau");
    s.register_module("shrt", shrt).unwrap();
    let result = cpsl_core::sh_transpile::transpile_sh("qr help");
    assert!(result.is_ok(), "transpile err: {:?}", result.err());
    let luau = result.unwrap().luau_source;
    let r = s.exec(&luau).unwrap();
    assert!(
        r.contains("qr.generate") || r.contains("qr generate"),
        "shell help should mention generate: {}",
        r
    );
}

// ── 8. Security tests ──────────────────────────────────────────

#[test]
fn generate_respects_mount_sandbox() {
    let s = sb(); // No mounts at all
    let err = s
        .exec(r#"qr.generate("hello", "/etc/passwd.png")"#)
        .unwrap_err();
    assert!(
        err.message.contains("No such file or directory")
            || err.message.contains("mount")
            || err.message.contains("denied"),
        "Should be denied by mount sandbox: {}",
        err.message
    );
}

#[test]
fn generate_cannot_write_outside_workspace() {
    let (s, _dir) = sb_with_workspace();
    let err = s
        .exec(r#"qr.generate("hello", "/etc/evil.png")"#)
        .unwrap_err();
    assert!(
        err.message.contains("No such file or directory")
            || err.message.contains("mount")
            || err.message.contains("denied"),
        "Should be denied outside workspace: {}",
        err.message
    );
}

#[test]
fn qr_no_dangerous_globals_exposed() {
    let s = sb();
    let r = s
        .exec(r#"
            return tostring(type(qr.generate)) .. " " ..
                   tostring(type(qr.to_string)) .. " " ..
                   tostring(rawget(qr, "io")) .. " " ..
                   tostring(rawget(qr, "os"))
        "#)
        .unwrap();
    assert_eq!(r, "function function nil nil");
}

#[test]
fn to_string_is_purely_computational() {
    let s = sb();
    // to_string should work without any mounts
    let r = s
        .exec(r#"
            local result = qr.to_string("test data")
            return tostring(#result > 0)
        "#)
        .unwrap();
    assert_eq!(r, "true");
}
