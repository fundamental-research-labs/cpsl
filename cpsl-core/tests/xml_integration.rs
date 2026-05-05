#![cfg(feature = "mod-xml")]

use cpsl_core::{MountTable, Sandbox, transpile};

fn sb() -> Sandbox {
    Sandbox::new().unwrap()
}

// ── parse ──────────────────────────────────────────────────────

#[test]
fn parse_simple_element() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local doc = xml.parse("<root>hello</root>")
            return doc.tag .. " " .. doc.text
        "#,
        )
        .unwrap();
    assert_eq!(r, "root hello");
}

#[test]
fn parse_attributes() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local doc = xml.parse('<item id="42" name="test"/>')
            return doc.tag .. " " .. doc.attrs.id .. " " .. doc.attrs.name
        "#,
        )
        .unwrap();
    assert_eq!(r, "item 42 test");
}

#[test]
fn parse_nested_elements() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local doc = xml.parse("<root><child>one</child><child>two</child></root>")
            return doc.tag .. " " .. #doc.children .. " " .. doc.children[1].text .. " " .. doc.children[2].text
        "#,
        )
        .unwrap();
    assert_eq!(r, "root 2 one two");
}

#[test]
fn parse_deep_nesting() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local doc = xml.parse("<a><b><c>deep</c></b></a>")
            return doc.children[1].children[1].text
        "#,
        )
        .unwrap();
    assert_eq!(r, "deep");
}

#[test]
fn parse_mixed_content() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local doc = xml.parse("<root><a>1</a><b>2</b></root>")
            return doc.children[1].tag .. " " .. doc.children[2].tag
        "#,
        )
        .unwrap();
    assert_eq!(r, "a b");
}

#[test]
fn parse_self_closing() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local doc = xml.parse('<root><br/><hr/></root>')
            return doc.children[1].tag .. " " .. doc.children[2].tag
        "#,
        )
        .unwrap();
    assert_eq!(r, "br hr");
}

#[test]
fn parse_empty_element() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local doc = xml.parse("<empty></empty>")
            return doc.tag .. " " .. tostring(#doc.children)
        "#,
        )
        .unwrap();
    assert_eq!(r, "empty 0");
}

// ── encode ─────────────────────────────────────────────────────

#[test]
fn encode_simple() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local result = xml.encode({tag = "root", attrs = {}, children = {}, text = "hello"})
            return result
        "#,
        )
        .unwrap();
    assert!(r.contains("<root>"), "got: {}", r);
    assert!(r.contains("hello"), "got: {}", r);
    assert!(r.contains("</root>"), "got: {}", r);
}

#[test]
fn encode_with_attrs() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local result = xml.encode({tag = "item", attrs = {id = "1"}, children = {}})
            return result
        "#,
        )
        .unwrap();
    assert!(r.contains("item"), "got: {}", r);
    assert!(r.contains("id="), "got: {}", r);
}

#[test]
fn encode_nested() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local doc = {
                tag = "root",
                attrs = {},
                children = {
                    {tag = "child", attrs = {}, children = {}, text = "one"},
                    {tag = "child", attrs = {}, children = {}, text = "two"},
                }
            }
            return xml.encode(doc)
        "#,
        )
        .unwrap();
    assert!(r.contains("<root>"), "got: {}", r);
    assert!(r.contains("<child>one</child>"), "got: {}", r);
    assert!(r.contains("<child>two</child>"), "got: {}", r);
    assert!(r.contains("</root>"), "got: {}", r);
}

#[test]
fn encode_self_closing() {
    let s = sb();
    let r = s
        .exec(
            r#"
            return xml.encode({tag = "br", attrs = {}, children = {}})
        "#,
        )
        .unwrap();
    assert!(r.contains("<br/>"), "got: {}", r);
}

// ── roundtrip ──────────────────────────────────────────────────

#[test]
fn roundtrip_parse_encode() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local original = "<root><a>1</a><b>2</b></root>"
            local doc = xml.parse(original)
            local encoded = xml.encode(doc)
            local reparsed = xml.parse(encoded)
            return reparsed.children[1].text .. " " .. reparsed.children[2].text
        "#,
        )
        .unwrap();
    assert_eq!(r, "1 2");
}

