//! Document reading module — extracts plain text from various file formats.
//!
//! Internal module, not exposed to Lua directly. The `doc` module (doc.rs)
//! wraps these functions with the Lua API.

use std::io::{Cursor, Read};
use std::path::Path;

/// Read mode: structural (local parsing) or vision (multimodal callback).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ReadMode {
    /// Deterministic local parsing — PDFium for PDFs, calamine for spreadsheets, etc.
    Structural,
    /// Multimodal analysis via vision callback (e.g. Gemini).
    Vision,
}

/// Options for reading a document.
pub struct ReadOptions {
    /// For spreadsheets: which sheet to read (1-indexed). None = all sheets.
    pub sheet: Option<usize>,
    /// Explicit read mode override. None = use format default.
    pub mode: Option<ReadMode>,
}

impl Default for ReadOptions {
    fn default() -> Self {
        Self {
            sheet: None,
            mode: None,
        }
    }
}

/// Page geometry for PDF output.
#[derive(Debug, Clone)]
pub struct PageOptions {
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

impl Default for PageOptions {
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

impl PageOptions {
    fn to_pdf_options(&self) -> native_webview_pdf::PdfOptions {
        native_webview_pdf::PdfOptions {
            page_width: self.page_width,
            page_height: self.page_height,
            margin_top: self.margin_top,
            margin_bottom: self.margin_bottom,
            margin_left: self.margin_left,
            margin_right: self.margin_right,
            landscape: self.landscape,
        }
    }
}

/// Supported document formats (detected from file extension).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DocFormat {
    Xlsx,
    Xls,
    Xlsm,
    Ods,
    Docx,
    Pdf,
    Rtf,
    Pptx,
    Csv,
    Txt,
    Json,
    Md,
    Html,
    // Image formats — detected for routing to vision callback, but not locally extractable
    Png,
    Jpg,
    Webp,
    Gif,
}

impl DocFormat {
    /// Detect format from a bare extension string (case-insensitive).
    pub fn from_ext(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "xlsx" => Some(Self::Xlsx),
            "xls" => Some(Self::Xls),
            "xlsm" => Some(Self::Xlsm),
            "ods" => Some(Self::Ods),
            "docx" => Some(Self::Docx),
            "pdf" => Some(Self::Pdf),
            "rtf" => Some(Self::Rtf),
            "pptx" => Some(Self::Pptx),
            "csv" => Some(Self::Csv),
            "txt" | "text" | "log" => Some(Self::Txt),
            "json" => Some(Self::Json),
            "md" | "markdown" => Some(Self::Md),
            "html" | "htm" => Some(Self::Html),
            "png" => Some(Self::Png),
            "jpg" | "jpeg" => Some(Self::Jpg),
            "webp" => Some(Self::Webp),
            "gif" => Some(Self::Gif),
            _ => None,
        }
    }

    /// Returns true for image formats that have no local extraction fallback.
    pub fn is_image(&self) -> bool {
        matches!(self, Self::Png | Self::Jpg | Self::Webp | Self::Gif)
    }

    /// Returns true for formats that should be routed through the vision callback
    /// (images and PDFs). Other formats are handled locally.
    pub fn needs_callback(&self) -> bool {
        self.is_image() || matches!(self, Self::Pdf)
    }

    /// Resolve the read mode for this format.
    ///
    /// Priority: explicit user override > format default.
    /// Format defaults: images/PDFs default to vision when a callback is present,
    /// everything else defaults to structural.
    pub fn resolve_mode(&self, explicit: Option<ReadMode>, has_vision: bool) -> ReadMode {
        if let Some(mode) = explicit {
            return mode;
        }
        // Default: vision for images/PDFs when callback is available
        if has_vision && self.needs_callback() {
            ReadMode::Vision
        } else {
            ReadMode::Structural
        }
    }

    /// Detect format from file extension in a path (case-insensitive).
    pub fn from_extension(path: &str) -> Option<Self> {
        let ext = Path::new(path).extension().and_then(|e| e.to_str())?;
        Self::from_ext(ext)
    }
}

/// Read a document file and extract its text content.
///
/// `data` is the raw file bytes. `format` determines the extraction method.
pub fn read_document(data: &[u8], format: DocFormat, opts: &ReadOptions) -> Result<String, String> {
    match format {
        DocFormat::Xlsx | DocFormat::Xls | DocFormat::Xlsm | DocFormat::Ods => {
            read_spreadsheet(data, opts)
        }
        DocFormat::Docx => read_docx(data),
        DocFormat::Pdf => read_pdf(data),
        DocFormat::Rtf => read_rtf(data),
        DocFormat::Pptx => read_pptx(data),
        DocFormat::Html => {
            let html =
                String::from_utf8(data.to_vec()).map_err(|e| format!("invalid UTF-8: {}", e))?;
            Ok(html_to_text(&html))
        }
        DocFormat::Csv | DocFormat::Txt | DocFormat::Json | DocFormat::Md => {
            // Plain text formats — just decode as UTF-8
            String::from_utf8(data.to_vec()).map_err(|e| format!("invalid UTF-8: {}", e))
        }
        DocFormat::Png | DocFormat::Jpg | DocFormat::Webp | DocFormat::Gif => {
            Err("unsupported format for local extraction — requires vision callback".to_string())
        }
    }
}

// ── Spreadsheet (XLSX/XLS/ODS) via calamine ────────────────────

fn read_spreadsheet(data: &[u8], opts: &ReadOptions) -> Result<String, String> {
    use calamine::{open_workbook_auto_from_rs, Reader};

    let cursor = Cursor::new(data);
    let mut workbook = open_workbook_auto_from_rs(cursor)
        .map_err(|e| format!("cannot open spreadsheet: {}", e))?;

    let sheets = workbook.sheet_names().to_owned();
    if sheets.is_empty() {
        return Ok(String::new());
    }

    let mut output = String::new();

    if let Some(sheet_num) = opts.sheet {
        // Read specific sheet (1-indexed)
        let idx = sheet_num
            .checked_sub(1)
            .ok_or("sheet number must be >= 1")?;
        let name = sheets.get(idx).ok_or(format!(
            "sheet {} does not exist (workbook has {} sheets)",
            sheet_num,
            sheets.len()
        ))?;
        let range = workbook
            .worksheet_range(name)
            .map_err(|e| format!("cannot read sheet '{}': {}", name, e))?;
        format_sheet_range(&mut output, name, &range);
    } else {
        // Read all sheets
        for name in &sheets {
            if let Ok(range) = workbook.worksheet_range(name) {
                if !output.is_empty() {
                    output.push('\n');
                }
                format_sheet_range(&mut output, name, &range);
            }
        }
    }

    Ok(output)
}

fn format_sheet_range(output: &mut String, name: &str, range: &calamine::Range<calamine::Data>) {
    use calamine::Data;

    output.push_str(&format!("--- {} ---\n", name));
    for row in range.rows() {
        let cells: Vec<String> = row
            .iter()
            .map(|cell| match cell {
                Data::Empty => String::new(),
                Data::String(s) => s.clone(),
                Data::Float(f) => {
                    if *f == (*f as i64) as f64 {
                        format!("{}", *f as i64)
                    } else {
                        f.to_string()
                    }
                }
                Data::Int(i) => i.to_string(),
                Data::Bool(b) => b.to_string(),
                Data::Error(e) => format!("#{:?}", e),
                Data::DateTime(dt) => format!("{}", dt),
                Data::DateTimeIso(s) => s.clone(),
                Data::DurationIso(s) => s.clone(),
            })
            .collect();
        output.push_str(&cells.join("\t"));
        output.push('\n');
    }
}

