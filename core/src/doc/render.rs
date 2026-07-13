//! Lua bindings for document format conversion and native web-view PDF rendering.

use super::{DOC_RENDER_FILE_PARAMS, DOC_RENDER_PARAMS};
use crate::doc_reader::{
    convert_file, is_binary_conversion, render_document, render_document_bytes, PageOptions,
    ReadOptions,
};
use crate::mount::MountTable;
use crate::sandbox::arg_error;
use crate::WEBVIEW_PDF_POLICY_ERROR;
use mlua::{Lua, MultiValue, Table, Value};
use std::path::Path;
use std::sync::Arc;

pub(super) fn register(
    lua: &Lua,
    doc: &Table,
    mounts: Arc<MountTable>,
    allow_webview_pdf_rendering: bool,
) -> Result<(), mlua::Error> {
    register_render(lua, doc, allow_webview_pdf_rendering)?;
    register_render_file(lua, doc, mounts, allow_webview_pdf_rendering)
}

fn register_render(
    lua: &Lua,
    doc: &Table,
    allow_webview_pdf_rendering: bool,
) -> Result<(), mlua::Error> {
    doc.set(
        "render",
        lua.create_function(move |lua, args: MultiValue| {
            let (text, from, to, opts) = parse_render_args(args)?;

            if is_binary_conversion(&to) {
                deny_disallowed_webview_pdf(
                    direct_render_uses_webview(&from, &to),
                    allow_webview_pdf_rendering,
                )?;
                let page = parse_page_options(opts.as_ref());
                let bytes = render_document_bytes(&text, &from, &to, &page)
                    .map_err(mlua::Error::external)?;
                lua.create_string(bytes)
            } else {
                let rendered = render_document(&text, &from, &to).map_err(mlua::Error::external)?;
                lua.create_string(rendered)
            }
        })?,
    )
}

fn parse_render_args(
    args: MultiValue,
) -> Result<(String, String, String, Option<Table>), mlua::Error> {
    if args.is_empty() {
        return Err(arg_error("doc.render", DOC_RENDER_PARAMS));
    }

    let first = args[0].clone();
    let from = string_arg(&args, 1);
    let to = string_arg(&args, 2);
    let opts = table_arg(&args, 3);
    match first {
        Value::Table(table) => {
            let text = required_table_string(&table, "text", 1, "doc.render")?;
            let from = required_table_string(&table, "from", 2, "doc.render")?;
            let to = required_table_string(&table, "to", 3, "doc.render")?;
            Ok((text, from, to, Some(table)))
        }
        Value::String(text) => Ok((
            text.to_string_lossy().to_string(),
            from.ok_or_else(|| mlua::Error::external("doc.render: missing 'from' format"))?,
            to.ok_or_else(|| mlua::Error::external("doc.render: missing 'to' format"))?,
            opts,
        )),
        _ => Err(mlua::Error::external(
            "doc.render: first arg must be a string or table",
        )),
    }
}

fn register_render_file(
    lua: &Lua,
    doc: &Table,
    mounts: Arc<MountTable>,
    allow_webview_pdf_rendering: bool,
) -> Result<(), mlua::Error> {
    doc.set(
        "renderFile",
        lua.create_function(move |_, args: MultiValue| {
            let (input, output, opts) = parse_render_file_args(args)?;
            let from = format_for_path(&input, opts.as_ref(), "from", "input")?;
            let to = format_for_path(&output, opts.as_ref(), "to", "output")?;

            deny_disallowed_webview_pdf(
                file_render_uses_webview(&from, &to),
                allow_webview_pdf_rendering,
            )?;

            let host_input = mounts.resolve_read(&input).map_err(mlua::Error::external)?;
            let host_output = mounts
                .resolve_write(&output)
                .map_err(mlua::Error::external)?;
            let data = std::fs::read(host_input).map_err(mlua::Error::external)?;
            let read_opts = ReadOptions {
                sheet: opts
                    .as_ref()
                    .and_then(|table| table.get::<i32>("sheet").ok())
                    .map(|sheet| sheet as usize),
                mode: None,
            };
            let page_opts = parse_page_options(opts.as_ref());
            let result = convert_file(&data, &from, &to, &read_opts, &page_opts)
                .map_err(mlua::Error::external)?;
            std::fs::write(host_output, result).map_err(mlua::Error::external)
        })?,
    )
}

