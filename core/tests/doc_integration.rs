#![cfg(feature = "mod-doc")]

use cpsl_core::{MountTable, Sandbox};
use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tempfile::TempDir;

fn sandbox_with_dir(dir: &TempDir) -> Sandbox {
    let mut mt = MountTable::new();
    mt.parse_and_add(&format!("{}:/data:ro", dir.path().display()))
        .unwrap();
    Sandbox::with_mounts(mt).unwrap()
}

fn sandbox_with_rw_dir(dir: &TempDir) -> Sandbox {
    let mut mt = MountTable::new();
    mt.parse_and_add(&format!("{}:/workspace", dir.path().display()))
        .unwrap();
    Sandbox::with_mounts(mt).unwrap()
}

// ── Helper: create minimal XLSX ─────────────────────────────────

fn create_minimal_xlsx(path: &std::path::Path) {
    use std::io::Write;
    let file = std::fs::File::create(path).unwrap();
    let mut zip = zip::ZipWriter::new(file);

    // [Content_Types].xml
    zip.start_file(
        "[Content_Types].xml",
        zip::write::SimpleFileOptions::default(),
    )
    .unwrap();
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
  <Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
  <Override PartName="/xl/sharedStrings.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml"/>
</Types>"#).unwrap();

    // _rels/.rels
    zip.start_file("_rels/.rels", zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
</Relationships>"#).unwrap();

    // xl/_rels/workbook.xml.rels
    zip.start_file(
        "xl/_rels/workbook.xml.rels",
        zip::write::SimpleFileOptions::default(),
    )
    .unwrap();
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/sharedStrings" Target="sharedStrings.xml"/>
</Relationships>"#).unwrap();

    // xl/workbook.xml
    zip.start_file("xl/workbook.xml", zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets>
    <sheet name="TestSheet" sheetId="1" r:id="rId1"/>
  </sheets>
</workbook>"#).unwrap();

    // xl/sharedStrings.xml
    zip.start_file(
        "xl/sharedStrings.xml",
        zip::write::SimpleFileOptions::default(),
    )
    .unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="2" uniqueCount="2">
  <si><t>Name</t></si>
  <si><t>Alice</t></si>
</sst>"#,
    )
    .unwrap();

    // xl/worksheets/sheet1.xml
    zip.start_file(
        "xl/worksheets/sheet1.xml",
        zip::write::SimpleFileOptions::default(),
    )
    .unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData>
    <row r="1">
      <c r="A1" t="s"><v>0</v></c>
      <c r="B1"><v>42</v></c>
    </row>
    <row r="2">
      <c r="A2" t="s"><v>1</v></c>
      <c r="B2"><v>100</v></c>
    </row>
  </sheetData>
</worksheet>"#,
    )
    .unwrap();

    zip.finish().unwrap();
}

// ── Helper: create minimal DOCX ─────────────────────────────────

fn create_minimal_docx(path: &std::path::Path) {
    let file = std::fs::File::create(path).unwrap();
    let mut zip = zip::ZipWriter::new(file);

    zip.start_file(
        "[Content_Types].xml",
        zip::write::SimpleFileOptions::default(),
    )
    .unwrap();
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"#).unwrap();

    zip.start_file("_rels/.rels", zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#).unwrap();

    zip.start_file(
        "word/document.xml",
        zip::write::SimpleFileOptions::default(),
    )
    .unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p><w:r><w:t>Hello from DOCX</w:t></w:r></w:p>
    <w:p><w:r><w:t>Second paragraph</w:t></w:r></w:p>
  </w:body>
</w:document>"#,
    )
    .unwrap();

    zip.finish().unwrap();
}

// ── Helper: create minimal PPTX ─────────────────────────────────

fn create_minimal_pptx(path: &std::path::Path) {
    let file = std::fs::File::create(path).unwrap();
    let mut zip = zip::ZipWriter::new(file);

    zip.start_file(
        "[Content_Types].xml",
        zip::write::SimpleFileOptions::default(),
    )
    .unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
</Types>"#,
    )
    .unwrap();

    zip.start_file(
        "ppt/slides/slide1.xml",
        zip::write::SimpleFileOptions::default(),
    )
    .unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
       xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cSld>
    <p:spTree>
      <p:sp><p:txBody><a:p><a:r><a:t>Slide One Title</a:t></a:r></a:p></p:txBody></p:sp>
    </p:spTree>
  </p:cSld>
</p:sld>"#,
    )
    .unwrap();

    zip.start_file(
        "ppt/slides/slide2.xml",
        zip::write::SimpleFileOptions::default(),
    )
    .unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
       xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cSld>
    <p:spTree>
      <p:sp><p:txBody><a:p><a:r><a:t>Slide Two Content</a:t></a:r></a:p></p:txBody></p:sp>
    </p:spTree>
  </p:cSld>
</p:sld>"#,
    )
    .unwrap();

    zip.finish().unwrap();
}

// ── XLSX Tests ──────────────────────────────────────────────────

#[test]
fn doc_read_xlsx() {
    let dir = TempDir::new().unwrap();
    create_minimal_xlsx(&dir.path().join("test.xlsx"));
    let sb = sandbox_with_dir(&dir);

    let result = sb.exec(r#"return doc.read("/data/test.xlsx")"#).unwrap();
    assert!(
        result.contains("TestSheet"),
        "should have sheet name: {}",
        result
    );
    assert!(result.contains("Name"), "should have 'Name': {}", result);
    assert!(result.contains("Alice"), "should have 'Alice': {}", result);
    assert!(result.contains("42"), "should have '42': {}", result);
    assert!(result.contains("100"), "should have '100': {}", result);
}

#[test]
fn doc_read_xlsx_specific_sheet() {
    let dir = TempDir::new().unwrap();
    create_minimal_xlsx(&dir.path().join("test.xlsx"));
    let sb = sandbox_with_dir(&dir);

    let result = sb
        .exec(r#"return doc.read("/data/test.xlsx", {sheet = 1})"#)
        .unwrap();
    assert!(result.contains("Name"), "should have data: {}", result);
}

#[test]
fn doc_read_xlsx_invalid_sheet() {
    let dir = TempDir::new().unwrap();
    create_minimal_xlsx(&dir.path().join("test.xlsx"));
    let sb = sandbox_with_dir(&dir);

    let err = sb
        .exec(r#"doc.read("/data/test.xlsx", {sheet = 99})"#)
        .unwrap_err();
    assert!(
        err.message.contains("does not exist"),
        "should error on bad sheet: {}",
        err.message
    );
}

// ── DOCX Tests ──────────────────────────────────────────────────

#[test]
fn doc_read_docx() {
    let dir = TempDir::new().unwrap();
    create_minimal_docx(&dir.path().join("test.docx"));
    let sb = sandbox_with_dir(&dir);

    let result = sb.exec(r#"return doc.read("/data/test.docx")"#).unwrap();
    assert!(
        result.contains("Hello from DOCX"),
        "should extract docx text: {}",
        result
    );
    assert!(
        result.contains("Second paragraph"),
        "should have second para: {}",
        result
    );
}

// ── PPTX Tests ──────────────────────────────────────────────────

#[test]
fn doc_read_pptx() {
    let dir = TempDir::new().unwrap();
    create_minimal_pptx(&dir.path().join("test.pptx"));
    let sb = sandbox_with_dir(&dir);

    let result = sb.exec(r#"return doc.read("/data/test.pptx")"#).unwrap();
    assert!(
        result.contains("Slide One Title"),
        "should have slide 1: {}",
        result
    );
    assert!(
        result.contains("Slide Two Content"),
        "should have slide 2: {}",
        result
    );
}

// ── RTF Tests ───────────────────────────────────────────────────

#[test]
fn doc_read_rtf() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("test.rtf"), r"{\rtf1 Hello RTF World}").unwrap();
    let sb = sandbox_with_dir(&dir);

    let result = sb.exec(r#"return doc.read("/data/test.rtf")"#).unwrap();
    assert!(
        result.contains("Hello RTF World"),
        "should extract rtf text: {}",
        result
    );
}

// ── Plain text formats ──────────────────────────────────────────

#[test]
fn doc_read_csv() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("data.csv"), "a,b,c\n1,2,3\n").unwrap();
    let sb = sandbox_with_dir(&dir);

    let result = sb.exec(r#"return doc.read("/data/data.csv")"#).unwrap();
    assert_eq!(result, "a,b,c\n1,2,3\n");
}

#[test]
fn doc_read_txt() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("note.txt"), "Hello plain text").unwrap();
    let sb = sandbox_with_dir(&dir);

    let result = sb.exec(r#"return doc.read("/data/note.txt")"#).unwrap();
    assert_eq!(result, "Hello plain text");
}

#[test]
fn doc_read_json() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("data.json"), r#"{"key": "value"}"#).unwrap();
    let sb = sandbox_with_dir(&dir);

    let result = sb.exec(r#"return doc.read("/data/data.json")"#).unwrap();
    assert_eq!(result, r#"{"key": "value"}"#);
}

#[test]
fn doc_read_markdown() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("readme.md"), "# Title\n\nBody text").unwrap();
    let sb = sandbox_with_dir(&dir);

    let result = sb.exec(r#"return doc.read("/data/readme.md")"#).unwrap();
    assert_eq!(result, "# Title\n\nBody text");
}

// ── Error cases ─────────────────────────────────────────────────

#[test]
fn doc_read_unsupported_format() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("file.xyz"), "data").unwrap();
    let sb = sandbox_with_dir(&dir);

    let err = sb.exec(r#"doc.read("/data/file.xyz")"#).unwrap_err();
    assert!(
        err.message.contains("unsupported"),
        "should error on unknown format: {}",
        err.message
    );
}

#[test]
fn doc_read_nonexistent_file() {
    let dir = TempDir::new().unwrap();
    let sb = sandbox_with_dir(&dir);

    let err = sb.exec(r#"doc.read("/data/missing.txt")"#).unwrap_err();
    assert!(
        err.message.contains("No such file"),
        "should error on missing file: {}",
        err.message
    );
}

#[test]
fn doc_read_outside_mount() {
    let sb = Sandbox::new().unwrap();
    let err = sb.exec(r#"doc.read("/etc/data.txt")"#).unwrap_err();
    assert!(
        err.message.contains("not mounted") || err.message.contains("No such file"),
        "should error on unmounted path: {}",
        err.message
    );
}

// ── doc.help() ──────────────────────────────────────────────────

#[test]
fn doc_module_help() {
    let sb = Sandbox::new().unwrap();
    let result = sb.exec("return doc.help()").unwrap();
    assert!(
        result.contains("doc — document reading"),
        "help: {}",
        result
    );
    assert!(result.contains("doc.read"), "help: {}", result);
}

#[test]
fn doc_nonexistent_fn_hint() {
    let sb = Sandbox::new().unwrap();
    let err = sb.exec("doc.foo()").unwrap_err();
    assert!(
        err.message.contains("doc.foo does not exist"),
        "msg: {}",
        err.message
    );
    assert!(
        err.message.contains("hint: call doc.help() for usage"),
        "msg: {}",
        err.message
    );
}

// ── global help() mentions doc ──────────────────────────────────

#[test]
fn global_help_mentions_doc_module() {
    let sb = Sandbox::new().unwrap();
    let result = sb.exec("return help()").unwrap();
    assert!(
        result.contains("doc"),
        "help should mention doc: {}",
        result
    );
}

// ── Python transpiler e2e ───────────────────────────────────────

#[test]
fn python_doc_read() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("test.txt"), "python test content").unwrap();

    let mut mt = MountTable::new();
    mt.parse_and_add(&format!("{}:/data:ro", dir.path().display()))
        .unwrap();
    let sb = Sandbox::with_mounts(mt).unwrap();

    let pyrt = include_str!("../../runtime/pyrt.luau");
    sb.setup_python_runtime(pyrt).unwrap();

    let py_code = r#"
text = doc.read("/data/test.txt")
print(text)
"#;
    let transpiled = cpsl_core::transpile::transpile(py_code).unwrap();
    let r = sb.exec(&transpiled.luau_source).unwrap();
    assert_eq!(r, "python test content");
}

// ── doc.render integration tests ────────────────────────────────

#[test]
fn doc_render_markdown_to_html() {
    let sb = Sandbox::new().unwrap();
    let r = sb
        .exec("return doc.render('# Hello\\n\\nWorld', 'markdown', 'html')")
        .unwrap();
    assert!(r.contains("<h1>Hello</h1>"), "got: {}", r);
    assert!(r.contains("<p>World</p>"), "got: {}", r);
}

#[test]
fn doc_render_html_to_text() {
    let sb = Sandbox::new().unwrap();
    let r = sb
        .exec("return doc.render('<p>Hello <b>World</b></p>', 'html', 'text')")
        .unwrap();
    assert_eq!(r, "Hello World");
}

#[test]
fn doc_render_unsupported_errors() {
    let sb = Sandbox::new().unwrap();
    let err = sb.exec("doc.render('text', 'txt', 'pdf')").unwrap_err();
    assert!(
        err.message.contains("unsupported conversion"),
        "should error: {}",
        err.message
    );
}

// ── doc.read for HTML files ─────────────────────────────────────

#[test]
fn doc_read_html_extracts_text() {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("page.html"),
        "<html><body><h1>Title</h1><p>Content here</p></body></html>",
    )
    .unwrap();
    let sb = sandbox_with_dir(&dir);

    let result = sb.exec(r#"return doc.read("/data/page.html")"#).unwrap();
    assert!(result.contains("Title"), "should have title: {}", result);
    assert!(
        result.contains("Content here"),
        "should have content: {}",
        result
    );
}

// ── doc.renderFile integration tests ────────────────────────────

#[test]
fn doc_render_file_md_to_html() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("input.md"), "# Hello\n\nWorld").unwrap();
    let sb = sandbox_with_rw_dir(&dir);

    sb.exec(r#"doc.renderFile("/workspace/input.md", "/workspace/output.html")"#)
        .unwrap();

    let output = std::fs::read_to_string(dir.path().join("output.html")).unwrap();
    assert!(output.contains("<h1>Hello</h1>"), "got: {}", output);
    assert!(output.contains("<p>World</p>"), "got: {}", output);
}

