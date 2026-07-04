//! XML module for the Luau sandbox.
//!
//! Exposes `xml.parse`, `xml.parseFile`, `xml.encode`, `xml.query`, `xml.text` as globals.
//! Uses `quick-xml` (already a dependency, pure Rust, high performance).

use crate::mount::MountTable;
use crate::pyrt_compat::unwrap_py_seq;
use crate::sandbox::{
    validate_args, wrap_module_with_help_hints, FnDoc, ModuleDoc, Param, ParamType, ReturnType,
};
use mlua::{Lua, MultiValue, Value};
use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
use quick_xml::reader::Reader;
use quick_xml::writer::Writer;
use quick_xml::XmlVersion;
use std::io::Cursor;
use std::sync::Arc;

pub(crate) static XML_DOC: ModuleDoc = ModuleDoc {
    name: "xml",
    summary: "XML parse, query & encode",
    functions: &[
        FnDoc {
            name: "parse",
            description: "Parse an XML string into a node tree (tag, attrs, children, text).",
            params: &[Param {
                name: "text",
                short: Some('t'),
                typ: ParamType::String,
                required: true,
                    fields: None,
            }],
            returns: ReturnType::Table,
            example: Some(r#"local doc = xml.parse("<root><item id='1'>Hello</item></root>")"#),
        },
        FnDoc {
            name: "parseFile",
            description: "Parse an XML file into a tree.",
            params: &[Param {
                name: "path",
                short: None,
                typ: ParamType::String,
                required: true,
                    fields: None,
            }],
            returns: ReturnType::Table,
            example: None,
        },
        FnDoc {
            name: "encode",
            description:
                "Encode a node tree (tag, attrs?, children?, text?) to an XML string.",
            params: &[Param {
                name: "tree",
                short: Some('t'),
                typ: ParamType::Table,
                required: true,
                    fields: None,
            }],
            returns: ReturnType::String,
            example: Some(r#"xml.encode({tag="item", attrs={id="1"}, text="Hello"})"#),
        },
        FnDoc {
            name: "query",
            description: "Filter nodes by simple path (e.g. \"root/child\"). Returns array of matching nodes.",
            params: &[
                Param {
                    name: "doc",
                    short: Some('d'),
                    typ: ParamType::Table,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "path",
                    short: Some('p'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
            ],
            returns: ReturnType::Table,
            example: Some(r#"xml.query({doc=doc, path="root/item"})"#),
        },
        FnDoc {
            name: "text",
            description: "Extract text content from a node recursively.",
            params: &[Param {
                name: "node",
                short: None,
                typ: ParamType::Table,
                required: true,
                    fields: None,
            }],
            returns: ReturnType::String,
            example: None,
        },
    ],
};

/// Intermediate representation of an XML node for parsing.
#[derive(Debug)]
enum XmlNode {
    Element {
        tag: String,
        attrs: Vec<(String, String)>,
        children: Vec<XmlNode>,
    },
    Text(String),
}

/// Parse XML text into a tree of XmlNode.
fn parse_xml_tree(xml_text: &str) -> Result<XmlNode, mlua::Error> {
    let mut reader = Reader::from_str(xml_text);
    reader.config_mut().trim_text(true);

    let mut stack: Vec<XmlNode> = Vec::new();
    // Push a virtual root to collect top-level nodes
    stack.push(XmlNode::Element {
        tag: String::new(),
        attrs: Vec::new(),
        children: Vec::new(),
    });

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let attrs: Vec<(String, String)> = e
                    .attributes()
                    .filter_map(|a| {
                        let a = a.ok()?;
                        let key = String::from_utf8_lossy(a.key.as_ref()).to_string();
                        let val = a
                            .normalized_value(XmlVersion::Implicit1_0)
                            .ok()?
                            .to_string();
                        Some((key, val))
                    })
                    .collect();
                stack.push(XmlNode::Element {
                    tag,
                    attrs,
                    children: Vec::new(),
                });
            }
            Ok(Event::End(_)) => {
                let node = stack.pop().ok_or_else(|| {
                    mlua::Error::external("XML parse error: unexpected closing tag")
                })?;
                if let Some(parent) = stack.last_mut() {
                    if let XmlNode::Element { children, .. } = parent {
                        children.push(node);
                    }
                }
            }
            Ok(Event::Empty(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let attrs: Vec<(String, String)> = e
                    .attributes()
                    .filter_map(|a| {
                        let a = a.ok()?;
                        let key = String::from_utf8_lossy(a.key.as_ref()).to_string();
                        let val = a
                            .normalized_value(XmlVersion::Implicit1_0)
                            .ok()?
                            .to_string();
                        Some((key, val))
                    })
                    .collect();
                if let Some(parent) = stack.last_mut() {
                    if let XmlNode::Element { children, .. } = parent {
                        children.push(XmlNode::Element {
                            tag,
                            attrs,
                            children: Vec::new(),
                        });
                    }
                }
            }
            Ok(Event::Text(ref e)) => {
                let text = e.decode().map_err(mlua::Error::external)?.to_string();
                if !text.is_empty() {
                    if let Some(parent) = stack.last_mut() {
                        if let XmlNode::Element { children, .. } = parent {
                            children.push(XmlNode::Text(text));
                        }
                    }
                }
            }
            Ok(Event::CData(ref e)) => {
                let text = String::from_utf8_lossy(e.as_ref()).to_string();
                if !text.is_empty() {
                    if let Some(parent) = stack.last_mut() {
                        if let XmlNode::Element { children, .. } = parent {
                            children.push(XmlNode::Text(text));
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {} // skip PI, comments, decl
            Err(e) => return Err(mlua::Error::external(format!("XML parse error: {}", e))),
        }
    }

    // The stack should have just the virtual root
    if stack.len() != 1 {
        return Err(mlua::Error::external("XML parse error: unclosed elements"));
    }

    let root = stack.pop().unwrap();
    if let XmlNode::Element { mut children, .. } = root {
        if children.len() == 1 {
            Ok(children.remove(0))
        } else if children.is_empty() {
            Err(mlua::Error::external("XML parse error: empty document"))
        } else {
            // Multiple top-level elements: wrap in virtual root
            Ok(XmlNode::Element {
                tag: String::new(),
                attrs: Vec::new(),
                children,
            })
        }
    } else {
        unreachable!()
    }
}

/// Convert an XmlNode tree to a Lua table.
/// Element: {tag=string, attrs={}, children={...}, text=string_or_nil}
/// For text-only elements, text is set directly.
fn xml_node_to_lua(lua: &Lua, node: &XmlNode) -> Result<Value, mlua::Error> {
    match node {
        XmlNode::Text(s) => Ok(Value::String(lua.create_string(s)?)),
        XmlNode::Element {
            tag,
            attrs,
            children,
        } => {
            let table = lua.create_table()?;
            table.set("tag", lua.create_string(tag.as_str())?)?;

            // attrs
            let attrs_table = lua.create_table()?;
            for (k, v) in attrs {
                attrs_table.set(k.as_str(), lua.create_string(v.as_str())?)?;
            }
            table.set("attrs", attrs_table)?;

            // children
            let children_table = lua.create_table()?;
            for (i, child) in children.iter().enumerate() {
                children_table.set(i + 1, xml_node_to_lua(lua, child)?)?;
            }
            table.set("children", children_table)?;

            // text: if this element has exactly one text child, set it directly
            if children.len() == 1 {
                if let XmlNode::Text(s) = &children[0] {
                    table.set("text", lua.create_string(s.as_str())?)?;
                }
            }

            Ok(Value::Table(table))
        }
    }
}

/// Convert a Lua table back to XML string.
fn lua_to_xml(table: &mlua::Table) -> Result<String, mlua::Error> {
    let mut writer = Writer::new(Cursor::new(Vec::new()));
    write_node(&mut writer, table)?;
    let result = writer.into_inner().into_inner();
    String::from_utf8(result).map_err(mlua::Error::external)
}

fn write_node(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    table: &mlua::Table,
) -> Result<(), mlua::Error> {
    let tag: String = table.get("tag")?;

    let mut elem = BytesStart::new(tag.clone());

    // Write attributes
    if let Ok(attrs) = table.get::<mlua::Table>("attrs") {
        for pair in attrs.pairs::<String, String>() {
            let (k, v) = pair?;
            elem.push_attribute((k.as_str(), v.as_str()));
        }
    }

    // Check if there are children (unwrap py.list if needed)
    let children: Option<mlua::Table> = table
        .get::<mlua::Table>("children")
        .ok()
        .map(|c| unwrap_py_seq(&c))
        .transpose()?;
    let has_children = children.as_ref().map(|c| c.raw_len() > 0).unwrap_or(false);

    // Check for text content (direct text field)
    let text: Option<String> = table.get("text").ok();

    if !has_children && text.is_none() {
        // Self-closing tag
        writer
            .write_event(Event::Empty(elem))
            .map_err(mlua::Error::external)?;
    } else {
        writer
            .write_event(Event::Start(elem))
            .map_err(mlua::Error::external)?;

        if let Some(ref children_table) = children {
            let len = children_table.raw_len();
            if len > 0 {
                for i in 1..=len {
                    let child: Value = children_table.get(i)?;
                    match child {
                        Value::String(s) => {
                            writer
                                .write_event(Event::Text(BytesText::new(&s.to_string_lossy())))
                                .map_err(mlua::Error::external)?;
                        }
                        Value::Table(t) => {
                            // Check if this is a text node (plain string in children) or element
                            if t.contains_key("tag")? {
                                write_node(writer, &t)?;
                            } else {
                                // Could be a text-only table? Skip.
                            }
                        }
                        _ => {}
                    }
                }
            } else if let Some(ref txt) = text {
                // No children array entries, but has text
                writer
                    .write_event(Event::Text(BytesText::new(txt)))
                    .map_err(mlua::Error::external)?;
            }
        } else if let Some(ref txt) = text {
            writer
                .write_event(Event::Text(BytesText::new(txt)))
                .map_err(mlua::Error::external)?;
        }

        writer
            .write_event(Event::End(BytesEnd::new(tag)))
            .map_err(mlua::Error::external)?;
    }

    Ok(())
}

/// Query nodes by a simple path like "root/child" or "root/child/grandchild".
/// Returns all matching nodes as an array.
fn query_nodes(lua: &Lua, node: &mlua::Table, path: &str) -> Result<Value, mlua::Error> {
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    let results = lua.create_table()?;
    let mut idx = 1;

    collect_by_path(lua, node, &segments, 0, &results, &mut idx)?;

    Ok(Value::Table(results))
}

fn collect_by_path(
    lua: &Lua,
    node: &mlua::Table,
    segments: &[&str],
    depth: usize,
    results: &mlua::Table,
    idx: &mut usize,
) -> Result<(), mlua::Error> {
    if depth >= segments.len() {
        return Ok(());
    }

    let tag: String = node.get::<String>("tag").unwrap_or_default();
    let segment = segments[depth];

    // Check if this node matches the current segment
    if tag == segment || segment == "*" {
        if depth == segments.len() - 1 {
            // This is the final segment — this node matches
            results.set(*idx, node.clone())?;
            *idx += 1;
        } else {
            // Continue matching children against next segments
            if let Ok(children) = node.get::<mlua::Table>("children") {
                for i in 1..=children.raw_len() {
                    let child: Value = children.get(i)?;
                    if let Value::Table(t) = child {
                        if t.contains_key("tag")? {
                            collect_by_path(lua, &t, segments, depth + 1, results, idx)?;
                        }
                    }
                }
            }
        }
    }

    // If depth == 0, also search children at depth 0 (the root tag must match first segment)
    // Actually, if the root tag doesn't match, we still need to search children
    if depth == 0 && tag != segment && segment != "*" {
        // The root doesn't match the first segment, try children
        if let Ok(children) = node.get::<mlua::Table>("children") {
            for i in 1..=children.raw_len() {
                let child: Value = children.get(i)?;
                if let Value::Table(t) = child {
                    if t.contains_key("tag")? {
                        collect_by_path(lua, &t, segments, depth, results, idx)?;
                    }
                }
            }
        }
    }

    Ok(())
}

/// Recursively extract all text content from a node.
fn extract_text(node: &mlua::Table) -> Result<String, mlua::Error> {
    let mut result = String::new();

    // Recurse into children (which includes text nodes as strings)
    if let Ok(children) = node.get::<mlua::Table>("children") {
        let len = children.raw_len();
        if len > 0 {
            for i in 1..=len {
                let child: Value = children.get(i)?;
                match child {
                    Value::String(s) => result.push_str(&s.to_string_lossy()),
                    Value::Table(t) => {
                        if t.contains_key("tag")? {
                            let child_text = extract_text(&t)?;
                            result.push_str(&child_text);
                        }
                    }
                    _ => {}
                }
            }
            return Ok(result);
        }
    }

    // No children — fall back to direct text field
    if let Ok(text) = node.get::<String>("text") {
        result.push_str(&text);
    }

    Ok(result)
}

/// Register `xml.*` globals in the Lua VM.
pub fn register_xml_globals(lua: &Lua, mounts: Arc<MountTable>) -> Result<(), mlua::Error> {
    let xml_table = lua.create_table()?;

    // xml.parse(text) -> table
    xml_table.set(
        "parse",
        lua.create_function(|lua, args: MultiValue| {
            let validated = validate_args(&args, XML_DOC.params("parse"), "xml.parse")?;
            let text = match &validated[0] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!("validate_args ensures string"),
            };
            let tree = parse_xml_tree(&text)?;
            xml_node_to_lua(lua, &tree)
        })?,
    )?;

    // xml.parseFile(path) -> table
    {
        let m = mounts.clone();
        xml_table.set(
            "parseFile",
            lua.create_function(move |lua, args: MultiValue| {
                let validated = validate_args(&args, XML_DOC.params("parseFile"), "xml.parseFile")?;
                let path = match &validated[0] {
                    Value::String(s) => s.to_string_lossy().to_string(),
                    _ => unreachable!("validate_args ensures string"),
                };
                let host_path = m.resolve_read(&path).map_err(mlua::Error::external)?;
                let data = std::fs::read_to_string(&host_path).map_err(mlua::Error::external)?;
                let tree = parse_xml_tree(&data)?;
                xml_node_to_lua(lua, &tree)
            })?,
        )?;
    }

    // xml.encode(table) -> string
    xml_table.set(
        "encode",
        lua.create_function(|_, args: MultiValue| {
            let validated = validate_args(&args, XML_DOC.params("encode"), "xml.encode")?;
            let table = match &validated[0] {
                Value::Table(t) => t.clone(),
                _ => unreachable!("validate_args ensures table"),
            };
            let xml_str = lua_to_xml(&table)?;
            Ok(xml_str)
        })?,
    )?;

    // xml.query(doc, path) -> table
    xml_table.set(
        "query",
        lua.create_function(|lua, args: MultiValue| {
            let validated = validate_args(&args, XML_DOC.params("query"), "xml.query")?;
            let doc = match &validated[0] {
                Value::Table(t) => t.clone(),
                _ => unreachable!("validate_args ensures table"),
            };
            let path = match &validated[1] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!("validate_args ensures string"),
            };
            query_nodes(lua, &doc, &path)
        })?,
    )?;

    // xml.text(node) -> string
    xml_table.set(
        "text",
        lua.create_function(|_, args: MultiValue| {
            let validated = validate_args(&args, XML_DOC.params("text"), "xml.text")?;
            let node = match &validated[0] {
                Value::Table(t) => t.clone(),
                _ => unreachable!("validate_args ensures table"),
            };
            extract_text(&node)
        })?,
    )?;

    crate::lua_util::register_help_functions(lua, &xml_table, &XML_DOC)?;

    lua.globals().set("xml", xml_table)?;
    wrap_module_with_help_hints(lua, "xml")?;

    Ok(())
}
