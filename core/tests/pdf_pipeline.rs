#![cfg(feature = "mod-doc")]

//! End-to-end tests for the markdown→PDF and html→PDF pipelines.
//!
//! Uses `harness = false` because native-webview-pdf (WKWebView on macOS)
//! requires the main thread, which Rust's test harness doesn't provide.

use cpsl_core::Sandbox;

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

    run_test!(test_markdown_to_pdf);
    run_test!(test_html_to_pdf);
    run_test!(test_markdown_to_pdf_with_table);
    run_test!(test_webview_pdf_default_denied);
    run_test!(test_unsupported_to_pdf_errors);

    println!("\n{} passed, {} failed", passed, failed);
    if failed > 0 {
        std::process::exit(1);
    }
}

fn test_markdown_to_pdf() {
    let sb = webview_pdf_sandbox();
    let code = concat!(
        "local md = '# Hello\\n\\nThis is **bold** and *italic*.'\n",
        "local pdf = doc.render(md, 'markdown', 'pdf')\n",
        "return string.sub(pdf, 1, 5)\n",
    );
    let result = sb.exec(code).unwrap();
    assert_eq!(result, "%PDF-", "markdown->PDF should produce valid PDF");
    println!("  PASS: test_markdown_to_pdf");
}

fn test_html_to_pdf() {
    let sb = webview_pdf_sandbox();
    let code = concat!(
        "local pdf = doc.render('<h1>Hello</h1><p>World</p>', 'html', 'pdf')\n",
        "return string.sub(pdf, 1, 5)\n",
    );
    let result = sb.exec(code).unwrap();
    assert_eq!(result, "%PDF-", "html->PDF should produce valid PDF");
    println!("  PASS: test_html_to_pdf");
}

fn test_markdown_to_pdf_with_table() {
    let sb = webview_pdf_sandbox();
    let code = concat!(
        "local md = '# Report\\n\\n| Name | Value |\\n|------|-------|\\n| Alpha | 100 |\\n| Beta | 200 |'\n",
        "local pdf = doc.render(md, 'markdown', 'pdf')\n",
        "local header = string.sub(pdf, 1, 5)\n",
        "local big_enough = #pdf > 1000\n",
        "return header .. ':' .. tostring(big_enough)\n",
    );
    let result = sb.exec(code).unwrap();
    assert_eq!(
        result, "%PDF-:true",
        "markdown table->PDF should produce valid PDF > 1KB"
    );
    println!("  PASS: test_markdown_to_pdf_with_table");
}

fn test_webview_pdf_default_denied() {
    let sb = Sandbox::new().unwrap();
    let err = sb
        .exec("doc.render('<h1>Private</h1>', 'html', 'pdf')")
        .unwrap_err();
    assert!(
        err.message
            .contains("Web-view-backed PDF rendering is disabled by network policy"),
        "unexpected error: {}",
        err.message
    );
    println!("  PASS: test_webview_pdf_default_denied");
}

fn test_unsupported_to_pdf_errors() {
    let sb = Sandbox::new().unwrap();
    let err = sb.exec("doc.render('text', 'txt', 'pdf')").unwrap_err();
    assert!(
        err.message.contains("unsupported conversion"),
        "should error: {}",
        err.message
    );
    println!("  PASS: test_unsupported_to_pdf_errors");
}

fn webview_pdf_sandbox() -> Sandbox {
    Sandbox::builder()
        .allow_webview_pdf_rendering(true)
        .build()
        .unwrap()
}