// ── DOCX (zip + XML) ───────────────────────────────────────────

fn read_docx(data: &[u8]) -> Result<String, String> {
    let cursor = Cursor::new(data);
    let mut archive =
        zip::ZipArchive::new(cursor).map_err(|e| format!("cannot open docx: {}", e))?;

    let mut xml = String::new();
    {
        let mut file = archive
            .by_name("word/document.xml")
            .map_err(|e| format!("cannot read word/document.xml: {}", e))?;
        file.read_to_string(&mut xml)
            .map_err(|e| format!("cannot read document XML: {}", e))?;
    }

    extract_text_from_xml(&xml, "w:t")
}

// ── PDF via PDFium (preferred) or pdf-extract (fallback) ────────

/// Extract text from a PDF using PDFium.
///
/// Iterates all pages, extracts full text from each, and concatenates
/// with newline separators. No `catch_unwind` needed — PDFium returns
/// proper errors instead of panicking.
#[cfg(feature = "pdfium-render")]
pub fn read_pdf_pdfium(
    engine: &crate::pdfium_engine::PdfiumEngine,
    data: &[u8],
) -> Result<String, String> {
    let doc = engine
        .pdfium()
        .load_pdf_from_byte_slice(data, None)
        .map_err(|e| format!("cannot load PDF: {}", e))?;

    let mut output = String::new();
    let pages = doc.pages();
    let page_count = pages.len();

    for i in 0..page_count {
        let page = pages
            .get(i)
            .map_err(|e| format!("cannot read page {}: {}", i, e))?;
        let text_obj = page
            .text()
            .map_err(|e| format!("cannot extract text from page {}: {}", i, e))?;
        let text = text_obj.all();
        if !text.is_empty() {
            if !output.is_empty() {
                output.push('\n');
            }
            output.push_str(&text);
        }
    }

    Ok(output)
}

/// PDF page size info (width and height in points).
#[cfg(feature = "pdfium-render")]
#[derive(Debug, Clone)]
pub struct PdfPageSize {
    pub width: f32,
    pub height: f32,
}

/// PDF document metadata returned by `pdf_info`.
#[cfg(feature = "pdfium-render")]
#[derive(Debug, Clone)]
pub struct PdfInfo {
    pub page_count: usize,
    pub page_sizes: Vec<PdfPageSize>,
    pub has_form: bool,
}

/// Get PDF metadata: page count, page sizes, and whether it has form fields.
#[cfg(feature = "pdfium-render")]
pub fn pdf_info(
    engine: &crate::pdfium_engine::PdfiumEngine,
    data: &[u8],
) -> Result<PdfInfo, String> {
    let doc = engine
        .pdfium()
        .load_pdf_from_byte_slice(data, None)
        .map_err(|e| format!("cannot load PDF: {}", e))?;

    let pages = doc.pages();
    let page_count = pages.len() as usize;
    let mut page_sizes = Vec::with_capacity(page_count);
    let mut has_form = false;

    for i in 0..pages.len() {
        let page = pages
            .get(i)
            .map_err(|e| format!("cannot read page {}: {}", i, e))?;
        page_sizes.push(PdfPageSize {
            width: page.width().value,
            height: page.height().value,
        });
        // Check for form fields via annotations
        if !has_form {
            let annotations = page.annotations();
            for j in 0..annotations.len() {
                if let Ok(annot) = annotations.get(j) {
                    if annot.as_form_field().is_some() {
                        has_form = true;
                        break;
                    }
                }
            }
        }
    }

    Ok(PdfInfo {
        page_count,
        page_sizes,
        has_form,
    })
}

/// Information about a single form field in a PDF.
#[cfg(feature = "pdfium-render")]
#[derive(Debug, Clone)]
pub struct FormFieldInfo {
    pub name: String,
    pub field_type: String,
    pub value: String,
    pub read_only: bool,
}

/// List all form fields in a PDF document.
///
/// Iterates all pages, inspects annotations for form fields, and returns
/// a list of `FormFieldInfo` structs with name, type, current value, and read-only status.
#[cfg(feature = "pdfium-render")]
pub fn pdf_form_fields(
    engine: &crate::pdfium_engine::PdfiumEngine,
    data: &[u8],
) -> Result<Vec<FormFieldInfo>, String> {
    use pdfium_render::prelude::*;

    let doc = engine
        .pdfium()
        .load_pdf_from_byte_slice(data, None)
        .map_err(|e| format!("cannot load PDF: {}", e))?;

    let mut fields = Vec::new();
    let pages = doc.pages();

    for i in 0..pages.len() {
        let page = pages
            .get(i)
            .map_err(|e| format!("cannot read page {}: {}", i, e))?;
        let annotations = page.annotations();
        for j in 0..annotations.len() {
            let annot = match annotations.get(j) {
                Ok(a) => a,
                Err(_) => continue,
            };
            let form_field = match annot.as_form_field() {
                Some(f) => f,
                None => continue,
            };
            let name = form_field.name().unwrap_or_default();
            let field_type = match form_field.field_type() {
                PdfFormFieldType::Text => "text",
                PdfFormFieldType::Checkbox => "checkbox",
                PdfFormFieldType::RadioButton => "radio",
                PdfFormFieldType::ComboBox => "combobox",
                PdfFormFieldType::ListBox => "listbox",
                PdfFormFieldType::Signature => "signature",
                PdfFormFieldType::PushButton => "pushbutton",
                PdfFormFieldType::Unknown => "unknown",
            };
            let value = match &form_field {
                PdfFormField::Text(t) => t.value().unwrap_or_default(),
                PdfFormField::Checkbox(c) => {
                    // pdfium-render's is_checked() only recognizes "Yes"
                    // as checked, but IRS/XFA forms use "1", "2", etc.
                    // Check the field value directly: anything other than
                    // "Off" (or absent) means checked.
                    let checked = match c.group_value() {
                        Some(ref v) if !v.is_empty() && v != "Off" => true,
                        _ => c.is_checked().unwrap_or(false),
                    };
                    if checked {
                        "true".to_string()
                    } else {
                        "false".to_string()
                    }
                }
                PdfFormField::RadioButton(r) => {
                    let checked = match r.group_value() {
                        Some(ref v) if !v.is_empty() && v != "Off" => true,
                        _ => r.is_checked().unwrap_or(false),
                    };
                    if checked {
                        "true".to_string()
                    } else {
                        "false".to_string()
                    }
                }
                PdfFormField::ComboBox(c) => c.value().unwrap_or_default(),
                PdfFormField::ListBox(l) => l.value().unwrap_or_default(),
                _ => String::new(),
            };
            let read_only = form_field.is_read_only();
            fields.push(FormFieldInfo {
                name,
                field_type: field_type.to_string(),
                value,
                read_only,
            });
        }
    }

    Ok(fields)
}