// ── query ──────────────────────────────────────────────────────

#[test]
fn query_direct_children() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local doc = xml.parse("<root><item>a</item><item>b</item><other>c</other></root>")
            local items = xml.query(doc, "root/item")
            return #items .. " " .. items[1].text .. " " .. items[2].text
        "#,
        )
        .unwrap();
    assert_eq!(r, "2 a b");
}

#[test]
fn query_deep_path() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local doc = xml.parse("<root><a><b>found</b></a></root>")
            local results = xml.query(doc, "root/a/b")
            return #results .. " " .. results[1].text
        "#,
        )
        .unwrap();
    assert_eq!(r, "1 found");
}

#[test]
fn query_no_match() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local doc = xml.parse("<root><a>1</a></root>")
            local results = xml.query(doc, "root/nonexistent")
            return tostring(#results)
        "#,
        )
        .unwrap();
    assert_eq!(r, "0");
}

// ── text ───────────────────────────────────────────────────────

#[test]
fn text_simple() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local doc = xml.parse("<root>hello world</root>")
            return xml.text(doc)
        "#,
        )
        .unwrap();
    assert_eq!(r, "hello world");
}

#[test]
fn text_nested() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local doc = xml.parse("<root><a>hello</a><b>world</b></root>")
            return xml.text(doc)
        "#,
        )
        .unwrap();
    assert_eq!(r, "helloworld");
}

#[test]
fn text_deep_nested() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local doc = xml.parse("<root><a><b>deep</b></a> <c>text</c></root>")
            return xml.text(doc)
        "#,
        )
        .unwrap();
    // Text extraction is recursive; space handling depends on trimming
    assert!(r.contains("deep"), "got: {}", r);
    assert!(r.contains("text"), "got: {}", r);
}

// ── error handling ─────────────────────────────────────────────

#[test]
fn parse_no_args_errors() {
    let s = sb();
    let err = s.exec("xml.parse()").unwrap_err();
    assert!(
        err.message.contains("missing required argument") && err.message.contains("text"),
        "msg: {}",
        err.message
    );
}

