//! Lua-facing browser automation module backed by an injectable native gateway.

use crate::lua_util::{is_lua_array, register_help_functions, value_type_name};
use crate::mount::MountTable;
use crate::sandbox::{
    arg_error, wrap_module_with_help_hints, FieldDoc, FnDoc, ModuleDoc, Param, ParamType,
    ReturnType,
};
use mlua::{Lua, MultiValue, Table, Value};
use serde_json::{Map, Number};
use std::path::Path;
use std::sync::Arc;

#[path = "webbrowser_upload.rs"]
mod webbrowser_upload;
use webbrowser_upload::{
    parse_arm_upload_args, parse_upload_args, resolve_upload_paths, validate_control_key,
};

#[cfg(test)]
#[path = "webbrowser_tests.rs"]
mod tests;

const DEFAULT_RESOURCE_MODE: &str = "lean";
const DEFAULT_WINDOW_WIDTH: i64 = 1200;
const DEFAULT_WINDOW_HEIGHT: i64 = 900;

pub trait WebBrowserGateway: Send + Sync {
    fn handle_json(&self, request_json: &str) -> Result<String, String>;
}

impl<F> WebBrowserGateway for F
where
    F: Fn(&str) -> Result<String, String> + Send + Sync,
{
    fn handle_json(&self, request_json: &str) -> Result<String, String> {
        self(request_json)
    }
}

const CREATE_OPTS_FIELDS: &[FieldDoc] = &[FieldDoc {
    name: "resource_mode",
    typ: "string",
    required: false,
    description: "Resource loading mode: \"lean\" (default) or \"full\"",
}];

const OPEN_OPTS_FIELDS: &[FieldDoc] = &[
    FieldDoc {
        name: "browser",
        typ: "string",
        required: false,
        description: "Browser id to reuse",
    },
    FieldDoc {
        name: "resource_mode",
        typ: "string",
        required: false,
        description: "Resource loading mode: \"lean\" (default) or \"full\"",
    },
    FieldDoc {
        name: "wait_resources",
        typ: "boolean",
        required: false,
        description: "Wait for network resources to become quiet",
    },
    FieldDoc {
        name: "resource_timeout",
        typ: "number",
        required: false,
        description: "Maximum resource wait time in seconds",
    },
];

const PAGE_OPTS_FIELDS: &[FieldDoc] = &[
    FieldDoc {
        name: "fields",
        typ: "string|table",
        required: false,
        description: "Requested page fields; host may use this to trim output",
    },
    FieldDoc {
        name: "selectors",
        typ: "boolean",
        required: false,
        description: "Include action selectors when supported",
    },
    FieldDoc {
        name: "action_details",
        typ: "boolean",
        required: false,
        description: "Include detailed action metadata when supported",
    },
    FieldDoc {
        name: "wait_resources",
        typ: "boolean",
        required: false,
        description: "Wait for resources before snapshotting",
    },
    FieldDoc {
        name: "resource_timeout",
        typ: "number",
        required: false,
        description: "Maximum resource wait time in seconds",
    },
];

const WAIT_OPTS_FIELDS: &[FieldDoc] = &[FieldDoc {
    name: "resource_timeout",
    typ: "number",
    required: false,
    description: "Maximum resource wait time in seconds",
}];

const TYPE_OPTS_FIELDS: &[FieldDoc] = &[
    FieldDoc {
        name: "backend",
        typ: "string",
        required: false,
        description: "Typing backend: \"auto\" (default), \"native\", or \"js\"; typing.backendUsed reports native, nativeTextInput, or js",
    },
    FieldDoc {
        name: "rhythm",
        typ: "string",
        required: false,
        description: "Typing rhythm: \"natural\" (default) or \"flat\"",
    },
    FieldDoc {
        name: "speed",
        typ: "number",
        required: false,
        description: "Typing speed factor; wb defaults to 4.0",
    },
    FieldDoc {
        name: "delay_min",
        typ: "number",
        required: false,
        description: "Minimum per-character delay in seconds",
    },
    FieldDoc {
        name: "delay_max",
        typ: "number",
        required: false,
        description: "Maximum per-character delay in seconds",
    },
];

const EVAL_OPTS_FIELDS: &[FieldDoc] = &[FieldDoc {
    name: "function_body",
    typ: "boolean",
    required: false,
    description: "Treat script as a function body",
}];

const SCREENSHOT_OPTS_FIELDS: &[FieldDoc] = &[
    FieldDoc {
        name: "resource_timeout",
        typ: "number",
        required: false,
        description: "Maximum resource wait time in seconds",
    },
    FieldDoc {
        name: "capture_delay",
        typ: "number",
        required: false,
        description: "Delay after resource wait before capture",
    },
];

