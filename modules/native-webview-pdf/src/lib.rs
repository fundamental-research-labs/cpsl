//! Platform-native HTML→PDF conversion.
//!
//! Uses the OS-native webview engine to render HTML and export PDF:
//! - Apple platforms: WKWebView + `createPDF`
//! - Windows: WebView2 + `PrintToPdfStream`
//! - Linux: WebKitGTK + print operation → cairo PDF surface
//!
//! Zero bundled engine — uses what the OS already ships.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(any(target_os = "macos", target_os = "ios"))]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

/// Options for PDF page layout.
#[derive(Debug, Clone)]
pub struct PdfOptions {
    /// Page width in inches (default 8.5).
    pub page_width: f64,
    /// Page height in inches (default 11.0).
    pub page_height: f64,
    /// Top margin in inches (default 0.45).
    pub margin_top: f64,
    /// Bottom margin in inches (default 0.45).
    pub margin_bottom: f64,
    /// Left margin in inches (default 0.45).
    pub margin_left: f64,
    /// Right margin in inches (default 0.45).
    pub margin_right: f64,
    /// Landscape orientation (default false).
    pub landscape: bool,
}

impl Default for PdfOptions {
    fn default() -> Self {
        Self {
            page_width: 8.5,
            page_height: 11.0,
            margin_top: 0.45,
            margin_bottom: 0.45,
            margin_left: 0.45,
            margin_right: 0.45,
            landscape: false,
        }
    }
}

/// Errors from PDF generation.
#[derive(Debug, thiserror::Error)]
pub enum PdfError {
    #[error("webview creation failed: {0}")]
    WebviewCreation(String),
    #[error("HTML loading failed: {0}")]
    HtmlLoading(String),
    #[error("PDF generation failed: {0}")]
    PdfGeneration(String),
    #[error("platform not supported")]
    Unsupported,
}

/// Render an HTML string to PDF bytes using the platform's native webview.
///
/// This is a blocking call. On Apple platforms it spins a runloop internally;
/// on Windows it pumps the message loop; on Linux it runs GTK main iteration.
pub fn html_to_pdf(html: &str, opts: &PdfOptions) -> Result<Vec<u8>, PdfError> {
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    {
        macos::html_to_pdf(html, opts)
    }
    #[cfg(target_os = "windows")]
    {
        windows::html_to_pdf(html, opts)
    }
    #[cfg(target_os = "linux")]
    {
        linux::html_to_pdf(html, opts)
    }
    #[cfg(not(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "windows",
        target_os = "linux"
    )))]
    {
        let _ = (html, opts);
        Err(PdfError::Unsupported)
    }
}
