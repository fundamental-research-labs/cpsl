#![cfg(feature = "mod-html")]

use cpsl_core::{transpile, Sandbox};

fn sb() -> Sandbox {
    Sandbox::new().unwrap()
}

// ── 1. Parse and select basic elements ──────────────────────────

#[test]
fn parse_and_select_basic() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local doc = html.parse("<html><body><p>Hello</p><p>World</p></body></html>")
            local items = html.select(doc, "p")
            return tostring(#items)
        "#,
        )
        .unwrap();
    assert_eq!(r, "2");
}

#[test]
fn select_returns_tag_text_html() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local doc = html.parse("<div><span class='hi'>Hello</span></div>")
            local items = html.select(doc, "span")
            local el = items[1]
            return el.tag .. "|" .. el.text .. "|" .. el.html
        "#,
        )
        .unwrap();
    assert_eq!(r, "span|Hello|Hello");
}

// ── 2. Complex CSS selectors ────────────────────────────────────

#[test]
fn select_by_class() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local doc = html.parse("<div><p class='intro'>A</p><p>B</p></div>")
            local items = html.select(doc, ".intro")
            return items[1].text
        "#,
        )
        .unwrap();
    assert_eq!(r, "A");
}

#[test]
fn select_by_id() {
    let s = sb();
    let r = s
        .exec(
            r##"
            local doc = html.parse("<div><p id='main'>Content</p><p>Other</p></div>")
            local items = html.select(doc, "#main")
            return items[1].text
        "##,
        )
        .unwrap();
    assert_eq!(r, "Content");
}

#[test]
fn select_nested() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local doc = html.parse("<div><ul><li>One</li><li>Two</li></ul></div>")
            local items = html.select(doc, "div ul li")
            return tostring(#items)
        "#,
        )
        .unwrap();
    assert_eq!(r, "2");
}

#[test]
fn select_attribute_selector() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local doc = html.parse('<div><a href="http://example.com">Link</a><a>NoHref</a></div>')
            local items = html.select(doc, "a[href]")
            return tostring(#items) .. "|" .. items[1].text
        "#,
        )
        .unwrap();
    assert_eq!(r, "1|Link");
}

// ── 3. select_one tests ─────────────────────────────────────────

#[test]
fn select_one_returns_first_match() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local doc = html.parse("<ul><li>First</li><li>Second</li></ul>")
            local el = html.select_one(doc, "li")
            return el.text
        "#,
        )
        .unwrap();
    assert_eq!(r, "First");
}

#[test]
fn select_one_returns_nil_for_no_match() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local doc = html.parse("<div>Hello</div>")
            local el = html.select_one(doc, "span")
            return tostring(el)
        "#,
        )
        .unwrap();
    assert_eq!(r, "nil");
}

// ── 4. text extraction ──────────────────────────────────────────

#[test]
fn text_from_document() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local doc = html.parse("<div>Hello <span>World</span></div>")
            local t = html.text(doc)
            return t
        "#,
        )
        .unwrap();
    assert!(r.contains("Hello"), "text: {}", r);
    assert!(r.contains("World"), "text: {}", r);
}

#[test]
fn text_from_element_table() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local doc = html.parse("<p>Some <b>bold</b> text</p>")
            local el = html.select_one(doc, "p")
            return html.text(el)
        "#,
        )
        .unwrap();
    assert_eq!(r, "Some bold text");
}

#[test]
fn text_from_raw_html_string() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local t = html.text("<p>Hello <b>World</b></p>")
            return t
        "#,
        )
        .unwrap();
    assert!(r.contains("Hello"), "text: {}", r);
    assert!(r.contains("World"), "text: {}", r);
}

// ── 5. attr extraction ──────────────────────────────────────────

#[test]
fn attr_returns_value() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local doc = html.parse('<a href="http://example.com" class="link">Go</a>')
            local el = html.select_one(doc, "a")
            return html.attr(el, "href")
        "#,
        )
        .unwrap();
    assert_eq!(r, "http://example.com");
}

#[test]
fn attr_returns_nil_for_missing() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local doc = html.parse('<a href="http://example.com">Go</a>')
            local el = html.select_one(doc, "a")
            return tostring(html.attr(el, "title"))
        "#,
        )
        .unwrap();
    assert_eq!(r, "nil");
}