#[test]
fn doc_render_file_html_to_txt() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("page.html"), "<p>Hello <b>World</b></p>").unwrap();
    let sb = sandbox_with_rw_dir(&dir);

    sb.exec(r#"doc.renderFile("/workspace/page.html", "/workspace/output.txt")"#)
        .unwrap();

    let output = std::fs::read_to_string(dir.path().join("output.txt")).unwrap();
    assert_eq!(output, "Hello World");
}

#[test]
fn doc_render_file_with_format_override() {
    let dir = TempDir::new().unwrap();
    // File has .txt extension but content is markdown
    std::fs::write(dir.path().join("readme.txt"), "# Override Test").unwrap();
    let sb = sandbox_with_rw_dir(&dir);

    sb.exec(r#"doc.renderFile("/workspace/readme.txt", "/workspace/out.html", {from="md"})"#)
        .unwrap();

    let output = std::fs::read_to_string(dir.path().join("out.html")).unwrap();
    assert!(output.contains("<h1>Override Test</h1>"), "got: {}", output);
}

#[test]
fn doc_render_file_unsupported_conversion() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("data.csv"), "a,b\n1,2\n").unwrap();
    let sb = sandbox_with_rw_dir(&dir);

    let err = sb
        .exec(r#"doc.renderFile("/workspace/data.csv", "/workspace/out.pdf")"#)
        .unwrap_err();
    assert!(
        err.message.contains("unsupported conversion"),
        "should error: {}",
        err.message
    );
}

#[test]
fn doc_render_file_no_extension_error() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("noext"), "data").unwrap();
    let sb = sandbox_with_rw_dir(&dir);

    let err = sb
        .exec(r#"doc.renderFile("/workspace/noext", "/workspace/out.txt")"#)
        .unwrap_err();
    assert!(
        err.message.contains("cannot infer input format"),
        "should error: {}",
        err.message
    );
}

#[test]
fn doc_render_file_help_mentions_render_file() {
    let sb = Sandbox::new().unwrap();
    let result = sb.exec("return doc.help()").unwrap();
    assert!(
        result.contains("doc.renderFile"),
        "help should mention renderFile: {}",
        result
    );
}

// ── Named-param table forms ─────────────────────────────────────

#[test]
fn doc_read_named_table() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("note.txt"), "named read").unwrap();
    let sb = sandbox_with_dir(&dir);

    let result = sb
        .exec(r#"return doc.read({path="/data/note.txt"})"#)
        .unwrap();
    assert_eq!(result, "named read");
}

#[test]
fn doc_read_named_table_positional_key() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("note.txt"), "positional key").unwrap();
    let sb = sandbox_with_dir(&dir);

    let result = sb
        .exec(r#"return doc.read({[1]="/data/note.txt"})"#)
        .unwrap();
    assert_eq!(result, "positional key");
}

#[test]
fn doc_read_named_table_with_sheet() {
    let dir = TempDir::new().unwrap();
    create_minimal_xlsx(&dir.path().join("test.xlsx"));
    let sb = sandbox_with_dir(&dir);

    let result = sb
        .exec(r#"return doc.read({path="/data/test.xlsx", sheet=1})"#)
        .unwrap();
    assert!(result.contains("Name"), "should have data: {}", result);
}

#[test]
fn doc_render_named_table() {
    let sb = Sandbox::new().unwrap();
    let r = sb
        .exec(r##"return doc.render({text="# Hello\n\nWorld", from="markdown", to="html"})"##)
        .unwrap();
    assert!(r.contains("<h1>Hello</h1>"), "got: {}", r);
}

#[test]
fn doc_render_file_named_table() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("input.md"), "# Named\n\nTable").unwrap();
    let sb = sandbox_with_rw_dir(&dir);

    sb.exec(r#"doc.renderFile({source="/workspace/input.md", target="/workspace/out.html"})"#)
        .unwrap();

    let output = std::fs::read_to_string(dir.path().join("out.html")).unwrap();
    assert!(output.contains("<h1>Named</h1>"), "got: {}", output);
}

#[test]
fn doc_render_file_named_table_with_opts() {
    let dir = TempDir::new().unwrap();
    // File has .txt extension but content is markdown
    std::fs::write(dir.path().join("input.txt"), "# Override").unwrap();
    let sb = sandbox_with_rw_dir(&dir);

    sb.exec(r#"doc.renderFile({source="/workspace/input.txt", target="/workspace/out.html", from="md"})"#)
        .unwrap();

    let output = std::fs::read_to_string(dir.path().join("out.html")).unwrap();
    assert!(output.contains("<h1>Override</h1>"), "got: {}", output);
}

#[test]
fn doc_render_file_named_table_positional_keys() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("input.md"), "# Positional").unwrap();
    let sb = sandbox_with_rw_dir(&dir);

    sb.exec(r#"doc.renderFile({[1]="/workspace/input.md", [2]="/workspace/out.html"})"#)
        .unwrap();

    let output = std::fs::read_to_string(dir.path().join("out.html")).unwrap();
    assert!(output.contains("<h1>Positional</h1>"), "got: {}", output);
}

// ── DocReadCallback + Cache Tests ────────────────────────────────

/// Helper: build sandbox with a callback and optional cache dir.
fn sandbox_with_callback(
    data_dir: &TempDir,
    cache_dir: Option<std::path::PathBuf>,
    callback: Arc<dyn Fn(&[u8], &str, &str) -> Result<String, String> + Send + Sync>,
) -> Sandbox {
    let mut mt = MountTable::new();
    mt.parse_and_add(&format!("{}:/data:ro", data_dir.path().display()))
        .unwrap();
    let mut builder = Sandbox::builder().mounts(mt).doc_read_callback(callback);
    if let Some(dir) = cache_dir {
        builder = builder.doc_cache_dir(dir);
    }
    builder.build().unwrap()
}

#[test]
fn doc_read_callback_returns_some() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("photo.png"), b"fake png data").unwrap();

    let call_count = Arc::new(AtomicUsize::new(0));
    let cc = call_count.clone();
    let cb: Arc<dyn Fn(&[u8], &str, &str) -> Result<String, String> + Send + Sync> =
        Arc::new(move |_data, filename, query| {
            cc.fetch_add(1, Ordering::SeqCst);
            assert_eq!(filename, "photo.png");
            assert!(query.contains("Extract all content"), "query: {}", query);
            Ok("extracted text from image".to_string())
        });

    let sb = sandbox_with_callback(&dir, None, cb);
    let result = sb.exec(r#"return doc.read("/data/photo.png")"#).unwrap();
    assert_eq!(result, "extracted text from image");
    assert_eq!(call_count.load(Ordering::SeqCst), 1);
}