/// Fill form fields in a PDF document and return the modified bytes.
///
/// Uses PDFium's raw C form-fill API instead of pdfium-render's annotation API.
/// The annotation API (`FPDFAnnot_SetStringValue`) only sets the widget `/V` entry
/// without generating appearance streams. The form-fill API (focus → select all →
/// replace selection → kill focus) simulates interactive input, which triggers
/// PDFium's internal appearance generation. This ensures:
/// - PDF viewers display the filled values correctly
/// - `FPDFPage_Flatten` can bake the values into page content
///
/// `field_values` maps field names to their new values.
/// For text fields, the value is a string.
/// For checkboxes/radios, use "true"/"false".
/// If `flatten` is true, form fields are baked into the page content.
#[cfg(feature = "pdfium-render")]
pub fn pdf_fill_form(
    engine: &crate::pdfium_engine::PdfiumEngine,
    data: &[u8],
    field_values: &[(String, String)],
    flatten: bool,
) -> Result<Vec<u8>, String> {
    use pdfium_render::prelude::*;
    use std::os::raw::{c_int, c_ulong, c_void};
    use std::pin::Pin;
    use std::ptr::null_mut;

    // PDFium constants (from fpdf_formfill.h / fpdfview.h)
    const ANNOT_WIDGET: c_int = 20; // FPDF_ANNOT_WIDGET
    const FF_TEXTFIELD: u32 = 6; // FF_TEXTFIELD
    const FF_CHECKBOX: u32 = 2; // FF_CHECKBOX
    const FF_RADIOBUTTON: u32 = 3; // FPDF_FORMFIELD_RADIOBUTTON
    const FF_COMBOBOX: u32 = 4; // FPDF_FORMFIELD_COMBOBOX
    const FF_LISTBOX: u32 = 5; // FPDF_FORMFIELD_LISTBOX
    const FLAT_PRINT_FLAG: c_int = 1; // FLAT_PRINT
    const FLAG_CHOICE_EDIT: u32 = 0x40000; // FPDF_FORMFLAG_CHOICE_EDIT

    let bindings = engine.pdfium().bindings();

    // ── Load document ────────────────────────────────────────────────
    let doc = bindings.FPDF_LoadMemDocument(data, None);
    if doc.is_null() {
        return Err("cannot load PDF".to_string());
    }

    // ── Init form-fill environment ───────────────────────────────────
    let mut ffi = Box::pin(FPDF_FORMFILLINFO {
        version: 2,
        Release: None,
        FFI_Invalidate: None,
        FFI_OutputSelectedRect: None,
        FFI_SetCursor: None,
        FFI_SetTimer: None,
        FFI_KillTimer: None,
        FFI_GetLocalTime: None,
        FFI_OnChange: None,
        FFI_GetPage: None,
        FFI_GetCurrentPage: None,
        FFI_GetRotation: None,
        FFI_ExecuteNamedAction: None,
        FFI_SetTextFieldFocus: None,
        FFI_DoURIAction: None,
        FFI_DoGoToAction: None,
        m_pJsPlatform: null_mut(),
        xfa_disabled: 0,
        FFI_DisplayCaret: None,
        FFI_GetCurrentPageIndex: None,
        FFI_SetCurrentPage: None,
        FFI_GotoURL: None,
        FFI_GetPageViewRect: None,
        FFI_PageEvent: None,
        FFI_PopupMenu: None,
        FFI_OpenFile: None,
        FFI_EmailTo: None,
        FFI_UploadTo: None,
        FFI_GetPlatform: None,
        FFI_GetLanguage: None,
        FFI_DownloadFromURL: None,
        FFI_PostRequestURL: None,
        FFI_PutRequestURL: None,
        FFI_OnFocusChange: None,
        FFI_DoURIActionWithKeyboardModifier: None,
    });
    let form = bindings.FPDFDOC_InitFormFillEnvironment(doc, Pin::as_mut(&mut ffi).get_mut());
    if form.is_null() {
        bindings.FPDF_CloseDocument(doc);
        return Err("cannot init form fill environment".to_string());
    }

    // ── Helpers ──────────────────────────────────────────────────────

    // Get form field name from annotation handle
    let get_field_name = |annot: FPDF_ANNOTATION| -> String {
        let len = bindings.FPDFAnnot_GetFormFieldName(form, annot, null_mut(), 0);
        if len == 0 {
            return String::new();
        }
        let mut buf = vec![0u8; len as usize];
        bindings.FPDFAnnot_GetFormFieldName(form, annot, buf.as_mut_ptr() as _, len);
        // buf is UTF-16LE with null terminator
        let u16s: Vec<u16> = buf
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .take_while(|&c| c != 0)
            .collect();
        String::from_utf16_lossy(&u16s)
    };

    // Convert &str to null-terminated UTF-16LE for PDFium
    let to_utf16le = |s: &str| -> Vec<u16> {
        let mut v: Vec<u16> = s.encode_utf16().collect();
        v.push(0); // null terminator
        v
    };

    // ── Fill fields ──────────────────────────────────────────────────
    let value_map: std::collections::HashMap<&str, &str> = field_values
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    let mut matched: std::collections::HashSet<String> = std::collections::HashSet::new();

    let page_count = bindings.FPDF_GetPageCount(doc);
    for i in 0..page_count {
        let page = bindings.FPDF_LoadPage(doc, i);
        if page.is_null() {
            continue;
        }
        bindings.FORM_OnAfterLoadPage(page, form);

        let annot_count = bindings.FPDFPage_GetAnnotCount(page);
        for j in 0..annot_count {
            let annot = bindings.FPDFPage_GetAnnot(page, j as c_int);
            if annot.is_null() {
                continue;
            }

            // Only process widget annotations (form fields)
            if bindings.FPDFAnnot_GetSubtype(annot) != ANNOT_WIDGET {
                bindings.FPDFPage_CloseAnnot(annot);
                continue;
            }

            let name = get_field_name(annot);
            let new_value = match value_map.get(name.as_str()) {
                Some(v) => *v,
                None => {
                    bindings.FPDFPage_CloseAnnot(annot);
                    continue;
                }
            };
            matched.insert(name.clone());

            let field_type = bindings.FPDFAnnot_GetFormFieldType(form, annot) as u32;

            match field_type {
                FF_TEXTFIELD => {
                    // Check read-only via flags
                    let flags = bindings.FPDFAnnot_GetFormFieldFlags(form, annot) as u32;
                    if flags & 1 != 0 {
                        // FPDF_FORMFLAG_READONLY = 1
                        bindings.FPDFPage_CloseAnnot(annot);
                        bindings.FORM_OnBeforeClosePage(page, form);
                        bindings.FPDF_ClosePage(page);
                        bindings.FPDFDOC_ExitFormFillEnvironment(form);
                        bindings.FPDF_CloseDocument(doc);
                        return Err(format!("field '{}' is read-only", name));
                    }
                    // Focus → select all → replace → kill focus
                    // This triggers PDFium's internal appearance generation.
                    bindings.FORM_SetFocusedAnnot(form, annot);
                    bindings.FORM_SelectAllText(form, page);
                    let utf16 = to_utf16le(new_value);
                    bindings.FORM_ReplaceSelection(form, page, utf16.as_ptr() as FPDF_WIDESTRING);
                    bindings.FORM_ForceToKillFocus(form);
                    // Also set /V entry explicitly — the form-fill API generates
                    // appearance streams but may not write /V, which PDF.js needs.
                    bindings.FPDFAnnot_SetStringValue_str(annot, "V", new_value);
                }
                FF_CHECKBOX | FF_RADIOBUTTON => {
                    let want_checked =
                        new_value == "true" || new_value == "1" || new_value == "yes";
                    let is_checked = bindings.is_true(bindings.FPDFAnnot_IsChecked(form, annot));

                    // We MUST go through the form-fill API (not direct dict
                    // manipulation) because:
                    //
                    //  1. FPDFAnnot_SetStringValue_str writes CPDF_String but
                    //     the PDF spec requires CPDF_Name for /V and /AS on
                    //     checkbox/radio widgets.  PDFium's own renderer calls
                    //     GetNameFor("AS") which returns empty for String
                    //     objects → no appearance match → invisible checkbox.
                    //
                    //  2. The on-state name varies per widget (e.g. "1", "2",
                    //     "3" in IRS forms).  FPDFAnnot_GetFormFieldExportValue
                    //     returns 0 on hybrid XFA/AcroForm PDFs.  The form-fill
                    //     path reads the correct name from /AP/N internally.
                    //
                    //  3. FPDFAnnot_SetAP(…, null) removes the appearance
                    //     streams entirely — the /AP dictionary is gone and
                    //     viewers render nothing.
                    //
                    // Strategy: focus the annotation by handle (not coordinates)
                    // and send a space character via FORM_OnChar to toggle.
                    //
                    // FORM_OnKeyDown is a no-op for checkboxes — PDFium's
                    // CFFL_CheckBox::OnKeyDown just returns true without
                    // toggling.  The actual toggle happens in OnChar, which
                    // calls SetCheck(!is_checked) → CommitData → SaveData →
                    // CPDFSDK_Widget::SetCheck → CPDF_FormField::CheckControl.
                    // This writes proper CPDF_Name objects for /V and /AS
                    // using the on-state name from /AP/N, and preserves the
                    // appearance streams.
                    if want_checked != is_checked {
                        bindings.FORM_SetFocusedAnnot(form, annot);
                        bindings.FORM_OnChar(form, page, 0x20, 0); // space
                        bindings.FORM_ForceToKillFocus(form);
                    }
                }
                FF_COMBOBOX | FF_LISTBOX => {
                    let flags = bindings.FPDFAnnot_GetFormFieldFlags(form, annot) as u32;
                    if flags & 1 != 0 {
                        bindings.FPDFPage_CloseAnnot(annot);
                        bindings.FORM_OnBeforeClosePage(page, form);
                        bindings.FPDF_ClosePage(page);
                        bindings.FPDFDOC_ExitFormFillEnvironment(form);
                        bindings.FPDF_CloseDocument(doc);
                        return Err(format!("field '{}' is read-only", name));
                    }

                    // For editable combo boxes, use the text input approach
                    if field_type == FF_COMBOBOX && (flags & FLAG_CHOICE_EDIT) != 0 {
                        bindings.FORM_SetFocusedAnnot(form, annot);
                        bindings.FORM_SelectAllText(form, page);
                        let utf16 = to_utf16le(new_value);
                        bindings.FORM_ReplaceSelection(
                            form,
                            page,
                            utf16.as_ptr() as FPDF_WIDESTRING,
                        );
                        bindings.FORM_ForceToKillFocus(form);
                        bindings.FPDFAnnot_SetStringValue_str(annot, "V", new_value);
                    } else {
                        // Non-editable: find option by label and select by index
                        let opt_count = bindings.FPDFAnnot_GetOptionCount(form, annot);
                        let mut found_idx: Option<c_int> = None;
                        for oi in 0..opt_count {
                            let label_len =
                                bindings.FPDFAnnot_GetOptionLabel(form, annot, oi, null_mut(), 0);
                            if label_len > 0 {
                                let mut buf = vec![0u8; label_len as usize];
                                bindings.FPDFAnnot_GetOptionLabel(
                                    form,
                                    annot,
                                    oi,
                                    buf.as_mut_ptr() as _,
                                    label_len,
                                );
                                let u16s: Vec<u16> = buf
                                    .chunks_exact(2)
                                    .map(|c| u16::from_le_bytes([c[0], c[1]]))
                                    .take_while(|&c| c != 0)
                                    .collect();
                                let label = String::from_utf16_lossy(&u16s);
                                if label == new_value {
                                    found_idx = Some(oi);
                                    break;
                                }
                            }
                        }
                        match found_idx {
                            Some(idx) => {
                                bindings.FORM_SetFocusedAnnot(form, annot);
                                bindings.FORM_SetIndexSelected(form, page, idx, 1);
                                bindings.FORM_ForceToKillFocus(form);
                                bindings.FPDFAnnot_SetStringValue_str(annot, "V", new_value);
                            }
                            None => {
                                // Collect valid options for error message
                                let mut opts = Vec::new();
                                for oi in 0..opt_count {
                                    let label_len = bindings.FPDFAnnot_GetOptionLabel(
                                        form,
                                        annot,
                                        oi,
                                        null_mut(),
                                        0,
                                    );
                                    if label_len > 0 {
                                        let mut buf = vec![0u8; label_len as usize];
                                        bindings.FPDFAnnot_GetOptionLabel(
                                            form,
                                            annot,
                                            oi,
                                            buf.as_mut_ptr() as _,
                                            label_len,
                                        );
                                        let u16s: Vec<u16> = buf
                                            .chunks_exact(2)
                                            .map(|c| u16::from_le_bytes([c[0], c[1]]))
                                            .take_while(|&c| c != 0)
                                            .collect();
                                        opts.push(String::from_utf16_lossy(&u16s));
                                    }
                                }
                                bindings.FPDFPage_CloseAnnot(annot);
                                bindings.FORM_OnBeforeClosePage(page, form);
                                bindings.FPDF_ClosePage(page);
                                bindings.FPDFDOC_ExitFormFillEnvironment(form);
                                bindings.FPDF_CloseDocument(doc);
                                return Err(format!(
                                    "field '{}': value '{}' not found. Valid options: {:?}",
                                    name, new_value, opts
                                ));
                            }
                        }
                    }
                }
                _ => {
                    bindings.FPDFPage_CloseAnnot(annot);
                    bindings.FORM_OnBeforeClosePage(page, form);
                    bindings.FPDF_ClosePage(page);
                    bindings.FPDFDOC_ExitFormFillEnvironment(form);
                    bindings.FPDF_CloseDocument(doc);
                    return Err(format!(
                        "field '{}' has unsupported type ({})",
                        name, field_type
                    ));
                }
            }

            bindings.FPDFPage_CloseAnnot(annot);
        }

        // Flatten this page if requested (after all fields are set)
        if flatten {
            bindings.FPDFPage_Flatten(page, FLAT_PRINT_FLAG);
        }

        bindings.FORM_OnBeforeClosePage(page, form);
        bindings.FPDF_ClosePage(page);
    }

    // Check for non-existent fields (before cleanup)
    for (name, _) in field_values {
        if !matched.contains(name.as_str()) {
            bindings.FPDFDOC_ExitFormFillEnvironment(form);
            bindings.FPDF_CloseDocument(doc);
            return Err(format!("field '{}' does not exist in the PDF", name));
        }
    }

    // ── Save to bytes ────────────────────────────────────────────────
    // Construct an FPDF_FILEWRITE that appends to a Vec<u8>.
    struct WriterState {
        buf: Vec<u8>,
    }
    #[repr(C)]
    struct FileWriteWithState {
        base: FPDF_FILEWRITE,
        state: *mut WriterState,
    }
    unsafe extern "C" fn write_block(
        this: *mut FPDF_FILEWRITE,
        data: *const c_void,
        size: c_ulong,
    ) -> c_int {
        let ext = this as *mut FileWriteWithState;
        let state = &mut *(*ext).state;
        let slice = std::slice::from_raw_parts(data as *const u8, size as usize);
        state.buf.extend_from_slice(slice);
        1 // success
    }

    let mut writer_state = WriterState {
        buf: Vec::with_capacity(data.len()),
    };
    let mut file_write = FileWriteWithState {
        base: FPDF_FILEWRITE {
            version: 1,
            WriteBlock: Some(write_block),
        },
        state: &mut writer_state as *mut WriterState,
    };
    let save_ok = bindings.FPDF_SaveAsCopy(
        doc,
        &mut file_write.base as *mut FPDF_FILEWRITE,
        0, // no special flags
    );

    // ── Cleanup ──────────────────────────────────────────────────────
    bindings.FPDFDOC_ExitFormFillEnvironment(form);
    bindings.FPDF_CloseDocument(doc);

    if bindings.is_true(save_ok) {
        Ok(writer_state.buf)
    } else {
        Err("cannot save PDF".to_string())
    }
}