#[test]
fn attr_class_value() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local doc = html.parse('<div class="main container">Hi</div>')
            local el = html.select_one(doc, "div")
            return html.attr(el, "class")
        "#,
        )
        .unwrap();
    assert_eq!(r, "main container");
}

// ── 6. inner_html and outer_html ────────────────────────────────

#[test]
fn inner_html_from_element() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local doc = html.parse("<div><b>Bold</b> text</div>")
            local el = html.select_one(doc, "div")
            return html.inner_html(el)
        "#,
        )
        .unwrap();
    assert_eq!(r, "<b>Bold</b> text");
}

#[test]
fn outer_html_from_element() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local doc = html.parse('<p class="x">Hello</p>')
            local el = html.select_one(doc, "p")
            return html.outer_html(el)
        "#,
        )
        .unwrap();
    assert!(r.contains("<p"), "outer: {}", r);
    assert!(r.contains("Hello"), "outer: {}", r);
    assert!(r.contains("</p>"), "outer: {}", r);
    assert!(r.contains("class"), "outer: {}", r);
}

#[test]
fn inner_html_from_document() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local doc = html.parse("<p>Hello</p>")
            local ih = html.inner_html(doc)
            return tostring(type(ih))
        "#,
        )
        .unwrap();
    assert_eq!(r, "string");
}

// ── 7. String shortcut (pass HTML string directly) ──────────────

#[test]
fn select_with_raw_html_string() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local items = html.select("<ul><li>A</li><li>B</li></ul>", "li")
            return tostring(#items)
        "#,
        )
        .unwrap();
    assert_eq!(r, "2");
}

#[test]
fn select_one_with_raw_html_string() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local el = html.select_one("<p>Hello</p><p>World</p>", "p")
            return el.text
        "#,
        )
        .unwrap();
    assert_eq!(r, "Hello");
}

// ── 8. Empty document / no matches ──────────────────────────────

#[test]
fn select_empty_document() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local doc = html.parse("")
            local items = html.select(doc, "p")
            return tostring(#items)
        "#,
        )
        .unwrap();
    assert_eq!(r, "0");
}

#[test]
fn select_no_matches() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local doc = html.parse("<div>Hello</div>")
            local items = html.select(doc, "span.missing")
            return tostring(#items)
        "#,
        )
        .unwrap();
    assert_eq!(r, "0");
}

// ── 9. Malformed HTML (html5ever is forgiving) ──────────────────

#[test]
fn malformed_html_unclosed_tags() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local doc = html.parse("<div><p>Unclosed paragraph<p>Another")
            local items = html.select(doc, "p")
            return tostring(#items)
        "#,
        )
        .unwrap();
    // html5ever should recover and produce 2 paragraphs
    assert_eq!(r, "2");
}

#[test]
fn malformed_html_no_html_tag() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local doc = html.parse("Just plain text")
            local t = html.text(doc)
            return t
        "#,
        )
        .unwrap();
    assert!(r.contains("Just plain text"), "text: {}", r);
}

// ── 10. Table form (shell dispatch) calling convention ──────────

#[test]
fn select_table_form_positional() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local doc = html.parse("<p>Hello</p>")
            local items = html.select({[1]=doc, [2]="p"})
            return items[1].text
        "#,
        )
        .unwrap();
    assert_eq!(r, "Hello");
}

#[test]
fn select_table_form_named() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local doc = html.parse("<p>Hello</p>")
            local items = html.select({doc=doc, selector="p"})
            return items[1].text
        "#,
        )
        .unwrap();
    assert_eq!(r, "Hello");
}

#[test]
fn attr_table_form() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local doc = html.parse('<a href="http://test.com">Link</a>')
            local el = html.select_one(doc, "a")
            return html.attr({element=el, name="href"})
        "#,
        )
        .unwrap();
    assert_eq!(r, "http://test.com");
}

// ── 11. Help function tests ─────────────────────────────────────

