//! Linux implementation: WebKitGTK + GtkPrintOperation → PDF.
//!
//! Creates an offscreen WebKitGTK WebView, loads the HTML, waits for load,
//! then uses PrintOperation to export to a PDF file. Reads the file and
//! returns the bytes.

use crate::{PdfError, PdfOptions};
use webkit2gtk::{LoadEvent, PrintOperation, PrintOperationExt, WebView, WebViewExt};

pub(crate) fn html_to_pdf(html: &str, opts: &PdfOptions) -> Result<Vec<u8>, PdfError> {
    // Initialize GTK (safe to call multiple times).
    gtk::init().map_err(|e| PdfError::WebviewCreation(format!("GTK init: {}", e)))?;

    let styled_html = prepare_html(html, opts);

    // Create an offscreen WebView.
    let webview = WebView::new();

    // Load HTML content.
    webview.load_html(&styled_html, None);

    // Wait for loading to finish by pumping the GTK main loop.
    let loaded = std::rc::Rc::new(std::cell::Cell::new(false));
    {
        let flag = loaded.clone();
        webview.connect_load_changed(move |_wv, event| {
            if event == LoadEvent::Finished {
                flag.set(true);
            }
        });
    }
    spin_gtk(|| loaded.get())?;

    // Set up print operation to export to a temporary PDF file.
    let tmpdir = std::env::temp_dir();
    let pdf_path = tmpdir.join(format!("native-webview-pdf-{}.pdf", std::process::id()));
    let output_uri = format!("file://{}", pdf_path.display());

    let print_op = PrintOperation::new(&webview);

    // Configure print settings for headless PDF export.
    let settings = gtk::PrintSettings::new();
    settings.set_printer("Print to File");
    settings.set("output-file-format", Some("pdf"));
    settings.set("output-uri", Some(output_uri.as_str()));

    // Apply page setup.
    let page_setup = gtk::PageSetup::new();
    let paper_size = gtk::PaperSize::new_custom(
        "custom",
        "Custom",
        opts.page_width * 25.4,  // inches → mm
        opts.page_height * 25.4, // inches → mm
        gtk::Unit::Mm,
    );
    page_setup.set_paper_size(&paper_size);
    page_setup.set_top_margin(opts.margin_top * 25.4, gtk::Unit::Mm);
    page_setup.set_bottom_margin(opts.margin_bottom * 25.4, gtk::Unit::Mm);
    page_setup.set_left_margin(opts.margin_left * 25.4, gtk::Unit::Mm);
    page_setup.set_right_margin(opts.margin_right * 25.4, gtk::Unit::Mm);
    if opts.landscape {
        page_setup.set_orientation(gtk::PageOrientation::Landscape);
    }

    print_op.set_print_settings(&settings);
    print_op.set_page_setup(&page_setup);

    // Print without showing a dialog.
    let finished = std::rc::Rc::new(std::cell::Cell::new(false));
    {
        let flag = finished.clone();
        print_op.connect_finished(move |_| {
            flag.set(true);
        });
    }
    print_op.print();

    // Wait for the print operation to finish.
    spin_gtk(|| finished.get())?;

    // Read the PDF file.
    let bytes = std::fs::read(&pdf_path)
        .map_err(|e| PdfError::PdfGeneration(format!("read PDF file: {}", e)))?;
    let _ = std::fs::remove_file(&pdf_path);

    if bytes.is_empty() {
        return Err(PdfError::PdfGeneration("empty PDF output".into()));
    }

    Ok(bytes)
}

/// Pump the GTK main loop until `done()` returns true. Times out after 30s.
fn spin_gtk(done: impl Fn() -> bool) -> Result<(), PdfError> {
    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(30);

    while !done() {
        if start.elapsed() > timeout {
            return Err(PdfError::PdfGeneration(
                "timed out waiting for webview".into(),
            ));
        }
        gtk::main_iteration_do(false);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    Ok(())
}

/// Wrap HTML with `@page` CSS rules, viewport meta, and explicit body width
/// so layout is deterministic regardless of the webview widget size.
/// `!important` on body width so content CSS can't override page geometry.
fn prepare_html(html: &str, opts: &PdfOptions) -> String {
    let (w, h) = if opts.landscape {
        (opts.page_height, opts.page_width)
    } else {
        (opts.page_width, opts.page_height)
    };
    let content_w_pt = (w - opts.margin_left - opts.margin_right) * 72.0;
    format!(
        r#"<meta name="viewport" content="width={content_w_pt}"><style>@page {{ size: {w}in {h}in; margin: {mt}in {mr}in {mb}in {ml}in; }} *, *::before, *::after {{ box-sizing: border-box; }} html, body {{ width: {content_w_pt}px !important; margin: 0 !important; padding: 0 !important; }}</style>{html}"#,
        mt = opts.margin_top,
        mr = opts.margin_right,
        mb = opts.margin_bottom,
        ml = opts.margin_left,
    )
}