/// Merge multiple PDF documents into one.
///
/// `pdf_data_list` is a list of (filename, raw bytes) pairs.
/// Pages are concatenated in order via `pages().append()`.
/// Returns the merged PDF bytes.
#[cfg(feature = "pdfium-render")]
pub fn pdf_merge(
    engine: &crate::pdfium_engine::PdfiumEngine,
    pdf_data_list: &[(&str, &[u8])],
) -> Result<Vec<u8>, String> {
    if pdf_data_list.is_empty() {
        return Err("mergePdf: 'paths' must contain at least one PDF".to_string());
    }

    let pdfium = engine.pdfium();

    // Load the first document as the base
    let mut base = pdfium
        .load_pdf_from_byte_vec(pdf_data_list[0].1.to_vec(), None)
        .map_err(|e| format!("cannot load '{}': {}", pdf_data_list[0].0, e))?;

    // Append remaining documents
    for (name, data) in &pdf_data_list[1..] {
        let other = pdfium
            .load_pdf_from_byte_slice(data, None)
            .map_err(|e| format!("cannot load '{}': {}", name, e))?;
        base.pages_mut()
            .append(&other)
            .map_err(|e| format!("cannot append '{}': {}", name, e))?;
    }

    base.save_to_bytes()
        .map_err(|e| format!("cannot save merged PDF: {}", e))
}

