//! Integration tests for native-webview-pdf.
//!
//! Uses `harness = false` because WKWebView requires the main thread,
//! and Rust's default test harness runs tests on non-main threads.

use native_webview_pdf::{html_to_pdf, PdfOptions};

fn main() {
    let mut passed = 0;
    let mut failed = 0;

    macro_rules! run_test {
        ($name:ident) => {
            match std::panic::catch_unwind(|| $name()) {
                Ok(_) => {
                    passed += 1;
                }
                Err(e) => {
                    let msg = if let Some(s) = e.downcast_ref::<&str>() {
                        s.to_string()
                    } else if let Some(s) = e.downcast_ref::<String>() {
                        s.clone()
                    } else {
                        "unknown panic".into()
                    };
                    eprintln!("  FAIL: {} — {}", stringify!($name), msg);
                    failed += 1;
                }
            }
        };
    }

    run_test!(test_basic_html_to_pdf);
    run_test!(test_css_styled_html_to_pdf);
    run_test!(test_flexbox_grid_layout);
    run_test!(test_landscape_pdf);
    run_test!(test_custom_margins);
    run_test!(test_full_html_document);
    println!("\n{} passed, {} failed", passed, failed);
    if failed > 0 {
        std::process::exit(1);
    }
}

fn test_basic_html_to_pdf() {
    let html = "<h1>Hello, World!</h1><p>This is a test PDF.</p>";
    let pdf = html_to_pdf(html, &PdfOptions::default()).expect("html_to_pdf failed");
    assert_pdf_valid(&pdf, "basic");
    println!("  PASS: test_basic_html_to_pdf ({} bytes)", pdf.len());
}

fn test_css_styled_html_to_pdf() {
    let html = r#"
        <html>
        <head><style>
            body { font-family: Helvetica, sans-serif; color: #333; }
            h1 { color: navy; border-bottom: 2px solid navy; }
            .highlight { background: yellow; padding: 4px; }
        </style></head>
        <body>
            <h1>Styled PDF</h1>
            <p>Normal text with <span class="highlight">highlighted</span> content.</p>
        </body>
        </html>
    "#;
    let pdf = html_to_pdf(html, &PdfOptions::default()).expect("CSS styled failed");
    assert_pdf_valid(&pdf, "css_styled");
    println!(
        "  PASS: test_css_styled_html_to_pdf ({} bytes)",
        pdf.len()
    );
}

fn test_flexbox_grid_layout() {
    let html = r#"
        <html><head><style>
            .flex { display: flex; gap: 16px; }
            .card { flex: 1; background: #f0f0f0; padding: 16px; border-radius: 8px; }
            .grid { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; margin-top: 24px; }
            .cell { background: #e0e0ff; padding: 12px; text-align: center; }
        </style></head>
        <body>
            <h2>Flexbox</h2>
            <div class="flex">
                <div class="card">Card A</div>
                <div class="card">Card B</div>
                <div class="card">Card C</div>
            </div>
            <h2>Grid</h2>
            <div class="grid">
                <div class="cell">1</div>
                <div class="cell">2</div>
                <div class="cell">3</div>
                <div class="cell">4</div>
            </div>
        </body></html>
    "#;
    let pdf = html_to_pdf(html, &PdfOptions::default()).expect("flexbox/grid failed");
    assert_pdf_valid(&pdf, "flexbox_grid");
    println!(
        "  PASS: test_flexbox_grid_layout ({} bytes)",
        pdf.len()
    );
}

fn test_landscape_pdf() {
    let html = "<h1>Landscape Mode</h1><p>Wide content for landscape.</p>";
    let opts = PdfOptions {
        landscape: true,
        ..PdfOptions::default()
    };
    let pdf = html_to_pdf(html, &opts).expect("landscape failed");
    assert_pdf_valid(&pdf, "landscape");
    println!("  PASS: test_landscape_pdf ({} bytes)", pdf.len());
}

fn test_custom_margins() {
    let html = "<h1>Custom Margins</h1><p>Tight margins.</p>";
    let opts = PdfOptions {
        margin_top: 0.25,
        margin_bottom: 0.25,
        margin_left: 0.25,
        margin_right: 0.25,
        ..PdfOptions::default()
    };
    let pdf = html_to_pdf(html, &opts).expect("custom margins failed");
    assert_pdf_valid(&pdf, "custom_margins");
    println!("  PASS: test_custom_margins ({} bytes)", pdf.len());
}

fn test_full_html_document() {
    let html = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <title>Full Document Test</title>
    <style>
        body { font-family: Georgia, serif; line-height: 1.6; max-width: 600px; margin: 0 auto; }
        h1 { font-size: 24pt; }
        table { border-collapse: collapse; width: 100%; }
        td, th { border: 1px solid #ccc; padding: 8px; }
    </style>
</head>
<body>
    <h1>Report</h1>
    <table>
        <tr><th>Name</th><th>Value</th></tr>
        <tr><td>Alpha</td><td>100</td></tr>
        <tr><td>Beta</td><td>200</td></tr>
    </table>
</body>
</html>"#;
    let pdf = html_to_pdf(html, &PdfOptions::default()).expect("full document failed");
    assert_pdf_valid(&pdf, "full_document");
    println!(
        "  PASS: test_full_html_document ({} bytes)",
        pdf.len()
    );
}

fn assert_pdf_valid(pdf: &[u8], label: &str) {
    assert!(
        pdf.starts_with(b"%PDF-"),
        "{}: should start with %PDF-, got {:?}",
        label,
        &pdf[..pdf.len().min(10)]
    );
    assert!(
        pdf.len() > 100,
        "{}: should be non-trivial, got {} bytes",
        label,
        pdf.len()
    );
}