fn parse_render_file_args(
    args: MultiValue,
) -> Result<(String, String, Option<Table>), mlua::Error> {
    if args.is_empty() {
        return Err(arg_error("doc.renderFile", DOC_RENDER_FILE_PARAMS));
    }

    let first = args[0].clone();
    let output = string_arg(&args, 1);
    let opts = table_arg(&args, 2);
    match first {
        Value::Table(table) => {
            let source = required_table_string(&table, "source", 1, "doc.renderFile")?;
            let target = required_table_string(&table, "target", 2, "doc.renderFile")?;
            Ok((source, target, Some(table)))
        }
        Value::String(input) => Ok((
            input.to_string_lossy().to_string(),
            output.ok_or_else(|| mlua::Error::external("doc.renderFile: missing output path"))?,
            opts,
        )),
        _ => Err(mlua::Error::external(
            "doc.renderFile: first arg must be a string or table",
        )),
    }
}

fn string_arg(args: &MultiValue, index: usize) -> Option<String> {
    match args.get(index) {
        Some(Value::String(value)) => Some(value.to_string_lossy().to_string()),
        _ => None,
    }
}

fn table_arg(args: &MultiValue, index: usize) -> Option<Table> {
    match args.get(index) {
        Some(Value::Table(table)) => Some(table.clone()),
        _ => None,
    }
}

fn required_table_string(
    table: &Table,
    field: &str,
    index: usize,
    function: &str,
) -> Result<String, mlua::Error> {
    table
        .get::<String>(field)
        .or_else(|_| table.get::<String>(index))
        .map_err(|_| {
            mlua::Error::external(format!(
                "{function}: table must have '{field}' or [{index}]"
            ))
        })
}

fn format_for_path(
    path: &str,
    opts: Option<&Table>,
    override_field: &str,
    kind: &str,
) -> Result<String, mlua::Error> {
    let format = opts
        .and_then(|table| table.get::<String>(override_field).ok())
        .unwrap_or_else(|| {
            Path::new(path)
                .extension()
                .and_then(|extension| extension.to_str())
                .unwrap_or("")
                .to_string()
        });
    if format.is_empty() {
        return Err(mlua::Error::external(format!(
            "cannot infer {kind} format from '{path}' — pass {{{override_field}=\"...\"}}"
        )));
    }
    Ok(format)
}

fn direct_render_uses_webview(from: &str, to: &str) -> bool {
    to == "pdf" && matches!(from, "html" | "markdown" | "md")
}

fn file_render_uses_webview(from: &str, to: &str) -> bool {
    to.eq_ignore_ascii_case("pdf")
        && matches!(
            from.to_ascii_lowercase().as_str(),
            "html" | "htm" | "markdown" | "md"
        )
}

fn deny_disallowed_webview_pdf(
    uses_webview: bool,
    allow_webview_pdf_rendering: bool,
) -> Result<(), mlua::Error> {
    if uses_webview && !allow_webview_pdf_rendering {
        return Err(mlua::Error::external(WEBVIEW_PDF_POLICY_ERROR));
    }
    Ok(())
}

fn parse_page_options(opts: Option<&Table>) -> PageOptions {
    let mut page = PageOptions::default();
    let Some(opts) = opts else {
        return page;
    };
    page.page_width = opts.get::<f64>("pageWidth").unwrap_or(page.page_width);
    page.page_height = opts.get::<f64>("pageHeight").unwrap_or(page.page_height);
    page.margin_top = opts.get::<f64>("marginTop").unwrap_or(page.margin_top);
    page.margin_bottom = opts
        .get::<f64>("marginBottom")
        .unwrap_or(page.margin_bottom);
    page.margin_left = opts.get::<f64>("marginLeft").unwrap_or(page.margin_left);
    page.margin_right = opts.get::<f64>("marginRight").unwrap_or(page.margin_right);
    page.landscape = opts.get::<bool>("landscape").unwrap_or(page.landscape);
    page
}

#[cfg(test)]
mod tests {
    use super::{direct_render_uses_webview, file_render_uses_webview};

    #[test]
    fn webview_detection_matches_supported_render_conversions() {
        assert!(direct_render_uses_webview("html", "pdf"));
        assert!(direct_render_uses_webview("markdown", "pdf"));
        assert!(direct_render_uses_webview("md", "pdf"));
        assert!(!direct_render_uses_webview("htm", "pdf"));
        assert!(!direct_render_uses_webview("txt", "pdf"));
    }

    #[test]
    fn webview_detection_matches_supported_file_conversions() {
        assert!(file_render_uses_webview("html", "pdf"));
        assert!(file_render_uses_webview("htm", "PDF"));
        assert!(file_render_uses_webview("markdown", "pdf"));
        assert!(file_render_uses_webview("md", "pdf"));
        assert!(!file_render_uses_webview("csv", "pdf"));
    }
}
