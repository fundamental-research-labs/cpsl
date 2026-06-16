//! Document module for the Luau sandbox.
//!
//! Exposes `doc.read(path, opts?)` for extracting text from various file formats.
//! Format is auto-detected from the file extension.

use crate::doc_reader::{
    convert_file, is_binary_conversion, read_document, render_document, render_document_bytes,
    DocFormat, PageOptions, ReadMode, ReadOptions,
};
use crate::mount::MountTable;
#[cfg(feature = "pdfium-render")]
use crate::pdfium_engine::PdfiumEngine;
use crate::sandbox::{
    arg_error, wrap_module_with_help_hints, FieldDoc, FnDoc, ModuleDoc, Param, ParamType,
    PendingRead, PendingReads, ReturnType, VisionCallback,
};
use mlua::{Lua, MultiValue, UserData, UserDataMethods};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[cfg(feature = "pdfium-render")]
mod pdf_utils;

/// Maximum number of concurrent vision API calls during readAsync batch resolution.
/// Prevents overwhelming the LLM API with too many parallel requests.
const MAX_CONCURRENT_READS: usize = 8;

/// A simple counting semaphore built on std primitives (Mutex + Condvar).
/// Used to limit concurrent threads without adding external dependencies.
struct Semaphore {
    state: Mutex<usize>,
    condvar: std::sync::Condvar,
}

impl Semaphore {
    fn new(permits: usize) -> Self {
        Self {
            state: Mutex::new(permits),
            condvar: std::sync::Condvar::new(),
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
        // Use unwrap_or_else to handle poisoned mutex gracefully —
        // critical because this runs in SemaphoreGuard::drop during unwinding.
        let mut count = self.state.lock().unwrap_or_else(|e| e.into_inner());
        *count += 1;
        self.condvar.notify_one();
    }

    /// Acquire a permit and return a guard that releases it on drop.
    /// Ensures the permit is released even if the holder panics.
    fn acquire_guard(&self) -> SemaphoreGuard<'_> {
        self.acquire();
        SemaphoreGuard(self)
    }
}

struct SemaphoreGuard<'a>(&'a Semaphore);

impl Drop for SemaphoreGuard<'_> {
    fn drop(&mut self) {
        self.0.release();
    }
}

/// Default extraction prompt sent to the vision model when no custom query is provided.
pub(crate) const DEFAULT_EXTRACTION_QUERY: &str =
    "Extract all content from this document as markdown. Preserve structure: tables as pipe-delimited, \
     lists as bullet points, headings with #. For any images, charts, diagrams, or visual elements, \
     describe them in detail using ![description](image) syntax. Report exactly what you see.";

/// Compute a cache key from file bytes and query string.
/// Format: `{sha256(file_bytes)}-{sha256(query)}` (hex-encoded).
pub(crate) fn cache_key(file_bytes: &[u8], query: &str) -> String {
    let file_hash = hex::encode(Sha256::digest(file_bytes));
    let query_hash = hex::encode(Sha256::digest(query.as_bytes()));
    format!("{}-{}", file_hash, query_hash)
}

/// Read cached text from disk. Returns `Some(text)` if the cache file exists.
pub(crate) fn cache_read(cache_dir: &Path, key: &str) -> Option<String> {
    let path = cache_dir.join(format!("{}.txt", key));
    std::fs::read_to_string(path).ok()
}

/// Write text to the disk cache. Creates the cache directory on first write.
pub(crate) fn cache_write(cache_dir: &Path, key: &str, text: &str) {
    let _ = std::fs::create_dir_all(cache_dir);
    let path = cache_dir.join(format!("{}.txt", key));
    let _ = std::fs::write(path, text);
}

const DOC_READ_OPTS_FIELDS_BASE: &[FieldDoc] = &[
    FieldDoc { name: "sheet", typ: "number", required: false, description: "Sheet number for spreadsheets (1-indexed)" },
    FieldDoc { name: "query", typ: "string", required: false, description: "Custom extraction prompt for vision-powered formats (images, PDFs). Default: generic extraction." },
];

const DOC_READ_OPTS_FIELDS_WITH_MODE: &[FieldDoc] = &[
    FieldDoc { name: "sheet", typ: "number", required: false, description: "Sheet number for spreadsheets (1-indexed)" },
    FieldDoc { name: "query", typ: "string", required: false, description: "Custom extraction prompt for vision-powered formats (images, PDFs). Default: generic extraction." },
    FieldDoc { name: "mode", typ: "string", required: false, description: "\"structural\" (local parsing) or \"vision\" (multimodal AI). Default: vision for images/PDFs, structural for everything else." },
];

const DOC_RENDER_OPTS_FIELDS: &[FieldDoc] = &[
    FieldDoc {
        name: "pageWidth",
        typ: "number",
        required: false,
        description: "Page width in inches (default 8.5 = US Letter)",
    },
    FieldDoc {
        name: "pageHeight",
        typ: "number",
        required: false,
        description: "Page height in inches (default 11 = US Letter)",
    },
    FieldDoc {
        name: "marginTop",
        typ: "number",
        required: false,
        description: "Top margin in inches (default 0.45)",
    },
    FieldDoc {
        name: "marginBottom",
        typ: "number",
        required: false,
        description: "Bottom margin in inches (default 0.45)",
    },
    FieldDoc {
        name: "marginLeft",
        typ: "number",
        required: false,
        description: "Left margin in inches (default 0.45)",
    },
    FieldDoc {
        name: "marginRight",
        typ: "number",
        required: false,
        description: "Right margin in inches (default 0.45)",
    },
    FieldDoc {
        name: "landscape",
        typ: "boolean",
        required: false,
        description: "Use landscape orientation",
    },
];

/// Params for doc.read() and doc.readAsync() — without mode option (no vision callback).
const DOC_READ_PARAMS: &[Param] = &[
    Param {
        name: "path",
        short: None,
        typ: ParamType::String,
        required: true,
        fields: None,
    },
    Param {
        name: "opts",
        short: None,
        typ: ParamType::Table,
        required: false,
        fields: Some(DOC_READ_OPTS_FIELDS_BASE),
    },
];

/// Params for doc.read() and doc.readAsync() — with mode option (vision callback present).
const DOC_READ_PARAMS_WITH_MODE: &[Param] = &[
    Param {
        name: "path",
        short: None,
        typ: ParamType::String,
        required: true,
        fields: None,
    },
    Param {
        name: "opts",
        short: None,
        typ: ParamType::Table,
        required: false,
        fields: Some(DOC_READ_OPTS_FIELDS_WITH_MODE),
    },
];

#[cfg(feature = "pdfium-render")]
const DOC_PDF_INFO_PARAMS: &[Param] = &[Param {
    name: "path",
    short: None,
    typ: ParamType::String,
    required: true,
    fields: None,
}];

#[cfg(feature = "pdfium-render")]
const DOC_FORM_FIELDS_PARAMS: &[Param] = &[Param {
    name: "path",
    short: None,
    typ: ParamType::String,
    required: true,
    fields: None,
}];

#[cfg(feature = "pdfium-render")]
const DOC_FILL_FORM_FIELDS: &[FieldDoc] = &[
    FieldDoc { name: "path", typ: "string", required: true, description: "Path to the PDF file" },
    FieldDoc { name: "fields", typ: "table", required: true, description: "Table mapping field names to new values. Strings for text fields, booleans for checkboxes/radios." },
    FieldDoc { name: "output", typ: "string", required: true, description: "Output path for the filled PDF" },
    FieldDoc { name: "flatten", typ: "boolean", required: false, description: "If true, bake form fields into page content (default: false)" },
];

#[cfg(feature = "pdfium-render")]
const DOC_FILL_FORM_PARAMS: &[Param] = &[Param {
    name: "opts",
    short: None,
    typ: ParamType::Table,
    required: true,
    fields: Some(DOC_FILL_FORM_FIELDS),
}];

#[cfg(feature = "pdfium-render")]
const DOC_MERGE_PDF_FIELDS: &[FieldDoc] = &[
    FieldDoc {
        name: "paths",
        typ: "table",
        required: true,
        description: "List of PDF file paths to merge (in order)",
    },
    FieldDoc {
        name: "output",
        typ: "string",
        required: true,
        description: "Output path for the merged PDF",
    },
];

#[cfg(feature = "pdfium-render")]
const DOC_MERGE_PDF_PARAMS: &[Param] = &[Param {
    name: "opts",
    short: None,
    typ: ParamType::Table,
    required: true,
    fields: Some(DOC_MERGE_PDF_FIELDS),
}];

#[cfg(feature = "pdfium-render")]
const DOC_SPLIT_PDF_FIELDS: &[FieldDoc] = &[
    FieldDoc {
        name: "path",
        typ: "string",
        required: true,
        description: "Path to the PDF file to split",
    },
    FieldDoc {
        name: "ranges",
        typ: "table",
        required: true,
        description: "List of page range strings, e.g. {\"1-3\", \"4-6\", \"7\"}",
    },
    FieldDoc {
        name: "outputDir",
        typ: "string",
        required: true,
        description: "Directory for output files (named split_1.pdf, split_2.pdf, ...)",
    },
];

#[cfg(feature = "pdfium-render")]
const DOC_SPLIT_PDF_PARAMS: &[Param] = &[Param {
    name: "opts",
    short: None,
    typ: ParamType::Table,
    required: true,
    fields: Some(DOC_SPLIT_PDF_FIELDS),
}];

