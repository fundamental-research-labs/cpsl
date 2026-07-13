//! Vision input preparation, caching, and concurrency helpers for document reads.

use crate::doc_reader::DocFormat;
#[cfg(feature = "pdfium-render")]
use crate::pdfium_engine::PdfiumEngine;
use crate::sandbox::VisionInput;
use sha2::{Digest, Sha256};
use std::path::Path;
#[cfg(feature = "pdfium-render")]
use std::sync::Arc;
use std::sync::{Condvar, Mutex};

/// Maximum number of concurrent vision API calls during readAsync batch resolution.
/// Prevents overwhelming the LLM API with too many parallel requests.
pub(super) const MAX_CONCURRENT_READS: usize = 8;

/// Default extraction prompt sent to the vision model when no custom query is provided.
pub(super) const DEFAULT_EXTRACTION_QUERY: &str =
    "Extract all content from this document as markdown. Preserve structure: tables as pipe-delimited, \
     lists as bullet points, headings with #. For any images, charts, diagrams, or visual elements, \
     describe them in detail using ![description](image) syntax. Report exactly what you see.";

/// A simple counting semaphore built on std primitives.
pub(super) struct Semaphore {
    state: Mutex<usize>,
    condvar: Condvar,
}

impl Semaphore {
    pub(super) fn new(permits: usize) -> Self {
        Self {
            state: Mutex::new(permits),
            condvar: Condvar::new(),
        }
    }

    fn acquire(&self) {
        let mut count = self.state.lock().unwrap();
        while *count == 0 {
            count = self.condvar.wait(count).unwrap();
        }
        *count -= 1;
    }

    fn release(&self) {
        // Handle a poisoned mutex gracefully because this runs during unwinding.
        let mut count = self.state.lock().unwrap_or_else(|error| error.into_inner());
        *count += 1;
        self.condvar.notify_one();
    }

    pub(super) fn acquire_guard(&self) -> SemaphoreGuard<'_> {
        self.acquire();
        SemaphoreGuard(self)
    }
}

pub(super) struct SemaphoreGuard<'a>(&'a Semaphore);

impl Drop for SemaphoreGuard<'_> {
    fn drop(&mut self) {
        self.0.release();
    }
}

/// Compute a cache key from file bytes and query string.
/// Format: `{sha256(file_bytes)}-{sha256(query)}` (hex-encoded).
pub(super) fn cache_key(file_bytes: &[u8], query: &str) -> String {
    let file_hash = hex::encode(Sha256::digest(file_bytes));
    let query_hash = hex::encode(Sha256::digest(query.as_bytes()));
    format!("{}-{}", file_hash, query_hash)
}

/// Read cached text from disk. Returns `Some(text)` if the file exists.
pub(super) fn cache_read(cache_dir: &Path, key: &str) -> Option<String> {
    let path = cache_dir.join(format!("{}.txt", key));
    std::fs::read_to_string(path).ok()
}

/// Write text to the disk cache. Creates the cache directory on first write.
pub(super) fn cache_write(cache_dir: &Path, key: &str, text: &str) {
    let _ = std::fs::create_dir_all(cache_dir);
    let path = cache_dir.join(format!("{}.txt", key));
    let _ = std::fs::write(path, text);
}

fn vision_media_type(format: DocFormat) -> &'static str {
    match format {
        DocFormat::Png => "image/png",
        DocFormat::Jpg => "image/jpeg",
        DocFormat::Webp => "image/webp",
        DocFormat::Gif => "image/gif",
        DocFormat::Pdf => "application/pdf",
        DocFormat::Txt => "text/plain",
        DocFormat::Csv => "text/csv",
        DocFormat::Json => "application/json",
        DocFormat::Md => "text/markdown",
        DocFormat::Html => "text/html",
        DocFormat::Xlsx => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        DocFormat::Xls => "application/vnd.ms-excel",
        DocFormat::Xlsm => "application/vnd.ms-excel.sheet.macroenabled.12",
        DocFormat::Ods => "application/vnd.oasis.opendocument.spreadsheet",
        DocFormat::Docx => {
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
        }
        DocFormat::Rtf => "application/rtf",
        DocFormat::Pptx => {
            "application/vnd.openxmlformats-officedocument.presentationml.presentation"
        }
    }
}

pub(super) fn vision_inputs(
    data: Vec<u8>,
    format: DocFormat,
    filename: String,
    #[cfg(feature = "pdfium-render")] pdfium_engine: Option<&Arc<PdfiumEngine>>,
) -> Result<Vec<VisionInput>, String> {
    #[cfg(feature = "pdfium-render")]
    if format == DocFormat::Pdf {
        if let Some(engine) = pdfium_engine {
            let pages = crate::doc_reader::render_pdf_pages_for_vision(engine, &data)?;
            let stem = Path::new(&filename)
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or("document");
            return Ok(pages
                .into_iter()
                .enumerate()
                .map(|(index, data)| VisionInput {
                    data,
                    filename: format!("{}-page-{}.png", stem, index + 1),
                    media_type: "image/png".to_string(),
                })
                .collect());
        }
    }

    #[cfg(feature = "mod-image")]
    if matches!(format, DocFormat::Webp | DocFormat::Gif) {
        use image::ImageFormat;

        let image_format = if format == DocFormat::Webp {
            ImageFormat::WebP
        } else {
            ImageFormat::Gif
        };
        let image = image::load_from_memory_with_format(&data, image_format)
            .map_err(|error| format!("cannot decode image for vision: {}", error))?;
        let mut output = std::io::Cursor::new(Vec::new());
        image
            .write_to(&mut output, ImageFormat::Png)
            .map_err(|error| format!("cannot encode image for vision: {}", error))?;
        return Ok(vec![VisionInput {
            data: output.into_inner(),
            filename: format!("{}.png", filename),
            media_type: "image/png".to_string(),
        }]);
    }

    Ok(vec![VisionInput {
        data,
        filename,
        media_type: vision_media_type(format).to_string(),
    }])
}
