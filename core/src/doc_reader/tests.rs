//! Tests for document reading, conversion, and rendering helpers.

use super::*;

#[test]
fn test_format_detection() {
    assert_eq!(
        DocFormat::from_extension("report.xlsx"),
        Some(DocFormat::Xlsx)
    );
    assert_eq!(
        DocFormat::from_extension("report.XLSX"),
        Some(DocFormat::Xlsx)
    );
    assert_eq!(DocFormat::from_extension("data.csv"), Some(DocFormat::Csv));
    assert_eq!(DocFormat::from_extension("doc.pdf"), Some(DocFormat::Pdf));
    assert_eq!(DocFormat::from_extension("doc.docx"), Some(DocFormat::Docx));
    assert_eq!(
        DocFormat::from_extension("slides.pptx"),
        Some(DocFormat::Pptx)
    );
    assert_eq!(DocFormat::from_extension("doc.rtf"), Some(DocFormat::Rtf));
    assert_eq!(DocFormat::from_extension("noext"), None);
    assert_eq!(DocFormat::from_extension("file.xyz"), None);
    // Image formats
    assert_eq!(DocFormat::from_extension("photo.png"), Some(DocFormat::Png));
    assert_eq!(DocFormat::from_extension("photo.PNG"), Some(DocFormat::Png));
    assert_eq!(DocFormat::from_extension("photo.jpg"), Some(DocFormat::Jpg));
    assert_eq!(
        DocFormat::from_extension("photo.jpeg"),
        Some(DocFormat::Jpg)
    );
    assert_eq!(
        DocFormat::from_extension("photo.JPEG"),
        Some(DocFormat::Jpg)
    );
    assert_eq!(
        DocFormat::from_extension("photo.webp"),
        Some(DocFormat::Webp)
    );
    assert_eq!(DocFormat::from_extension("photo.gif"), Some(DocFormat::Gif));
}

#[test]
fn test_image_format_local_extraction_fails() {
    let data = b"fake image data";
    for format in [
        DocFormat::Png,
        DocFormat::Jpg,
        DocFormat::Webp,
        DocFormat::Gif,
    ] {
        let result = read_document(data, format, &ReadOptions::default());
        assert!(result.is_err(), "expected error for {:?}", format);
        assert!(result.unwrap_err().contains("requires vision callback"));
    }
}

#[test]
fn test_format_needs_callback() {
    assert!(DocFormat::Png.needs_callback());
    assert!(DocFormat::Jpg.needs_callback());
    assert!(DocFormat::Webp.needs_callback());
    assert!(DocFormat::Gif.needs_callback());
    assert!(DocFormat::Pdf.needs_callback());
    assert!(!DocFormat::Xlsx.needs_callback());
    assert!(!DocFormat::Csv.needs_callback());
    assert!(!DocFormat::Txt.needs_callback());
}

#[test]
fn test_plain_text_roundtrip() {
    let data = b"Hello, world!";
    let result = read_document(data, DocFormat::Txt, &ReadOptions::default()).unwrap();
    assert_eq!(result, "Hello, world!");
}

#[test]
fn test_csv_roundtrip() {
    let data = b"a,b,c\n1,2,3\n";
    let result = read_document(data, DocFormat::Csv, &ReadOptions::default()).unwrap();
    assert_eq!(result, "a,b,c\n1,2,3\n");
}

#[test]
fn test_xml_text_extraction() {
    let xml = r#"<root><w:t>Hello </w:t><w:t>World</w:t></root>"#;
    let result = extract_text_from_xml(xml, "w:t").unwrap();
    assert_eq!(result, "Hello World");
}

#[test]
fn test_xml_text_extraction_pptx() {
    let xml = r#"<root><a:t>Slide </a:t><a:t>Title</a:t></root>"#;
    let result = extract_text_from_xml(xml, "a:t").unwrap();
    assert_eq!(result, "Slide Title");
}

#[test]
fn test_slide_number_extraction() {
    assert_eq!(extract_slide_number("ppt/slides/slide1.xml"), 1);
    assert_eq!(extract_slide_number("ppt/slides/slide12.xml"), 12);
    assert_eq!(extract_slide_number("ppt/slides/slide100.xml"), 100);
}

#[test]
fn test_rtf_basic() {
    // Minimal RTF document
    let rtf = r"{\rtf1 Hello World}";
    let result = read_rtf(rtf.as_bytes()).unwrap();
    assert!(result.contains("Hello World"), "got: {}", result);
}

#[test]
fn test_format_detection_html() {
    assert_eq!(
        DocFormat::from_extension("page.html"),
        Some(DocFormat::Html)
    );
    assert_eq!(DocFormat::from_extension("page.htm"), Some(DocFormat::Html));
}

// ── Markdown → HTML ─────────────────────────────────────────

#[test]
fn test_markdown_to_html_headings() {
    let html = markdown_to_html("# Hello\n\nWorld");
    assert!(html.contains("<h1>Hello</h1>"), "got: {}", html);
    assert!(html.contains("<p>World</p>"), "got: {}", html);
}

#[test]
fn test_markdown_to_html_bold_italic() {
    let html = markdown_to_html("**bold** and *italic*");
    assert!(html.contains("<strong>bold</strong>"), "got: {}", html);
    assert!(html.contains("<em>italic</em>"), "got: {}", html);
}

#[test]
fn test_markdown_to_html_links() {
    let html = markdown_to_html("[click](https://example.com)");
    assert!(
        html.contains("href=\"https://example.com\""),
        "got: {}",
        html
    );
    assert!(html.contains("click"), "got: {}", html);
}