#[cfg(feature = "pdfium-render")]
const DOC_EDIT_PAGES_FIELDS: &[FieldDoc] = &[
    FieldDoc { name: "path", typ: "string", required: true, description: "Path to the PDF file" },
    FieldDoc { name: "operations", typ: "table", required: true, description: "List of operations: {type=\"delete\", pages={1,3}} or {type=\"rotate\", pages={1}, degrees=90} or {type=\"reorder\", order={3,1,2}}" },
    FieldDoc { name: "output", typ: "string", required: true, description: "Output path for the modified PDF" },
];

#[cfg(feature = "pdfium-render")]
const DOC_EDIT_PAGES_PARAMS: &[Param] = &[Param {
    name: "opts",
    short: None,
    typ: ParamType::Table,
    required: true,
    fields: Some(DOC_EDIT_PAGES_FIELDS),
}];

#[cfg(feature = "pdfium-render")]
const DOC_ADD_ANNOTATION_FIELDS: &[FieldDoc] = &[
    FieldDoc { name: "path", typ: "string", required: true, description: "Path to the PDF file" },
    FieldDoc { name: "page", typ: "number", required: true, description: "Page number (1-indexed)" },
    FieldDoc { name: "type", typ: "string", required: true, description: "Annotation type: \"text\", \"freeText\", \"highlight\", \"underline\", \"strikeout\", \"square\", \"stamp\"" },
    FieldDoc { name: "x", typ: "number", required: true, description: "X position in PDF points (origin = bottom-left)" },
    FieldDoc { name: "y", typ: "number", required: true, description: "Y position in PDF points (origin = bottom-left)" },
    FieldDoc { name: "width", typ: "number", required: true, description: "Width in PDF points" },
    FieldDoc { name: "height", typ: "number", required: true, description: "Height in PDF points" },
    FieldDoc { name: "color", typ: "string", required: false, description: "Color as hex string \"#RRGGBB\" or \"#RRGGBBAA\" (default: yellow for highlight, red for others)" },
    FieldDoc { name: "contents", typ: "string", required: false, description: "Text content (for text/freeText/stamp) or popup text (for markup annotations)" },
    FieldDoc { name: "output", typ: "string", required: true, description: "Output path for the annotated PDF" },
];

#[cfg(feature = "pdfium-render")]
const DOC_ADD_ANNOTATION_PARAMS: &[Param] = &[Param {
    name: "opts",
    short: None,
    typ: ParamType::Table,
    required: true,
    fields: Some(DOC_ADD_ANNOTATION_FIELDS),
}];

#[cfg(feature = "pdfium-render")]
const DOC_WATERMARK_FIELDS: &[FieldDoc] = &[
    FieldDoc { name: "path", typ: "string", required: true, description: "Path to the PDF file" },
    FieldDoc { name: "text", typ: "string", required: true, description: "Watermark text" },
    FieldDoc { name: "fontSize", typ: "number", required: false, description: "Font size in points (default: 48)" },
    FieldDoc { name: "color", typ: "string", required: false, description: "Color as hex string \"#RRGGBB\" or \"#RRGGBBAA\" (default: \"#00000040\" — black at 25% opacity)" },
    FieldDoc { name: "rotation", typ: "number", required: false, description: "Rotation in degrees counter-clockwise (default: 45)" },
    FieldDoc { name: "pages", typ: "string", required: false, description: "Page range string like \"1-3\" or \"all\" (default: \"all\")" },
    FieldDoc { name: "output", typ: "string", required: true, description: "Output path for the watermarked PDF" },
];

#[cfg(feature = "pdfium-render")]
const DOC_WATERMARK_PARAMS: &[Param] = &[Param {
    name: "opts",
    short: None,
    typ: ParamType::Table,
    required: true,
    fields: Some(DOC_WATERMARK_FIELDS),
}];

const DOC_RENDER_PARAMS: &[Param] = &[
    Param {
        name: "text",
        short: None,
        typ: ParamType::String,
        required: true,
        fields: None,
    },
    Param {
        name: "from",
        short: None,
        typ: ParamType::String,
        required: true,
        fields: None,
    },
    Param {
        name: "to",
        short: None,
        typ: ParamType::String,
        required: true,
        fields: None,
    },
    Param {
        name: "opts",
        short: None,
        typ: ParamType::Table,
        required: false,
        fields: Some(DOC_RENDER_OPTS_FIELDS),
    },
];

const DOC_RENDER_FILE_PARAMS: &[Param] = &[
    Param {
        name: "source",
        short: None,
        typ: ParamType::String,
        required: true,
        fields: None,
    },
    Param {
        name: "target",
        short: None,
        typ: ParamType::String,
        required: true,
        fields: None,
    },
    Param {
        name: "opts",
        short: None,
        typ: ParamType::Table,
        required: false,
        fields: Some(DOC_RENDER_OPTS_FIELDS),
    },
];