#[test]
fn doc_read_txt_uses_structural_with_callback() {
    // Text files default to structural mode even when callback is registered
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("note.txt"), "local text content").unwrap();

    let call_count = Arc::new(AtomicUsize::new(0));
    let cc = call_count.clone();
    let cb: Arc<dyn Fn(&[u8], &str, &str) -> Result<String, String> + Send + Sync> =
        Arc::new(move |_data, _filename, _query| {
            cc.fetch_add(1, Ordering::SeqCst);
            Err("should not be called".to_string())
        });

    let sb = sandbox_with_callback(&dir, None, cb);
    let result = sb.exec(r#"return doc.read("/data/note.txt")"#).unwrap();
    assert_eq!(result, "local text content");
    assert_eq!(call_count.load(Ordering::SeqCst), 0); // callback NOT called
}

#[test]
fn doc_read_explicit_vision_mode_on_txt() {
    // Explicitly setting mode="vision" on a text file calls the callback
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("data.txt"), "text content").unwrap();

    let cb: Arc<dyn Fn(&[u8], &str, &str) -> Result<String, String> + Send + Sync> =
        Arc::new(|_data, _filename, _query| Ok("vision result for txt".to_string()));

    let sb = sandbox_with_callback(&dir, None, cb);
    let result = sb
        .exec(r#"return doc.read("/data/data.txt", {mode="vision"})"#)
        .unwrap();
    assert_eq!(result, "vision result for txt");
}

#[test]
fn doc_read_vision_err_propagates() {
    // Vision mode errors propagate — no fallback to structural
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("photo.png"), b"fake image").unwrap();

    let cb: Arc<dyn Fn(&[u8], &str, &str) -> Result<String, String> + Send + Sync> =
        Arc::new(|_data, _filename, _query| Err("network error".to_string()));

    let sb = sandbox_with_callback(&dir, None, cb);
    let err = sb.exec(r#"doc.read("/data/photo.png")"#).unwrap_err();
    assert!(
        err.message.contains("network error"),
        "should propagate callback error: {}",
        err.message
    );
}

#[test]
fn doc_read_callback_err_propagates_for_images() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("photo.png"), b"fake png").unwrap();

    let cb: Arc<dyn Fn(&[u8], &str, &str) -> Result<String, String> + Send + Sync> =
        Arc::new(|_data, _filename, _query| Err("vision service down".to_string()));

    let sb = sandbox_with_callback(&dir, None, cb);
    let err = sb.exec(r#"doc.read("/data/photo.png")"#).unwrap_err();
    assert!(
        err.message.contains("vision service down"),
        "should propagate: {}",
        err.message
    );
}

#[test]
fn doc_read_custom_query_passthrough() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("chart.png"), b"fake png").unwrap();

    let captured_query = Arc::new(std::sync::Mutex::new(String::new()));
    let cq = captured_query.clone();
    let cb: Arc<dyn Fn(&[u8], &str, &str) -> Result<String, String> + Send + Sync> =
        Arc::new(move |_data, _filename, query| {
            *cq.lock().unwrap() = query.to_string();
            Ok("result".to_string())
        });

    let sb = sandbox_with_callback(&dir, None, cb);
    sb.exec(r#"return doc.read("/data/chart.png", {query = "extract all tables"})"#)
        .unwrap();
    assert_eq!(*captured_query.lock().unwrap(), "extract all tables");
}

// ── Mode System Tests ────────────────────────────────────────────

#[test]
fn doc_read_structural_override_on_pdf_skips_callback() {
    // Explicitly setting mode="structural" on a PDF skips the vision callback
    let dir = TempDir::new().unwrap();
    // Create a minimal valid PDF
    std::fs::write(dir.path().join("test.pdf"), b"%PDF-1.0\n1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj\n2 0 obj<</Type/Pages/Kids[3 0 R]/Count 1>>endobj\n3 0 obj<</Type/Page/MediaBox[0 0 612 792]/Parent 2 0 R>>endobj\nxref\n0 4\n0000000000 65535 f \n0000000009 00000 n \n0000000058 00000 n \n0000000115 00000 n \ntrailer<</Size 4/Root 1 0 R>>\nstartxref\n190\n%%EOF").unwrap();

    let call_count = Arc::new(AtomicUsize::new(0));
    let cc = call_count.clone();
    let cb: Arc<dyn Fn(&[u8], &str, &str) -> Result<String, String> + Send + Sync> =
        Arc::new(move |_data, _filename, _query| {
            cc.fetch_add(1, Ordering::SeqCst);
            Ok("should not be used".to_string())
        });

    let sb = sandbox_with_callback(&dir, None, cb);
    // PDF with mode=structural uses local pdf-extract, not callback
    let _result = sb.exec(r#"return doc.read("/data/test.pdf", {mode="structural"})"#);
    assert_eq!(call_count.load(Ordering::SeqCst), 0); // callback NOT called
}

#[test]
fn doc_read_vision_mode_without_callback_errors_for_images() {
    // Images default to vision, but without callback → error
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("photo.png"), b"fake png").unwrap();

    let sb = sandbox_with_dir(&dir);
    let err = sb.exec(r#"doc.read("/data/photo.png")"#).unwrap_err();
    assert!(
        err.message.contains("requires vision") || err.message.contains("vision callback"),
        "should error without vision callback: {}",
        err.message
    );
}

#[test]
fn doc_read_invalid_mode_errors() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("test.txt"), "content").unwrap();
    let sb = sandbox_with_dir(&dir);

    let err = sb
        .exec(r#"doc.read("/data/test.txt", {mode="invalid"})"#)
        .unwrap_err();
    assert!(
        err.message.contains("invalid mode"),
        "should error on invalid mode: {}",
        err.message
    );
}

#[test]
fn doc_help_without_callback_omits_mode() {
    let sb = Sandbox::new().unwrap();
    let result = sb.exec("return doc.help()").unwrap();
    assert!(
        !result.contains("mode"),
        "help without callback should not mention mode: {}",
        result
    );
    // Should still show structural-only description
    assert!(
        result.contains("structural"),
        "help should mention structural: {}",
        result
    );
}

#[test]
fn doc_help_with_callback_shows_mode() {
    let dir = TempDir::new().unwrap();
    let cb: Arc<dyn Fn(&[u8], &str, &str) -> Result<String, String> + Send + Sync> =
        Arc::new(|_data, _filename, _query| Ok("result".to_string()));

    let sb = sandbox_with_callback(&dir, None, cb);
    let result = sb.exec("return doc.help()").unwrap();
    assert!(
        result.contains("mode"),
        "help with callback should show mode option: {}",
        result
    );
    assert!(
        result.contains("structural"),
        "help should mention structural: {}",
        result
    );
    assert!(
        result.contains("vision"),
        "help should mention vision: {}",
        result
    );
}

// ── Disk Cache Tests ─────────────────────────────────────────────

#[test]
fn doc_read_disk_cache_miss_then_hit() {
    let dir = TempDir::new().unwrap();
    let cache_dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("photo.png"), b"image bytes").unwrap();

    let call_count = Arc::new(AtomicUsize::new(0));
    let cc = call_count.clone();
    let cb: Arc<dyn Fn(&[u8], &str, &str) -> Result<String, String> + Send + Sync> =
        Arc::new(move |_data, _filename, _query| {
            cc.fetch_add(1, Ordering::SeqCst);
            Ok("gemini result".to_string())
        });

    let sb = sandbox_with_callback(&dir, Some(cache_dir.path().to_path_buf()), cb.clone());

    // First call: cache miss → callback called
    let r1 = sb.exec(r#"return doc.read("/data/photo.png")"#).unwrap();
    assert_eq!(r1, "gemini result");
    assert_eq!(call_count.load(Ordering::SeqCst), 1);

    // Second call: cache hit → callback NOT called
    let r2 = sb.exec(r#"return doc.read("/data/photo.png")"#).unwrap();
    assert_eq!(r2, "gemini result");
    assert_eq!(call_count.load(Ordering::SeqCst), 1); // Still 1!
}

#[test]
fn doc_read_disk_cache_different_query_different_key() {
    let dir = TempDir::new().unwrap();
    let cache_dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("photo.png"), b"image bytes").unwrap();

    let call_count = Arc::new(AtomicUsize::new(0));
    let cc = call_count.clone();
    let cb: Arc<dyn Fn(&[u8], &str, &str) -> Result<String, String> + Send + Sync> =
        Arc::new(move |_data, _filename, query| {
            cc.fetch_add(1, Ordering::SeqCst);
            Ok(format!("result for: {}", query))
        });

    let sb = sandbox_with_callback(&dir, Some(cache_dir.path().to_path_buf()), cb);

    let r1 = sb
        .exec(r#"return doc.read("/data/photo.png", {query = "extract tables"})"#)
        .unwrap();
    assert_eq!(r1, "result for: extract tables");

    let r2 = sb
        .exec(r#"return doc.read("/data/photo.png", {query = "describe image"})"#)
        .unwrap();
    assert_eq!(r2, "result for: describe image");

    // Both calls should have triggered the callback (different queries)
    assert_eq!(call_count.load(Ordering::SeqCst), 2);
}

// ── readAsync Tests ──────────────────────────────────────────────

#[test]
fn doc_read_async_single_future_resolve() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("photo.png"), b"png data").unwrap();

    let cb: Arc<dyn Fn(&[u8], &str, &str) -> Result<String, String> + Send + Sync> =
        Arc::new(|_data, _filename, _query| Ok("async result".to_string()));

    let sb = sandbox_with_callback(&dir, None, cb);
    let result = sb
        .exec(
            r#"
local f = doc.readAsync("/data/photo.png")
return f:await()
"#,
        )
        .unwrap();
    assert_eq!(result, "async result");
}