/// Split a PDF into multiple parts by page ranges.
///
/// `ranges` is a list of page range strings like "1-3", "4", "5-7".
/// Pages are 1-indexed to match user expectations.
/// Returns a list of PDF byte vectors, one per range.
#[cfg(feature = "pdfium-render")]
pub fn pdf_split(
    engine: &crate::pdfium_engine::PdfiumEngine,
    data: &[u8],
    ranges: &[String],
) -> Result<Vec<Vec<u8>>, String> {
    if ranges.is_empty() {
        return Err("splitPdf: 'ranges' must contain at least one range".to_string());
    }

    let pdfium = engine.pdfium();
    let source = pdfium
        .load_pdf_from_byte_slice(data, None)
        .map_err(|e| format!("cannot load PDF: {}", e))?;
    let total_pages = source.pages().len();

    let mut results = Vec::with_capacity(ranges.len());

    for range_str in ranges {
        let mut dest = pdfium
            .create_new_pdf()
            .map_err(|e| format!("cannot create PDF: {}", e))?;

        // Parse range string and validate
        let page_indices = parse_page_range(range_str, total_pages)?;
        if page_indices.is_empty() {
            return Err(format!("splitPdf: range '{}' selects no pages", range_str));
        }

        // Copy pages one at a time to preserve order
        for (dest_idx, src_idx) in page_indices.iter().enumerate() {
            dest.pages_mut()
                .copy_page_from_document(&source, *src_idx, dest_idx as u16)
                .map_err(|e| format!("cannot copy page {}: {}", src_idx + 1, e))?;
        }

        let bytes = dest
            .save_to_bytes()
            .map_err(|e| format!("cannot save split PDF: {}", e))?;
        results.push(bytes);
    }

    Ok(results)
}

/// Edit pages in a PDF: delete, rotate, or reorder.
///
/// `operations` is a list of `PageOperation` to apply sequentially.
/// Returns the modified PDF bytes.
#[cfg(feature = "pdfium-render")]
#[derive(Debug, Clone)]
pub enum PageOperation {
    /// Delete pages (0-indexed).
    Delete(Vec<u16>),
    /// Rotate pages (0-indexed page, degrees: 0/90/180/270).
    Rotate(Vec<(u16, u16)>),
    /// Reorder: new ordering given as 0-indexed page indices.
    Reorder(Vec<u16>),
}

#[cfg(feature = "pdfium-render")]
pub fn pdf_edit_pages(
    engine: &crate::pdfium_engine::PdfiumEngine,
    data: &[u8],
    operations: &[PageOperation],
) -> Result<Vec<u8>, String> {
    use pdfium_render::prelude::PdfPageRenderRotation;

    let pdfium = engine.pdfium();
    let doc = pdfium
        .load_pdf_from_byte_vec(data.to_vec(), None)
        .map_err(|e| format!("cannot load PDF: {}", e))?;

    for op in operations {
        match op {
            PageOperation::Delete(indices) => {
                // Delete in reverse order to preserve indices
                let mut sorted = indices.clone();
                sorted.sort();
                sorted.dedup();
                let page_count = doc.pages().len();
                for &idx in sorted.iter().rev() {
                    if idx >= page_count {
                        return Err(format!(
                            "editPages: page {} does not exist (document has {} pages)",
                            idx + 1,
                            page_count
                        ));
                    }
                    let page = doc
                        .pages()
                        .get(idx)
                        .map_err(|e| format!("cannot access page {}: {}", idx + 1, e))?;
                    page.delete()
                        .map_err(|e| format!("cannot delete page {}: {}", idx + 1, e))?;
                }
            }
            PageOperation::Rotate(rotations) => {
                for &(idx, degrees) in rotations {
                    let page_count = doc.pages().len();
                    if idx >= page_count {
                        return Err(format!(
                            "editPages: page {} does not exist (document has {} pages)",
                            idx + 1,
                            page_count
                        ));
                    }
                    let rotation = match degrees {
                        0 => PdfPageRenderRotation::None,
                        90 => PdfPageRenderRotation::Degrees90,
                        180 => PdfPageRenderRotation::Degrees180,
                        270 => PdfPageRenderRotation::Degrees270,
                        _ => {
                            return Err(format!(
                                "editPages: invalid rotation {} (must be 0, 90, 180, or 270)",
                                degrees
                            ))
                        }
                    };
                    let mut page = doc
                        .pages()
                        .get(idx)
                        .map_err(|e| format!("cannot access page {}: {}", idx + 1, e))?;
                    page.set_rotation(rotation);
                }
            }
            PageOperation::Reorder(new_order) => {
                let page_count = doc.pages().len();
                if new_order.len() != page_count as usize {
                    return Err(format!(
                        "editPages: reorder list has {} entries but document has {} pages",
                        new_order.len(),
                        page_count
                    ));
                }
                // Validate all indices
                for &idx in new_order {
                    if idx >= page_count {
                        return Err(format!(
                            "editPages: reorder references page {} but document has {} pages",
                            idx + 1,
                            page_count
                        ));
                    }
                }
                // Create a new document with pages in the specified order
                let mut dest = pdfium
                    .create_new_pdf()
                    .map_err(|e| format!("cannot create PDF: {}", e))?;
                for (dest_idx, &src_idx) in new_order.iter().enumerate() {
                    dest.pages_mut()
                        .copy_page_from_document(&doc, src_idx, dest_idx as u16)
                        .map_err(|e| format!("cannot copy page {}: {}", src_idx + 1, e))?;
                }
                // We can't mutate `doc` in-place easily, so return the new document
                return dest
                    .save_to_bytes()
                    .map_err(|e| format!("cannot save reordered PDF: {}", e));
            }
        }
    }

    doc.save_to_bytes()
        .map_err(|e| format!("cannot save edited PDF: {}", e))
}