/// Static doc for structural-only mode (no vision callback).
pub(crate) static DOC_MOD_DOC: ModuleDoc = ModuleDoc {
    name: "doc",
    summary: "document reading & conversion; structural parsing (pdf, xlsx, docx, pptx, html, ...)",
    functions: &[
        FnDoc {
            name: "read",
            description:
                "Read a document file and extract its text. Format auto-detected from extension.\n    \
                 Supported: xlsx, xls, xlsm, ods, docx, pdf, rtf, pptx, csv, txt, json, md, html.\n    \
                 All formats use local structural parsing.",
            params: DOC_READ_PARAMS,
            returns: ReturnType::String,
            example: Some(r#"local text = doc.read("/attachments/report.pdf")"#),
        },
        FnDoc {
            name: "readAsync",
            description:
                "Read a document asynchronously. Returns a DocFuture immediately (no blocking).\n    \
                 Call :await() on the future to get the result.",
            params: DOC_READ_PARAMS,
            returns: ReturnType::UserData,
            example: Some(
                "local f1 = doc.readAsync(\"/attachments/doc1.pdf\")\n\
                 local f2 = doc.readAsync(\"/attachments/doc2.pdf\")\n\
                 local text1 = f1:await()\n\
                 local text2 = f2:await()"
            ),
        },
        DOC_RENDER_FN,
        DOC_RENDER_FILE_FN,
    ],
};

/// Static doc for vision-enabled mode (vision callback registered).
static DOC_MOD_DOC_VISION: ModuleDoc = ModuleDoc {
    name: "doc",
    summary: "document reading & conversion; structural + vision modes (pdf, xlsx, docx, images, ...)",
    functions: &[
        FnDoc {
            name: "read",
            description:
                "Read a document file and extract its text. Format auto-detected from extension.\n    \
                 Supported: xlsx, xls, xlsm, ods, docx, pdf, rtf, pptx, csv, txt, json, md, html, png, jpg, webp, gif.\n    \
                 Two modes: \"structural\" (local parsing) and \"vision\" (AI multimodal analysis).\n    \
                 Defaults: images/PDFs → vision, everything else → structural.\n    \
                 Override with opts.mode. Use opts.query to customize the vision extraction prompt.",
            params: DOC_READ_PARAMS_WITH_MODE,
            returns: ReturnType::String,
            example: Some(r#"local text = doc.read("/attachments/chart.png", {query = "extract all tables"})"#),
        },
        FnDoc {
            name: "readAsync",
            description:
                "Read a document asynchronously. Returns a DocFuture immediately (no blocking).\n    \
                 Call :await() on the future to get the result. The first :await() triggers parallel\n    \
                 resolution of ALL pending vision futures at once.\n    \
                 NOTE: Issue all readAsync() calls BEFORE the first :await() to get parallelism.\n    \
                 Interleaving readAsync/await/readAsync/await results in sequential calls.",
            params: DOC_READ_PARAMS_WITH_MODE,
            returns: ReturnType::UserData,
            example: Some(
                "local f1 = doc.readAsync(\"/attachments/page1.png\")\n\
                 local f2 = doc.readAsync(\"/attachments/page2.png\")\n\
                 local text1 = f1:await()  -- resolves f1+f2 in parallel\n\
                 local text2 = f2:await()  -- instant (already resolved)"
            ),
        },
        DOC_RENDER_FN,
        DOC_RENDER_FILE_FN,
    ],
};

#[cfg(feature = "pdfium-render")]
const DOC_PDF_INFO_DESC: &str =
    "Get PDF metadata: page count, page sizes, and form field detection.\n    \
         Always uses local PDFium parsing (never vision).";

#[cfg(feature = "pdfium-render")]
const DOC_PDF_FORM_FIELDS_DESC: &str =
    "List all form fields in a PDF. Returns a table of {name, type, value, readOnly} per field.\n    \
         Types: \"text\", \"checkbox\", \"radio\", \"combobox\", \"listbox\", \"signature\", \"pushbutton\", \"unknown\".\n    \
         Returns an empty table for non-form PDFs.";

#[cfg(feature = "pdfium-render")]
const DOC_PDF_FILL_DESC: &str =
    "Fill form fields in a PDF and save the result.\n    \
         Sets field values from a {fieldName = value} table. Strings for text, booleans for checkboxes/radios.\n    \
         Set flatten=true to bake fields into page content (removes interactive fields).";

#[cfg(feature = "pdfium-render")]
const DOC_PDF_MERGE_DESC: &str = "Merge multiple PDFs into one. Pages are concatenated in order.";

#[cfg(feature = "pdfium-render")]
const DOC_PDF_SPLIT_DESC: &str = "Split a PDF into multiple files by page ranges.\n    \
         Ranges are 1-indexed strings like \"1-3\", \"4\", \"5-7\".\n    \
         Output files are named split_1.pdf, split_2.pdf, etc.";

#[cfg(feature = "pdfium-render")]
const DOC_PDF_EDIT_PAGES_DESC: &str =
    "Delete, rotate, or reorder pages in a PDF.\n    \
         Operations: {type=\"delete\", pages={1,3}}, {type=\"rotate\", pages={1}, degrees=90}, {type=\"reorder\", order={3,1,2}}.\n    \
         Page numbers are 1-indexed.";

#[cfg(feature = "pdfium-render")]
const DOC_PDF_ANNOTATE_DESC: &str =
    "Add an annotation to a PDF page.\n    \
         Supported types: \"text\" (sticky note), \"freeText\" (text on page), \"highlight\", \"underline\", \"strikeout\", \"square\", \"stamp\".\n    \
         Coordinates are in PDF points (1 point = 1/72 inch), origin at bottom-left.";

#[cfg(feature = "pdfium-render")]
const DOC_PDF_WATERMARK_DESC: &str = "Add a text watermark to PDF pages.\n    \
         Text is centered on each page. Use rotation for diagonal watermarks.\n    \
         Set pages to a range string like \"1-3\" or omit for all pages.";

#[cfg(feature = "pdfium-render")]
const DOC_PDF_INFO_FN: FnDoc = FnDoc {
    name: "pdfInfo",
    description: DOC_PDF_INFO_DESC,
    params: DOC_PDF_INFO_PARAMS,
    returns: ReturnType::Table,
    example: Some(
        r#"local info = doc.pdfInfo("/data/form.pdf")  -- {pageCount=2, pageSizes={{width=612, height=792}}, hasForm=true}"#,
    ),
};

#[cfg(feature = "pdfium-render")]
const DOC_PDF_FORM_FIELDS_FN: FnDoc = FnDoc {
    name: "formFields",
    description: DOC_PDF_FORM_FIELDS_DESC,
    params: DOC_FORM_FIELDS_PARAMS,
    returns: ReturnType::Table,
    example: Some(
        r#"local fields = doc.formFields("/data/form.pdf")
for _, f in ipairs(fields) do print(f.name, f.type, f.value) end"#,
    ),
};

#[cfg(feature = "pdfium-render")]
const DOC_PDF_FILL_FN: FnDoc = FnDoc {
    name: "fillForm",
    description: DOC_PDF_FILL_DESC,
    params: DOC_FILL_FORM_PARAMS,
    returns: ReturnType::Void,
    example: Some(
        r#"doc.fillForm({path="/data/form.pdf", fields={name="Alice", agree=true}, output="/out/filled.pdf"})"#,
    ),
};

#[cfg(feature = "pdfium-render")]
const DOC_PDF_MERGE_FN: FnDoc = FnDoc {
    name: "mergePdf",
    description: DOC_PDF_MERGE_DESC,
    params: DOC_MERGE_PDF_PARAMS,
    returns: ReturnType::Void,
    example: Some(
        r#"doc.mergePdf({paths={"/data/a.pdf", "/data/b.pdf", "/data/c.pdf"}, output="/out/merged.pdf"})"#,
    ),
};

#[cfg(feature = "pdfium-render")]
const DOC_PDF_SPLIT_FN: FnDoc = FnDoc {
    name: "splitPdf",
    description: DOC_PDF_SPLIT_DESC,
    params: DOC_SPLIT_PDF_PARAMS,
    returns: ReturnType::Table,
    example: Some(
        r#"local paths = doc.splitPdf({path="/data/doc.pdf", ranges={"1-3", "4-6"}, outputDir="/out/"})"#,
    ),
};

#[cfg(feature = "pdfium-render")]
const DOC_PDF_EDIT_PAGES_FN: FnDoc = FnDoc {
    name: "editPages",
    description: DOC_PDF_EDIT_PAGES_DESC,
    params: DOC_EDIT_PAGES_PARAMS,
    returns: ReturnType::Void,
    example: Some(
        r#"doc.editPages({path="/data/doc.pdf", operations={{type="rotate", pages={1}, degrees=90}}, output="/out/edited.pdf"})"#,
    ),
};

#[cfg(feature = "pdfium-render")]
const DOC_PDF_ANNOTATE_FN: FnDoc = FnDoc {
    name: "addAnnotation",
    description: DOC_PDF_ANNOTATE_DESC,
    params: DOC_ADD_ANNOTATION_PARAMS,
    returns: ReturnType::Void,
    example: Some(
        r##"doc.addAnnotation({path="/data/doc.pdf", page=1, type="highlight", x=72, y=700, width=200, height=14, color="#FFFF00", output="/out/annotated.pdf"})"##,
    ),
};

#[cfg(feature = "pdfium-render")]
const DOC_PDF_WATERMARK_FN: FnDoc = FnDoc {
    name: "watermark",
    description: DOC_PDF_WATERMARK_DESC,
    params: DOC_WATERMARK_PARAMS,
    returns: ReturnType::Void,
    example: Some(
        r##"doc.watermark({path="/data/doc.pdf", text="CONFIDENTIAL", fontSize=72, color="#FF000040", rotation=45, output="/out/watermarked.pdf"})"##,
    ),
};

/// Module doc for structural-only mode WITH PDFium.
#[cfg(feature = "pdfium-render")]
pub(crate) static DOC_MOD_DOC_PDFIUM: ModuleDoc = ModuleDoc {
    name: "doc",
    summary: "document reading & conversion; structural parsing (pdf, xlsx, docx, pptx, html, ...)",
    functions: &[
        FnDoc {
            name: "read",
            description:
                "Read a document file and extract its text. Format auto-detected from extension.\n    \
                 Supported: xlsx, xls, xlsm, ods, docx, pdf, rtf, pptx, csv, txt, json, md, html.\n    \
                 All formats use local structural parsing.",
            params: DOC_READ_PARAMS,
            returns: ReturnType::String,
            example: Some(r#"local text = doc.read("/attachments/report.pdf")"#),
        },
        FnDoc {
            name: "readAsync",
            description:
                "Read a document asynchronously. Returns a DocFuture immediately (no blocking).\n    \
                 Call :await() on the future to get the result.",
            params: DOC_READ_PARAMS,
            returns: ReturnType::UserData,
            example: Some(
                "local f1 = doc.readAsync(\"/attachments/doc1.pdf\")\n\
                 local f2 = doc.readAsync(\"/attachments/doc2.pdf\")\n\
                 local text1 = f1:await()\n\
                 local text2 = f2:await()"
            ),
        },
        DOC_PDF_INFO_FN,
        DOC_PDF_FORM_FIELDS_FN,
        DOC_PDF_FILL_FN,
        DOC_PDF_MERGE_FN,
        DOC_PDF_SPLIT_FN,
        DOC_PDF_EDIT_PAGES_FN,
        DOC_PDF_ANNOTATE_FN,
        DOC_PDF_WATERMARK_FN,
        DOC_RENDER_FN,
        DOC_RENDER_FILE_FN,
    ],
};

/// Module doc for vision-enabled mode WITH PDFium.
#[cfg(feature = "pdfium-render")]
static DOC_MOD_DOC_VISION_PDFIUM: ModuleDoc = ModuleDoc {
    name: "doc",
    summary: "document reading & conversion; structural + vision modes (pdf, xlsx, docx, images, ...)",
    functions: &[
        FnDoc {
            name: "read",
            description:
                "Read a document file and extract its text. Format auto-detected from extension.\n    \
                 Supported: xlsx, xls, xlsm, ods, docx, pdf, rtf, pptx, csv, txt, json, md, html, png, jpg, webp, gif.\n    \
                 Two modes: \"structural\" (local parsing) and \"vision\" (AI multimodal analysis).\n    \
                 Defaults: images/PDFs → vision, everything else → structural.\n    \
                 Override with opts.mode. Use opts.query to customize the vision extraction prompt.",
            params: DOC_READ_PARAMS_WITH_MODE,
            returns: ReturnType::String,
            example: Some(r#"local text = doc.read("/attachments/chart.png", {query = "extract all tables"})"#),
        },
        FnDoc {
            name: "readAsync",
            description:
                "Read a document asynchronously. Returns a DocFuture immediately (no blocking).\n    \
                 Call :await() on the future to get the result. The first :await() triggers parallel\n    \
                 resolution of ALL pending vision futures at once.\n    \
                 NOTE: Issue all readAsync() calls BEFORE the first :await() to get parallelism.\n    \
                 Interleaving readAsync/await/readAsync/await results in sequential calls.",
            params: DOC_READ_PARAMS_WITH_MODE,
            returns: ReturnType::UserData,
            example: Some(
                "local f1 = doc.readAsync(\"/attachments/page1.png\")\n\
                 local f2 = doc.readAsync(\"/attachments/page2.png\")\n\
                 local text1 = f1:await()  -- resolves f1+f2 in parallel\n\
                 local text2 = f2:await()  -- instant (already resolved)"
            ),
        },
        DOC_PDF_INFO_FN,
        DOC_PDF_FORM_FIELDS_FN,
        DOC_PDF_FILL_FN,
        DOC_PDF_MERGE_FN,
        DOC_PDF_SPLIT_FN,
        DOC_PDF_EDIT_PAGES_FN,
        DOC_PDF_ANNOTATE_FN,
        DOC_PDF_WATERMARK_FN,
        DOC_RENDER_FN,
        DOC_RENDER_FILE_FN,
    ],
};

const DOC_RENDER_FN: FnDoc = FnDoc {
    name: "render",
    description:
        "Convert text between formats.\n    Supported paths: markdown→html, html→text, markdown→pdf, html→pdf.\n    PDF output is returned as a binary string.",
    params: DOC_RENDER_PARAMS,
    returns: ReturnType::String,
    example: Some(r##"doc.render({text="# Hello", from="markdown", to="html"})"##),
};

const DOC_RENDER_FILE_FN: FnDoc = FnDoc {
    name: "renderFile",
    description:
        "Convert a file and write the result. Formats auto-detected from extensions.\n    Render: md→html, md→pdf, html→pdf, html→txt.\n    Extract: xlsx, docx, pdf, rtf, pptx → txt.",
    params: DOC_RENDER_FILE_PARAMS,
    returns: ReturnType::Void,
    example: Some(r#"doc.renderFile({source="/workspace/report.md", target="/artifacts/report.pdf"})"#),
};

/// Extract page geometry from a Lua options table, falling back to defaults.
fn parse_page_options(opts: Option<&mlua::Table>) -> PageOptions {
    let mut p = PageOptions::default();
    if let Some(t) = opts {
        if let Ok(v) = t.get::<f64>("pageWidth") {
            p.page_width = v;
        }
        if let Ok(v) = t.get::<f64>("pageHeight") {
            p.page_height = v;
        }
        if let Ok(v) = t.get::<f64>("marginTop") {
            p.margin_top = v;
        }
        if let Ok(v) = t.get::<f64>("marginBottom") {
            p.margin_bottom = v;
        }
        if let Ok(v) = t.get::<f64>("marginLeft") {
            p.margin_left = v;
        }
        if let Ok(v) = t.get::<f64>("marginRight") {
            p.margin_right = v;
        }
        if let Ok(v) = t.get::<bool>("landscape") {
            p.landscape = v;
        }
    }
    p
}

/// A deferred read result returned by `doc.readAsync()`.
/// Exposes a single method `:await()` that blocks until the result is available.
struct DocFuture {
    result_slot: Arc<Mutex<Option<Result<String, String>>>>,
    pending_reads: PendingReads,
    callback: Option<VisionCallback>,
    cache_dir: Option<PathBuf>,
}

impl UserData for DocFuture {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("await", |_, this, ()| {
            // If already resolved, return immediately
            {
                let slot = this.result_slot.lock().unwrap();
                if let Some(ref result) = *slot {
                    return match result {
                        Ok(text) => Ok(text.clone()),
                        Err(e) => Err(mlua::Error::external(e.clone())),
                    };
                }
            }

            // Drain ALL pending reads and resolve them in parallel
            let pending: Vec<PendingRead> = {
                let mut queue = this.pending_reads.lock().unwrap();
                queue.drain(..).collect()
            };

            if pending.is_empty() {
                // No pending reads — this shouldn't happen for an unresolved future,
                // but handle gracefully
                return Err(mlua::Error::external(
                    "doc future: no pending reads to resolve",
                ));
            }

            // Resolve pending reads in parallel, bounded by a semaphore so at
            // most MAX_CONCURRENT_READS threads call the vision API at once.
            // Unlike chunk-based batching, a semaphore avoids head-of-line
            // blocking: as soon as any slot frees up, the next read starts.
            let semaphore = Semaphore::new(MAX_CONCURRENT_READS);

            std::thread::scope(|s| {
                let handles: Vec<_> = pending
                    .iter()
                    .map(|pr| {
                        let callback = &this.callback;
                        let cache_dir = &this.cache_dir;
                        let sem = &semaphore;
                        s.spawn(move || {
                            let _permit = sem.acquire_guard();
                            resolve_pending_read(pr, callback.as_ref(), cache_dir.as_ref());
                        })
                    })
                    .collect();

                for h in handles {
                    if let Err(e) = h.join() {
                        let msg = e
                            .downcast_ref::<String>()
                            .map(|s| s.as_str())
                            .or_else(|| e.downcast_ref::<&str>().copied())
                            .unwrap_or("unknown panic");
                        eprintln!(
                            "[doc-vision] thread panicked during readAsync resolution: {}",
                            msg
                        );
                    }
                }
            });

            // Return this future's result
            let slot = this.result_slot.lock().unwrap();
            match slot.as_ref() {
                Some(Ok(text)) => Ok(text.clone()),
                Some(Err(e)) => Err(mlua::Error::external(e.clone())),
                None => Err(mlua::Error::external(
                    "doc future: result not available after resolution",
                )),
            }
        });
    }
}

/// Resolve a single pending read: try cache → callback (authoritative when present) → local extraction.
fn resolve_pending_read(
    pr: &PendingRead,
    callback: Option<&VisionCallback>,
    cache_dir: Option<&PathBuf>,
) {
    // Check disk cache first
    if let Some(dir) = cache_dir {
        if let Some(cached) = cache_read(dir, &pr.cache_key) {
            *pr.result_slot.lock().unwrap() = Some(Ok(cached));
            return;
        }
    }

    // Try callback
    if let Some(cb) = callback {
        match cb(&pr.data, &pr.filename, &pr.query) {
            Ok(text) => {
                // Cache the result
                if let Some(dir) = cache_dir {
                    cache_write(dir, &pr.cache_key, &text);
                }
                *pr.result_slot.lock().unwrap() = Some(Ok(text));
                return;
            }
            Err(e) => {
                // Callback failed — no fallback when callback is provided
                *pr.result_slot.lock().unwrap() = Some(Err(e));
                return;
            }
        }
    }

    // Local extraction fallback
    let result = read_document(&pr.data, pr.format, &pr.read_opts);
    *pr.result_slot.lock().unwrap() = Some(result);
}

/// Parse common arguments for doc.read() and doc.readAsync():
/// Returns (path, read_opts, query).
fn parse_doc_read_args(
    fn_name: &str,
    args: &MultiValue,
) -> Result<(String, ReadOptions, String), mlua::Error> {
    if args.is_empty() {
        return Err(arg_error(fn_name, DOC_READ_PARAMS));
    }
    let first = args[0].clone();
    let opts_opt = args.get(1).and_then(|v| match v {
        mlua::Value::Table(t) => Some(t.clone()),
        _ => None,
    });
    // Accept both positional and named-param table forms:
    //   doc.read("/file.xlsx", {sheet=2, query="..."})
    //   doc.read({path="/file.xlsx", sheet=2, query="..."})
    let (path, opts): (String, Option<mlua::Table>) = match first {
        mlua::Value::Table(ref t) => {
            let path = t
                .get::<String>("path")
                .or_else(|_| t.get::<String>(1))
                .map_err(|_| {
                    mlua::Error::external(format!("{}: table must have 'path' or [1]", fn_name))
                })?;
            (path, Some(t.clone()))
        }
        mlua::Value::String(ref s) => (s.to_string_lossy().to_string(), opts_opt),
        _ => {
            return Err(mlua::Error::external(format!(
                "{}: first arg must be a string or table",
                fn_name
            )))
        }
    };

    let sheet = opts
        .as_ref()
        .and_then(|t| t.get::<i32>("sheet").ok())
        .map(|n| n as usize);
    let query = opts
        .as_ref()
        .and_then(|t| t.get::<String>("query").ok())
        .unwrap_or_else(|| DEFAULT_EXTRACTION_QUERY.to_string());
    let mode = opts
        .as_ref()
        .and_then(|t| t.get::<String>("mode").ok())
        .map(|s| match s.as_str() {
            "structural" => Ok(ReadMode::Structural),
            "vision" => Ok(ReadMode::Vision),
            other => Err(mlua::Error::external(format!(
                "{}: invalid mode '{}' (expected 'structural' or 'vision')",
                fn_name, other
            ))),
        })
        .transpose()?;
    let read_opts = ReadOptions { sheet, mode };

    Ok((path, read_opts, query))
}

/// Register `doc.*` globals in the Lua VM.
pub(crate) fn register_doc_globals(
    lua: &Lua,
    mounts: Arc<MountTable>,
    vision_callback: Option<VisionCallback>,
    cache_dir: Option<PathBuf>,
    #[cfg(feature = "pdfium-render")] pdfium_engine: Option<Arc<PdfiumEngine>>,
) -> Result<(), mlua::Error> {
    let doc = lua.create_table()?;
    let pending_reads: PendingReads = Arc::new(Mutex::new(Vec::new()));

    register_doc_readers(
        lua,
        &doc,
        mounts.clone(),
        vision_callback.clone(),
        cache_dir.clone(),
        pending_reads.clone(),
        #[cfg(feature = "pdfium-render")]
        pdfium_engine.clone(),
    )?;

    #[cfg(feature = "pdfium-render")]
    register_doc_pdf(lua, &doc, mounts.clone(), pdfium_engine.clone())?;

    register_doc_rendering(lua, &doc, mounts.clone())?;

    // Select documentation based on compiled features and vision callback.
    // PDF functions stay visible when PDFium is compiled in but not available;
    // calls then return the same "requires PDFium" runtime error as before.
    let mod_doc = match vision_callback.is_some() {
        #[cfg(feature = "pdfium-render")]
        true => &DOC_MOD_DOC_VISION_PDFIUM,
        #[cfg(feature = "pdfium-render")]
        false => &DOC_MOD_DOC_PDFIUM,
        #[cfg(not(feature = "pdfium-render"))]
        true => &DOC_MOD_DOC_VISION,
        #[cfg(not(feature = "pdfium-render"))]
        false => &DOC_MOD_DOC,
    };
    crate::lua_util::register_help_functions(lua, &doc, mod_doc)?;

    lua.globals().set("doc", doc)?;
    wrap_module_with_help_hints(lua, "doc")?;

    Ok(())
}

fn register_doc_readers(
    lua: &Lua,
    doc: &mlua::Table,
    mounts: Arc<MountTable>,
    vision_callback: Option<VisionCallback>,
    cache_dir: Option<PathBuf>,
    pending_reads: PendingReads,
    #[cfg(feature = "pdfium-render")] pdfium_engine: Option<Arc<PdfiumEngine>>,
) -> Result<(), mlua::Error> {
    // doc.read(path, opts?) -> string
    // Also accepts: doc.read({path=..., sheet=N, query=..., mode=...})
    {
        let m = mounts.clone();
        let cb = vision_callback.clone();
        let cd = cache_dir.clone();
        #[cfg(feature = "pdfium-render")]
        let pe = pdfium_engine.clone();
        doc.set(
        "read",
        lua.create_function(move |_, args: MultiValue| {
            let (path, read_opts, query) = parse_doc_read_args("doc.read", &args)?;

            // Detect format
            let format = DocFormat::from_extension(&path).ok_or_else(|| {
                mlua::Error::external(format!(
                    "unsupported file format: {}",
                    std::path::Path::new(&path)
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("(no extension)")
                ))
            })?;

            // Resolve mode: explicit > format default
            let mode = format.resolve_mode(read_opts.mode, cb.is_some());

            // Resolve path through mount table and read raw bytes
            let host_path = m.resolve_read(&path).map_err(mlua::Error::external)?;
            let data = std::fs::read(&host_path).map_err(mlua::Error::external)?;

            match mode {
                ReadMode::Structural => {
                    // For PDFs, prefer PDFium when available
                    #[cfg(feature = "pdfium-render")]
                    if format == DocFormat::Pdf {
                        if let Some(ref engine) = pe {
                            return crate::doc_reader::read_pdf_pdfium(engine, &data)
                                .map_err(mlua::Error::external);
                        }
                    }
                    // Local extraction fallback — never calls callback
                    read_document(&data, format, &read_opts)
                        .map_err(mlua::Error::external)
                }
                ReadMode::Vision => {
                    let callback = cb.as_ref().ok_or_else(|| {
                        mlua::Error::external(
                            "vision mode requires a vision callback (not available in this environment)"
                        )
                    })?;

                    let filename = std::path::Path::new(&path)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("file")
                        .to_string();

                    let key = cache_key(&data, &query);

                    // Check disk cache (vision results only)
                    if let Some(ref dir) = cd {
                        if let Some(cached) = cache_read(dir, &key) {
                            return Ok(cached);
                        }
                    }

                    match callback(&data, &filename, &query) {
                        Ok(text) => {
                            if let Some(ref dir) = cd {
                                cache_write(dir, &key, &text);
                            }
                            Ok(text)
                        }
                        Err(e) => Err(mlua::Error::external(e)),
                    }
                }
            }
        })?,
    )?;
    }

    // doc.readAsync(path, opts?) -> DocFuture
    // Returns immediately. Resolution deferred to :await().
    {
        let m = mounts.clone();
        let cb = vision_callback.clone();
        let cd = cache_dir.clone();
        let pq = pending_reads.clone();
        #[cfg(feature = "pdfium-render")]
        let pe = pdfium_engine.clone();
        doc.set(
        "readAsync",
        lua.create_function(move |_, args: MultiValue| {
            let (path, read_opts, query) = parse_doc_read_args("doc.readAsync", &args)?;

            // Detect format (fail fast)
            let format = DocFormat::from_extension(&path).ok_or_else(|| {
                mlua::Error::external(format!(
                    "unsupported file format: {}",
                    std::path::Path::new(&path)
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("(no extension)")
                ))
            })?;

            // Resolve mode
            let mode = format.resolve_mode(read_opts.mode, cb.is_some());

            // Resolve path and read bytes (fail fast)
            let host_path = m.resolve_read(&path).map_err(mlua::Error::external)?;
            let data = std::fs::read(&host_path).map_err(mlua::Error::external)?;

            let result_slot = Arc::new(Mutex::new(None));

            match mode {
                ReadMode::Structural => {
                    // For PDFs, prefer PDFium when available
                    #[cfg(feature = "pdfium-render")]
                    let result = if format == DocFormat::Pdf {
                        if let Some(ref engine) = pe {
                            crate::doc_reader::read_pdf_pdfium(engine, &data)
                        } else {
                            read_document(&data, format, &read_opts)
                        }
                    } else {
                        read_document(&data, format, &read_opts)
                    };
                    #[cfg(not(feature = "pdfium-render"))]
                    let result = read_document(&data, format, &read_opts);
                    // Structural reads resolve immediately in-place (no queuing)
                    *result_slot.lock().unwrap() = Some(result);
                }
                ReadMode::Vision => {
                    if cb.is_none() {
                        *result_slot.lock().unwrap() = Some(Err(
                            "vision mode requires a vision callback (not available in this environment)".to_string()
                        ));
                    } else {
                        let filename = std::path::Path::new(&path)
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("file")
                            .to_string();
                        let key = cache_key(&data, &query);

                        // Check disk cache — if hit, pre-resolve
                        if let Some(ref dir) = cd {
                            if let Some(cached) = cache_read(dir, &key) {
                                *result_slot.lock().unwrap() = Some(Ok(cached));
                                return Ok(DocFuture {
                                    result_slot,
                                    pending_reads: pq.clone(),
                                    callback: cb.clone(),
                                    cache_dir: cd.clone(),
                                });
                            }
                        }

                        // Defer to pending queue for parallel resolution
                        let mut queue = pq.lock().unwrap();
                        queue.push(PendingRead {
                            data,
                            filename,
                            format,
                            query,
                            read_opts,
                            cache_key: key,
                            result_slot: result_slot.clone(),
                        });
                    }
                }
            }

            Ok(DocFuture {
                result_slot,
                pending_reads: pq.clone(),
                callback: cb.clone(),
                cache_dir: cd.clone(),
            })
        })?,
    )?;
    }

    Ok(())
}

#[cfg(feature = "pdfium-render")]
fn register_doc_pdf(
    lua: &Lua,
    doc: &mlua::Table,
    mounts: Arc<MountTable>,
    pdfium_engine: Option<Arc<PdfiumEngine>>,
) -> Result<(), mlua::Error> {
    register_doc_pdf_metadata(lua, doc, mounts.clone(), pdfium_engine.clone())?;
    register_doc_pdf_page_ops(lua, doc, mounts.clone(), pdfium_engine.clone())?;
    register_doc_pdf_annotations(lua, doc, mounts.clone(), pdfium_engine.clone())?;
    Ok(())
}

#[cfg(feature = "pdfium-render")]
fn parse_pdf_path_arg(
    fn_name: &str,
    args: &MultiValue,
    params: &'static [Param],
) -> Result<String, mlua::Error> {
    if args.is_empty() {
        return Err(arg_error(fn_name, params));
    }
    match &args[0] {
        mlua::Value::String(s) => Ok(s.to_string_lossy().to_string()),
        mlua::Value::Table(t) => t
            .get::<String>("path")
            .or_else(|_| t.get::<String>(1))
            .map_err(|_| {
                mlua::Error::external(format!("{fn_name}: table must have 'path' or [1]"))
            }),
        _ => Err(mlua::Error::external(format!(
            "{fn_name}: first arg must be a string or table"
        ))),
    }
}

#[cfg(feature = "pdfium-render")]
fn require_pdfium_engine<'a>(
    engine: &'a Option<Arc<PdfiumEngine>>,
    fn_name: &str,
) -> Result<&'a PdfiumEngine, mlua::Error> {
    engine
        .as_deref()
        .ok_or_else(|| mlua::Error::external(format!("{fn_name} requires PDFium (not available)")))
}

#[cfg(feature = "pdfium-render")]
fn read_mounted_pdf(mounts: &MountTable, path: &str) -> Result<Vec<u8>, mlua::Error> {
    let host_path = mounts.resolve_read(path).map_err(mlua::Error::external)?;
    std::fs::read(&host_path).map_err(mlua::Error::external)
}

#[cfg(feature = "pdfium-render")]
fn pdf_backend_error(error: String) -> mlua::Error {
    mlua::Error::external(canonical_pdf_backend_error(error))
}

#[cfg(feature = "pdfium-render")]
fn canonical_pdf_backend_error(error: String) -> String {
    for (legacy, canonical) in [
        ("pdfInfo:", "doc.pdfInfo:"),
        ("formFields:", "doc.formFields:"),
        ("fillForm:", "doc.fillForm:"),
        ("mergePdf:", "doc.mergePdf:"),
        ("splitPdf:", "doc.splitPdf:"),
        ("editPages:", "doc.editPages:"),
        ("addAnnotation:", "doc.addAnnotation:"),
        ("watermark:", "doc.watermark:"),
    ] {
        if let Some(rest) = error.strip_prefix(legacy) {
            return format!("{canonical}{rest}");
        }
    }
    error
}

#[cfg(feature = "pdfium-render")]
fn register_doc_pdf_metadata(
    lua: &Lua,
    doc: &mlua::Table,
    mounts: Arc<MountTable>,
    pdfium_engine: Option<Arc<PdfiumEngine>>,
) -> Result<(), mlua::Error> {
    // doc.pdfInfo(path) -> table — PDF metadata (always structural/PDFium)
    #[cfg(feature = "pdfium-render")]
    {
        let m = mounts.clone();
        let pe = pdfium_engine.clone();
        doc.set(
            "pdfInfo",
            lua.create_function(move |lua, args: MultiValue| {
                let path = parse_pdf_path_arg("doc.pdfInfo", &args, DOC_PDF_INFO_PARAMS)?;
                let engine = require_pdfium_engine(&pe, "doc.pdfInfo")?;
                let data = read_mounted_pdf(&m, &path)?;
                let info = crate::doc_reader::pdf_info(engine, &data).map_err(pdf_backend_error)?;

                let result = lua.create_table()?;
                result.set("pageCount", info.page_count as i64)?;
                result.set("hasForm", info.has_form)?;

                let sizes = lua.create_table()?;
                for (i, ps) in info.page_sizes.iter().enumerate() {
                    let page_t = lua.create_table()?;
                    page_t.set("width", ps.width as f64)?;
                    page_t.set("height", ps.height as f64)?;
                    sizes.set((i + 1) as i64, page_t)?;
                }
                result.set("pageSizes", sizes)?;

                Ok(mlua::Value::Table(result))
            })?,
        )?;
    }

    // doc.formFields(path) -> table — list form fields in a PDF
    #[cfg(feature = "pdfium-render")]
    {
        let m = mounts.clone();
        let pe = pdfium_engine.clone();
        doc.set(
            "formFields",
            lua.create_function(move |lua, args: MultiValue| {
                let path = parse_pdf_path_arg("doc.formFields", &args, DOC_FORM_FIELDS_PARAMS)?;
                let engine = require_pdfium_engine(&pe, "doc.formFields")?;
                let data = read_mounted_pdf(&m, &path)?;
                let fields =
                    crate::doc_reader::pdf_form_fields(engine, &data).map_err(pdf_backend_error)?;

                let result = lua.create_table()?;
                for (i, f) in fields.iter().enumerate() {
                    let field_t = lua.create_table()?;
                    field_t.set("name", f.name.as_str())?;
                    field_t.set("type", f.field_type.as_str())?;
                    field_t.set("value", f.value.as_str())?;
                    field_t.set("readOnly", f.read_only)?;
                    result.set((i + 1) as i64, field_t)?;
                }

                Ok(mlua::Value::Table(result))
            })?,
        )?;
    }

    Ok(())
}

#[cfg(feature = "pdfium-render")]
fn register_doc_pdf_page_ops(
    lua: &Lua,
    doc: &mlua::Table,
    mounts: Arc<MountTable>,
    pdfium_engine: Option<Arc<PdfiumEngine>>,
) -> Result<(), mlua::Error> {
    // doc.fillForm({path, fields, output, flatten?}) -> void
    #[cfg(feature = "pdfium-render")]
    {
        let m = mounts.clone();
        let pe = pdfium_engine.clone();
        doc.set(
            "fillForm",
            lua.create_function(move |_, args: MultiValue| {
                if args.is_empty() {
                    return Err(arg_error("doc.fillForm", DOC_FILL_FORM_PARAMS));
                }
                let opts = match &args[0] {
                    mlua::Value::Table(t) => t.clone(),
                    _ => return Err(mlua::Error::external(
                        "doc.fillForm: argument must be a table {path, fields, output, flatten?}",
                    )),
                };

                let path: String = opts
                    .get::<String>("path")
                    .map_err(|_| mlua::Error::external("doc.fillForm: missing 'path' field"))?;
                let fields_table: mlua::Table = opts
                    .get::<mlua::Table>("fields")
                    .map_err(|_| mlua::Error::external("doc.fillForm: missing 'fields' table"))?;
                let output: String = opts
                    .get::<String>("output")
                    .map_err(|_| mlua::Error::external("doc.fillForm: missing 'output' field"))?;
                let flatten: bool = opts.get::<bool>("flatten").unwrap_or(false);

                // Convert the Lua fields table to Vec<(String, String)>
                let mut field_values: Vec<(String, String)> = Vec::new();
                for pair in fields_table.pairs::<String, mlua::Value>() {
                    let (key, val) = pair.map_err(|e| {
                        mlua::Error::external(format!("doc.fillForm: invalid fields table: {}", e))
                    })?;
                    let str_val = match val {
                        mlua::Value::String(s) => s.to_string_lossy().to_string(),
                        mlua::Value::Boolean(b) => {
                            if b {
                                "true".to_string()
                            } else {
                                "false".to_string()
                            }
                        }
                        mlua::Value::Integer(n) => n.to_string(),
                        mlua::Value::Number(n) => n.to_string(),
                        _ => {
                            return Err(mlua::Error::external(format!(
                                "doc.fillForm: field '{}' has unsupported value type",
                                key
                            )))
                        }
                    };
                    field_values.push((key, str_val));
                }

                let engine = require_pdfium_engine(&pe, "doc.fillForm")?;
                let data = read_mounted_pdf(&m, &path)?;

                let result_bytes =
                    crate::doc_reader::pdf_fill_form(engine, &data, &field_values, flatten)
                        .map_err(pdf_backend_error)?;

                let host_output = m.resolve_write(&output).map_err(mlua::Error::external)?;
                std::fs::write(&host_output, result_bytes).map_err(mlua::Error::external)?;

                Ok(())
            })?,
        )?;
    }

    // doc.mergePdf({paths, output}) -> void
    #[cfg(feature = "pdfium-render")]
    {
        let m = mounts.clone();
        let pe = pdfium_engine.clone();
        doc.set(
            "mergePdf",
            lua.create_function(move |_, args: MultiValue| {
                if args.is_empty() {
                    return Err(arg_error("doc.mergePdf", DOC_MERGE_PDF_PARAMS));
                }
                let opts = match &args[0] {
                    mlua::Value::Table(t) => t.clone(),
                    _ => {
                        return Err(mlua::Error::external(
                            "doc.mergePdf: argument must be a table {paths, output}",
                        ))
                    }
                };

                let paths_table: mlua::Table = opts
                    .get::<mlua::Table>("paths")
                    .map_err(|_| mlua::Error::external("doc.mergePdf: missing 'paths' table"))?;
                let output: String = opts
                    .get::<String>("output")
                    .map_err(|_| mlua::Error::external("doc.mergePdf: missing 'output' field"))?;

                let engine = require_pdfium_engine(&pe, "doc.mergePdf")?;

                // Read all PDF files
                let mut pdf_data: Vec<(String, Vec<u8>)> = Vec::new();
                for i in 1..=paths_table.len()? {
                    let path: String = paths_table.get::<String>(i)?;
                    let data = read_mounted_pdf(&m, &path)?;
                    pdf_data.push((path, data));
                }

                let refs: Vec<(&str, &[u8])> = pdf_data
                    .iter()
                    .map(|(name, data)| (name.as_str(), data.as_slice()))
                    .collect();

                let result_bytes =
                    crate::doc_reader::pdf_merge(engine, &refs).map_err(pdf_backend_error)?;

                let host_output = m.resolve_write(&output).map_err(mlua::Error::external)?;
                std::fs::write(&host_output, result_bytes).map_err(mlua::Error::external)?;

                Ok(())
            })?,
        )?;
    }

    // doc.splitPdf({path, ranges, outputDir}) -> table of output paths
    #[cfg(feature = "pdfium-render")]
    {
        let m = mounts.clone();
        let pe = pdfium_engine.clone();
        doc.set(
            "splitPdf",
            lua.create_function(move |lua, args: MultiValue| {
                if args.is_empty() {
                    return Err(arg_error("doc.splitPdf", DOC_SPLIT_PDF_PARAMS));
                }
                let opts = match &args[0] {
                    mlua::Value::Table(t) => t.clone(),
                    _ => {
                        return Err(mlua::Error::external(
                            "doc.splitPdf: argument must be a table {path, ranges, outputDir}",
                        ))
                    }
                };

                let path: String = opts
                    .get::<String>("path")
                    .map_err(|_| mlua::Error::external("doc.splitPdf: missing 'path' field"))?;
                let ranges_table: mlua::Table = opts
                    .get::<mlua::Table>("ranges")
                    .map_err(|_| mlua::Error::external("doc.splitPdf: missing 'ranges' table"))?;
                let output_dir: String = opts.get::<String>("outputDir").map_err(|_| {
                    mlua::Error::external("doc.splitPdf: missing 'outputDir' field")
                })?;

                let engine = require_pdfium_engine(&pe, "doc.splitPdf")?;

                // Parse ranges
                let mut ranges: Vec<String> = Vec::new();
                for i in 1..=ranges_table.len()? {
                    let r: String = ranges_table.get::<String>(i)?;
                    ranges.push(r);
                }

                // Read source PDF
                let data = read_mounted_pdf(&m, &path)?;

                let parts = crate::doc_reader::pdf_split(engine, &data, &ranges)
                    .map_err(pdf_backend_error)?;

                // Write output files and collect paths
                let result = lua.create_table()?;
                for (i, bytes) in parts.iter().enumerate() {
                    let filename = format!("split_{}.pdf", i + 1);
                    let virtual_path = if output_dir.ends_with('/') {
                        format!("{}{}", output_dir, filename)
                    } else {
                        format!("{}/{}", output_dir, filename)
                    };
                    let host_output = m
                        .resolve_write(&virtual_path)
                        .map_err(mlua::Error::external)?;
                    std::fs::write(&host_output, bytes).map_err(mlua::Error::external)?;
                    result.set((i + 1) as i64, virtual_path)?;
                }

                Ok(mlua::Value::Table(result))
            })?,
        )?;
    }

    // doc.editPages({path, operations, output}) -> void
    #[cfg(feature = "pdfium-render")]
    {
        let m = mounts.clone();
        let pe = pdfium_engine.clone();
        let edit_pages = lua.create_function(move |_, args: MultiValue| {
            use crate::doc_reader::PageOperation;

            if args.is_empty() {
                return Err(arg_error("doc.editPages", DOC_EDIT_PAGES_PARAMS));
            }
            let opts = match &args[0] {
                mlua::Value::Table(t) => t.clone(),
                _ => return Err(mlua::Error::external(
                    "doc.editPages: argument must be a table {path, operations, output}"
                )),
            };

            let path: String = opts.get::<String>("path")
                .map_err(|_| mlua::Error::external("doc.editPages: missing 'path' field"))?;
            let ops_table: mlua::Table = opts.get::<mlua::Table>("operations")
                .map_err(|_| mlua::Error::external("doc.editPages: missing 'operations' table"))?;
            let output: String = opts.get::<String>("output")
                .map_err(|_| mlua::Error::external("doc.editPages: missing 'output' field"))?;

            let engine = require_pdfium_engine(&pe, "doc.editPages")?;

            // Parse operations from Lua table
            let mut operations: Vec<PageOperation> = Vec::new();
            for i in 1..=ops_table.len()? {
                let op: mlua::Table = ops_table.get::<mlua::Table>(i)?;
                let op_type: String = op.get::<String>("type")
                    .map_err(|_| mlua::Error::external(
                        "doc.editPages: each operation must have a 'type' field"
                    ))?;

                match op_type.as_str() {
                    "delete" => {
                        let pages_table: mlua::Table = op.get::<mlua::Table>("pages")
                            .map_err(|_| mlua::Error::external(
                                "doc.editPages: delete operation requires 'pages' table"
                            ))?;
                        let mut pages: Vec<u16> = Vec::new();
                        for j in 1..=pages_table.len()? {
                            let p: i64 = pages_table.get::<i64>(j)?;
                            if p < 1 {
                                return Err(mlua::Error::external(
                                    "doc.editPages: page numbers are 1-indexed"
                                ));
                            }
                            pages.push((p - 1) as u16); // Convert to 0-indexed
                        }
                        operations.push(PageOperation::Delete(pages));
                    }
                    "rotate" => {
                        let pages_table: mlua::Table = op.get::<mlua::Table>("pages")
                            .map_err(|_| mlua::Error::external(
                                "doc.editPages: rotate operation requires 'pages' table"
                            ))?;
                        let degrees: i64 = op.get::<i64>("degrees")
                            .map_err(|_| mlua::Error::external(
                                "doc.editPages: rotate operation requires 'degrees' field"
                            ))?;
                        let mut rotations: Vec<(u16, u16)> = Vec::new();
                        for j in 1..=pages_table.len()? {
                            let p: i64 = pages_table.get::<i64>(j)?;
                            if p < 1 {
                                return Err(mlua::Error::external(
                                    "doc.editPages: page numbers are 1-indexed"
                                ));
                            }
                            rotations.push(((p - 1) as u16, degrees as u16));
                        }
                        operations.push(PageOperation::Rotate(rotations));
                    }
                    "reorder" => {
                        let order_table: mlua::Table = op.get::<mlua::Table>("order")
                            .map_err(|_| mlua::Error::external(
                                "doc.editPages: reorder operation requires 'order' table"
                            ))?;
                        let mut order: Vec<u16> = Vec::new();
                        for j in 1..=order_table.len()? {
                            let p: i64 = order_table.get::<i64>(j)?;
                            if p < 1 {
                                return Err(mlua::Error::external(
                                    "doc.editPages: page numbers are 1-indexed"
                                ));
                            }
                            order.push((p - 1) as u16); // Convert to 0-indexed
                        }
                        operations.push(PageOperation::Reorder(order));
                    }
                    other => {
                        return Err(mlua::Error::external(format!(
                            "doc.editPages: unknown operation type '{}' (expected 'delete', 'rotate', or 'reorder')",
                            other
                        )));
                    }
                }
            }

            // Read source PDF
            let data = read_mounted_pdf(&m, &path)?;

            let result_bytes = crate::doc_reader::pdf_edit_pages(engine, &data, &operations)
                .map_err(pdf_backend_error)?;

            let host_output = m.resolve_write(&output).map_err(mlua::Error::external)?;
            std::fs::write(&host_output, result_bytes).map_err(mlua::Error::external)?;

            Ok(())
        })?;
        doc.set("editPages", edit_pages)?;
    }

    Ok(())
}

#[cfg(feature = "pdfium-render")]
fn register_doc_pdf_annotations(
    lua: &Lua,
    doc: &mlua::Table,
    mounts: Arc<MountTable>,
    pdfium_engine: Option<Arc<PdfiumEngine>>,
) -> Result<(), mlua::Error> {
    // doc.addAnnotation({path, page, type, x, y, width, height, color?, contents?, output}) -> void
    #[cfg(feature = "pdfium-render")]
    {
        let m = mounts.clone();
        let pe = pdfium_engine.clone();
        let add_annotation = lua.create_function(move |_, args: MultiValue| {
            use crate::doc_reader::{AnnotationParams, AnnotationType};

            if args.is_empty() {
                return Err(arg_error("doc.addAnnotation", DOC_ADD_ANNOTATION_PARAMS));
            }
            let opts = match &args[0] {
                mlua::Value::Table(t) => t.clone(),
                _ => return Err(mlua::Error::external(
                    "doc.addAnnotation: argument must be a table {path, page, type, x, y, width, height, output, ...}"
                )),
            };

            let path: String = opts.get::<String>("path")
                .map_err(|_| mlua::Error::external("doc.addAnnotation: missing 'path' field"))?;
            let page: i64 = opts.get::<i64>("page")
                .map_err(|_| mlua::Error::external("doc.addAnnotation: missing 'page' field"))?;
            if page < 1 {
                return Err(mlua::Error::external("doc.addAnnotation: page numbers are 1-indexed"));
            }
            let annot_type_str: String = opts.get::<String>("type")
                .map_err(|_| mlua::Error::external("doc.addAnnotation: missing 'type' field"))?;
            let x: f64 = opts.get::<f64>("x")
                .map_err(|_| mlua::Error::external("doc.addAnnotation: missing 'x' field"))?;
            let y: f64 = opts.get::<f64>("y")
                .map_err(|_| mlua::Error::external("doc.addAnnotation: missing 'y' field"))?;
            let width: f64 = opts.get::<f64>("width")
                .map_err(|_| mlua::Error::external("doc.addAnnotation: missing 'width' field"))?;
            let height: f64 = opts.get::<f64>("height")
                .map_err(|_| mlua::Error::external("doc.addAnnotation: missing 'height' field"))?;
            let output: String = opts.get::<String>("output")
                .map_err(|_| mlua::Error::external("doc.addAnnotation: missing 'output' field"))?;

            let annotation_type = match annot_type_str.as_str() {
                "text" => AnnotationType::Text,
                "freeText" => AnnotationType::FreeText,
                "highlight" => AnnotationType::Highlight,
                "underline" => AnnotationType::Underline,
                "strikeout" => AnnotationType::Strikeout,
                "square" => AnnotationType::Square,
                "stamp" => AnnotationType::Stamp,
                other => return Err(mlua::Error::external(format!(
                    "doc.addAnnotation: unknown type '{}' (expected: text, freeText, highlight, underline, strikeout, square, stamp)",
                    other
                ))),
            };

            // Parse optional color from hex string
            let color = opts.get::<String>("color").ok().map(|s| pdf_utils::parse_hex_color(&s))
                .transpose()
                .map_err(mlua::Error::external)?;

            let contents = opts.get::<String>("contents").ok();

            let engine = require_pdfium_engine(&pe, "doc.addAnnotation")?;
            let data = read_mounted_pdf(&m, &path)?;

            let params = AnnotationParams {
                page: (page - 1) as u16,
                annotation_type,
                x: x as f32,
                y: y as f32,
                width: width as f32,
                height: height as f32,
                color,
                contents,
            };

            let result_bytes = crate::doc_reader::pdf_add_annotation(engine, &data, &params)
                .map_err(pdf_backend_error)?;

            let host_output = m.resolve_write(&output).map_err(mlua::Error::external)?;
            std::fs::write(&host_output, result_bytes).map_err(mlua::Error::external)?;

            Ok(())
        })?;
        doc.set("addAnnotation", add_annotation)?;
    }

    // doc.watermark({path, text, fontSize?, color?, rotation?, pages?, output}) -> void
    #[cfg(feature = "pdfium-render")]
    {
        let m = mounts.clone();
        let pe = pdfium_engine.clone();
        doc.set(
            "watermark",
            lua.create_function(move |_, args: MultiValue| {
                use crate::doc_reader::WatermarkParams;

                if args.is_empty() {
                    return Err(arg_error("doc.watermark", DOC_WATERMARK_PARAMS));
                }
                let opts = match &args[0] {
                    mlua::Value::Table(t) => t.clone(),
                    _ => {
                        return Err(mlua::Error::external(
                            "doc.watermark: argument must be a table {path, text, output, ...}",
                        ))
                    }
                };

                let path: String = opts
                    .get::<String>("path")
                    .map_err(|_| mlua::Error::external("doc.watermark: missing 'path' field"))?;
                let text: String = opts
                    .get::<String>("text")
                    .map_err(|_| mlua::Error::external("doc.watermark: missing 'text' field"))?;
                let output: String = opts
                    .get::<String>("output")
                    .map_err(|_| mlua::Error::external("doc.watermark: missing 'output' field"))?;

                let font_size: f32 = opts.get::<f64>("fontSize").unwrap_or(48.0) as f32;
                let rotation: f32 = opts.get::<f64>("rotation").unwrap_or(45.0) as f32;

                // Parse optional color, default to semi-transparent black
                let color = match opts.get::<String>("color") {
                    Ok(s) => pdf_utils::parse_hex_color(&s).map_err(mlua::Error::external)?,
                    Err(_) => (0, 0, 0, 64), // #00000040
                };

                // Parse page range
                let pages = match opts.get::<String>("pages") {
                    Ok(s) if s == "all" => None,
                    Ok(s) => {
                        // We need to know total pages to validate ranges.
                        // Load the PDF to get page count first.
                        let engine = require_pdfium_engine(&pe, "doc.watermark")?;
                        let data = read_mounted_pdf(&m, &path)?;
                        let info = crate::doc_reader::pdf_info(engine, &data)
                            .map_err(pdf_backend_error)?;
                        let indices =
                            crate::doc_reader::parse_page_range_public(&s, info.page_count as u16)
                                .map_err(mlua::Error::external)?;
                        // Store data so we don't re-read later
                        // Actually, let's just parse pages and continue. We'll re-read in the function.
                        Some(indices)
                    }
                    Err(_) => None,
                };

                let engine = require_pdfium_engine(&pe, "doc.watermark")?;
                let data = read_mounted_pdf(&m, &path)?;

                let params = WatermarkParams {
                    text,
                    font_size,
                    color,
                    rotation,
                    pages,
                };

                let result_bytes = crate::doc_reader::pdf_watermark(engine, &data, &params)
                    .map_err(pdf_backend_error)?;

                let host_output = m.resolve_write(&output).map_err(mlua::Error::external)?;
                std::fs::write(&host_output, result_bytes).map_err(mlua::Error::external)?;

                Ok(())
            })?,
        )?;
    }

    Ok(())
}

fn register_doc_rendering(
    lua: &Lua,
    doc: &mlua::Table,
    mounts: Arc<MountTable>,
) -> Result<(), mlua::Error> {
    // doc.render(text, from, to, opts?) -> string (or binary string for PDF)
    // Also accepts: doc.render({text=..., from=..., to=..., ...pageOpts})
    doc.set(
        "render",
        lua.create_function(|lua, args: MultiValue| {
            if args.is_empty() {
                return Err(arg_error("doc.render", DOC_RENDER_PARAMS));
            }
            let first = args[0].clone();
            let from_opt = args.get(1).and_then(|v| match v {
                mlua::Value::String(s) => Some(s.to_string_lossy().to_string()),
                _ => None,
            });
            let to_opt = args.get(2).and_then(|v| match v {
                mlua::Value::String(s) => Some(s.to_string_lossy().to_string()),
                _ => None,
            });
            let opts_opt = args.get(3).and_then(|v| match v {
                mlua::Value::Table(t) => Some(t.clone()),
                _ => None,
            });
            let (text, from, to, opts): (String, String, String, Option<mlua::Table>) = match first
            {
                mlua::Value::Table(ref t) => {
                    let text = t
                        .get::<String>("text")
                        .or_else(|_| t.get::<String>(1))
                        .map_err(|_| {
                            mlua::Error::external("doc.render: table must have 'text' or [1]")
                        })?;
                    let from = t
                        .get::<String>("from")
                        .or_else(|_| t.get::<String>(2))
                        .map_err(|_| {
                            mlua::Error::external("doc.render: table must have 'from' or [2]")
                        })?;
                    let to = t
                        .get::<String>("to")
                        .or_else(|_| t.get::<String>(3))
                        .map_err(|_| {
                            mlua::Error::external("doc.render: table must have 'to' or [3]")
                        })?;
                    (text, from, to, Some(t.clone()))
                }
                mlua::Value::String(ref s) => {
                    let text = s.to_string_lossy().to_string();
                    let from = from_opt.ok_or_else(|| {
                        mlua::Error::external("doc.render: missing 'from' format")
                    })?;
                    let to = to_opt
                        .ok_or_else(|| mlua::Error::external("doc.render: missing 'to' format"))?;
                    (text, from, to, opts_opt)
                }
                _ => {
                    return Err(mlua::Error::external(
                        "doc.render: first arg must be a string or table",
                    ))
                }
            };

            if is_binary_conversion(&to) {
                let page = parse_page_options(opts.as_ref());
                let bytes = render_document_bytes(&text, &from, &to, &page)
                    .map_err(mlua::Error::external)?;
                lua.create_string(bytes)
            } else {
                let s = render_document(&text, &from, &to).map_err(mlua::Error::external)?;
                lua.create_string(s)
            }
        })?,
    )?;

    // doc.renderFile(input, output, opts?) -> nil
    // Also accepts: doc.renderFile({source=..., target=..., ...opts})
    {
        let m = mounts.clone();
        doc.set(
            "renderFile",
            lua.create_function(move |_, args: MultiValue| {
                if args.is_empty() {
                    return Err(arg_error("doc.renderFile", DOC_RENDER_FILE_PARAMS));
                }
                let first = args[0].clone();
                let output_opt = args.get(1).and_then(|v| match v {
                    mlua::Value::String(s) => Some(s.to_string_lossy().to_string()),
                    _ => None,
                });
                let opts_opt = args.get(2).and_then(|v| match v {
                    mlua::Value::Table(t) => Some(t.clone()),
                    _ => None,
                });
                // Accept both positional and named-param table forms:
                //   doc.renderFile("/in.md", "/out.pdf", {landscape=true})
                //   doc.renderFile({source="/in.md", target="/out.pdf", landscape=true})
                let (input, output, opts): (String, String, Option<mlua::Table>) = match first {
                    mlua::Value::Table(ref t) => {
                        let source = t
                            .get::<String>("source")
                            .or_else(|_| t.get::<String>(1))
                            .map_err(|_| {
                                mlua::Error::external(
                                    "doc.renderFile: table must have 'source' or [1]",
                                )
                            })?;
                        let target = t
                            .get::<String>("target")
                            .or_else(|_| t.get::<String>(2))
                            .map_err(|_| {
                                mlua::Error::external(
                                    "doc.renderFile: table must have 'target' or [2]",
                                )
                            })?;
                        (source, target, Some(t.clone()))
                    }
                    mlua::Value::String(ref s) => {
                        let input = s.to_string_lossy().to_string();
                        let output = output_opt.ok_or_else(|| {
                            mlua::Error::external("doc.renderFile: missing output path")
                        })?;
                        (input, output, opts_opt)
                    }
                    _ => {
                        return Err(mlua::Error::external(
                            "doc.renderFile: first arg must be a string or table",
                        ))
                    }
                };

                let from_override = opts.as_ref().and_then(|t| t.get::<String>("from").ok());
                let to_override = opts.as_ref().and_then(|t| t.get::<String>("to").ok());
                let sheet = opts
                    .as_ref()
                    .and_then(|t| t.get::<i32>("sheet").ok())
                    .map(|n| n as usize);
                let read_opts = ReadOptions { sheet, mode: None };
                let page_opts = parse_page_options(opts.as_ref());

                // Infer formats from extensions (or use overrides)
                let from_ext = from_override.unwrap_or_else(|| {
                    std::path::Path::new(&input)
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("")
                        .to_string()
                });
                let to_ext = to_override.unwrap_or_else(|| {
                    std::path::Path::new(&output)
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("")
                        .to_string()
                });

                if from_ext.is_empty() {
                    return Err(mlua::Error::external(format!(
                        "cannot infer input format from '{}' — pass {{from=\"...\"}}",
                        input
                    )));
                }
                if to_ext.is_empty() {
                    return Err(mlua::Error::external(format!(
                        "cannot infer output format from '{}' — pass {{to=\"...\"}}",
                        output
                    )));
                }

                // Resolve paths through mount table
                let host_input = m.resolve_read(&input).map_err(mlua::Error::external)?;
                let host_output = m.resolve_write(&output).map_err(mlua::Error::external)?;

                // Read input
                let data = std::fs::read(&host_input).map_err(mlua::Error::external)?;

                // Convert
                let result = convert_file(&data, &from_ext, &to_ext, &read_opts, &page_opts)
                    .map_err(mlua::Error::external)?;

                // Write output
                std::fs::write(&host_output, result).map_err(mlua::Error::external)?;

                Ok(())
            })?,
        )?;
    }

    Ok(())
}