#[test]
fn doc_read_async_batch_resolution() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("a.png"), b"data a").unwrap();
    std::fs::write(dir.path().join("b.png"), b"data b").unwrap();
    std::fs::write(dir.path().join("c.png"), b"data c").unwrap();

    let call_count = Arc::new(AtomicUsize::new(0));
    let cc = call_count.clone();
    let cb: Arc<dyn Fn(&[u8], &str, &str) -> Result<String, String> + Send + Sync> =
        Arc::new(move |data, filename, _query| {
            cc.fetch_add(1, Ordering::SeqCst);
            Ok(format!("result for {} ({}B)", filename, data.len()))
        });

    let sb = sandbox_with_callback(&dir, None, cb);
    let result = sb
        .exec(
            r#"
local f1 = doc.readAsync("/data/a.png")
local f2 = doc.readAsync("/data/b.png")
local f3 = doc.readAsync("/data/c.png")
local r1 = f1:await()
local r2 = f2:await()
local r3 = f3:await()
return r1 .. "|" .. r2 .. "|" .. r3
"#,
        )
        .unwrap();
    assert!(result.contains("result for a.png"), "got: {}", result);
    assert!(result.contains("result for b.png"), "got: {}", result);
    assert!(result.contains("result for c.png"), "got: {}", result);
    assert_eq!(call_count.load(Ordering::SeqCst), 3);
}

#[test]
fn doc_read_async_local_only_pre_resolved_without_callback() {
    // Without callback, local-only formats are pre-resolved immediately
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("data.txt"), "local text").unwrap();

    let sb = sandbox_with_dir(&dir);
    let result = sb
        .exec(
            r#"
local f = doc.readAsync("/data/data.txt")
return f:await()
"#,
        )
        .unwrap();
    assert_eq!(result, "local text");
}

#[test]
fn doc_read_async_txt_uses_structural_with_callback() {
    // Text files use structural mode by default even when callback is present
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("data.txt"), "local text").unwrap();

    let call_count = Arc::new(AtomicUsize::new(0));
    let cc = call_count.clone();
    let cb: Arc<dyn Fn(&[u8], &str, &str) -> Result<String, String> + Send + Sync> =
        Arc::new(move |_data, _filename, _query| {
            cc.fetch_add(1, Ordering::SeqCst);
            Ok("should not be called".to_string())
        });

    let sb = sandbox_with_callback(&dir, None, cb);
    let result = sb
        .exec(
            r#"
local f = doc.readAsync("/data/data.txt")
return f:await()
"#,
        )
        .unwrap();
    assert_eq!(result, "local text");
    assert_eq!(call_count.load(Ordering::SeqCst), 0); // callback NOT called
}

#[test]
fn doc_read_async_images_use_vision_csv_uses_structural() {
    // Images default to vision (callback), CSV defaults to structural (local)
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("photo.png"), b"image data").unwrap();
    std::fs::write(dir.path().join("data.csv"), "a,b\n1,2\n").unwrap();

    let cb: Arc<dyn Fn(&[u8], &str, &str) -> Result<String, String> + Send + Sync> =
        Arc::new(|_data, filename, _query| Ok(format!("vision: {}", filename)));

    let sb = sandbox_with_callback(&dir, None, cb);
    let result = sb
        .exec(
            r#"
local f1 = doc.readAsync("/data/photo.png")
local f2 = doc.readAsync("/data/data.csv")
local r1 = f1:await()
local r2 = f2:await()
return r1 .. "|" .. r2
"#,
        )
        .unwrap();
    // photo.png → vision callback, data.csv → structural (local extraction)
    assert_eq!(result, "vision: photo.png|a,b\n1,2\n");
}

#[test]
fn doc_read_async_error_on_await_for_callback_formats() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("photo.png"), b"png data").unwrap();

    let cb: Arc<dyn Fn(&[u8], &str, &str) -> Result<String, String> + Send + Sync> =
        Arc::new(|_data, _filename, _query| Err("api error".to_string()));

    let sb = sandbox_with_callback(&dir, None, cb);
    let err = sb
        .exec(
            r#"
local f = doc.readAsync("/data/photo.png")
f:await()
"#,
        )
        .unwrap_err();
    assert!(err.message.contains("api error"), "got: {}", err.message);
}

#[test]
fn doc_read_async_error_on_read_async_for_local_format_failure() {
    // File doesn't exist — should fail fast on readAsync(), not on await()
    let dir = TempDir::new().unwrap();
    let cb: Arc<dyn Fn(&[u8], &str, &str) -> Result<String, String> + Send + Sync> =
        Arc::new(|_data, _filename, _query| Err("should not be called".to_string()));

    let sb = sandbox_with_callback(&dir, None, cb);
    let err = sb
        .exec(r#"doc.readAsync("/data/missing.txt")"#)
        .unwrap_err();
    assert!(
        err.message.contains("No such file"),
        "should fail fast: {}",
        err.message
    );
}

#[test]
fn doc_read_async_disk_cache_hit_pre_resolves() {
    let dir = TempDir::new().unwrap();
    let cache_dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("photo.png"), b"image bytes").unwrap();

    let call_count = Arc::new(AtomicUsize::new(0));
    let cc = call_count.clone();
    let cb: Arc<dyn Fn(&[u8], &str, &str) -> Result<String, String> + Send + Sync> =
        Arc::new(move |_data, _filename, _query| {
            cc.fetch_add(1, Ordering::SeqCst);
            Ok("fresh result".to_string())
        });

    // First sandbox: populate cache
    let sb1 = sandbox_with_callback(&dir, Some(cache_dir.path().to_path_buf()), cb.clone());
    let r1 = sb1.exec(r#"return doc.read("/data/photo.png")"#).unwrap();
    assert_eq!(r1, "fresh result");
    assert_eq!(call_count.load(Ordering::SeqCst), 1);

    // Second sandbox (simulating restart): readAsync should get cache hit
    let sb2 = sandbox_with_callback(&dir, Some(cache_dir.path().to_path_buf()), cb);
    let r2 = sb2
        .exec(
            r#"
local f = doc.readAsync("/data/photo.png")
return f:await()
"#,
        )
        .unwrap();
    assert_eq!(r2, "fresh result");
    assert_eq!(call_count.load(Ordering::SeqCst), 1); // Still 1 — cache hit
}

#[test]
fn doc_read_async_partial_batch_failure() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("good.png"), b"good image").unwrap();
    std::fs::write(dir.path().join("bad.png"), b"bad image").unwrap();

    let cb: Arc<dyn Fn(&[u8], &str, &str) -> Result<String, String> + Send + Sync> =
        Arc::new(|_data, filename, _query| {
            if filename == "good.png" {
                Ok("good result".to_string())
            } else {
                Err("processing failed".to_string())
            }
        });

    let sb = sandbox_with_callback(&dir, None, cb);
    // good.png should succeed, bad.png should fail
    let result = sb
        .exec(
            r#"
local f1 = doc.readAsync("/data/good.png")
local f2 = doc.readAsync("/data/bad.png")
local ok1, r1 = pcall(function() return f1:await() end)
local ok2, r2 = pcall(function() return f2:await() end)
return tostring(ok1) .. "|" .. r1 .. "|" .. tostring(ok2)
"#,
        )
        .unwrap();
    assert!(result.contains("true|good result|false"), "got: {}", result);
}

#[test]
fn doc_read_async_exceeds_concurrency_limit() {
    // Create 12 files — exceeds MAX_CONCURRENT_READS (8) to exercise the semaphore.
    let dir = TempDir::new().unwrap();
    for i in 0..12 {
        std::fs::write(
            dir.path().join(format!("img{}.png", i)),
            format!("data{}", i),
        )
        .unwrap();
    }

    let call_count = Arc::new(AtomicUsize::new(0));
    let cc = call_count.clone();
    let cb: Arc<dyn Fn(&[u8], &str, &str) -> Result<String, String> + Send + Sync> =
        Arc::new(move |_data, filename, _query| {
            cc.fetch_add(1, Ordering::SeqCst);
            Ok(format!("result-{}", filename))
        });

    let sb = sandbox_with_callback(&dir, None, cb);
    let result = sb
        .exec(
            r#"
local futures = {}
for i = 0, 11 do
    futures[i + 1] = doc.readAsync("/data/img" .. i .. ".png")
end
local results = {}
for i = 1, 12 do
    results[i] = futures[i]:await()
end
return table.concat(results, "|")
"#,
        )
        .unwrap();

    // All 12 should have resolved
    assert_eq!(call_count.load(Ordering::SeqCst), 12);
    for i in 0..12 {
        assert!(
            result.contains(&format!("result-img{}.png", i)),
            "missing result for img{}.png in: {}",
            i,
            result
        );
    }
}

