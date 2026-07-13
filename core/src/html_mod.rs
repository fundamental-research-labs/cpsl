//! HTML parsing module for the Luau sandbox.
//!
//! Exposes `html.parse`, `html.select`, `html.select_one`, `html.text`,
//! `html.attr`, `html.inner_html`, and `html.outer_html` as globals.
//! Uses the `scraper` crate — pure computation, no filesystem or network access.
//!
//! The document handle from `html.parse()` is a Lua table containing the raw
//! HTML source under `__html_source` and a `__type` marker. This avoids the
//! Send/Sync constraint that `mlua::UserData` requires (scraper::Html uses
//! non-atomic tendrils internally).

use crate::lua_util::register_help_functions;
use crate::sandbox::{
    validate_args, wrap_module_with_help_hints, FnDoc, ModuleDoc, Param, ParamType, ReturnType,
};
use mlua::{Lua, MultiValue, Value};
use scraper::{Html, Selector};

// ---------------------------------------------------------------------------
// Module documentation
// ---------------------------------------------------------------------------

pub(crate) static HTML_DOC: ModuleDoc = ModuleDoc {
    name: "html",
    summary: "HTML parsing with CSS selectors",
    functions: &[
        FnDoc {
            name: "parse",
            description:
                "Parse an HTML string into a document handle for querying with select/select_one.",
            params: &[Param {
                name: "html",
                short: Some('h'),
                typ: ParamType::String,
                required: true,
                    fields: None,
            }],
            returns: ReturnType::Value,
            example: None,
        },
        FnDoc {
            name: "select",
            description: "Select all elements matching a CSS selector. Returns a list of {tag, text, html, attrs} tables. Accepts a document handle or raw HTML string as the first argument.",
            params: &[
                Param {
                    name: "doc",
                    short: Some('d'),
                    typ: ParamType::Value,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "selector",
                    short: Some('s'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
            ],
            returns: ReturnType::Table,
            example: Some(r#"local links = html.select(page, "a.nav-link") -- list of {tag, text, attrs}"#),
        },
        FnDoc {
            name: "select_one",
            description: "Select the first element matching a CSS selector, or nil if none. Returns a {tag, text, html, attrs} table. Accepts a document handle or raw HTML string.",
            params: &[
                Param {
                    name: "doc",
                    short: Some('d'),
                    typ: ParamType::Value,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "selector",
                    short: Some('s'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
            ],
            returns: ReturnType::Value,
            example: None,
        },
        FnDoc {
            name: "text",
            description: "Extract all inner text from a document, element table, or raw HTML string. Text nodes are concatenated.",
            params: &[Param {
                name: "doc",
                short: Some('d'),
                typ: ParamType::Value,
                required: true,
                    fields: None,
            }],
            returns: ReturnType::String,
            example: None,
        },
        FnDoc {
            name: "attr",
            description:
                "Get the value of an attribute from an element table. Returns nil if not found.",
            params: &[
                Param {
                    name: "element",
                    short: Some('e'),
                    typ: ParamType::Table,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "name",
                    short: Some('n'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
            ],
            returns: ReturnType::Value,
            example: None,
        },
        FnDoc {
            name: "inner_html",
            description: "Get the inner HTML of an element table or document handle.",
            params: &[Param {
                name: "element",
                short: Some('e'),
                typ: ParamType::Value,
                required: true,
                    fields: None,
            }],
            returns: ReturnType::String,
            example: None,
        },
        FnDoc {
            name: "outer_html",
            description: "Get the outer HTML of an element table (includes the element's own tag).",
            params: &[Param {
                name: "element",
                short: Some('e'),
                typ: ParamType::Value,
                required: true,
                    fields: None,
            }],
            returns: ReturnType::String,
            example: None,
        },
    ],
};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Marker value stored in `__type` to identify document-handle tables.
const DOC_MARKER: &str = "__html_doc";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Compile a CSS selector, returning a friendly Lua error on failure.
fn compile_selector(sel: &str, fn_name: &str) -> Result<Selector, mlua::Error> {
    Selector::parse(sel).map_err(|e| {
        mlua::Error::external(format!(
            "{}: invalid CSS selector '{}': {:?}",
            fn_name, sel, e
        ))
    })
}

/// Extract the HTML source string from a Lua value.
/// Accepts:
/// - A document-handle table (has `__html_source` key)
/// - A raw Lua string
/// Returns the source HTML for parsing.
fn resolve_html_source(val: &Value) -> Result<String, mlua::Error> {
    match val {
        Value::Table(t) => {
            let source: Value = t.get("__html_source")?;
            match source {
                Value::String(s) => Ok(s.to_string_lossy().to_string()),
                _ => Err(mlua::Error::external(
                    "expected an html document handle (from html.parse) or an HTML string",
                )),
            }
        }
        Value::String(s) => Ok(s.to_string_lossy().to_string()),
        _ => Err(mlua::Error::external(
            "expected an html document handle (from html.parse) or an HTML string",
        )),
    }
}

/// Check if a Lua table is a document handle (has `__html_source` key).
fn is_doc_handle(t: &mlua::Table) -> bool {
    matches!(t.get::<Value>("__html_source"), Ok(Value::String(_)))
}

/// Extract a single Value argument from MultiValue, handling both:
/// - Positional form: `fn(value)`
/// - Table form: `fn({param_name=value})` or `fn({[1]=value})`
///
/// This is needed for functions with a single `Value`-typed param where the
/// value itself may be a table (document handle or element table). The generic
/// `validate_args` misinterprets a table argument as table-form wrapping.
fn extract_single_value_arg(
    args: &MultiValue,
    param_name: &str,
    fn_name: &str,
) -> Result<Value, mlua::Error> {
    let vals: Vec<Value> = args.iter().cloned().collect();
    if vals.is_empty() {
        return Err(mlua::Error::external(format!(
            "{}: missing required argument '{}'\n  hint: call html.help() for usage",
            fn_name, param_name
        )));
    }
    if vals.len() == 1 {
        let val = &vals[0];
        // If it's a table, check if it's a document handle or element table
        // (not a table-form wrapper).
        if let Value::Table(t) = val {
            // If it looks like a doc handle or element table, return as-is
            if is_doc_handle(t)
                || matches!(t.get::<Value>("text"), Ok(Value::String(_)))
                || matches!(t.get::<Value>("_outer_html"), Ok(Value::String(_)))
            {
                return Ok(val.clone());
            }
            // Otherwise try table-form: check for named key or positional [1]
            let inner: Value = t
                .get::<Value>(param_name)
                .ok()
                .filter(|v| !matches!(v, Value::Nil))
                .or_else(|| t.get::<Value>(1).ok().filter(|v| !matches!(v, Value::Nil)))
                .unwrap_or(Value::Nil);
            if !matches!(inner, Value::Nil) {
                return Ok(inner);
            }
            // Ambiguous: could still be an element table without expected keys.
            // Return it as-is and let the caller handle the error.
            return Ok(val.clone());
        }
        return Ok(val.clone());
    }
    // Multiple args — just take the first
    Ok(vals[0].clone())
}

/// Build a Lua table representing an element: {tag, text, html, attrs, _outer_html}.
fn element_to_table(lua: &Lua, el: scraper::ElementRef<'_>) -> Result<mlua::Table, mlua::Error> {
    let tbl = lua.create_table()?;

    // tag name
    tbl.set("tag", lua.create_string(el.value().name())?)?;

    // text — all text nodes concatenated
    let text: String = el.text().collect();
    tbl.set("text", lua.create_string(&text)?)?;

    // html — inner HTML
    tbl.set("html", lua.create_string(&el.inner_html())?)?;

    // attrs — table of attribute name → value
    let attrs = lua.create_table()?;
    for (name, val) in el.value().attrs() {
        attrs.set(lua.create_string(name)?, lua.create_string(val)?)?;
    }
    tbl.set("attrs", attrs)?;

    // _outer_html — stored for outer_html() function
    tbl.set("_outer_html", lua.create_string(&el.html())?)?;

    Ok(tbl)
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/// Register `html.*` globals in the Lua VM.
pub fn register_html_globals(lua: &Lua) -> Result<(), mlua::Error> {
    let html_table = lua.create_table()?;

    // --- html.parse(html_string) -> document handle table ---
    html_table.set(
        "parse",
        lua.create_function(|lua, args: MultiValue| {
            let validated = validate_args(&args, HTML_DOC.params("parse"), "html.parse")?;
            let html_str = match &validated[0] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };

            // Build a table-based document handle: { __type = "__html_doc", __html_source = "..." }
            let doc = lua.create_table()?;
            doc.set("__type", DOC_MARKER)?;
            doc.set("__html_source", lua.create_string(&html_str)?)?;

            // Add a __tostring metamethod for nice printing
            let mt = lua.create_table()?;
            let source_len = html_str.len();
            let preview: String = {
                let parsed = Html::parse_document(&html_str);
                let text: String = parsed.root_element().text().collect();
                text.chars().take(80).collect()
            };
            let tostring_val = format!("HtmlDocument({} chars, \"{}...\")", source_len, preview);
            mt.set(
                "__tostring",
                lua.create_function(move |_, _: Value| Ok(tostring_val.clone()))?,
            )?;
            doc.set_metatable(Some(mt))?;

            Ok(Value::Table(doc))
        })?,
    )?;

    // --- html.select(doc_or_html, selector) -> list of {tag, text, html, attrs} ---
    html_table.set(
        "select",
        lua.create_function(|lua, args: MultiValue| {
            let validated = validate_args(&args, HTML_DOC.params("select"), "html.select")?;
            let source = resolve_html_source(&validated[0])?;
            let selector_str = match &validated[1] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };

            let parsed = Html::parse_document(&source);
            let sel = compile_selector(&selector_str, "html.select")?;
            let results = lua.create_table()?;
            let mut idx = 1i64;

            for el in parsed.select(&sel) {
                let entry = element_to_table(lua, el)?;
                results.raw_set(idx, entry)?;
                idx += 1;
            }

            Ok(Value::Table(results))
        })?,
    )?;

    // --- html.select_one(doc_or_html, selector) -> {tag, text, html, attrs} or nil ---
    html_table.set(
        "select_one",
        lua.create_function(|lua, args: MultiValue| {
            let validated = validate_args(&args, HTML_DOC.params("select_one"), "html.select_one")?;
            let source = resolve_html_source(&validated[0])?;
            let selector_str = match &validated[1] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };

            let parsed = Html::parse_document(&source);
            let sel = compile_selector(&selector_str, "html.select_one")?;

            match parsed.select(&sel).next() {
                Some(el) => {
                    let entry = element_to_table(lua, el)?;
                    Ok(Value::Table(entry))
                }
                None => Ok(Value::Nil),
            }
        })?,
    )?;

    // --- html.text(doc_or_element) -> string ---
    html_table.set(
        "text",
        lua.create_function(|_, args: MultiValue| {
            let val = extract_single_value_arg(&args, "doc", "html.text")?;

            match &val {
                Value::Table(t) => {
                    // Check if it's a document handle (has __html_source)
                    let source: Value = t.get("__html_source")?;
                    if let Value::String(s) = source {
                        let html_str = s.to_string_lossy().to_string();
                        let parsed = Html::parse_document(&html_str);
                        let text: String = parsed.root_element().text().collect();
                        return Ok(text);
                    }
                    // Otherwise treat as element table — return the .text field
                    let text: Value = t.get("text")?;
                    match text {
                        Value::String(s) => Ok(s.to_string_lossy().to_string()),
                        _ => Err(mlua::Error::external(
                            "html.text: table has no 'text' field (not an element table)",
                        )),
                    }
                }
                // Raw HTML string — parse and extract text
                Value::String(s) => {
                    let html_str = s.to_string_lossy().to_string();
                    let parsed = Html::parse_document(&html_str);
                    let text: String = parsed.root_element().text().collect();
                    Ok(text)
                }
                _ => Err(mlua::Error::external(
                    "html.text: expected a document handle, element table, or HTML string",
                )),
            }
        })?,
    )?;

    // --- html.attr(element, name) -> string or nil ---
    html_table.set(
        "attr",
        lua.create_function(|_, args: MultiValue| {
            let validated = validate_args(&args, HTML_DOC.params("attr"), "html.attr")?;
            let element = match &validated[0] {
                Value::Table(t) => t.clone(),
                _ => unreachable!(),
            };
            let attr_name = match &validated[1] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };

            let attrs: Value = element.get("attrs")?;
            match attrs {
                Value::Table(attrs_table) => {
                    let val: Value = attrs_table.get(attr_name.as_str())?;
                    Ok(val)
                }
                _ => Ok(Value::Nil),
            }
        })?,
    )?;

    // --- html.inner_html(element_or_doc) -> string ---
    html_table.set(
        "inner_html",
        lua.create_function(|_, args: MultiValue| {
            let val = extract_single_value_arg(&args, "element", "html.inner_html")?;

            match &val {
                Value::Table(t) => {
                    // Check if it's a document handle
                    let source: Value = t.get("__html_source")?;
                    if let Value::String(s) = source {
                        let html_str = s.to_string_lossy().to_string();
                        let parsed = Html::parse_document(&html_str);
                        return Ok(parsed.root_element().inner_html());
                    }
                    // Element table — return .html field
                    let html_val: Value = t.get("html")?;
                    match html_val {
                        Value::String(s) => Ok(s.to_string_lossy().to_string()),
                        _ => Err(mlua::Error::external(
                            "html.inner_html: table has no 'html' field (not an element table)",
                        )),
                    }
                }
                Value::String(s) => {
                    let html_str = s.to_string_lossy().to_string();
                    let parsed = Html::parse_document(&html_str);
                    Ok(parsed.root_element().inner_html())
                }
                _ => Err(mlua::Error::external(
                    "html.inner_html: expected an element table, document handle, or HTML string",
                )),
            }
        })?,
    )?;

    // --- html.outer_html(element) -> string ---
    html_table.set(
        "outer_html",
        lua.create_function(|_, args: MultiValue| {
            let val = extract_single_value_arg(&args, "element", "html.outer_html")?;

            match &val {
                Value::Table(t) => {
                    // Check if it's a document handle
                    let source: Value = t.get("__html_source")?;
                    if let Value::String(s) = source {
                        let html_str = s.to_string_lossy().to_string();
                        let parsed = Html::parse_document(&html_str);
                        return Ok(parsed.root_element().html());
                    }
                    // Element table — return ._outer_html field
                    let outer: Value = t.get("_outer_html")?;
                    match outer {
                        Value::String(s) => Ok(s.to_string_lossy().to_string()),
                        _ => Err(mlua::Error::external(
                            "html.outer_html: table has no '_outer_html' field (not an element table)",
                        )),
                    }
                }
                Value::String(s) => {
                    let html_str = s.to_string_lossy().to_string();
                    let parsed = Html::parse_document(&html_str);
                    Ok(parsed.root_element().html())
                }
                _ => Err(mlua::Error::external(
                    "html.outer_html: expected an element table, document handle, or HTML string",
                )),
            }
        })?,
    )?;

    register_help_functions(lua, &html_table, &HTML_DOC)?;

    lua.globals().set("html", html_table)?;
    wrap_module_with_help_hints(lua, "html")?;

    Ok(())
}