/// Public wrapper for `parse_page_range` so other modules (e.g. doc.rs) can use it.
#[cfg(feature = "pdfium-render")]
pub fn parse_page_range_public(range: &str, total_pages: u16) -> Result<Vec<u16>, String> {
    parse_page_range(range, total_pages)
}

/// Parse a page range string like "1-3" or "5" into 0-indexed page indices.
/// Pages in the range string are 1-indexed (user-facing).
#[cfg(feature = "pdfium-render")]
fn parse_page_range(range: &str, total_pages: u16) -> Result<Vec<u16>, String> {
    let range = range.trim();
    let mut indices = Vec::new();

    for part in range.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some((start_str, end_str)) = part.split_once('-') {
            let start: u16 = start_str
                .trim()
                .parse()
                .map_err(|_| format!("invalid page number: '{}'", start_str.trim()))?;
            let end: u16 = end_str
                .trim()
                .parse()
                .map_err(|_| format!("invalid page number: '{}'", end_str.trim()))?;
            if start == 0 || end == 0 {
                return Err("page numbers are 1-indexed".to_string());
            }
            if start > total_pages || end > total_pages {
                return Err(format!(
                    "page range {}-{} exceeds document page count ({})",
                    start, end, total_pages
                ));
            }
            if start > end {
                return Err(format!("invalid range: {} > {}", start, end));
            }
            for i in start..=end {
                indices.push(i - 1); // Convert to 0-indexed
            }
        } else {
            let page: u16 = part
                .parse()
                .map_err(|_| format!("invalid page number: '{}'", part))?;
            if page == 0 {
                return Err("page numbers are 1-indexed".to_string());
            }
            if page > total_pages {
                return Err(format!(
                    "page {} exceeds document page count ({})",
                    page, total_pages
                ));
            }
            indices.push(page - 1); // Convert to 0-indexed
        }
    }

    Ok(indices)
}

/// Annotation type supported by `doc.addAnnotation()`.
#[cfg(feature = "pdfium-render")]
#[derive(Debug, Clone)]
pub enum AnnotationType {
    /// Sticky note icon at a point.
    Text,
    /// Free-form text directly on the page.
    FreeText,
    /// Highlight markup over a rectangular area.
    Highlight,
    /// Underline markup over a rectangular area.
    Underline,
    /// Strikeout markup over a rectangular area.
    Strikeout,
    /// Rectangle outline.
    Square,
    /// Rubber stamp.
    Stamp,
}

/// Parameters for `pdf_add_annotation`.
#[cfg(feature = "pdfium-render")]
#[derive(Debug, Clone)]
pub struct AnnotationParams {
    /// 0-indexed page number.
    pub page: u16,
    pub annotation_type: AnnotationType,
    /// Position/area in PDF points (origin = bottom-left).
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    /// Optional fill color (r, g, b, a).
    pub color: Option<(u8, u8, u8, u8)>,
    /// Text contents (for text, freeText, stamp annotations; also used as popup text for markup annotations).
    pub contents: Option<String>,
}

/// Add an annotation to a PDF and return the modified bytes.
#[cfg(feature = "pdfium-render")]
pub fn pdf_add_annotation(
    engine: &crate::pdfium_engine::PdfiumEngine,
    data: &[u8],
    params: &AnnotationParams,
) -> Result<Vec<u8>, String> {
    use pdfium_render::prelude::*;

    let pdfium = engine.pdfium();
    let doc = pdfium
        .load_pdf_from_byte_vec(data.to_vec(), None)
        .map_err(|e| format!("cannot load PDF: {}", e))?;

    let page_count = doc.pages().len();
    if params.page >= page_count {
        return Err(format!(
            "addAnnotation: page {} does not exist (document has {} pages)",
            params.page + 1,
            page_count
        ));
    }

    let mut page = doc
        .pages()
        .get(params.page)
        .map_err(|e| format!("cannot access page {}: {}", params.page + 1, e))?;

    let bounds = PdfRect::new_from_values(
        params.y,                 // bottom
        params.x,                 // left
        params.y + params.height, // top
        params.x + params.width,  // right
    );

    let color = params.color.map(|(r, g, b, a)| PdfColor::new(r, g, b, a));

    let annotations = page.annotations_mut();

    match params.annotation_type {
        AnnotationType::Text => {
            let contents = params.contents.as_deref().unwrap_or("");
            let mut annot = annotations
                .create_text_annotation(contents)
                .map_err(|e| format!("cannot create text annotation: {}", e))?;
            annot
                .set_bounds(bounds)
                .map_err(|e| format!("cannot set annotation bounds: {}", e))?;
            if let Some(c) = color {
                let _ = annot.set_fill_color(c);
            }
        }
        AnnotationType::FreeText => {
            let contents = params.contents.as_deref().unwrap_or("");
            let mut annot = annotations
                .create_free_text_annotation(contents)
                .map_err(|e| format!("cannot create freeText annotation: {}", e))?;
            annot
                .set_bounds(bounds)
                .map_err(|e| format!("cannot set annotation bounds: {}", e))?;
            if let Some(c) = color {
                let _ = annot.set_fill_color(c);
            }
        }
        AnnotationType::Highlight => {
            let mut annot = annotations
                .create_highlight_annotation()
                .map_err(|e| format!("cannot create highlight annotation: {}", e))?;
            annot
                .set_bounds(bounds)
                .map_err(|e| format!("cannot set annotation bounds: {}", e))?;
            annot
                .attachment_points_mut()
                .create_attachment_point_at_end(PdfQuadPoints::from_rect(&bounds))
                .map_err(|e| format!("cannot set highlight attachment points: {}", e))?;
            if let Some(c) = color {
                let _ = annot.set_fill_color(c);
            }
            if let Some(ref text) = params.contents {
                let _ = annot.set_contents(text);
            }
        }
        AnnotationType::Underline => {
            let mut annot = annotations
                .create_underline_annotation()
                .map_err(|e| format!("cannot create underline annotation: {}", e))?;
            annot
                .set_bounds(bounds)
                .map_err(|e| format!("cannot set annotation bounds: {}", e))?;
            annot
                .attachment_points_mut()
                .create_attachment_point_at_end(PdfQuadPoints::from_rect(&bounds))
                .map_err(|e| format!("cannot set underline attachment points: {}", e))?;
            if let Some(c) = color {
                let _ = annot.set_stroke_color(c);
            }
            if let Some(ref text) = params.contents {
                let _ = annot.set_contents(text);
            }
        }
        AnnotationType::Strikeout => {
            let mut annot = annotations
                .create_strikeout_annotation()
                .map_err(|e| format!("cannot create strikeout annotation: {}", e))?;
            annot
                .set_bounds(bounds)
                .map_err(|e| format!("cannot set annotation bounds: {}", e))?;
            annot
                .attachment_points_mut()
                .create_attachment_point_at_end(PdfQuadPoints::from_rect(&bounds))
                .map_err(|e| format!("cannot set strikeout attachment points: {}", e))?;
            if let Some(c) = color {
                let _ = annot.set_stroke_color(c);
            }
            if let Some(ref text) = params.contents {
                let _ = annot.set_contents(text);
            }
        }
        AnnotationType::Square => {
            let mut annot = annotations
                .create_square_annotation()
                .map_err(|e| format!("cannot create square annotation: {}", e))?;
            annot
                .set_bounds(bounds)
                .map_err(|e| format!("cannot set annotation bounds: {}", e))?;
            if let Some(c) = color {
                let _ = annot.set_stroke_color(c);
            }
            if let Some(ref text) = params.contents {
                let _ = annot.set_contents(text);
            }
        }
        AnnotationType::Stamp => {
            let mut annot = annotations
                .create_stamp_annotation()
                .map_err(|e| format!("cannot create stamp annotation: {}", e))?;
            annot
                .set_bounds(bounds)
                .map_err(|e| format!("cannot set annotation bounds: {}", e))?;
            if let Some(c) = color {
                let _ = annot.set_fill_color(c);
            }
            if let Some(ref text) = params.contents {
                let _ = annot.set_contents(text);
            }
        }
    }

    doc.save_to_bytes()
        .map_err(|e| format!("cannot save annotated PDF: {}", e))
}