#[test]
fn parse_invalid_xml() {
    let s = sb();
    let err = s.exec(r#"xml.parse("<unclosed>")"#).unwrap_err();
    assert!(!err.message.is_empty(), "should error: {}", err.message);
}

// ── validate_args error messages ────────────────────────────────

#[test]
fn parse_no_args_mentions_text_param() {
    let s = sb();
    let err = s.exec("xml.parse()").unwrap_err();
    assert!(err.message.contains("'text'"), "should mention param name: {}", err.message);
    assert!(err.message.contains("string"), "should mention type: {}", err.message);
    assert!(err.message.contains("Usage:"), "should include inline usage: {}", err.message);
}

#[test]
fn parse_file_no_args_mentions_path_param() {
    let s = sb();
    let err = s.exec("xml.parseFile()").unwrap_err();
    assert!(err.message.contains("'path'"), "should mention param name: {}", err.message);
    assert!(err.message.contains("Usage:"), "should include inline usage: {}", err.message);
}

#[test]
fn query_no_args_mentions_doc_and_path() {
    let s = sb();
    let err = s.exec("xml.query()").unwrap_err();
    assert!(err.message.contains("'doc'"), "should mention doc: {}", err.message);
    assert!(err.message.contains("'path'"), "should mention path: {}", err.message);
    assert!(err.message.contains("Usage:"), "should include inline usage: {}", err.message);
}

#[test]
fn text_no_args_mentions_node_param() {
    let s = sb();
    let err = s.exec("xml.text()").unwrap_err();
    assert!(err.message.contains("'node'"), "should mention param name: {}", err.message);
    assert!(err.message.contains("Usage:"), "should include inline usage: {}", err.message);
}

#[test]
fn encode_no_args_mentions_tree_param() {
    let s = sb();
    let err = s.exec("xml.encode()").unwrap_err();
    assert!(err.message.contains("'tree'"), "should mention param name: {}", err.message);
    assert!(err.message.contains("Usage:"), "should include inline usage: {}", err.message);
}

#[test]
fn parse_wrong_type_mentions_expected_type() {
    let s = sb();
    let err = s.exec("xml.parse(42)").unwrap_err();
    assert!(err.message.contains("expected string"), "should mention expected type: {}", err.message);
    assert!(err.message.contains("Usage:"), "should include inline usage: {}", err.message);
}

// ── help ───────────────────────────────────────────────────────

#[test]
fn xml_help_returns_help() {
    let s = sb();
    let r = s.exec("return xml.help()").unwrap();
    assert!(r.contains("xml — XML parse, query & encode"), "help: {}", r);
    assert!(r.contains("xml.parse"), "help: {}", r);
    assert!(r.contains("xml.encode"), "help: {}", r);
    assert!(r.contains("xml.query"), "help: {}", r);
    assert!(r.contains("xml.text"), "help: {}", r);
}

#[test]
fn xml_nonexistent_fn_hint() {
    let s = sb();
    let err = s.exec("xml.foo()").unwrap_err();
    assert!(
        err.message.contains("xml.foo does not exist"),
        "msg: {}",
        err.message
    );
    assert!(
        err.message.contains("hint: call xml.help() for usage"),
        "msg: {}",
        err.message
    );
}

#[test]
fn global_help_mentions_xml() {
    let s = sb();
    let r = s.exec("return help()").unwrap();
    assert!(r.contains("xml"), "global help should list xml: {}", r);
}

// ── help format modes ──────────────────────────────────────────

#[test]
fn xml_lua_help_uses_structured_params() {
    // xml.help() should show Lua-style: xml.parse(text) -> table
    let s = sb();
    let r = s.exec("xml.help()").unwrap();
    assert!(r.contains("xml.parse(text: string) -> table"), "Lua help should show structured params: {}", r);
    assert!(r.contains("xml.query(doc: table, path: string) -> table"), "Lua help should show multi-param: {}", r);
    assert!(r.contains("xml.encode(tree: table) -> string"), "Lua help should show encode: {}", r);
    assert!(r.contains("xml.text(node: table) -> string"), "Lua help should show text: {}", r);
}

#[test]
fn xml_shell_help_uses_flag_syntax() {
    // Via sh.run(), bare `xml` should show shell-style: xml parse --text <string>
    let s = sb();
    let shrt = include_str!("../../shrt.luau");
    s.register_module("shrt", shrt).unwrap();
    let luau = cpsl_core::sh_transpile::transpile_sh("xml").unwrap().luau_source;
    let r = s.exec(&luau).unwrap();
    assert!(r.contains("xml parse -t/--text <string>"), "shell help should use flag syntax: {}", r);
    assert!(r.contains("xml query -d/--doc <JSON> -p/--path <string>"), "shell help should show multi-param flags: {}", r);
    assert!(r.contains("xml encode -t/--tree <JSON>"), "shell help should show encode flags: {}", r);
}

// ── dual-signature (table form) ────────────────────────────────

#[test]
fn parse_table_form() {
    // xml.parse({[1]="<root>hi</root>"}) — shell dispatch form
    let s = sb();
    let r = s
        .exec(r#"local doc = xml.parse({[1]="<root>hi</root>"}); return doc.tag .. " " .. doc.text"#)
        .unwrap();
    assert_eq!(r, "root hi");
}

#[test]
fn parse_file_table_form() {
    // xml.parseFile({[1]="/workspace/doc.xml"}) — shell dispatch form
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::write(dir.path().join("doc.xml"), "<item>hello</item>").unwrap();
    let mut table = MountTable::new();
    table
        .parse_and_add(&format!("{}:/workspace", dir.path().display()))
        .unwrap();
    let s = Sandbox::with_mounts(table).unwrap();
    let r = s
        .exec(r#"local doc = xml.parseFile({[1]="/workspace/doc.xml"}); return doc.text"#)
        .unwrap();
    assert_eq!(r, "hello");
}

#[test]
fn query_table_form() {
    // xml.query({[1]=doc, [2]="root/item"}) — shell dispatch form
    let s = sb();
    let r = s
        .exec(
            r#"
            local doc = xml.parse("<root><item>a</item><item>b</item></root>")
            local items = xml.query({[1]=doc, [2]="root/item"})
            return #items .. " " .. items[1].text .. " " .. items[2].text
        "#,
        )
        .unwrap();
    assert_eq!(r, "2 a b");
}

#[test]
fn query_ordered_still_works() {
    // Ensure ordered calling convention is not broken
    let s = sb();
    let r = s
        .exec(
            r#"
            local doc = xml.parse("<root><child>yes</child></root>")
            local results = xml.query(doc, "root/child")
            return results[1].text
        "#,
        )
        .unwrap();
    assert_eq!(r, "yes");
}

// ── shell round-trip ───────────────────────────────────────────

#[test]
fn shell_xml_parse_roundtrip() {
    // `xml parse "<root>hi</root>"` via sh.run() should display parsed result as JSON
    let s = sb();
    let shrt = include_str!("../../shrt.luau");
    s.register_module("shrt", shrt).unwrap();
    let result = cpsl_core::sh_transpile::transpile_sh(r#"xml parse "<root>hi</root>""#);
    assert!(result.is_ok());
    let luau = result.unwrap().luau_source;
    let r = s.exec(&luau).unwrap();
    // Should contain the tag and text from the parsed XML (auto-serialized as JSON)
    assert!(r.contains("root"), "got: {}", r);
    assert!(r.contains("hi"), "got: {}", r);
}

#[test]
fn shell_xml_parse_file_roundtrip() {
    // `xml parseFile /workspace/doc.xml` via sh.run()
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::write(dir.path().join("doc.xml"), "<data>test</data>").unwrap();
    let mut table = MountTable::new();
    table
        .parse_and_add(&format!("{}:/workspace", dir.path().display()))
        .unwrap();
    let s = Sandbox::with_mounts(table).unwrap();
    let shrt = include_str!("../../shrt.luau");
    s.register_module("shrt", shrt).unwrap();
    let result = cpsl_core::sh_transpile::transpile_sh(r#"xml parseFile /workspace/doc.xml"#);
    assert!(result.is_ok());
    let luau = result.unwrap().luau_source;
    let r = s.exec(&luau).unwrap();
    assert!(r.contains("data"), "got: {}", r);
    assert!(r.contains("test"), "got: {}", r);
}

// ── Python transpiler e2e ──────────────────────────────────────

#[test]
fn python_lxml_import_maps_to_xml() {
    let py_code = r#"
from lxml import etree
doc = xml.parse("<root><a>1</a></root>")
print(doc.tag)
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    assert!(
        transpiled.luau_source.contains("xml"),
        "transpiled: {}",
        transpiled.luau_source
    );
}

#[test]
fn python_bs4_import_maps_to_xml() {
    let py_code = r#"
from bs4 import BeautifulSoup
doc = xml.parse("<root>hello</root>")
print(xml.text(doc))
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    assert!(
        transpiled.luau_source.contains("xml"),
        "transpiled: {}",
        transpiled.luau_source
    );
}

#[test]
fn python_xml_parse_e2e() {
    let s = sb();
    let pyrt = include_str!("../../pyrt.luau");
    s.setup_python_runtime(pyrt).unwrap();

    let py_code = r#"
import xml
doc = xml.parse("<root><item>hello</item></root>")
print(xml.text(doc))
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    let r = s.exec(&transpiled.luau_source).unwrap();
    assert!(r.contains("hello"), "got: {}", r);
}

#[test]
fn python_xml_encode_e2e() {
    let s = sb();
    let pyrt = include_str!("../../pyrt.luau");
    s.setup_python_runtime(pyrt).unwrap();

    let py_code = r#"
import xml
doc = xml.parse("<root>test</root>")
result = xml.encode(doc)
print(result)
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    let r = s.exec(&transpiled.luau_source).unwrap();
    assert!(r.contains("<root>"), "got: {}", r);
    assert!(r.contains("test"), "got: {}", r);
}