// ── PDFium Integration Tests ─────────────────────────────────────

#[cfg(feature = "pdfium-render")]
mod pdfium_tests {
    use cpsl_core::{MountTable, PdfiumEngine, Sandbox};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    fn crate_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }

    fn engine() -> Arc<PdfiumEngine> {
        Arc::new(
            PdfiumEngine::discover(Some(&crate_root()))
                .expect("PDFium should be discoverable (run scripts/download-pdfium.sh)"),
        )
    }

    fn fixtures_dir() -> PathBuf {
        crate_root().join("tests").join("fixtures").join("pdf")
    }

    /// Build sandbox with PDFium engine and fixtures mounted at /pdf/.
    fn sandbox_with_pdfium() -> Sandbox {
        let mut mt = MountTable::new();
        mt.parse_and_add(&format!("{}:/pdf:ro", fixtures_dir().display()))
            .unwrap();
        Sandbox::builder()
            .mounts(mt)
            .pdfium_engine(engine())
            .build()
            .unwrap()
    }

    /// Build sandbox with PDFium engine, vision callback, and fixtures.
    fn sandbox_with_pdfium_and_callback(
        callback: Arc<dyn Fn(&[u8], &str, &str) -> Result<String, String> + Send + Sync>,
    ) -> Sandbox {
        let mut mt = MountTable::new();
        mt.parse_and_add(&format!("{}:/pdf:ro", fixtures_dir().display()))
            .unwrap();
        Sandbox::builder()
            .mounts(mt)
            .pdfium_engine(engine())
            .doc_read_callback(callback)
            .build()
            .unwrap()
    }

    // ── Structural extraction via PDFium ──────────────────────────

    #[test]
    fn pdfium_read_simple_text() {
        let sb = sandbox_with_pdfium();
        let result = sb
            .exec(r#"return doc.read("/pdf/simple_text.pdf")"#)
            .unwrap();
        assert!(
            result.contains("Hello"),
            "should extract text from simple PDF, got: {}",
            result
        );
    }

    #[test]
    fn pdfium_read_multi_page() {
        let sb = sandbox_with_pdfium();
        let result = sb
            .exec(r#"return doc.read("/pdf/multi_page.pdf")"#)
            .unwrap();
        assert!(
            result.contains("Page 1") && result.contains("Page 2") && result.contains("Page 3"),
            "should extract text from all 3 pages, got: {}",
            result
        );
    }

    #[test]
    fn pdfium_read_unicode() {
        let sb = sandbox_with_pdfium();
        let result = sb
            .exec(r#"return doc.read("/pdf/unicode_text.pdf")"#)
            .unwrap();
        assert!(!result.is_empty(), "should extract unicode text, got empty");
    }

    #[test]
    fn pdfium_read_tables() {
        let sb = sandbox_with_pdfium();
        let result = sb.exec(r#"return doc.read("/pdf/tables.pdf")"#).unwrap();
        assert!(!result.is_empty(), "should extract text from table PDF");
    }

    #[test]
    fn pdfium_read_utf16_metadata_no_panic() {
        // This PDF caused pdf-extract to panic — PDFium should handle it fine
        let sb = sandbox_with_pdfium();
        let result = sb.exec(r#"return doc.read("/pdf/utf16_metadata.pdf")"#);
        // Either succeeds or returns an error — but should NOT panic
        match result {
            Ok(_) => {}
            Err(e) => assert!(
                !e.message.contains("panic"),
                "should not panic: {}",
                e.message
            ),
        }
    }

    #[test]
    fn pdfium_read_empty_pdf() {
        let sb = sandbox_with_pdfium();
        let result = sb.exec(r#"return doc.read("/pdf/empty.pdf")"#).unwrap();
        assert!(
            result.trim().is_empty(),
            "empty PDF should produce empty text, got: {}",
            result
        );
    }

    // ── Flat PDF helpers ─────────────────────────────────────────

    #[test]
    fn pdf_helpers_are_flat_without_pdf_namespace() {
        let sb = sandbox_with_pdfium();
        let result = sb
            .exec(
                r#"
	if rawget(doc, "pdf") ~= nil then
	    return "unexpected doc.pdf namespace"
	end
	local names = {"pdfInfo", "formFields", "fillForm", "mergePdf", "splitPdf", "editPages", "addAnnotation", "watermark"}
	for _, name in ipairs(names) do
	    if type(doc[name]) ~= "function" then
	        return "missing doc." .. name
	    end
	end
	return "ok"
"#,
            )
            .unwrap();
        assert_eq!(result, "ok");
    }

    #[test]
    fn pdf_info_simple() {
        let sb = sandbox_with_pdfium();
        let result = sb
            .exec(
                r#"
local info = doc.pdfInfo("/pdf/simple_text.pdf")
return info.pageCount .. "|" .. tostring(info.hasForm) .. "|" .. #info.pageSizes
"#,
            )
            .unwrap();
        assert_eq!(result, "1|false|1", "simple_text.pdf info: {}", result);
    }

    #[test]
    fn pdf_info_multi_page() {
        let sb = sandbox_with_pdfium();
        let result = sb
            .exec(
                r#"
local info = doc.pdfInfo("/pdf/multi_page.pdf")
return info.pageCount
"#,
            )
            .unwrap();
        assert_eq!(result, "3", "multi_page.pdf should have 3 pages");
    }

    #[test]
    fn pdf_info_form_fields() {
        let sb = sandbox_with_pdfium();
        let result = sb
            .exec(
                r#"
local info = doc.pdfInfo("/pdf/form_fields.pdf")
return tostring(info.hasForm)
"#,
            )
            .unwrap();
        assert_eq!(result, "true", "form_fields.pdf should have forms");
    }

    #[test]
    fn pdf_info_page_sizes() {
        let sb = sandbox_with_pdfium();
        let result = sb
            .exec(
                r#"
local info = doc.pdfInfo("/pdf/simple_text.pdf")
local ps = info.pageSizes[1]
return (ps.width > 0 and ps.height > 0) and "ok" or "bad"
"#,
            )
            .unwrap();
        assert_eq!(result, "ok", "page sizes should be positive");
    }

    #[test]
    fn pdf_info_help_visible() {
        let sb = sandbox_with_pdfium();
        let result = sb.exec("return doc.help()").unwrap();
        for function_name in [
            "doc.pdfInfo",
            "doc.formFields",
            "doc.fillForm",
            "doc.mergePdf",
            "doc.splitPdf",
            "doc.editPages",
            "doc.addAnnotation",
            "doc.watermark",
        ] {
            assert!(
                result.contains(function_name),
                "doc.help should mention {function_name}: {result}"
            );
        }
    }

    #[test]
    fn pdf_flat_functions_are_callable_with_flat_errors() {
        let sb = sandbox_with_pdfium();
        let result = sb
            .exec(
                r#"
local info = doc.pdfInfo("/pdf/simple_text.pdf")
local fields = doc.formFields("/pdf/simple_text.pdf")
local ok, err = pcall(doc.mergePdf, {paths={}, output="/out/merged.pdf"})
local has_canonical_error = string.find(tostring(err), "doc.mergePdf") ~= nil
return table.concat({
    tostring(info.pageCount),
    tostring(#fields),
    tostring(ok),
    tostring(has_canonical_error),
}, "|")
"#,
            )
            .unwrap();
        assert_eq!(result, "1|0|false|true");
    }

    // ── Mode override: structural bypasses callback ──────────────

    #[test]
    fn pdfium_structural_mode_bypasses_callback() {
        let call_count = Arc::new(AtomicUsize::new(0));
        let cc = call_count.clone();
        let cb: Arc<dyn Fn(&[u8], &str, &str) -> Result<String, String> + Send + Sync> =
            Arc::new(move |_data, _filename, _query| {
                cc.fetch_add(1, Ordering::SeqCst);
                Ok("vision result".to_string())
            });

        let sb = sandbox_with_pdfium_and_callback(cb);
        let result = sb
            .exec(r#"return doc.read("/pdf/simple_text.pdf", {mode="structural"})"#)
            .unwrap();
        assert!(
            result.contains("Hello"),
            "structural mode should use PDFium: {}",
            result
        );
        assert_eq!(
            call_count.load(Ordering::SeqCst),
            0,
            "callback should NOT be called in structural mode"
        );
    }

    #[test]
    fn pdfium_default_vision_with_callback() {
        let call_count = Arc::new(AtomicUsize::new(0));
        let cc = call_count.clone();
        let cb: Arc<dyn Fn(&[u8], &str, &str) -> Result<String, String> + Send + Sync> =
            Arc::new(move |_data, _filename, _query| {
                cc.fetch_add(1, Ordering::SeqCst);
                Ok("vision analysis".to_string())
            });

        let sb = sandbox_with_pdfium_and_callback(cb);
        let result = sb
            .exec(r#"return doc.read("/pdf/simple_text.pdf")"#)
            .unwrap();
        assert_eq!(result, "vision analysis");
        assert_eq!(
            call_count.load(Ordering::SeqCst),
            1,
            "callback should be called in default vision mode"
        );
    }

    // ── readAsync with PDFium ────────────────────────────────────

    /// Build sandbox with PDFium, fixtures at /pdf/ (ro), and a writable output dir.
    fn sandbox_with_pdfium_and_output(output_dir: &std::path::Path) -> Sandbox {
        let mut mt = MountTable::new();
        mt.parse_and_add(&format!("{}:/pdf:ro", fixtures_dir().display()))
            .unwrap();
        mt.parse_and_add(&format!("{}:/out", output_dir.display()))
            .unwrap();
        Sandbox::builder()
            .mounts(mt)
            .pdfium_engine(engine())
            .build()
            .unwrap()
    }

    // ── Form field listing ──────────────────────────────────────────

    #[test]
    fn form_fields_lists_fields() {
        let sb = sandbox_with_pdfium();
        let result = sb
            .exec(
                r#"
local fields = doc.formFields("/pdf/form_fields.pdf")
local out = {}
for _, f in ipairs(fields) do
    table.insert(out, f.name .. ":" .. f.type)
end
return table.concat(out, ",")
"#,
            )
            .unwrap();
        // The form_fields.pdf has text, checkbox, and combobox fields
        assert!(!result.is_empty(), "should list form fields, got empty");
        assert!(
            result.contains("text"),
            "should contain a text field type: {}",
            result
        );
    }

    #[test]
    fn form_fields_empty_for_non_form_pdf() {
        let sb = sandbox_with_pdfium();
        let result = sb
            .exec(
                r#"
local fields = doc.formFields("/pdf/simple_text.pdf")
return tostring(#fields)
"#,
            )
            .unwrap();
        assert_eq!(result, "0", "non-form PDF should have 0 fields");
    }

    #[test]
    fn form_fields_returns_read_only_flag() {
        let sb = sandbox_with_pdfium();
        let result = sb
            .exec(
                r#"
local fields = doc.formFields("/pdf/form_fields.pdf")
local types = {}
for _, f in ipairs(fields) do
    table.insert(types, tostring(f.readOnly))
end
return table.concat(types, ",")
"#,
            )
            .unwrap();
        // Should contain boolean strings
        assert!(
            result.contains("true") || result.contains("false"),
            "readOnly should be boolean: {}",
            result
        );
    }

    // ── Form filling ────────────────────────────────────────────────

    #[test]
    fn fill_form_round_trip() {
        let output_dir = tempfile::TempDir::new().unwrap();
        let sb = sandbox_with_pdfium_and_output(output_dir.path());
        // Fill text field, write output, then re-read with formFields to verify
        let result = sb
            .exec(
                r#"
-- First, discover field names
local fields = doc.formFields("/pdf/form_fields.pdf")
local text_field_name = nil
for _, f in ipairs(fields) do
    if f.type == "text" then
        text_field_name = f.name
        break
    end
end

if text_field_name == nil then
    error("no text field found in form_fields.pdf")
end

-- Fill the text field
local fill_fields = {}
fill_fields[text_field_name] = "TestValue123"
doc.fillForm({path="/pdf/form_fields.pdf", fields=fill_fields, output="/out/filled.pdf"})

-- Re-read the filled PDF
local fields2 = doc.formFields("/out/filled.pdf")
for _, f in ipairs(fields2) do
    if f.name == text_field_name then
        return f.value
    end
end
return "FIELD_NOT_FOUND"
"#,
            )
            .unwrap();
        assert_eq!(
            result, "TestValue123",
            "filled value should be readable: {}",
            result
        );
    }

    #[test]
    fn fill_form_nonexistent_field_errors() {
        let output_dir = tempfile::TempDir::new().unwrap();
        let sb = sandbox_with_pdfium_and_output(output_dir.path());
        let err = sb
            .exec(
                r#"
doc.fillForm({
    path="/pdf/form_fields.pdf",
    fields={nonexistent_field_xyz="value"},
    output="/out/filled.pdf"
})
"#,
            )
            .unwrap_err();
        assert!(
            err.message.contains("does not exist"),
            "should error on non-existent field: {}",
            err.message
        );
    }

    #[test]
    fn fill_form_flatten_removes_fields() {
        let output_dir = tempfile::TempDir::new().unwrap();
        let sb = sandbox_with_pdfium_and_output(output_dir.path());
        let result = sb
            .exec(
                r#"
-- First find a text field
local fields = doc.formFields("/pdf/form_fields.pdf")
local text_field_name = nil
for _, f in ipairs(fields) do
    if f.type == "text" then
        text_field_name = f.name
        break
    end
end

if text_field_name == nil then
    error("no text field found")
end

-- Fill and flatten
local fill_fields = {}
fill_fields[text_field_name] = "FlattenTest"
doc.fillForm({path="/pdf/form_fields.pdf", fields=fill_fields, output="/out/flat.pdf", flatten=true})

-- Re-read: flattened PDF should have no form fields
local fields2 = doc.formFields("/out/flat.pdf")
return tostring(#fields2)
"#,
            )
            .unwrap();
        assert_eq!(
            result, "0",
            "flattened PDF should have 0 form fields, got: {}",
            result
        );
    }

    #[test]
    fn pdfium_read_async_structural() {
        let sb = sandbox_with_pdfium();
        let result = sb
            .exec(
                r#"
local f = doc.readAsync("/pdf/simple_text.pdf")
return f:await()
"#,
            )
            .unwrap();
        assert!(
            result.contains("Hello"),
            "async structural should use PDFium: {}",
            result
        );
    }

    // ── Merge PDF ──────────────────────────────────────────────────

    #[test]
    fn merge_two_pdfs_page_count() {
        let output_dir = tempfile::TempDir::new().unwrap();
        let sb = sandbox_with_pdfium_and_output(output_dir.path());
        // simple_text.pdf has 1 page, multi_page.pdf has 3 pages → merged should have 4
        let result = sb
            .exec(
                r#"
doc.mergePdf({
    paths={"/pdf/simple_text.pdf", "/pdf/multi_page.pdf"},
    output="/out/merged.pdf"
})
local info = doc.pdfInfo("/out/merged.pdf")
return tostring(info.pageCount)
"#,
            )
            .unwrap();
        assert_eq!(
            result, "4",
            "merged PDF should have 4 pages, got: {}",
            result
        );
    }

    #[test]
    fn merge_three_pdfs_page_count() {
        let output_dir = tempfile::TempDir::new().unwrap();
        let sb = sandbox_with_pdfium_and_output(output_dir.path());
        // 1 + 3 + 1 = 5 pages
        let result = sb
            .exec(
                r#"
doc.mergePdf({
    paths={"/pdf/simple_text.pdf", "/pdf/multi_page.pdf", "/pdf/simple_text.pdf"},
    output="/out/merged.pdf"
})
local info = doc.pdfInfo("/out/merged.pdf")
return tostring(info.pageCount)
"#,
            )
            .unwrap();
        assert_eq!(
            result, "5",
            "merged 3 PDFs should have 5 pages, got: {}",
            result
        );
    }

    #[test]
    fn merge_preserves_text_content() {
        let output_dir = tempfile::TempDir::new().unwrap();
        let sb = sandbox_with_pdfium_and_output(output_dir.path());
        let result = sb
            .exec(
                r#"
doc.mergePdf({
    paths={"/pdf/simple_text.pdf", "/pdf/multi_page.pdf"},
    output="/out/merged.pdf"
})
return doc.read("/out/merged.pdf")
"#,
            )
            .unwrap();
        assert!(
            result.contains("Hello") && result.contains("Page 1"),
            "merged PDF should contain text from both sources, got: {}",
            result
        );
    }

    #[test]
    fn merge_missing_file_errors() {
        let output_dir = tempfile::TempDir::new().unwrap();
        let sb = sandbox_with_pdfium_and_output(output_dir.path());
        let err = sb
            .exec(
                r#"
doc.mergePdf({
    paths={"/pdf/simple_text.pdf", "/pdf/nonexistent.pdf"},
    output="/out/merged.pdf"
})
"#,
            )
            .unwrap_err();
        assert!(
            err.message.contains("nonexistent")
                || err.message.contains("No such file")
                || err.message.contains("not found"),
            "should error on missing file: {}",
            err.message
        );
    }

    // ── Split PDF ──────────────────────────────────────────────────

    #[test]
    fn split_pdf_into_parts() {
        let output_dir = tempfile::TempDir::new().unwrap();
        let sb = sandbox_with_pdfium_and_output(output_dir.path());
        // multi_page.pdf has 3 pages → split into {1-2} and {3}
        let result = sb
            .exec(
                r#"
local paths = doc.splitPdf({
    path="/pdf/multi_page.pdf",
    ranges={"1-2", "3"},
    outputDir="/out/"
})
local info1 = doc.pdfInfo(paths[1])
local info2 = doc.pdfInfo(paths[2])
return info1.pageCount .. "," .. info2.pageCount
"#,
            )
            .unwrap();
        assert_eq!(
            result, "2,1",
            "split should produce 2-page and 1-page parts, got: {}",
            result
        );
    }

    #[test]
    fn split_pdf_returns_paths() {
        let output_dir = tempfile::TempDir::new().unwrap();
        let sb = sandbox_with_pdfium_and_output(output_dir.path());
        let result = sb
            .exec(
                r#"
local paths = doc.splitPdf({
    path="/pdf/multi_page.pdf",
    ranges={"1", "2-3"},
    outputDir="/out/"
})
return #paths .. "|" .. paths[1] .. "|" .. paths[2]
"#,
            )
            .unwrap();
        assert!(
            result.contains("2|")
                && result.contains("split_1.pdf")
                && result.contains("split_2.pdf"),
            "should return 2 output paths, got: {}",
            result
        );
    }

    #[test]
    fn split_pdf_single_pages() {
        let output_dir = tempfile::TempDir::new().unwrap();
        let sb = sandbox_with_pdfium_and_output(output_dir.path());
        // Split each page individually
        let result = sb
            .exec(
                r#"
local paths = doc.splitPdf({
    path="/pdf/multi_page.pdf",
    ranges={"1", "2", "3"},
    outputDir="/out/"
})
local counts = {}
for _, p in ipairs(paths) do
    local info = doc.pdfInfo(p)
    table.insert(counts, tostring(info.pageCount))
end
return table.concat(counts, ",")
"#,
            )
            .unwrap();
        assert_eq!(
            result, "1,1,1",
            "each split part should have 1 page, got: {}",
            result
        );
    }

    #[test]
    fn split_pdf_invalid_range_errors() {
        let output_dir = tempfile::TempDir::new().unwrap();
        let sb = sandbox_with_pdfium_and_output(output_dir.path());
        let err = sb
            .exec(
                r#"
doc.splitPdf({
    path="/pdf/multi_page.pdf",
    ranges={"1-10"},
    outputDir="/out/"
})
"#,
            )
            .unwrap_err();
        assert!(
            err.message.contains("exceeds"),
            "should error on invalid range: {}",
            err.message
        );
    }

    // ── Edit Pages: delete ──────────────────────────────────────────

    #[test]
    fn edit_pages_delete() {
        let output_dir = tempfile::TempDir::new().unwrap();
        let sb = sandbox_with_pdfium_and_output(output_dir.path());
        // multi_page.pdf has 3 pages → delete page 2 → should have 2
        let result = sb
            .exec(
                r#"
doc.editPages({
    path="/pdf/multi_page.pdf",
    operations={{type="delete", pages={2}}},
    output="/out/edited.pdf"
})
local info = doc.pdfInfo("/out/edited.pdf")
return tostring(info.pageCount)
"#,
            )
            .unwrap();
        assert_eq!(
            result, "2",
            "should have 2 pages after deleting one, got: {}",
            result
        );
    }

    #[test]
    fn edit_pages_delete_multiple() {
        let output_dir = tempfile::TempDir::new().unwrap();
        let sb = sandbox_with_pdfium_and_output(output_dir.path());
        // Delete pages 1 and 3 from 3-page doc → 1 page left
        let result = sb
            .exec(
                r#"
doc.editPages({
    path="/pdf/multi_page.pdf",
    operations={{type="delete", pages={1, 3}}},
    output="/out/edited.pdf"
})
local info = doc.pdfInfo("/out/edited.pdf")
return tostring(info.pageCount)
"#,
            )
            .unwrap();
        assert_eq!(
            result, "1",
            "should have 1 page after deleting two, got: {}",
            result
        );
    }

    // ── Edit Pages: rotate ──────────────────────────────────────────

    #[test]
    fn edit_pages_rotate() {
        let output_dir = tempfile::TempDir::new().unwrap();
        let sb = sandbox_with_pdfium_and_output(output_dir.path());
        // Rotate page 1 by 90 degrees — verify it produces a valid PDF
        let result = sb
            .exec(
                r#"
doc.editPages({
    path="/pdf/multi_page.pdf",
    operations={{type="rotate", pages={1}, degrees=90}},
    output="/out/rotated.pdf"
})
local info = doc.pdfInfo("/out/rotated.pdf")
return tostring(info.pageCount)
"#,
            )
            .unwrap();
        assert_eq!(
            result, "3",
            "rotated PDF should still have 3 pages, got: {}",
            result
        );
    }

    #[test]
    fn edit_pages_rotate_invalid_degrees_errors() {
        let output_dir = tempfile::TempDir::new().unwrap();
        let sb = sandbox_with_pdfium_and_output(output_dir.path());
        let err = sb
            .exec(
                r#"
doc.editPages({
    path="/pdf/multi_page.pdf",
    operations={{type="rotate", pages={1}, degrees=45}},
    output="/out/rotated.pdf"
})
"#,
            )
            .unwrap_err();
        assert!(
            err.message.contains("invalid rotation"),
            "should error on invalid rotation: {}",
            err.message
        );
    }

    // ── Edit Pages: reorder ──────────────────────────────────────────

    #[test]
    fn edit_pages_reorder() {
        let output_dir = tempfile::TempDir::new().unwrap();
        let sb = sandbox_with_pdfium_and_output(output_dir.path());
        // Reverse 3 pages: {3,2,1}
        let result = sb
            .exec(
                r#"
doc.editPages({
    path="/pdf/multi_page.pdf",
    operations={{type="reorder", order={3, 2, 1}}},
    output="/out/reordered.pdf"
})
local info = doc.pdfInfo("/out/reordered.pdf")
return tostring(info.pageCount)
"#,
            )
            .unwrap();
        assert_eq!(
            result, "3",
            "reordered PDF should still have 3 pages, got: {}",
            result
        );
    }

    #[test]
    fn edit_pages_reorder_verifies_content() {
        let output_dir = tempfile::TempDir::new().unwrap();
        let sb = sandbox_with_pdfium_and_output(output_dir.path());
        // Reverse pages, then read text to verify order changed
        let result = sb
            .exec(
                r#"
doc.editPages({
    path="/pdf/multi_page.pdf",
    operations={{type="reorder", order={3, 2, 1}}},
    output="/out/reordered.pdf"
})
return doc.read("/out/reordered.pdf")
"#,
            )
            .unwrap();
        // The text should contain Page 3 before Page 1
        let pos3 = result.find("Page 3");
        let pos1 = result.find("Page 1");
        assert!(
            pos3.is_some() && pos1.is_some() && pos3.unwrap() < pos1.unwrap(),
            "after reordering [3,2,1], Page 3 should come before Page 1 in text, got: {}",
            result
        );
    }

    #[test]
    fn edit_pages_reorder_wrong_count_errors() {
        let output_dir = tempfile::TempDir::new().unwrap();
        let sb = sandbox_with_pdfium_and_output(output_dir.path());
        let err = sb
            .exec(
                r#"
doc.editPages({
    path="/pdf/multi_page.pdf",
    operations={{type="reorder", order={1, 2}}},
    output="/out/reordered.pdf"
})
"#,
            )
            .unwrap_err();
        assert!(
            err.message.contains("2 entries") && err.message.contains("3 pages"),
            "should error on wrong reorder count: {}",
            err.message
        );
    }

    #[test]
    fn edit_pages_invalid_page_errors() {
        let output_dir = tempfile::TempDir::new().unwrap();
        let sb = sandbox_with_pdfium_and_output(output_dir.path());
        let err = sb
            .exec(
                r#"
doc.editPages({
    path="/pdf/multi_page.pdf",
    operations={{type="delete", pages={99}}},
    output="/out/edited.pdf"
})
"#,
            )
            .unwrap_err();
        assert!(
            err.message.contains("does not exist"),
            "should error on invalid page: {}",
            err.message
        );
    }

    #[test]
    fn edit_pages_unknown_operation_errors() {
        let output_dir = tempfile::TempDir::new().unwrap();
        let sb = sandbox_with_pdfium_and_output(output_dir.path());
        let err = sb
            .exec(
                r#"
doc.editPages({
    path="/pdf/multi_page.pdf",
    operations={{type="shuffle"}},
    output="/out/edited.pdf"
})
"#,
            )
            .unwrap_err();
        assert!(
            err.message.contains("unknown operation"),
            "should error on unknown operation: {}",
            err.message
        );
    }

    // ── Help includes new functions ──────────────────────────────────

    #[test]
    fn help_includes_page_manipulation_functions() {
        let sb = sandbox_with_pdfium();
        let result = sb.exec("return doc.help()").unwrap();
        assert!(
            result.contains("doc.mergePdf"),
            "help should mention doc.mergePdf: {}",
            result
        );
        assert!(
            result.contains("doc.splitPdf"),
            "help should mention doc.splitPdf: {}",
            result
        );
        assert!(
            result.contains("doc.editPages"),
            "help should mention doc.editPages: {}",
            result
        );
    }

    // ── Annotations ─────────────────────────────────────────────────

    #[test]
    fn add_text_annotation() {
        let output_dir = tempfile::TempDir::new().unwrap();
        let sb = sandbox_with_pdfium_and_output(output_dir.path());
        sb.exec(
            r#"
doc.addAnnotation({
    path="/pdf/simple_text.pdf",
    page=1,
    type="text",
    x=72, y=700, width=24, height=24,
    contents="This is a note",
    output="/out/annotated.pdf"
})
"#,
        )
        .unwrap();
        // Verify output is a valid PDF with the same page count
        let result = sb
            .exec(r#"return tostring(doc.pdfInfo("/out/annotated.pdf").pageCount)"#)
            .unwrap();
        assert_eq!(result, "1");
    }

    #[test]
    fn add_highlight_annotation() {
        let output_dir = tempfile::TempDir::new().unwrap();
        let sb = sandbox_with_pdfium_and_output(output_dir.path());
        sb.exec(
            r##"
doc.addAnnotation({
    path="/pdf/simple_text.pdf",
    page=1,
    type="highlight",
    x=72, y=700, width=200, height=14,
    color="#FFFF00",
    contents="Highlighted text",
    output="/out/highlighted.pdf"
})
"##,
        )
        .unwrap();
        let result = sb
            .exec(r#"return tostring(doc.pdfInfo("/out/highlighted.pdf").pageCount)"#)
            .unwrap();
        assert_eq!(result, "1");
    }

    #[test]
    fn add_free_text_annotation() {
        let output_dir = tempfile::TempDir::new().unwrap();
        let sb = sandbox_with_pdfium_and_output(output_dir.path());
        sb.exec(
            r#"
doc.addAnnotation({
    path="/pdf/simple_text.pdf",
    page=1,
    type="freeText",
    x=72, y=500, width=200, height=30,
    contents="Free text on the page",
    output="/out/freetext.pdf"
})
"#,
        )
        .unwrap();
        let result = sb
            .exec(r#"return tostring(doc.pdfInfo("/out/freetext.pdf").pageCount)"#)
            .unwrap();
        assert_eq!(result, "1");
    }

    #[test]
    fn add_square_annotation() {
        let output_dir = tempfile::TempDir::new().unwrap();
        let sb = sandbox_with_pdfium_and_output(output_dir.path());
        sb.exec(
            r##"
doc.addAnnotation({
    path="/pdf/simple_text.pdf",
    page=1,
    type="square",
    x=72, y=600, width=100, height=50,
    color="#FF0000",
    output="/out/square.pdf"
})
"##,
        )
        .unwrap();
        let result = sb
            .exec(r#"return tostring(doc.pdfInfo("/out/square.pdf").pageCount)"#)
            .unwrap();
        assert_eq!(result, "1");
    }

    #[test]
    fn add_underline_annotation() {
        let output_dir = tempfile::TempDir::new().unwrap();
        let sb = sandbox_with_pdfium_and_output(output_dir.path());
        sb.exec(
            r#"
doc.addAnnotation({
    path="/pdf/simple_text.pdf",
    page=1,
    type="underline",
    x=72, y=700, width=200, height=14,
    output="/out/underlined.pdf"
})
"#,
        )
        .unwrap();
        let result = sb
            .exec(r#"return tostring(doc.pdfInfo("/out/underlined.pdf").pageCount)"#)
            .unwrap();
        assert_eq!(result, "1");
    }

    #[test]
    fn add_strikeout_annotation() {
        let output_dir = tempfile::TempDir::new().unwrap();
        let sb = sandbox_with_pdfium_and_output(output_dir.path());
        sb.exec(
            r#"
doc.addAnnotation({
    path="/pdf/simple_text.pdf",
    page=1,
    type="strikeout",
    x=72, y=700, width=200, height=14,
    output="/out/strikeout.pdf"
})
"#,
        )
        .unwrap();
        let result = sb
            .exec(r#"return tostring(doc.pdfInfo("/out/strikeout.pdf").pageCount)"#)
            .unwrap();
        assert_eq!(result, "1");
    }

    #[test]
    fn add_stamp_annotation() {
        let output_dir = tempfile::TempDir::new().unwrap();
        let sb = sandbox_with_pdfium_and_output(output_dir.path());
        sb.exec(
            r#"
doc.addAnnotation({
    path="/pdf/simple_text.pdf",
    page=1,
    type="stamp",
    x=200, y=400, width=150, height=50,
    contents="APPROVED",
    output="/out/stamped.pdf"
})
"#,
        )
        .unwrap();
        let result = sb
            .exec(r#"return tostring(doc.pdfInfo("/out/stamped.pdf").pageCount)"#)
            .unwrap();
        assert_eq!(result, "1");
    }

    #[test]
    fn add_annotation_invalid_type_errors() {
        let output_dir = tempfile::TempDir::new().unwrap();
        let sb = sandbox_with_pdfium_and_output(output_dir.path());
        let err = sb
            .exec(
                r#"
doc.addAnnotation({
    path="/pdf/simple_text.pdf",
    page=1,
    type="circle",
    x=72, y=700, width=50, height=50,
    output="/out/annotated.pdf"
})
"#,
            )
            .unwrap_err();
        assert!(
            err.message.contains("unknown type"),
            "should error on unknown annotation type: {}",
            err.message
        );
    }

    #[test]
    fn add_annotation_invalid_page_errors() {
        let output_dir = tempfile::TempDir::new().unwrap();
        let sb = sandbox_with_pdfium_and_output(output_dir.path());
        let err = sb
            .exec(
                r#"
doc.addAnnotation({
    path="/pdf/simple_text.pdf",
    page=99,
    type="text",
    x=72, y=700, width=24, height=24,
    output="/out/annotated.pdf"
})
"#,
            )
            .unwrap_err();
        assert!(
            err.message.contains("does not exist"),
            "should error on invalid page: {}",
            err.message
        );
    }

    #[test]
    fn add_annotation_with_rgba_color() {
        let output_dir = tempfile::TempDir::new().unwrap();
        let sb = sandbox_with_pdfium_and_output(output_dir.path());
        // Test #RRGGBBAA color format
        sb.exec(
            r##"
doc.addAnnotation({
    path="/pdf/simple_text.pdf",
    page=1,
    type="highlight",
    x=72, y=700, width=200, height=14,
    color="#FFFF0080",
    output="/out/annotated.pdf"
})
"##,
        )
        .unwrap();
    }

    #[test]
    fn add_annotation_on_multipage_pdf() {
        let output_dir = tempfile::TempDir::new().unwrap();
        let sb = sandbox_with_pdfium_and_output(output_dir.path());
        // Add annotation to page 2 of a 3-page doc
        sb.exec(
            r#"
doc.addAnnotation({
    path="/pdf/multi_page.pdf",
    page=2,
    type="text",
    x=72, y=700, width=24, height=24,
    contents="Note on page 2",
    output="/out/annotated.pdf"
})
"#,
        )
        .unwrap();
        let result = sb
            .exec(r#"return tostring(doc.pdfInfo("/out/annotated.pdf").pageCount)"#)
            .unwrap();
        assert_eq!(result, "3");
    }

    // ── Watermarks ──────────────────────────────────────────────────

    #[test]
    fn watermark_all_pages() {
        let output_dir = tempfile::TempDir::new().unwrap();
        let sb = sandbox_with_pdfium_and_output(output_dir.path());
        sb.exec(
            r#"
doc.watermark({
    path="/pdf/multi_page.pdf",
    text="CONFIDENTIAL",
    output="/out/watermarked.pdf"
})
"#,
        )
        .unwrap();
        let result = sb
            .exec(r#"return tostring(doc.pdfInfo("/out/watermarked.pdf").pageCount)"#)
            .unwrap();
        assert_eq!(result, "3");
    }

    #[test]
    fn watermark_with_custom_options() {
        let output_dir = tempfile::TempDir::new().unwrap();
        let sb = sandbox_with_pdfium_and_output(output_dir.path());
        sb.exec(
            r##"
doc.watermark({
    path="/pdf/multi_page.pdf",
    text="DRAFT",
    fontSize=72,
    color="#FF000040",
    rotation=30,
    output="/out/watermarked.pdf"
})
"##,
        )
        .unwrap();
        let result = sb
            .exec(r#"return tostring(doc.pdfInfo("/out/watermarked.pdf").pageCount)"#)
            .unwrap();
        assert_eq!(result, "3");
    }

    #[test]
    fn watermark_selective_pages() {
        let output_dir = tempfile::TempDir::new().unwrap();
        let sb = sandbox_with_pdfium_and_output(output_dir.path());
        // Only watermark page 1
        sb.exec(
            r#"
doc.watermark({
    path="/pdf/multi_page.pdf",
    text="PAGE ONE ONLY",
    pages="1",
    output="/out/watermarked.pdf"
})
"#,
        )
        .unwrap();
        let result = sb
            .exec(r#"return tostring(doc.pdfInfo("/out/watermarked.pdf").pageCount)"#)
            .unwrap();
        assert_eq!(result, "3");
    }

    #[test]
    fn watermark_page_range() {
        let output_dir = tempfile::TempDir::new().unwrap();
        let sb = sandbox_with_pdfium_and_output(output_dir.path());
        // Watermark pages 2-3
        sb.exec(
            r#"
doc.watermark({
    path="/pdf/multi_page.pdf",
    text="PAGES 2-3",
    pages="2-3",
    output="/out/watermarked.pdf"
})
"#,
        )
        .unwrap();
        let result = sb
            .exec(r#"return tostring(doc.pdfInfo("/out/watermarked.pdf").pageCount)"#)
            .unwrap();
        assert_eq!(result, "3");
    }

    #[test]
    fn watermark_invalid_page_range_errors() {
        let output_dir = tempfile::TempDir::new().unwrap();
        let sb = sandbox_with_pdfium_and_output(output_dir.path());
        let err = sb
            .exec(
                r#"
doc.watermark({
    path="/pdf/multi_page.pdf",
    text="TOO FAR",
    pages="1-99",
    output="/out/watermarked.pdf"
})
"#,
            )
            .unwrap_err();
        assert!(
            err.message.contains("exceeds"),
            "should error on invalid page range: {}",
            err.message
        );
    }

    #[test]
    fn watermark_pages_all() {
        let output_dir = tempfile::TempDir::new().unwrap();
        let sb = sandbox_with_pdfium_and_output(output_dir.path());
        // Explicit "all" pages
        sb.exec(
            r#"
doc.watermark({
    path="/pdf/multi_page.pdf",
    text="ALL PAGES",
    pages="all",
    output="/out/watermarked.pdf"
})
"#,
        )
        .unwrap();
    }

    #[test]
    fn watermark_zero_rotation() {
        let output_dir = tempfile::TempDir::new().unwrap();
        let sb = sandbox_with_pdfium_and_output(output_dir.path());
        sb.exec(
            r#"
doc.watermark({
    path="/pdf/simple_text.pdf",
    text="NO ROTATION",
    rotation=0,
    output="/out/watermarked.pdf"
})
"#,
        )
        .unwrap();
        let result = sb
            .exec(r#"return tostring(doc.pdfInfo("/out/watermarked.pdf").pageCount)"#)
            .unwrap();
        assert_eq!(result, "1");
    }

    // ── Help includes annotation & watermark functions ───────────────

    #[test]
    fn help_includes_annotation_and_watermark_functions() {
        let sb = sandbox_with_pdfium();
        let result = sb.exec("return doc.help()").unwrap();
        assert!(
            result.contains("doc.addAnnotation"),
            "help should mention doc.addAnnotation: {}",
            result
        );
        assert!(
            result.contains("doc.watermark"),
            "help should mention doc.watermark: {}",
            result
        );
    }
}