/// Parameters for `pdf_watermark`.
#[cfg(feature = "pdfium-render")]
#[derive(Debug, Clone)]
pub struct WatermarkParams {
    pub text: String,
    pub font_size: f32,
    /// RGBA color.
    pub color: (u8, u8, u8, u8),
    /// Rotation in degrees (counter-clockwise).
    pub rotation: f32,
    /// Optional page filter. None = all pages.
    /// Pages are 0-indexed internally.
    pub pages: Option<Vec<u16>>,
}

/// Add a text watermark to pages of a PDF and return the modified bytes.
#[cfg(feature = "pdfium-render")]
pub fn pdf_watermark(
    engine: &crate::pdfium_engine::PdfiumEngine,
    data: &[u8],
    params: &WatermarkParams,
) -> Result<Vec<u8>, String> {
    use pdfium_render::prelude::*;

    let pdfium = engine.pdfium();
    let mut doc = pdfium
        .load_pdf_from_byte_vec(data.to_vec(), None)
        .map_err(|e| format!("cannot load PDF: {}", e))?;

    let page_count = doc.pages().len();

    // Validate page indices if specified
    if let Some(ref pages) = params.pages {
        for &p in pages {
            if p >= page_count {
                return Err(format!(
                    "watermark: page {} does not exist (document has {} pages)",
                    p + 1,
                    page_count
                ));
            }
        }
    }

    let font = doc.fonts_mut().helvetica();
    let font_size = PdfPoints::new(params.font_size);
    let color = PdfColor::new(
        params.color.0,
        params.color.1,
        params.color.2,
        params.color.3,
    );

    doc.pages()
        .watermark(|group, index, width, height| {
            // Skip pages not in the filter
            if let Some(ref pages) = params.pages {
                if !pages.contains(&index) {
                    return Ok(());
                }
            }

            let mut text_obj = PdfPageTextObject::new(&doc, &params.text, font, font_size)?;

            text_obj.set_fill_color(color)?;

            // Center the text on the page, then apply rotation
            let text_width = text_obj.width()?;
            let text_height = text_obj.height()?;
            let center_x = width / 2.0;
            let center_y = height / 2.0;

            // Move to center
            text_obj.translate(center_x - text_width / 2.0, center_y - text_height / 2.0)?;

            // Apply rotation around center if requested
            if params.rotation != 0.0 {
                // Translate to origin, rotate, translate back
                let matrix = PdfMatrix::identity()
                    .translate(
                        PdfPoints::new(-center_x.value),
                        PdfPoints::new(-center_y.value),
                    )?
                    .rotate_counter_clockwise_degrees(params.rotation as PdfMatrixValue)?
                    .translate(center_x, center_y)?;
                // Reset to identity then apply the combined transform
                text_obj.reset_matrix_to_identity()?;
                text_obj.apply_matrix(matrix)?;
            }

            group.push(&mut text_obj.into())
        })
        .map_err(|e| format!("cannot apply watermark: {}", e))?;

    doc.save_to_bytes()
        .map_err(|e| format!("cannot save watermarked PDF: {}", e))
}

/// Fallback PDF reader when PDFium is not available.
/// Returns an error indicating PDFium is required for PDF extraction.
fn read_pdf(_data: &[u8]) -> Result<String, String> {
    Err("PDF text extraction requires PDFium (enable pdfium-render feature and provide PDFium binary)".to_string())
}

// ── RTF via rtf-parser ──────────────────────────────────────────

fn read_rtf(data: &[u8]) -> Result<String, String> {
    let text =
        String::from_utf8(data.to_vec()).map_err(|e| format!("invalid RTF encoding: {}", e))?;
    let doc = rtf_parser::parse_rtf(text);
    Ok(doc.get_text())
}

// ── PPTX (zip + XML) ───────────────────────────────────────────

fn read_pptx(data: &[u8]) -> Result<String, String> {
    let cursor = Cursor::new(data);
    let mut archive =
        zip::ZipArchive::new(cursor).map_err(|e| format!("cannot open pptx: {}", e))?;

    let mut output = String::new();
    let mut slide_names: Vec<String> = Vec::new();

    // Collect slide file names
    for i in 0..archive.len() {
        if let Ok(entry) = archive.by_index(i) {
            let name = entry.name().to_string();
            if name.starts_with("ppt/slides/slide") && name.ends_with(".xml") {
                slide_names.push(name);
            }
        }
    }

    // Sort numerically (slide1.xml, slide2.xml, ...)
    slide_names.sort_by(|a, b| {
        let num_a = extract_slide_number(a);
        let num_b = extract_slide_number(b);
        num_a.cmp(&num_b)
    });

    for name in &slide_names {
        let mut xml = String::new();
        {
            let mut file = archive
                .by_name(name)
                .map_err(|e| format!("cannot read {}: {}", name, e))?;
            file.read_to_string(&mut xml)
                .map_err(|e| format!("cannot read slide XML: {}", e))?;
        }

        let slide_text = extract_text_from_xml(&xml, "a:t")?;
        if !slide_text.is_empty() {
            if !output.is_empty() {
                output.push('\n');
            }
            output.push_str(&slide_text);
        }
    }

    Ok(output)
}

fn extract_slide_number(name: &str) -> usize {
    // ppt/slides/slide12.xml → 12
    let after_slide = name
        .rsplit('/')
        .next()
        .unwrap_or("")
        .strip_prefix("slide")
        .unwrap_or("");
    after_slide
        .strip_suffix(".xml")
        .unwrap_or("")
        .parse()
        .unwrap_or(0)
}

// ── Shared XML text extraction ──────────────────────────────────