pub(crate) static WEBBROWSER_DOC: ModuleDoc = ModuleDoc {
    name: "webbrowser",
    summary: "native WebKit automation for browsing, authenticated site interaction, and file transfers",
    functions: &[
        FnDoc {
            name: "click",
            description: "Click an action id or a viewport coordinate.",
            params: &[
                Param {
                    name: "browser",
                    short: Some('b'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "target",
                    short: Some('t'),
                    typ: ParamType::Value,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "y",
                    short: Some('y'),
                    typ: ParamType::Number,
                    required: false,
                    fields: None,
                },
            ],
            returns: ReturnType::Table,
            example: Some(r#"webbrowser.click(browser, "a1")"#),
        },
        FnDoc {
            name: "create",
            description: "Create an empty browser. CPSL defaults to lean resource loading.",
            params: &[Param {
                name: "opts",
                short: None,
                typ: ParamType::Table,
                required: false,
                fields: Some(CREATE_OPTS_FIELDS),
            }],
            returns: ReturnType::Table,
            example: Some(r#"local browser = webbrowser.create({resource_mode="lean"}).browser"#),
        },
        FnDoc {
            name: "drag",
            description: "Send a mouse drag event to a viewport coordinate.",
            params: coordinate_params(),
            returns: ReturnType::Table,
            example: Some(r#"webbrowser.drag(browser, 300, 420)"#),
        },
        FnDoc {
            name: "eval",
            description: "Evaluate JavaScript in a browser. Returns a table; read the JavaScript result from the value field.",
            params: &[
                Param {
                    name: "browser",
                    short: Some('b'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "script",
                    short: Some('s'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "opts",
                    short: None,
                    typ: ParamType::Table,
                    required: false,
                    fields: Some(EVAL_OPTS_FIELDS),
                },
            ],
            returns: ReturnType::Table,
            example: Some(
                r#"local title = webbrowser.eval(browser, "return document.title", {function_body=true}).value"#,
            ),
        },
        FnDoc {
            name: "fill",
            description: "Set an input action to text without natural typing delays.",
            params: text_action_params(),
            returns: ReturnType::Table,
            example: Some(r#"webbrowser.fill(browser, "a3", "search terms")"#),
        },
        FnDoc {
            name: "hide",
            description: "Hide a browser window without closing the browser.",
            params: browser_only_params(),
            returns: ReturnType::Table,
            example: Some(r#"webbrowser.hide(browser)"#),
        },
        FnDoc {
            name: "list",
            description: "List active and saved browsers.",
            params: &[],
            returns: ReturnType::Table,
            example: Some(r#"local browsers = webbrowser.list().browsers"#),
        },
        FnDoc {
            name: "open",
            description: "Open a URL in a browser. CPSL defaults to lean resource loading.",
            params: &[
                Param {
                    name: "url",
                    short: Some('u'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "opts",
                    short: None,
                    typ: ParamType::Table,
                    required: false,
                    fields: Some(OPEN_OPTS_FIELDS),
                },
            ],
            returns: ReturnType::Table,
            example: Some(r#"local browser = webbrowser.open("https://example.com").browser"#),
        },
        FnDoc {
            name: "page",
            description: "Return page text, metadata, resources, and available actions.",
            params: &[
                Param {
                    name: "browser",
                    short: Some('b'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "opts",
                    short: None,
                    typ: ParamType::Table,
                    required: false,
                    fields: Some(PAGE_OPTS_FIELDS),
                },
            ],
            returns: ReturnType::Table,
            example: Some(
                r#"local page = webbrowser.page(browser, {fields={"title","url","text"}})"#,
            ),
        },
        FnDoc {
            name: "press",
            description: "Send a mouse down event to a viewport coordinate.",
            params: coordinate_params(),
            returns: ReturnType::Table,
            example: Some(r#"webbrowser.press(browser, 300, 420)"#),
        },
        FnDoc {
            name: "release",
            description: "Send a mouse up event to a viewport coordinate.",
            params: coordinate_params(),
            returns: ReturnType::Table,
            example: Some(r#"webbrowser.release(browser, 300, 420)"#),
        },
        FnDoc {
            name: "remove",
            description: "Remove active browsers and saved sessions.",
            params: &[Param {
                name: "browser",
                short: Some('b'),
                typ: ParamType::Value,
                required: true,
                fields: None,
            }],
            returns: ReturnType::Table,
            example: Some(r#"webbrowser.remove({browsers={browser}})"#),
        },
        FnDoc {
            name: "resize",
            description: "Resize the browser automation viewport.",
            params: &[
                Param {
                    name: "browser",
                    short: Some('b'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "width",
                    short: Some('w'),
                    typ: ParamType::Number,
                    required: false,
                    fields: None,
                },
                Param {
                    name: "height",
                    short: Some('h'),
                    typ: ParamType::Number,
                    required: false,
                    fields: None,
                },
            ],
            returns: ReturnType::Table,
            example: Some(r#"webbrowser.resize(browser, 1200, 900)"#),
        },
        FnDoc {
            name: "screenshot",
            description: "Capture the browser viewport to a sandbox writable path.",
            params: &[
                Param {
                    name: "browser",
                    short: Some('b'),
                    typ: ParamType::String,
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
                Param {
                    name: "opts",
                    short: None,
                    typ: ParamType::Table,
                    required: false,
                    fields: Some(SCREENSHOT_OPTS_FIELDS),
                },
            ],
            returns: ReturnType::Table,
            example: Some(r#"webbrowser.screenshot(browser, "/tmp/page.png")"#),
        },
        FnDoc {
            name: "scroll",
            description: "Scroll at a viewport coordinate.",
            params: &[
                Param {
                    name: "browser",
                    short: Some('b'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "x",
                    short: Some('x'),
                    typ: ParamType::Number,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "y",
                    short: Some('y'),
                    typ: ParamType::Number,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "delta_x",
                    short: None,
                    typ: ParamType::Number,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "delta_y",
                    short: None,
                    typ: ParamType::Number,
                    required: true,
                    fields: None,
                },
            ],
            returns: ReturnType::Table,
            example: Some(r#"webbrowser.scroll(browser, 400, 500, 0, 600)"#),
        },
        FnDoc {
            name: "show",
            description:
                "Show the browser UI. Lean browsers should be promoted to full mode by the host.",
            params: browser_only_params(),
            returns: ReturnType::Table,
            example: Some(r#"webbrowser.show(browser)"#),
        },
        FnDoc {
            name: "submit",
            description: "Submit a form action.",
            params: &[
                Param {
                    name: "browser",
                    short: Some('b'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "action",
                    short: Some('a'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
            ],
            returns: ReturnType::Table,
            example: Some(r#"webbrowser.submit(browser, "a4")"#),
        },
        FnDoc {
            name: "type",
            description: "Focus an action and append text with natural pacing; native host text input is used when available.",
            params: &[
                Param {
                    name: "browser",
                    short: Some('b'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "action",
                    short: Some('a'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "text",
                    short: Some('t'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "opts",
                    short: None,
                    typ: ParamType::Table,
                    required: false,
                    fields: Some(TYPE_OPTS_FIELDS),
                },
            ],
            returns: ReturnType::Table,
            example: Some(r#"webbrowser.type(browser, "a2", "hello", {speed=4.0})"#),
        },
        FnDoc {
            name: "key_press",
            description: "Send one supported control key to an action without changing its text first. Use exact names such as Enter or Escape, not modifier combinations. The returned keyPress.pageConsumed flag reports whether a DOM handler prevented the key's default behavior.",
            params: &[
                Param {
                    name: "browser",
                    short: Some('b'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "action",
                    short: Some('a'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "key",
                    short: Some('k'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
            ],
            returns: ReturnType::Table,
            example: Some(r#"webbrowser.key_press(browser, "a2", "Enter")"#),
        },
        FnDoc {
            name: "arm_upload",
            description: "Advanced fallback: arm sandbox files for the next native file chooser; prefer upload() when the final chooser action is discoverable.",
            params: &[
                Param {
                    name: "browser",
                    short: Some('b'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "paths",
                    short: Some('p'),
                    typ: ParamType::Value,
                    required: true,
                    fields: None,
                },
            ],
            returns: ReturnType::Table,
            example: Some(
                r#"webbrowser.arm_upload(browser, {"/attachments/conversation/photo.jpg"})"#,
            ),
        },
        FnDoc {
            name: "upload",
            description: "Atomically click a chooser-opening action and provide sandbox files; success confirms WebKit consumed the selection.",
            params: &[
                Param {
                    name: "browser",
                    short: Some('b'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "action",
                    short: Some('a'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "paths",
                    short: Some('p'),
                    typ: ParamType::Value,
                    required: true,
                    fields: None,
                },
            ],
            returns: ReturnType::Table,
            example: Some(
                r#"webbrowser.upload(browser, "a3", {"/attachments/conversation/photo.jpg"})"#,
            ),
        },
        FnDoc {
            name: "wait_resources",
            description: "Wait for resources to become quiet and return a page summary.",
            params: &[
                Param {
                    name: "browser",
                    short: Some('b'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "opts",
                    short: None,
                    typ: ParamType::Table,
                    required: false,
                    fields: Some(WAIT_OPTS_FIELDS),
                },
            ],
            returns: ReturnType::Table,
            example: Some(r#"webbrowser.wait_resources(browser, {resource_timeout=3})"#),
        },
    ],
};

const fn browser_only_params() -> &'static [Param] {
    &[Param {
        name: "browser",
        short: Some('b'),
        typ: ParamType::String,
        required: true,
        fields: None,
    }]
}

const fn coordinate_params() -> &'static [Param] {
    &[
        Param {
            name: "browser",
            short: Some('b'),
            typ: ParamType::String,
            required: true,
            fields: None,
        },
        Param {
            name: "x",
            short: Some('x'),
            typ: ParamType::Number,
            required: true,
            fields: None,
        },
        Param {
            name: "y",
            short: Some('y'),
            typ: ParamType::Number,
            required: true,
            fields: None,
        },
    ]
}

const fn text_action_params() -> &'static [Param] {
    &[
        Param {
            name: "browser",
            short: Some('b'),
            typ: ParamType::String,
            required: true,
            fields: None,
        },
        Param {
            name: "action",
            short: Some('a'),
            typ: ParamType::String,
            required: true,
            fields: None,
        },
        Param {
            name: "text",
            short: Some('t'),
            typ: ParamType::String,
            required: true,
            fields: None,
        },
    ]
}

pub(crate) fn register_webbrowser_globals(
    lua: &Lua,
    gateway: Arc<dyn WebBrowserGateway>,
    mounts: Arc<MountTable>,
) -> Result<(), mlua::Error> {
    let webbrowser = lua.create_table()?;

    {
        let gateway = gateway.clone();
        webbrowser.set(
            "create",
            lua.create_function(move |lua, args: MultiValue| {
                let opts = optional_only_table(&args, "webbrowser.create")?;
                let mut request = request("browserCreate");
                set_resource_mode(&mut request, opts.as_ref(), true)?;
                dispatch(lua, &*gateway, request)
            })?,
        )?;
    }

    {
        let gateway = gateway.clone();
        webbrowser.set(
            "open",
            lua.create_function(move |lua, args: MultiValue| {
                let (browser, url, opts) = parse_open_args(&args)?;
                let mut request = request("open");
                set_optional_string_field(&mut request, "browser", browser);
                request.insert("url".to_string(), serde_json::Value::String(url));
                set_resource_mode(&mut request, opts.as_ref(), true)?;
                set_resource_loading(&mut request, opts.as_ref(), false)?;
                dispatch(lua, &*gateway, request)
            })?,
        )?;
    }

    {
        let gateway = gateway.clone();
        webbrowser.set(
            "list",
            lua.create_function(move |lua, args: MultiValue| {
                ensure_no_args(&args, "webbrowser.list")?;
                dispatch(lua, &*gateway, request("browserList"))
            })?,
        )?;
    }

    {
        let gateway = gateway.clone();
        webbrowser.set(
            "remove",
            lua.create_function(move |lua, args: MultiValue| {
                let mut request = request("browserRemove");
                set_remove_target(&mut request, &args)?;
                dispatch(lua, &*gateway, request)
            })?,
        )?;
    }

    register_browser_only(lua, &webbrowser, "show", "browserShow", gateway.clone())?;
    register_browser_only(lua, &webbrowser, "hide", "browserHide", gateway.clone())?;

    {
        let gateway = gateway.clone();
        webbrowser.set(
            "resize",
            lua.create_function(move |lua, args: MultiValue| {
                let (browser, width, height) = parse_resize_args(&args)?;
                let mut request = request("browserResize");
                request.insert("browser".to_string(), serde_json::Value::String(browser));
                request.insert(
                    "windowWidth".to_string(),
                    serde_json::Value::Number(width.into()),
                );
                request.insert(
                    "windowHeight".to_string(),
                    serde_json::Value::Number(height.into()),
                );
                dispatch(lua, &*gateway, request)
            })?,
        )?;
    }

    {
        let gateway = gateway.clone();
        webbrowser.set(
            "page",
            lua.create_function(move |lua, args: MultiValue| {
                let (browser, opts) = browser_with_opts(&args, "webbrowser.page")?;
                let mut request = request("page");
                request.insert("browser".to_string(), serde_json::Value::String(browser));
                set_resource_loading(&mut request, opts.as_ref(), false)?;
                set_page_options(&mut request, opts.as_ref())?;
                dispatch(lua, &*gateway, request)
            })?,
        )?;
    }

    {
        let gateway = gateway.clone();
        webbrowser.set(
            "wait_resources",
            lua.create_function(move |lua, args: MultiValue| {
                let (browser, opts) = browser_with_opts(&args, "webbrowser.wait_resources")?;
                let mut request = request("waitResources");
                request.insert("browser".to_string(), serde_json::Value::String(browser));
                set_resource_loading(&mut request, opts.as_ref(), true)?;
                dispatch(lua, &*gateway, request)
            })?,
        )?;
    }

    {
        let gateway = gateway.clone();
        webbrowser.set(
            "click",
            lua.create_function(move |lua, args: MultiValue| {
                let request = parse_click_request(&args)?;
                dispatch(lua, &*gateway, request)
            })?,
        )?;
    }

    register_coordinate(lua, &webbrowser, "press", gateway.clone())?;
    register_coordinate(lua, &webbrowser, "drag", gateway.clone())?;
    register_coordinate(lua, &webbrowser, "release", gateway.clone())?;

    {
        let gateway = gateway.clone();
        webbrowser.set(
            "scroll",
            lua.create_function(move |lua, args: MultiValue| {
                let (browser, x, y, delta_x, delta_y) = parse_scroll_args(&args)?;
                let mut request = coordinate_request("scroll", browser, x, y);
                request.insert(
                    "deltaX".to_string(),
                    number_value(delta_x, "webbrowser.scroll", "delta_x")?,
                );
                request.insert(
                    "deltaY".to_string(),
                    number_value(delta_y, "webbrowser.scroll", "delta_y")?,
                );
                dispatch(lua, &*gateway, request)
            })?,
        )?;
    }

    register_text_action(lua, &webbrowser, "fill", "fill", gateway.clone(), false)?;
    register_text_action(lua, &webbrowser, "type", "type", gateway.clone(), true)?;
    register_text_action(
        lua,
        &webbrowser,
        "key_press",
        "keyPress",
        gateway.clone(),
        false,
    )?;

    {
        let gateway = gateway.clone();
        let mounts = mounts.clone();
        webbrowser.set(
            "arm_upload",
            lua.create_function(move |lua, args: MultiValue| {
                let (browser, paths) = parse_arm_upload_args(&args)?;
                let host_paths = resolve_upload_paths(&mounts, &paths, "webbrowser.arm_upload")?;

                let mut request = request("armUpload");
                request.insert("browser".to_string(), serde_json::Value::String(browser));
                request.insert(
                    "sourcePaths".to_string(),
                    serde_json::Value::Array(host_paths),
                );
                request.insert(
                    "virtualSourcePaths".to_string(),
                    serde_json::Value::Array(
                        paths.into_iter().map(serde_json::Value::String).collect(),
                    ),
                );
                dispatch(lua, &*gateway, request)
            })?,
        )?;
    }

    {
        let gateway = gateway.clone();
        let mounts = mounts.clone();
        webbrowser.set(
            "upload",
            lua.create_function(move |lua, args: MultiValue| {
                let (browser, action, paths) = parse_upload_args(&args)?;
                let host_paths = resolve_upload_paths(&mounts, &paths, "webbrowser.upload")?;

                let mut request = request("upload");
                request.insert("browser".to_string(), serde_json::Value::String(browser));
                request.insert("action".to_string(), serde_json::Value::String(action));
                request.insert(
                    "sourcePaths".to_string(),
                    serde_json::Value::Array(host_paths),
                );
                request.insert(
                    "virtualSourcePaths".to_string(),
                    serde_json::Value::Array(
                        paths.into_iter().map(serde_json::Value::String).collect(),
                    ),
                );
                dispatch(lua, &*gateway, request)
            })?,
        )?;
    }

    {
        let gateway = gateway.clone();
        webbrowser.set(
            "submit",
            lua.create_function(move |lua, args: MultiValue| {
                let (browser, action) = parse_submit_args(&args)?;
                let mut request = request("submit");
                request.insert("browser".to_string(), serde_json::Value::String(browser));
                request.insert("action".to_string(), serde_json::Value::String(action));
                dispatch(lua, &*gateway, request)
            })?,
        )?;
    }

    {
        let gateway = gateway.clone();
        webbrowser.set(
            "eval",
            lua.create_function(move |lua, args: MultiValue| {
                let (browser, script, opts) = parse_eval_args(&args)?;
                let mut request = request("eval");
                request.insert("browser".to_string(), serde_json::Value::String(browser));
                request.insert("script".to_string(), serde_json::Value::String(script));
                if let Some(function_body) = optional_bool(
                    &opts,
                    "webbrowser.eval",
                    &["function_body", "functionBody", "body"],
                )? {
                    request.insert(
                        "functionBody".to_string(),
                        serde_json::Value::Bool(function_body),
                    );
                }
                dispatch(lua, &*gateway, request)
            })?,
        )?;
    }

    {
        let gateway = gateway.clone();
        webbrowser.set(
            "screenshot",
            lua.create_function(move |lua, args: MultiValue| {
                let (browser, path, opts) = parse_screenshot_args(&args)?;
                validate_screenshot_path(&path)?;
                let host_path = mounts
                    .resolve_write_deep(&path)
                    .map_err(mlua::Error::external)?;
                if let Some(parent) = host_path.parent() {
                    std::fs::create_dir_all(parent).map_err(mlua::Error::external)?;
                }

                let mut request = request("screenshot");
                request.insert("browser".to_string(), serde_json::Value::String(browser));
                request.insert(
                    "destinationPath".to_string(),
                    serde_json::Value::String(host_path.to_string_lossy().to_string()),
                );
                request.insert(
                    "virtualDestinationPath".to_string(),
                    serde_json::Value::String(path),
                );
                set_resource_loading(&mut request, opts.as_ref(), true)?;
                if let Some(delay) = optional_number(
                    &opts,
                    "webbrowser.screenshot",
                    &["capture_delay", "screenshotDelay"],
                )? {
                    request.insert(
                        "screenshotDelay".to_string(),
                        number_value(delay, "webbrowser.screenshot", "capture_delay")?,
                    );
                }
                dispatch(lua, &*gateway, request)
            })?,
        )?;
    }

    register_help_functions(lua, &webbrowser, &WEBBROWSER_DOC)?;
    lua.globals().set("webbrowser", webbrowser)?;
    wrap_module_with_help_hints(lua, "webbrowser")?;

    Ok(())
}

fn register_browser_only(
    lua: &Lua,
    webbrowser: &Table,
    lua_name: &'static str,
    command: &'static str,
    gateway: Arc<dyn WebBrowserGateway>,
) -> Result<(), mlua::Error> {
    webbrowser.set(
        lua_name,
        lua.create_function(move |lua, args: MultiValue| {
            let browser = browser_arg(&args, &format!("webbrowser.{lua_name}"))?;
            let mut request = request(command);
            request.insert("browser".to_string(), serde_json::Value::String(browser));
            dispatch(lua, &*gateway, request)
        })?,
    )
}

fn register_coordinate(
    lua: &Lua,
    webbrowser: &Table,
    name: &'static str,
    gateway: Arc<dyn WebBrowserGateway>,
) -> Result<(), mlua::Error> {
    webbrowser.set(
        name,
        lua.create_function(move |lua, args: MultiValue| {
            let (browser, x, y) = parse_coordinate_args(&args, &format!("webbrowser.{name}"))?;
            dispatch(lua, &*gateway, coordinate_request(name, browser, x, y))
        })?,
    )
}

fn register_text_action(
    lua: &Lua,
    webbrowser: &Table,
    lua_name: &'static str,
    command: &'static str,
    gateway: Arc<dyn WebBrowserGateway>,
    include_typing_opts: bool,
) -> Result<(), mlua::Error> {
    webbrowser.set(
        lua_name,
        lua.create_function(move |lua, args: MultiValue| {
            let (browser, action, text, opts) =
                parse_text_action_args(&args, &format!("webbrowser.{lua_name}"))?;
            if command == "keyPress" {
                validate_control_key(&text)?;
            }
            let mut request = request(command);
            request.insert("browser".to_string(), serde_json::Value::String(browser));
            request.insert("action".to_string(), serde_json::Value::String(action));
            request.insert("value".to_string(), serde_json::Value::String(text));
            if include_typing_opts {
                set_typing_options(&mut request, opts.as_ref())?;
            }
            dispatch(lua, &*gateway, request)
        })?,
    )
}

fn request(command: &str) -> Map<String, serde_json::Value> {
    let mut request = Map::new();
    request.insert(
        "source".to_string(),
        serde_json::Value::String("cpsl".to_string()),
    );
    request.insert(
        "command".to_string(),
        serde_json::Value::String(command.to_string()),
    );
    request
}

fn coordinate_request(
    action: &str,
    browser: String,
    x: f64,
    y: f64,
) -> Map<String, serde_json::Value> {
    let mut request = request("coordinate");
    request.insert("browser".to_string(), serde_json::Value::String(browser));
    request.insert(
        "coordinateAction".to_string(),
        serde_json::Value::String(action.to_string()),
    );
    request.insert(
        "x".to_string(),
        number_value(x, "webbrowser.coordinate", "x").expect("finite number already parsed"),
    );
    request.insert(
        "y".to_string(),
        number_value(y, "webbrowser.coordinate", "y").expect("finite number already parsed"),
    );
    request
}

fn dispatch(
    lua: &Lua,
    gateway: &dyn WebBrowserGateway,
    request: Map<String, serde_json::Value>,
) -> Result<Value, mlua::Error> {
    let request_json = serde_json::to_string(&serde_json::Value::Object(request))
        .map_err(mlua::Error::external)?;
    let response_json = gateway
        .handle_json(&request_json)
        .map_err(mlua::Error::external)?;
    let response: serde_json::Value =
        serde_json::from_str(&response_json).map_err(mlua::Error::external)?;
    if let Some(object) = response.as_object() {
        if object.get("ok") == Some(&serde_json::Value::Bool(false)) {
            return Err(mlua::Error::external(response_error_message(object)));
        }
        if let Some(result) = object.get("result") {
            return json_to_lua(lua, result);
        }
    }
    json_to_lua(lua, &response)
}

fn response_error_message(object: &Map<String, serde_json::Value>) -> String {
    if let Some(error) = object.get("error") {
        if let Some(message) = error.as_str() {
            return format!("webbrowser: {message}");
        }
        if let Some(message) = error
            .as_object()
            .and_then(|error| error.get("message"))
            .and_then(serde_json::Value::as_str)
        {
            return format!("webbrowser: {message}");
        }
    }
    if let Some(message) = object.get("message").and_then(serde_json::Value::as_str) {
        return format!("webbrowser: {message}");
    }
    "webbrowser: browser gateway request failed".to_string()
}

fn parse_open_args(
    args: &MultiValue,
) -> Result<(Option<String>, String, Option<Table>), mlua::Error> {
    if let Some(table) = single_table_arg(args) {
        let url = required_string(&table, "webbrowser.open", &["url"])?;
        let browser = optional_string(&Some(table.clone()), "webbrowser.open", &["browser"])?;
        return Ok((browser, url, Some(table)));
    }

    match args.len() {
        1 => Ok((
            None,
            value_string(&args[0], "webbrowser.open", "url")?,
            None,
        )),
        2 => match &args[1] {
            Value::Table(opts) => Ok((
                optional_string(&Some(opts.clone()), "webbrowser.open", &["browser"])?,
                value_string(&args[0], "webbrowser.open", "url")?,
                Some(opts.clone()),
            )),
            _ => Ok((
                Some(value_string(&args[0], "webbrowser.open", "browser")?),
                value_string(&args[1], "webbrowser.open", "url")?,
                None,
            )),
        },
        3 => {
            let opts = value_table(&args[2], "webbrowser.open", "opts")?;
            Ok((
                Some(value_string(&args[0], "webbrowser.open", "browser")?),
                value_string(&args[1], "webbrowser.open", "url")?,
                Some(opts),
            ))
        }
        _ => Err(arg_error("webbrowser.open", WEBBROWSER_DOC.params("open"))),
    }
}

fn parse_resize_args(args: &MultiValue) -> Result<(String, i64, i64), mlua::Error> {
    if let Some(table) = single_table_arg(args) {
        let browser = required_string(&table, "webbrowser.resize", &["browser"])?;
        let width = optional_integer(&Some(table.clone()), "webbrowser.resize", &["width"])?
            .unwrap_or(DEFAULT_WINDOW_WIDTH);
        let height = optional_integer(&Some(table.clone()), "webbrowser.resize", &["height"])?
            .unwrap_or(DEFAULT_WINDOW_HEIGHT);
        return Ok((browser, width, height));
    }

    match args.len() {
        1 => Ok((
            value_string(&args[0], "webbrowser.resize", "browser")?,
            DEFAULT_WINDOW_WIDTH,
            DEFAULT_WINDOW_HEIGHT,
        )),
        3 => Ok((
            value_string(&args[0], "webbrowser.resize", "browser")?,
            value_integer(&args[1], "webbrowser.resize", "width")?,
            value_integer(&args[2], "webbrowser.resize", "height")?,
        )),
        _ => Err(arg_error(
            "webbrowser.resize",
            WEBBROWSER_DOC.params("resize"),
        )),
    }
}

fn parse_click_request(args: &MultiValue) -> Result<Map<String, serde_json::Value>, mlua::Error> {
    if let Some(table) = single_table_arg(args) {
        let browser = required_string(&table, "webbrowser.click", &["browser"])?;
        if let Some(action) = optional_string(
            &Some(table.clone()),
            "webbrowser.click",
            &["action", "target"],
        )? {
            let mut request = request("click");
            request.insert("browser".to_string(), serde_json::Value::String(browser));
            request.insert("action".to_string(), serde_json::Value::String(action));
            return Ok(request);
        }
        let x = required_number(&table, "webbrowser.click", &["x"])?;
        let y = required_number(&table, "webbrowser.click", &["y"])?;
        return Ok(coordinate_request("click", browser, x, y));
    }

    match args.len() {
        2 => {
            let mut request = request("click");
            request.insert(
                "browser".to_string(),
                serde_json::Value::String(value_string(&args[0], "webbrowser.click", "browser")?),
            );
            request.insert(
                "action".to_string(),
                serde_json::Value::String(value_string(&args[1], "webbrowser.click", "target")?),
            );
            Ok(request)
        }
        3 => Ok(coordinate_request(
            "click",
            value_string(&args[0], "webbrowser.click", "browser")?,
            value_number(&args[1], "webbrowser.click", "x")?,
            value_number(&args[2], "webbrowser.click", "y")?,
        )),
        _ => Err(arg_error(
            "webbrowser.click",
            WEBBROWSER_DOC.params("click"),
        )),
    }
}

fn parse_coordinate_args(
    args: &MultiValue,
    fn_name: &str,
) -> Result<(String, f64, f64), mlua::Error> {
    if let Some(table) = single_table_arg(args) {
        return Ok((
            required_string(&table, fn_name, &["browser"])?,
            required_number(&table, fn_name, &["x"])?,
            required_number(&table, fn_name, &["y"])?,
        ));
    }
    if args.len() != 3 {
        return Err(mlua::Error::external(format!(
            "{fn_name}: missing required arguments 'browser' (string), 'x' (number), and 'y' (number)"
        )));
    }
    Ok((
        value_string(&args[0], fn_name, "browser")?,
        value_number(&args[1], fn_name, "x")?,
        value_number(&args[2], fn_name, "y")?,
    ))
}

fn parse_scroll_args(args: &MultiValue) -> Result<(String, f64, f64, f64, f64), mlua::Error> {
    if let Some(table) = single_table_arg(args) {
        return Ok((
            required_string(&table, "webbrowser.scroll", &["browser"])?,
            required_number(&table, "webbrowser.scroll", &["x"])?,
            required_number(&table, "webbrowser.scroll", &["y"])?,
            required_number(&table, "webbrowser.scroll", &["delta_x", "deltaX"])?,
            required_number(&table, "webbrowser.scroll", &["delta_y", "deltaY"])?,
        ));
    }
    if args.len() != 5 {
        return Err(arg_error(
            "webbrowser.scroll",
            WEBBROWSER_DOC.params("scroll"),
        ));
    }
    Ok((
        value_string(&args[0], "webbrowser.scroll", "browser")?,
        value_number(&args[1], "webbrowser.scroll", "x")?,
        value_number(&args[2], "webbrowser.scroll", "y")?,
        value_number(&args[3], "webbrowser.scroll", "delta_x")?,
        value_number(&args[4], "webbrowser.scroll", "delta_y")?,
    ))
}

fn parse_text_action_args(
    args: &MultiValue,
    fn_name: &str,
) -> Result<(String, String, String, Option<Table>), mlua::Error> {
    if let Some(table) = single_table_arg(args) {
        return Ok((
            required_string(&table, fn_name, &["browser"])?,
            required_string(&table, fn_name, &["action"])?,
            required_string(&table, fn_name, &["text", "value"])?,
            Some(table),
        ));
    }

    match args.len() {
        3 => Ok((
            value_string(&args[0], fn_name, "browser")?,
            value_string(&args[1], fn_name, "action")?,
            value_string(&args[2], fn_name, "text")?,
            None,
        )),
        4 => Ok((
            value_string(&args[0], fn_name, "browser")?,
            value_string(&args[1], fn_name, "action")?,
            value_string(&args[2], fn_name, "text")?,
            Some(value_table(&args[3], fn_name, "opts")?),
        )),
        _ => Err(mlua::Error::external(format!(
            "{fn_name}: missing required arguments 'browser' (string), 'action' (string), and 'text' (string)"
        ))),
    }
}

fn parse_submit_args(args: &MultiValue) -> Result<(String, String), mlua::Error> {
    if let Some(table) = single_table_arg(args) {
        return Ok((
            required_string(&table, "webbrowser.submit", &["browser"])?,
            required_string(&table, "webbrowser.submit", &["action"])?,
        ));
    }
    if args.len() != 2 {
        return Err(arg_error(
            "webbrowser.submit",
            WEBBROWSER_DOC.params("submit"),
        ));
    }
    Ok((
        value_string(&args[0], "webbrowser.submit", "browser")?,
        value_string(&args[1], "webbrowser.submit", "action")?,
    ))
}

fn parse_eval_args(args: &MultiValue) -> Result<(String, String, Option<Table>), mlua::Error> {
    if let Some(table) = single_table_arg(args) {
        return Ok((
            required_string(&table, "webbrowser.eval", &["browser"])?,
            required_string(&table, "webbrowser.eval", &["script"])?,
            Some(table),
        ));
    }

    match args.len() {
        2 => Ok((
            value_string(&args[0], "webbrowser.eval", "browser")?,
            value_string(&args[1], "webbrowser.eval", "script")?,
            None,
        )),
        3 => Ok((
            value_string(&args[0], "webbrowser.eval", "browser")?,
            value_string(&args[1], "webbrowser.eval", "script")?,
            Some(value_table(&args[2], "webbrowser.eval", "opts")?),
        )),
        _ => Err(arg_error("webbrowser.eval", WEBBROWSER_DOC.params("eval"))),
    }
}

fn parse_screenshot_args(
    args: &MultiValue,
) -> Result<(String, String, Option<Table>), mlua::Error> {
    if let Some(table) = single_table_arg(args) {
        return Ok((
            required_string(&table, "webbrowser.screenshot", &["browser"])?,
            required_string(
                &table,
                "webbrowser.screenshot",
                &["path", "destination", "destination_path", "destinationPath"],
            )?,
            Some(table),
        ));
    }

    match args.len() {
        2 => Ok((
            value_string(&args[0], "webbrowser.screenshot", "browser")?,
            value_string(&args[1], "webbrowser.screenshot", "path")?,
            None,
        )),
        3 => Ok((
            value_string(&args[0], "webbrowser.screenshot", "browser")?,
            value_string(&args[1], "webbrowser.screenshot", "path")?,
            Some(value_table(&args[2], "webbrowser.screenshot", "opts")?),
        )),
        _ => Err(arg_error(
            "webbrowser.screenshot",
            WEBBROWSER_DOC.params("screenshot"),
        )),
    }
}

fn browser_arg(args: &MultiValue, fn_name: &str) -> Result<String, mlua::Error> {
    if let Some(table) = single_table_arg(args) {
        return required_string(&table, fn_name, &["browser"]);
    }
    if args.len() != 1 {
        return Err(mlua::Error::external(format!(
            "{fn_name}: missing required argument 'browser' (string)"
        )));
    }
    value_string(&args[0], fn_name, "browser")
}

fn browser_with_opts(
    args: &MultiValue,
    fn_name: &str,
) -> Result<(String, Option<Table>), mlua::Error> {
    if let Some(table) = single_table_arg(args) {
        return Ok((required_string(&table, fn_name, &["browser"])?, Some(table)));
    }
    match args.len() {
        1 => Ok((value_string(&args[0], fn_name, "browser")?, None)),
        2 => Ok((
            value_string(&args[0], fn_name, "browser")?,
            Some(value_table(&args[1], fn_name, "opts")?),
        )),
        _ => Err(mlua::Error::external(format!(
            "{fn_name}: missing required argument 'browser' (string)"
        ))),
    }
}

fn set_remove_target(
    request: &mut Map<String, serde_json::Value>,
    args: &MultiValue,
) -> Result<(), mlua::Error> {
    if args.is_empty() {
        return Err(arg_error(
            "webbrowser.remove",
            WEBBROWSER_DOC.params("remove"),
        ));
    }
    if let Some(table) = single_table_arg(args) {
        if optional_bool(
            &Some(table.clone()),
            "webbrowser.remove",
            &["all", "all_browsers", "allBrowsers"],
        )?
        .unwrap_or(false)
        {
            request.insert("allBrowsers".to_string(), serde_json::Value::Bool(true));
            return Ok(());
        }
        if let Some(browser) =
            optional_string(&Some(table.clone()), "webbrowser.remove", &["browser"])?
        {
            request.insert(
                "browsers".to_string(),
                serde_json::Value::Array(vec![serde_json::Value::String(browser)]),
            );
            return Ok(());
        }
        let browsers_value = table_field(&table, &["browsers"])?;
        let browsers = match browsers_value {
            Value::Table(browsers) => string_array(&browsers, "webbrowser.remove", "browsers")?,
            Value::Nil => string_array(&table, "webbrowser.remove", "browser")?,
            other => {
                return Err(mlua::Error::external(format!(
                    "webbrowser.remove: argument 'browsers' expected table, got {}",
                    other.type_name()
                )))
            }
        };
        request.insert(
            "browsers".to_string(),
            serde_json::Value::Array(
                browsers
                    .into_iter()
                    .map(serde_json::Value::String)
                    .collect(),
            ),
        );
        return Ok(());
    }

    let mut browsers = Vec::with_capacity(args.len());
    for value in args {
        browsers.push(serde_json::Value::String(value_string(
            value,
            "webbrowser.remove",
            "browser",
        )?));
    }
    request.insert("browsers".to_string(), serde_json::Value::Array(browsers));
    Ok(())
}

fn set_resource_mode(
    request: &mut Map<String, serde_json::Value>,
    opts: Option<&Table>,
    use_default: bool,
) -> Result<(), mlua::Error> {
    let mode = optional_string(
        &opts.cloned(),
        "webbrowser",
        &["resource_mode", "resourceMode"],
    )?
    .or_else(|| use_default.then(|| DEFAULT_RESOURCE_MODE.to_string()));
    if let Some(mode) = mode {
        match mode.as_str() {
            "lean" | "full" => {
                request.insert("resourceMode".to_string(), serde_json::Value::String(mode));
            }
            _ => {
                return Err(mlua::Error::external(format!(
                    "webbrowser: resource_mode must be \"lean\" or \"full\", got {mode:?}"
                )))
            }
        }
    }
    Ok(())
}

fn set_resource_loading(
    request: &mut Map<String, serde_json::Value>,
    opts: Option<&Table>,
    wait_default: bool,
) -> Result<(), mlua::Error> {
    let opts = opts.cloned();
    let wait = optional_bool(
        &opts,
        "webbrowser",
        &["wait_resources", "waitResources", "wait"],
    )?
    .unwrap_or(wait_default);
    if wait {
        request.insert(
            "waitForResources".to_string(),
            serde_json::Value::Bool(true),
        );
    }
    if let Some(timeout) = optional_number(
        &opts,
        "webbrowser",
        &["resource_timeout", "resourceTimeout"],
    )? {
        request.insert(
            "resourceTimeout".to_string(),
            number_value(timeout, "webbrowser", "resource_timeout")?,
        );
        request.insert(
            "waitForResources".to_string(),
            serde_json::Value::Bool(true),
        );
    }
    Ok(())
}

fn set_page_options(
    request: &mut Map<String, serde_json::Value>,
    opts: Option<&Table>,
) -> Result<(), mlua::Error> {
    let Some(opts) = opts else {
        return Ok(());
    };
    match table_field(opts, &["fields"])? {
        Value::Nil => {}
        Value::String(s) => {
            request.insert(
                "fields".to_string(),
                serde_json::Value::String(s.to_string_lossy().to_string()),
            );
        }
        Value::Table(t) => {
            request.insert("fields".to_string(), lua_to_json(&Value::Table(t))?);
        }
        other => {
            return Err(mlua::Error::external(format!(
                "webbrowser.page: option 'fields' expected string or table, got {}",
                other.type_name()
            )))
        }
    }
    if let Some(selectors) = optional_bool(&Some(opts.clone()), "webbrowser.page", &["selectors"])?
    {
        request.insert("selectors".to_string(), serde_json::Value::Bool(selectors));
    }
    if let Some(details) = optional_bool(
        &Some(opts.clone()),
        "webbrowser.page",
        &["action_details", "actionDetails"],
    )? {
        request.insert(
            "actionDetails".to_string(),
            serde_json::Value::Bool(details),
        );
    }
    Ok(())
}

fn set_typing_options(
    request: &mut Map<String, serde_json::Value>,
    opts: Option<&Table>,
) -> Result<(), mlua::Error> {
    let opts = opts.cloned();
    if let Some(backend) = optional_string(
        &opts,
        "webbrowser.type",
        &["backend", "typing_backend", "typingBackend"],
    )? {
        request.insert(
            "typingBackend".to_string(),
            serde_json::Value::String(backend),
        );
    }
    if let Some(rhythm) = optional_string(
        &opts,
        "webbrowser.type",
        &["rhythm", "typing_rhythm", "typingRhythm"],
    )? {
        request.insert(
            "typingRhythm".to_string(),
            serde_json::Value::String(rhythm),
        );
    }
    if let Some(speed) = optional_number(
        &opts,
        "webbrowser.type",
        &["speed", "typing_speed", "typingSpeed"],
    )? {
        request.insert(
            "typingSpeed".to_string(),
            number_value(speed, "webbrowser.type", "speed")?,
        );
    }
    if let Some(delay_min) =
        optional_number(&opts, "webbrowser.type", &["delay_min", "typingDelayMin"])?
    {
        request.insert(
            "typingDelayMin".to_string(),
            number_value(delay_min, "webbrowser.type", "delay_min")?,
        );
    }
    if let Some(delay_max) =
        optional_number(&opts, "webbrowser.type", &["delay_max", "typingDelayMax"])?
    {
        request.insert(
            "typingDelayMax".to_string(),
            number_value(delay_max, "webbrowser.type", "delay_max")?,
        );
    }
    Ok(())
}

fn set_optional_string_field(
    request: &mut Map<String, serde_json::Value>,
    key: &str,
    value: Option<String>,
) {
    if let Some(value) = value {
        request.insert(key.to_string(), serde_json::Value::String(value));
    }
}

fn optional_only_table(args: &MultiValue, fn_name: &str) -> Result<Option<Table>, mlua::Error> {
    match args.len() {
        0 => Ok(None),
        1 => Ok(Some(value_table(&args[0], fn_name, "opts")?)),
        _ => Err(mlua::Error::external(format!(
            "{fn_name}: expected optional opts table, got {} arguments",
            args.len()
        ))),
    }
}

fn ensure_no_args(args: &MultiValue, fn_name: &str) -> Result<(), mlua::Error> {
    if args.is_empty() {
        Ok(())
    } else {
        Err(mlua::Error::external(format!(
            "{fn_name}: expected no arguments, got {}",
            args.len()
        )))
    }
}

fn single_table_arg(args: &MultiValue) -> Option<Table> {
    if args.len() == 1 {
        if let Some(Value::Table(table)) = args.get(0) {
            return Some(table.clone());
        }
    }
    None
}

fn table_field(table: &Table, aliases: &[&str]) -> Result<Value, mlua::Error> {
    for alias in aliases {
        let value = table.get::<Value>(*alias)?;
        if !matches!(value, Value::Nil) {
            return Ok(value);
        }
    }
    Ok(Value::Nil)
}

fn required_string(table: &Table, fn_name: &str, aliases: &[&str]) -> Result<String, mlua::Error> {
    match table_field(table, aliases)? {
        Value::String(s) => Ok(s.to_string_lossy().to_string()),
        Value::Nil => Err(mlua::Error::external(format!(
            "{fn_name}: missing required argument '{}' (string)",
            aliases[0]
        ))),
        other => Err(mlua::Error::external(format!(
            "{fn_name}: argument '{}' expected string, got {}",
            aliases[0],
            other.type_name()
        ))),
    }
}

fn required_number(table: &Table, fn_name: &str, aliases: &[&str]) -> Result<f64, mlua::Error> {
    match table_field(table, aliases)? {
        Value::Integer(value) => Ok(value as f64),
        Value::Number(value) => Ok(value),
        Value::Nil => Err(mlua::Error::external(format!(
            "{fn_name}: missing required argument '{}' (number)",
            aliases[0]
        ))),
        other => Err(mlua::Error::external(format!(
            "{fn_name}: argument '{}' expected number, got {}",
            aliases[0],
            other.type_name()
        ))),
    }
}

fn optional_string(
    table: &Option<Table>,
    fn_name: &str,
    aliases: &[&str],
) -> Result<Option<String>, mlua::Error> {
    let Some(table) = table else {
        return Ok(None);
    };
    match table_field(table, aliases)? {
        Value::Nil => Ok(None),
        Value::String(s) => Ok(Some(s.to_string_lossy().to_string())),
        other => Err(mlua::Error::external(format!(
            "{fn_name}: option '{}' expected string, got {}",
            aliases[0],
            other.type_name()
        ))),
    }
}

fn optional_bool(
    table: &Option<Table>,
    fn_name: &str,
    aliases: &[&str],
) -> Result<Option<bool>, mlua::Error> {
    let Some(table) = table else {
        return Ok(None);
    };
    match table_field(table, aliases)? {
        Value::Nil => Ok(None),
        Value::Boolean(value) => Ok(Some(value)),
        other => Err(mlua::Error::external(format!(
            "{fn_name}: option '{}' expected boolean, got {}",
            aliases[0],
            other.type_name()
        ))),
    }
}

fn optional_number(
    table: &Option<Table>,
    fn_name: &str,
    aliases: &[&str],
) -> Result<Option<f64>, mlua::Error> {
    let Some(table) = table else {
        return Ok(None);
    };
    match table_field(table, aliases)? {
        Value::Nil => Ok(None),
        Value::Integer(value) => Ok(Some(value as f64)),
        Value::Number(value) => Ok(Some(value)),
        other => Err(mlua::Error::external(format!(
            "{fn_name}: option '{}' expected number, got {}",
            aliases[0],
            other.type_name()
        ))),
    }
}

fn optional_integer(
    table: &Option<Table>,
    fn_name: &str,
    aliases: &[&str],
) -> Result<Option<i64>, mlua::Error> {
    let Some(table) = table else {
        return Ok(None);
    };
    match table_field(table, aliases)? {
        Value::Nil => Ok(None),
        Value::Integer(value) => Ok(Some(value)),
        Value::Number(value) if value.fract() == 0.0 => Ok(Some(value as i64)),
        Value::Number(_) => Err(mlua::Error::external(format!(
            "{fn_name}: option '{}' expected integer",
            aliases[0]
        ))),
        other => Err(mlua::Error::external(format!(
            "{fn_name}: option '{}' expected number, got {}",
            aliases[0],
            other.type_name()
        ))),
    }
}

fn value_string(value: &Value, fn_name: &str, name: &str) -> Result<String, mlua::Error> {
    match value {
        Value::String(s) => Ok(s.to_string_lossy().to_string()),
        Value::Nil => Err(mlua::Error::external(format!(
            "{fn_name}: missing required argument '{name}' (string)"
        ))),
        other => Err(mlua::Error::external(format!(
            "{fn_name}: argument '{name}' expected string, got {}",
            other.type_name()
        ))),
    }
}

fn value_number(value: &Value, fn_name: &str, name: &str) -> Result<f64, mlua::Error> {
    match value {
        Value::Integer(value) => Ok(*value as f64),
        Value::Number(value) => Ok(*value),
        Value::Nil => Err(mlua::Error::external(format!(
            "{fn_name}: missing required argument '{name}' (number)"
        ))),
        other => Err(mlua::Error::external(format!(
            "{fn_name}: argument '{name}' expected number, got {}",
            other.type_name()
        ))),
    }
}

fn value_integer(value: &Value, fn_name: &str, name: &str) -> Result<i64, mlua::Error> {
    match value {
        Value::Integer(value) => Ok(*value),
        Value::Number(value) if value.fract() == 0.0 => Ok(*value as i64),
        Value::Number(_) => Err(mlua::Error::external(format!(
            "{fn_name}: argument '{name}' expected integer"
        ))),
        Value::Nil => Err(mlua::Error::external(format!(
            "{fn_name}: missing required argument '{name}' (number)"
        ))),
        other => Err(mlua::Error::external(format!(
            "{fn_name}: argument '{name}' expected number, got {}",
            other.type_name()
        ))),
    }
}

fn value_table(value: &Value, fn_name: &str, name: &str) -> Result<Table, mlua::Error> {
    match value {
        Value::Table(table) => Ok(table.clone()),
        Value::Nil => Err(mlua::Error::external(format!(
            "{fn_name}: missing required argument '{name}' (table)"
        ))),
        other => Err(mlua::Error::external(format!(
            "{fn_name}: argument '{name}' expected table, got {}",
            other.type_name()
        ))),
    }
}

fn string_array(table: &Table, fn_name: &str, name: &str) -> Result<Vec<String>, mlua::Error> {
    let len = table.raw_len();
    if len == 0 || !is_lua_array(table, len) {
        return Err(mlua::Error::external(format!(
            "{fn_name}: argument '{name}' expected array table"
        )));
    }
    let mut values = Vec::with_capacity(len);
    for i in 1..=len {
        let value: Value = table.raw_get(i)?;
        values.push(value_string(&value, fn_name, name)?);
    }
    Ok(values)
}

fn number_value(value: f64, fn_name: &str, name: &str) -> Result<serde_json::Value, mlua::Error> {
    let number = Number::from_f64(value).ok_or_else(|| {
        mlua::Error::external(format!("{fn_name}: argument '{name}' must be finite"))
    })?;
    Ok(serde_json::Value::Number(number))
}

fn validate_screenshot_path(path: &str) -> Result<(), mlua::Error> {
    let extension = Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if matches!(extension.as_str(), "png" | "jpg" | "jpeg") {
        Ok(())
    } else {
        Err(mlua::Error::external(
            "webbrowser.screenshot: path must end in .png, .jpg, or .jpeg",
        ))
    }
}

fn json_to_lua(lua: &Lua, value: &serde_json::Value) -> Result<Value, mlua::Error> {
    match value {
        serde_json::Value::Null => Ok(Value::Nil),
        serde_json::Value::Bool(value) => Ok(Value::Boolean(*value)),
        serde_json::Value::Number(number) => {
            if let Some(value) = number.as_i64() {
                if value >= i32::MIN as i64 && value <= i32::MAX as i64 {
                    Ok(Value::Integer(value as mlua::Integer))
                } else {
                    Ok(Value::Number(value as f64))
                }
            } else if let Some(value) = number.as_f64() {
                Ok(Value::Number(value))
            } else {
                Ok(Value::Nil)
            }
        }
        serde_json::Value::String(value) => Ok(Value::String(lua.create_string(value)?)),
        serde_json::Value::Array(values) => {
            let table = lua.create_table()?;
            for (index, item) in values.iter().enumerate() {
                table.set(index + 1, json_to_lua(lua, item)?)?;
            }
            Ok(Value::Table(table))
        }
        serde_json::Value::Object(values) => {
            let table = lua.create_table()?;
            for (key, item) in values {
                table.set(key.as_str(), json_to_lua(lua, item)?)?;
            }
            Ok(Value::Table(table))
        }
    }
}

fn lua_to_json(value: &Value) -> Result<serde_json::Value, mlua::Error> {
    match value {
        Value::Nil => Ok(serde_json::Value::Null),
        Value::Boolean(value) => Ok(serde_json::Value::Bool(*value)),
        Value::Integer(value) => Ok(serde_json::Value::Number((*value).into())),
        Value::Number(value) => number_value(*value, "webbrowser", "value"),
        Value::String(value) => Ok(serde_json::Value::String(
            value.to_string_lossy().to_string(),
        )),
        Value::Table(table) => {
            let len = table.raw_len();
            if len > 0 && is_lua_array(table, len) {
                let mut values = Vec::with_capacity(len);
                for i in 1..=len {
                    let value: Value = table.raw_get(i)?;
                    values.push(lua_to_json(&value)?);
                }
                Ok(serde_json::Value::Array(values))
            } else {
                let mut map = Map::new();
                for pair in table.clone().pairs::<Value, Value>() {
                    let (key, value) = pair?;
                    let key = match key {
                        Value::String(value) => value.to_string_lossy().to_string(),
                        Value::Integer(value) => value.to_string(),
                        Value::Number(value) => value.to_string(),
                        other => {
                            return Err(mlua::Error::external(format!(
                                "JSON object keys must be strings, got {}",
                                value_type_name(&other)
                            )))
                        }
                    };
                    map.insert(key, lua_to_json(&value)?);
                }
                Ok(serde_json::Value::Object(map))
            }
        }
        Value::Function(_) => Err(mlua::Error::external("cannot encode function as JSON")),
        other => Err(mlua::Error::external(format!(
            "cannot encode {} as JSON",
            value_type_name(other)
        ))),
    }
}