#[test]
fn html_help_returns_help_text() {
    let s = sb();
    let r = s.exec("return html.help()").unwrap();
    assert!(r.contains("html"), "help: {}", r);
    assert!(r.contains("html.parse"), "help: {}", r);
    assert!(r.contains("html.select"), "help: {}", r);
    assert!(r.contains("html.select_one"), "help: {}", r);
    assert!(r.contains("html.text"), "help: {}", r);
    assert!(r.contains("html.attr"), "help: {}", r);
    assert!(r.contains("html.inner_html"), "help: {}", r);
    assert!(r.contains("html.outer_html"), "help: {}", r);
}

#[test]
fn html_nonexistent_fn_hint() {
    let s = sb();
    let err = s.exec("html.foo()").unwrap_err();
    assert!(
        err.message.contains("html.foo does not exist"),
        "msg: {}",
        err.message
    );
    assert!(
        err.message.contains("hint: call html.help() for usage"),
        "msg: {}",
        err.message
    );
}

#[test]
fn global_help_mentions_html() {
    let s = sb();
    let r = s.exec("return help()").unwrap();
    assert!(r.contains("html"), "global help should list html: {}", r);
}

// ── 12. Error handling ──────────────────────────────────────────

#[test]
fn parse_no_args_errors() {
    let s = sb();
    let err = s.exec("html.parse()").unwrap_err();
    assert!(
        err.message.contains("bad argument") || err.message.contains("missing"),
        "msg: {}",
        err.message
    );
}

#[test]
fn select_no_args_errors() {
    let s = sb();
    let err = s.exec("html.select()").unwrap_err();
    assert!(
        err.message.contains("bad argument") || err.message.contains("missing"),
        "msg: {}",
        err.message
    );
}

#[test]
fn select_invalid_selector_errors() {
    let s = sb();
    let err = s
        .exec(
            r#"
            local doc = html.parse("<div>Hello</div>")
            html.select(doc, ":::")
        "#,
        )
        .unwrap_err();
    assert!(
        err.message.contains("invalid CSS selector"),
        "msg: {}",
        err.message
    );
}

#[test]
fn attr_wrong_type_errors() {
    let s = sb();
    let err = s.exec(r#"html.attr(42, "href")"#).unwrap_err();
    assert!(
        err.message.contains("table") || err.message.contains("expected"),
        "msg: {}",
        err.message
    );
}

#[test]
fn parse_wrong_type_errors() {
    let s = sb();
    let err = s.exec("html.parse(42)").unwrap_err();
    assert!(
        err.message.contains("string") || err.message.contains("expected"),
        "msg: {}",
        err.message
    );
}

// ── 13. Document handle tostring ────────────────────────────────

#[test]
fn document_handle_tostring() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local doc = html.parse("<p>Hello World</p>")
            return tostring(doc)
        "#,
        )
        .unwrap();
    assert!(r.contains("HtmlDocument"), "tostring: {}", r);
}

// ── 14. Python transpiler tests ─────────────────────────────────

#[test]
fn python_import_bs4_maps_to_html() {
    let py_code = r#"
import bs4
result = bs4.select("<p>Hi</p>", "p")
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    assert!(
        transpiled.luau_source.contains("html"),
        "transpiled: {}",
        transpiled.luau_source
    );
}

#[test]
fn python_import_selectolax_maps_to_html() {
    let py_code = r#"
import selectolax
result = selectolax.parse("<div>Hi</div>")
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    assert!(
        transpiled.luau_source.contains("html"),
        "transpiled: {}",
        transpiled.luau_source
    );
}

#[test]
fn python_bs4_select_e2e() {
    let s = sb();
    let pyrt = include_str!("../../runtime/pyrt.luau");
    s.setup_python_runtime(pyrt).unwrap();

    let py_code = r#"
import bs4
items = bs4.select("<ul><li>A</li><li>B</li></ul>", "li")
print(len(items))
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    let r = s.exec(&transpiled.luau_source).unwrap();
    assert_eq!(r, "2");
}

#[test]
fn python_html_parse_and_select_e2e() {
    let s = sb();
    let pyrt = include_str!("../../runtime/pyrt.luau");
    s.setup_python_runtime(pyrt).unwrap();

    let py_code = r#"
import html
doc = html.parse("<div><p>Hello</p></div>")
el = html.select_one(doc, "p")
print(el.text)
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    let r = s.exec(&transpiled.luau_source).unwrap();
    assert_eq!(r, "Hello");
}