/// Extract text content from XML nodes with the given tag name.
/// Uses quick-xml for efficient streaming parse.
fn extract_text_from_xml(xml: &str, tag_name: &str) -> Result<String, String> {
    use quick_xml::events::Event;
    use quick_xml::reader::Reader;

    let mut reader = Reader::from_str(xml);
    let mut output = String::new();
    let mut in_target_tag = false;
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let local = e.local_name();
                if local.as_ref() == tag_name.split(':').last().unwrap_or(tag_name).as_bytes() {
                    // Check namespace prefix matches if specified
                    if tag_name.contains(':') {
                        let prefix = tag_name.split(':').next().unwrap();
                        if let Some(ns_prefix) = e.name().prefix() {
                            if ns_prefix.as_ref() == prefix.as_bytes() {
                                in_target_tag = true;
                            }
                        }
                    } else {
                        in_target_tag = true;
                    }
                }
            }
            Ok(Event::Text(ref e)) if in_target_tag => {
                let text = e.decode().map_err(|e| format!("XML decode error: {}", e))?;
                output.push_str(&text);
            }
            Ok(Event::End(_)) => {
                in_target_tag = false;
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("XML parse error: {}", e)),
            _ => {}
        }
        buf.clear();
    }

    Ok(output)
}

// ── File-to-file conversion ──────────────────────────────────────

/// Map a file extension to a render format name used by render_document.
fn render_format_from_ext(ext: &str) -> Option<&'static str> {
    match ext {
        "md" | "markdown" => Some("markdown"),
        "html" | "htm" => Some("html"),
        "txt" | "text" | "log" => Some("text"),
        "pdf" => Some("pdf"),
        _ => None,
    }
}

/// Convert file data from one format to another, returning the output bytes.
/// Formats are identified by file extension strings (case-insensitive).
///
/// Supported conversions:
/// - Render: md→html, md→pdf, html→pdf, html→txt
/// - Extract: xlsx/xls/xlsm/ods/docx/pdf/rtf/pptx → txt
pub fn convert_file(
    data: &[u8],
    from_ext: &str,
    to_ext: &str,
    opts: &ReadOptions,
    page: &PageOptions,
) -> Result<Vec<u8>, String> {
    let from_lower = from_ext.to_lowercase();
    let to_lower = to_ext.to_lowercase();
    let input_format = DocFormat::from_ext(&from_lower);

    // Binary/structured input formats need extraction first
    let is_binary_input = matches!(
        input_format,
        Some(
            DocFormat::Xlsx
                | DocFormat::Xls
                | DocFormat::Xlsm
                | DocFormat::Ods
                | DocFormat::Docx
                | DocFormat::Pdf
                | DocFormat::Rtf
                | DocFormat::Pptx
        )
    );

    if is_binary_input {
        let fmt = input_format.unwrap();
        let text = read_document(data, fmt, opts)?;
        if matches!(to_lower.as_str(), "txt" | "text" | "log") {
            return Ok(text.into_bytes());
        }
        return Err(format!(
            "unsupported conversion: .{} → .{} (binary formats can only extract to .txt)",
            from_ext, to_ext
        ));
    }

    // Text input — try render pipeline (md→html, html→pdf, etc.)
    let from_render = render_format_from_ext(&from_lower);
    let to_render = render_format_from_ext(&to_lower);

    if let (Some(from), Some(to)) = (from_render, to_render) {
        let text =
            String::from_utf8(data.to_vec()).map_err(|e| format!("invalid UTF-8 input: {}", e))?;
        if is_binary_conversion(to) {
            return render_document_bytes(&text, from, to, page);
        }
        return render_document(&text, from, to).map(|s| s.into_bytes());
    }

    // Text passthrough: csv/json/txt → txt
    if matches!(to_lower.as_str(), "txt" | "text" | "log") {
        if let Some(format) = input_format {
            let text = read_document(data, format, opts)?;
            return Ok(text.into_bytes());
        }
    }

    Err(format!(
        "unsupported conversion: .{} → .{}",
        from_ext, to_ext
    ))
}

// ── Format conversion ────────────────────────────────────────────

/// Convert text between formats (text output).
pub fn render_document(text: &str, from: &str, to: &str) -> Result<String, String> {
    match (from, to) {
        ("markdown", "html") | ("md", "html") => Ok(markdown_to_html(text)),
        ("html", "text") | ("html", "txt") => Ok(html_to_text(text)),
        _ => Err(format!(
            "unsupported conversion: {} → {} (supported: markdown→html, html→text, markdown→pdf, html→pdf)",
            from, to
        )),
    }
}

/// Returns true if the given conversion produces binary output (e.g., PDF).
pub fn is_binary_conversion(to: &str) -> bool {
    to == "pdf"
}

/// Convert text to binary format (PDF).
pub fn render_document_bytes(
    text: &str,
    from: &str,
    to: &str,
    page: &PageOptions,
) -> Result<Vec<u8>, String> {
    let pdf_opts = page.to_pdf_options();
    match (from, to) {
        ("html", "pdf") => {
            native_webview_pdf::html_to_pdf(text, &pdf_opts).map_err(|e| format!("HTML→PDF: {}", e))
        }
        ("markdown", "pdf") | ("md", "pdf") => {
            let html = markdown_to_html(text);
            native_webview_pdf::html_to_pdf(&html, &pdf_opts)
                .map_err(|e| format!("markdown→PDF: {}", e))
        }
        _ => Err(format!(
            "unsupported conversion: {} → {} (binary)",
            from, to
        )),
    }
}

// ── Markdown → HTML via pulldown-cmark ──────────────────────────

fn markdown_to_html(md: &str) -> String {
    use pulldown_cmark::{Options, Parser};

    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);

    let parser = Parser::new_ext(md, options);
    let mut html_output = String::new();
    pulldown_cmark::html::push_html(&mut html_output, parser);
    html_output
}

// ── HTML → plain text ───────────────────────────────────────────

/// Extract plain text from HTML by stripping tags.
/// Lightweight approach: parse tags with quick-xml, collect text nodes.
fn html_to_text(html: &str) -> String {
    // Wrap in a root element to make it valid XML for the parser.
    // Handle common HTML entities by pre-processing.
    let wrapped = format!("<root>{}</root>", html);

    use quick_xml::events::Event;
    use quick_xml::reader::Reader;

    let mut reader = Reader::from_str(&wrapped);
    reader.config_mut().check_end_names = false;
    let mut output = String::new();
    let mut buf = Vec::new();
    let mut skip_depth = 0u32; // depth inside <script>/<style>

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let tag = e.local_name();
                let tag_str = std::str::from_utf8(tag.as_ref()).unwrap_or("");
                match tag_str {
                    "script" | "style" => skip_depth += 1,
                    "br" | "p" | "div" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "li" | "tr"
                    | "blockquote" => {
                        if !output.is_empty() && !output.ends_with('\n') {
                            output.push('\n');
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) => {
                let tag = e.local_name();
                let tag_str = std::str::from_utf8(tag.as_ref()).unwrap_or("");
                if tag_str == "script" || tag_str == "style" {
                    skip_depth = skip_depth.saturating_sub(1);
                }
            }
            Ok(Event::Text(ref e)) if skip_depth == 0 => {
                if let Ok(text) = e.decode() {
                    output.push_str(&text);
                }
            }
            Ok(Event::Empty(ref e)) => {
                let tag = e.local_name();
                let tag_str = std::str::from_utf8(tag.as_ref()).unwrap_or("");
                if tag_str == "br" {
                    output.push('\n');
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => {
                // If XML parsing fails, fall back to simple regex-free tag stripping
                return strip_tags_simple(html);
            }
            _ => {}
        }
        buf.clear();
    }

    output.trim().to_string()
}

/// Simple fallback: strip HTML tags without a parser.
fn strip_tags_simple(html: &str) -> String {
    let mut output = String::new();
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => output.push(ch),
            _ => {}
        }
    }
    output.trim().to_string()
}

#[cfg(test)]
mod tests;