#[test]
fn test_markdown_to_html_code_blocks() {
    let html = markdown_to_html("```\ncode\n```");
    assert!(html.contains("<code>"), "got: {}", html);
    assert!(html.contains("code"), "got: {}", html);
}

#[test]
fn test_markdown_to_html_tables() {
    let md = "| A | B |\n|---|---|\n| 1 | 2 |";
    let html = markdown_to_html(md);
    assert!(html.contains("<table>"), "got: {}", html);
    assert!(html.contains("<td>1</td>"), "got: {}", html);
}

// ── HTML → text ─────────────────────────────────────────────

#[test]
fn test_html_to_text_basic() {
    let text = html_to_text("<p>Hello <b>World</b></p>");
    assert_eq!(text, "Hello World");
}

#[test]
fn test_html_to_text_strips_script() {
    let text = html_to_text("<p>Text</p><script>alert('xss')</script><p>More</p>");
    assert!(!text.contains("alert"), "should strip script: {}", text);
    assert!(text.contains("Text"), "got: {}", text);
    assert!(text.contains("More"), "got: {}", text);
}

#[test]
fn test_html_to_text_strips_style() {
    let text = html_to_text("<style>body{color:red}</style><p>Content</p>");
    assert!(!text.contains("color"), "should strip style: {}", text);
    assert!(text.contains("Content"), "got: {}", text);
}

#[test]
fn test_html_to_text_newlines_on_block_elements() {
    let text = html_to_text("<h1>Title</h1><p>Para 1</p><p>Para 2</p>");
    assert!(text.contains("Title\nPara 1\nPara 2"), "got: {}", text);
}

// ── render_document ─────────────────────────────────────────

#[test]
fn test_render_markdown_to_html() {
    let result = render_document("# Test", "markdown", "html").unwrap();
    assert!(result.contains("<h1>Test</h1>"), "got: {}", result);
}

#[test]
fn test_render_html_to_text() {
    let result = render_document("<p>Hello</p>", "html", "text").unwrap();
    assert_eq!(result, "Hello");
}

#[test]
fn test_render_unsupported_conversion() {
    let err = render_document("text", "txt", "pdf").unwrap_err();
    assert!(err.contains("unsupported conversion"), "got: {}", err);
}

// ── DocFormat::from_ext ────────────────────────────────────────

#[test]
fn test_from_ext() {
    assert_eq!(DocFormat::from_ext("xlsx"), Some(DocFormat::Xlsx));
    assert_eq!(DocFormat::from_ext("HTML"), Some(DocFormat::Html));
    assert_eq!(DocFormat::from_ext("md"), Some(DocFormat::Md));
    assert_eq!(DocFormat::from_ext("pdf"), Some(DocFormat::Pdf));
    assert_eq!(DocFormat::from_ext("png"), Some(DocFormat::Png));
    assert_eq!(DocFormat::from_ext("jpg"), Some(DocFormat::Jpg));
    assert_eq!(DocFormat::from_ext("jpeg"), Some(DocFormat::Jpg));
    assert_eq!(DocFormat::from_ext("webp"), Some(DocFormat::Webp));
    assert_eq!(DocFormat::from_ext("gif"), Some(DocFormat::Gif));
    assert_eq!(DocFormat::from_ext("xyz"), None);
}

#[test]
fn test_from_ext_matches_from_extension() {
    // Every path-based detection should agree with ext-based detection
    for (path, ext) in &[
        ("file.xlsx", "xlsx"),
        ("file.pdf", "pdf"),
        ("file.html", "html"),
        ("file.md", "md"),
        ("file.csv", "csv"),
    ] {
        assert_eq!(
            DocFormat::from_extension(path),
            DocFormat::from_ext(ext),
            "mismatch for {}/{}",
            path,
            ext
        );
    }
}

// ── convert_file ──────────────────────────────────────────────

#[test]
fn test_convert_md_to_html() {
    let data = b"# Hello\n\nWorld";
    let result = convert_file(
        data,
        "md",
        "html",
        &ReadOptions::default(),
        &PageOptions::default(),
    )
    .unwrap();
    let html = String::from_utf8(result).unwrap();
    assert!(html.contains("<h1>Hello</h1>"), "got: {}", html);
    assert!(html.contains("<p>World</p>"), "got: {}", html);
}

#[test]
fn test_convert_html_to_txt() {
    let data = b"<p>Hello <b>World</b></p>";
    let result = convert_file(
        data,
        "html",
        "txt",
        &ReadOptions::default(),
        &PageOptions::default(),
    )
    .unwrap();
    let text = String::from_utf8(result).unwrap();
    assert_eq!(text, "Hello World");
}

#[test]
fn test_convert_csv_to_txt() {
    let data = b"a,b,c\n1,2,3\n";
    let result = convert_file(
        data,
        "csv",
        "txt",
        &ReadOptions::default(),
        &PageOptions::default(),
    )
    .unwrap();
    let text = String::from_utf8(result).unwrap();
    assert_eq!(text, "a,b,c\n1,2,3\n");
}

#[test]
fn test_convert_unsupported_pair() {
    let data = b"hello";
    let err = convert_file(
        data,
        "csv",
        "pdf",
        &ReadOptions::default(),
        &PageOptions::default(),
    )
    .unwrap_err();
    assert!(err.contains("unsupported conversion"), "got: {}", err);
}

#[test]
fn test_convert_case_insensitive() {
    let data = b"# Test";
    let result = convert_file(
        data,
        "MD",
        "HTML",
        &ReadOptions::default(),
        &PageOptions::default(),
    )
    .unwrap();
    let html = String::from_utf8(result).unwrap();
    assert!(html.contains("<h1>Test</h1>"), "got: {}", html);
}
