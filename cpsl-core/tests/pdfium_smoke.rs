#![cfg(feature = "pdfium-render")]

use pdfium_render::prelude::PdfFormFieldCommon;
use std::path::PathBuf;
use std::sync::Arc;
use cpsl_core::PdfiumEngine;

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn engine() -> PdfiumEngine {
    PdfiumEngine::discover(Some(&crate_root()))
        .expect("PDFium should be discoverable from crate root (run scripts/download-pdfium.sh)")
}

fn fixtures_dir() -> PathBuf {
    crate_root().join("tests").join("fixtures").join("pdf")
}

// ── Smoke: load library ─────────────────────────────────────────

#[test]
fn pdfium_loads_successfully() {
    let eng = engine();
    assert!(
        eng.lib_path().exists() || eng.lib_path().to_str() == Some("<system>"),
        "lib_path should point to a real file or <system>"
    );
}

// ── Smoke: create blank document, add page, save ────────────────

#[test]
fn create_blank_document_and_save() {
    let eng = engine();
    let pdfium = eng.pdfium();

    let mut doc = pdfium.create_new_pdf().expect("create new PDF");
    let pages = doc.pages_mut();
    pages
        .create_page_at_end(pdfium_render::prelude::PdfPagePaperSize::a4())
        .expect("add A4 page");
    let _ = pages;

    let bytes = doc.save_to_bytes().expect("save to bytes");
    assert!(bytes.len() > 10, "saved PDF should have content");
    assert!(
        bytes.starts_with(b"%PDF-"),
        "saved PDF should start with %PDF- header"
    );
}

// ── Smoke: load fixture, extract text ───────────────────────────

#[test]
fn load_simple_text_fixture() {
    let eng = engine();
    let pdfium = eng.pdfium();
    let data = std::fs::read(fixtures_dir().join("simple_text.pdf")).unwrap();

    let doc = pdfium
        .load_pdf_from_byte_slice(&data, None)
        .expect("load simple_text.pdf");

    assert_eq!(doc.pages().len(), 1, "should have 1 page");

    let page = doc.pages().get(0).expect("get page 0");
    let text = page.text().expect("get text object");
    let all = text.all();
    assert!(
        all.contains("Hello"),
        "text should contain 'Hello', got: {all}"
    );
}

// ── Smoke: multi-page ───────────────────────────────────────────

#[test]
fn load_multi_page_fixture() {
    let eng = engine();
    let data = std::fs::read(fixtures_dir().join("multi_page.pdf")).unwrap();

    let doc = eng.pdfium().load_pdf_from_byte_slice(&data, None).unwrap();
    assert_eq!(doc.pages().len(), 3, "should have 3 pages");

    for i in 0..3 {
        let page = doc.pages().get(i as u16).unwrap();
        let text = page.text().unwrap().all();
        let expected = format!("Page {} of 3", i + 1);
        assert!(
            text.contains(&expected),
            "page {i} should contain '{expected}', got: {text}"
        );
    }
}

// ── Smoke: form fields ──────────────────────────────────────────

#[test]
fn enumerate_form_fields() {
    let eng = engine();
    let data = std::fs::read(fixtures_dir().join("form_fields.pdf")).unwrap();

    let doc = eng.pdfium().load_pdf_from_byte_slice(&data, None).unwrap();
    assert!(doc.pages().len() >= 1);

    // Collect form field names from annotations on page 0
    let page = doc.pages().get(0).unwrap();
    let annotations = page.annotations();
    let mut field_names = Vec::new();

    for i in 0..annotations.len() {
        if let Ok(annot) = annotations.get(i) {
            if let Some(form_field) = annot.as_form_field() {
                let name = form_field.name().unwrap_or_default();
                if !name.is_empty() {
                    field_names.push(name);
                }
            }
        }
    }

    // We expect at least text fields, checkboxes, and radio buttons
    assert!(
        field_names.len() >= 4,
        "expected at least 4 form fields, got {}: {:?}",
        field_names.len(),
        field_names
    );

    // Check specific known fields
    assert!(
        field_names.iter().any(|n: &String| n.contains("full_name")),
        "should find full_name field, got: {:?}",
        field_names
    );
}

// ── Smoke: empty PDF ────────────────────────────────────────────

#[test]
fn load_empty_pdf() {
    let eng = engine();
    let data = std::fs::read(fixtures_dir().join("empty.pdf")).unwrap();

    let doc = eng.pdfium().load_pdf_from_byte_slice(&data, None).unwrap();
    assert_eq!(doc.pages().len(), 1, "empty.pdf should have 1 blank page");

    let text = doc.pages().get(0).unwrap().text().unwrap().all();
    assert!(text.trim().is_empty(), "empty page should have no text");
}

// ── Smoke: UTF-16 metadata PDF (doesn't panic) ─────────────────

#[test]
fn load_utf16_metadata_pdf_no_panic() {
    let eng = engine();
    let data = std::fs::read(fixtures_dir().join("utf16_metadata.pdf")).unwrap();

    // Should load without panicking (unlike pdf-extract on some UTF-16 PDFs)
    let doc = eng.pdfium().load_pdf_from_byte_slice(&data, None).unwrap();
    assert_eq!(doc.pages().len(), 1);
}

// ── Smoke: PdfiumEngine is Send + Sync ──────────────────────────

#[test]
fn engine_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<PdfiumEngine>();

    // Also works through Arc
    let eng = Arc::new(engine());
    let eng2 = eng.clone();
    let handle = std::thread::spawn(move || {
        let _ = eng2.lib_path();
    });
    handle.join().unwrap();
}