// ── 15. Shell dispatch tests ────────────────────────────────────

#[test]
fn shell_html_select() {
    let s = sb();
    let shrt = include_str!("../../runtime/shrt.luau");
    s.register_module("shrt", shrt).unwrap();
    let result =
        cpsl_core::sh_transpile::transpile_sh(r#"html select "<ul><li>Item</li></ul>" "li""#);
    assert!(result.is_ok(), "transpile err: {:?}", result.err());
    let luau = result.unwrap().luau_source;
    let r = s.exec(&luau).unwrap();
    assert!(
        r.contains("Item"),
        "expected select result containing Item, got: {}",
        r
    );
}

#[test]
fn shell_html_text() {
    let s = sb();
    let shrt = include_str!("../../runtime/shrt.luau");
    s.register_module("shrt", shrt).unwrap();
    let result = cpsl_core::sh_transpile::transpile_sh(r#"html text "<p>Hello World</p>""#);
    assert!(result.is_ok(), "transpile err: {:?}", result.err());
    let luau = result.unwrap().luau_source;
    let r = s.exec(&luau).unwrap();
    assert!(
        r.contains("Hello World"),
        "expected text containing Hello World, got: {}",
        r
    );
}

// ── 16. Sandbox safety tests ────────────────────────────────────

#[test]
fn html_no_dangerous_globals_exposed() {
    let s = sb();
    let r = s
        .exec(
            r#"
            return tostring(type(html.parse)) .. " " ..
                   tostring(type(html.select)) .. " " ..
                   tostring(type(html.text)) .. " " ..
                   tostring(rawget(html, "io")) .. " " ..
                   tostring(rawget(html, "os"))
        "#,
        )
        .unwrap();
    assert_eq!(r, "function function function nil nil");
}

#[test]
fn html_is_purely_computational() {
    let s = sb();
    let r = s
        .exec(
            r#"
            -- All html operations should work without any fs/network
            local doc = html.parse("<div><p class='a'>X</p><p>Y</p></div>")
            local items = html.select(doc, "p")
            local count = tostring(#items)
            local first = items[1].text
            local el = html.select_one(doc, ".a")
            local attr_val = html.attr(el, "class")
            local t = html.text(doc)
            return count .. "|" .. first .. "|" .. attr_val
        "#,
        )
        .unwrap();
    assert_eq!(r, "2|X|a");
}

// ── 17. Attrs table test ────────────────────────────────────────

#[test]
fn element_attrs_table() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local doc = html.parse('<input type="text" name="field" value="hello" />')
            local el = html.select_one(doc, "input")
            return el.attrs.type .. "|" .. el.attrs.name .. "|" .. el.attrs.value
        "#,
        )
        .unwrap();
    assert_eq!(r, "text|field|hello");
}

// ── 18. Multiple elements with different content ────────────────

#[test]
fn select_multiple_elements_content() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local doc = html.parse("<div><p>First</p><p>Second</p><p>Third</p></div>")
            local items = html.select(doc, "p")
            local parts = {}
            for _, el in ipairs(items) do
                table.insert(parts, el.text)
            end
            return table.concat(parts, ",")
        "#,
        )
        .unwrap();
    assert_eq!(r, "First,Second,Third");
}

// ── 19. No parse_file function exposed ──────────────────────────

#[test]
fn no_parse_file_exposed() {
    let s = sb();
    let r = s
        .exec(r#"return tostring(rawget(html, "parse_file"))"#)
        .unwrap();
    assert_eq!(r, "nil");
}

// ── 20. Chained operations ──────────────────────────────────────

#[test]
fn chained_parse_select_attr() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local doc = html.parse('<nav><a href="/home">Home</a><a href="/about">About</a></nav>')
            local links = html.select(doc, "a")
            local parts = {}
            for _, link in ipairs(links) do
                table.insert(parts, html.attr(link, "href") .. ":" .. link.text)
            end
            return table.concat(parts, ",")
        "#,
        )
        .unwrap();
    assert_eq!(r, "/home:Home,/about:About");
}
