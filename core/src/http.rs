//! Lua-facing HTTP module backed by the policy-gated native HTTP gateway.

use crate::sandbox::{
    arg_error, wrap_module_with_help_hints, FieldDoc, FnDoc, ModuleDoc, Param, ParamType,
    ReturnType,
};
use mlua::{Lua, MultiValue};
use native_http::{Headers, HttpGateway, Method, Request};
use std::sync::Arc;

const HTTP_OPTS_FIELDS: &[FieldDoc] = &[
    FieldDoc {
        name: "headers",
        typ: "table",
        required: false,
        description: "Request headers {[name] = value}",
    },
    FieldDoc {
        name: "body",
        typ: "string",
        required: false,
        description: "Request body (string or JSON-encodable table)",
    },
];

pub(crate) static HTTP_DOC: ModuleDoc = ModuleDoc {
    name: "http",
    summary: "HTTP requests (policy-gated, credentials auto-injected)",
    functions: &[
        FnDoc {
            name: "get",
            description: "Send a GET request. Returns {status, body, headers, ok}.",
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
                    fields: Some(HTTP_OPTS_FIELDS),
                },
            ],
            returns: ReturnType::Table,
            example: Some(
                r#"local resp = http.get("https://api.example.com/data", {headers = {Authorization = "Bearer tok"}})"#,
            ),
        },
        FnDoc {
            name: "post",
            description: "Send a POST request. Returns {status, body, headers, ok}.",
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
                    fields: Some(HTTP_OPTS_FIELDS),
                },
            ],
            returns: ReturnType::Table,
            example: Some(
                r#"http.post("https://api.example.com/items", {body = json.encode({name = "test"}), headers = {["Content-Type"] = "application/json"}})"#,
            ),
        },
        FnDoc {
            name: "put",
            description: "Send a PUT request. Returns {status, body, headers, ok}.",
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
                    fields: Some(HTTP_OPTS_FIELDS),
                },
            ],
            returns: ReturnType::Table,
            example: None,
        },
        FnDoc {
            name: "patch",
            description: "Send a PATCH request. Returns {status, body, headers, ok}.",
            params: &[
                Param {
                    name: "url",
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
                    fields: Some(HTTP_OPTS_FIELDS),
                },
            ],
            returns: ReturnType::Table,
            example: None,
        },
        FnDoc {
            name: "delete",
            description: "Send a DELETE request. Returns {status, body, headers, ok}.",
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
                    fields: Some(HTTP_OPTS_FIELDS),
                },
            ],
            returns: ReturnType::Table,
            example: None,
        },
        FnDoc {
            name: "request",
            description: "Send a request with any HTTP method.",
            params: &[
                Param {
                    name: "method",
                    short: Some('m'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
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
                    fields: Some(HTTP_OPTS_FIELDS),
                },
            ],
            returns: ReturnType::Table,
            example: Some(
                r#"http.request({method="PATCH", url="https://api.example.com/items/1", body=json.encode({name="updated"}), headers={["Content-Type"]="application/json"}})"#,
            ),
        },
    ],
};

pub(crate) fn register_http_globals(
    lua: &Lua,
    gateway: Arc<HttpGateway>,
) -> Result<(), mlua::Error> {
    let http = lua.create_table()?;

    // Helper methods: get, post, put, patch, delete
    for (idx, (name, method)) in [
        ("get", Method::Get),
        ("post", Method::Post),
        ("put", Method::Put),
        ("patch", Method::Patch),
        ("delete", Method::Delete),
    ]
    .iter()
    .enumerate()
    {
        let gw = gateway.clone();
        let fn_name = format!("http.{}", name);
        let doc_idx = idx;
        http.set(
            *name,
            lua.create_function(move |lua, args: MultiValue| {
                if args.is_empty() {
                    return Err(arg_error(&fn_name, HTTP_DOC.functions[doc_idx].params));
                }
                let (url, opts) = match &args[0] {
                    mlua::Value::Table(ref t) => {
                        let url = t.get::<String>(1).map_err(|_| {
                            mlua::Error::external(format!(
                                "{}: missing required argument 'url' (string)",
                                fn_name
                            ))
                        })?;
                        (url, Some(t.clone()))
                    }
                    mlua::Value::String(ref s) => {
                        let opts = args.get(1).and_then(|v| match v {
                            mlua::Value::Table(t) => Some(t.clone()),
                            _ => None,
                        });
                        (s.to_string_lossy().to_string(), opts)
                    }
                    _ => {
                        return Err(mlua::Error::external(format!(
                            "{}: argument 'url' expected string, got {}",
                            fn_name,
                            args[0].type_name()
                        )))
                    }
                };
                do_request(lua, &gw, method.clone(), url, opts)
            })?,
        )?;
    }

    // http.request(method_str, url, opts?)
    {
        let gw = gateway.clone();
        http.set(
            "request",
            lua.create_function(move |lua, args: MultiValue| {
                if args.is_empty() {
                    return Err(arg_error("http.request", HTTP_DOC.params("request")));
                }
                let (method_str, url, opts) = match &args[0] {
                    mlua::Value::Table(ref t) => {
                        let m = t
                            .get::<String>(1)
                            .or_else(|_| t.get::<String>("method"))
                            .map_err(|_| {
                                mlua::Error::external(
                                    "http.request: missing required argument 'method' (string)",
                                )
                            })?;
                        let u = t
                            .get::<String>(2)
                            .or_else(|_| t.get::<String>("url"))
                            .map_err(|_| {
                                mlua::Error::external(
                                    "http.request: missing required argument 'url' (string)",
                                )
                            })?;
                        (m, u, Some(t.clone()))
                    }
                    mlua::Value::String(ref s) => {
                        let m = s.to_string_lossy().to_string();
                        let u = match args.get(1) {
                            Some(mlua::Value::String(s)) => s.to_string_lossy().to_string(),
                            _ => {
                                return Err(mlua::Error::external(
                                    "http.request: missing required argument 'url' (string)",
                                ))
                            }
                        };
                        let opts = args.get(2).and_then(|v| match v {
                            mlua::Value::Table(t) => Some(t.clone()),
                            _ => None,
                        });
                        (m, u, opts)
                    }
                    _ => {
                        return Err(mlua::Error::external(
                            "http.request: argument 'method' expected string, got ".to_string()
                                + args[0].type_name(),
                        ))
                    }
                };
                let method = parse_method(&method_str).map_err(mlua::Error::external)?;
                do_request(lua, &gw, method, url, opts)
            })?,
        )?;
    }

    crate::lua_util::register_help_functions(lua, &http, &HTTP_DOC)?;

    lua.globals().set("http", http)?;

    wrap_module_with_help_hints(lua, "http")?;

    Ok(())
}

fn do_request(
    lua: &Lua,
    gateway: &HttpGateway,
    method: Method,
    url: String,
    opts: Option<mlua::Table>,
) -> Result<mlua::Table, mlua::Error> {
    let mut headers = Headers::new();
    let mut body: Option<Vec<u8>> = None;

    if let Some(opts) = opts {
        if let Some(h) = opts.get::<Option<mlua::Table>>("headers")? {
            for pair in h.pairs::<String, String>() {
                let (k, v) = pair?;
                headers.insert(k, v);
            }
        }
        if let Some(b) = opts.get::<Option<String>>("body")? {
            body = Some(b.into_bytes());
        }
    }

    let request = Request {
        method,
        url,
        headers,
        body,
    };

    let response = gateway.request(request).map_err(mlua::Error::external)?;

    // Build Lua response table
    let resp_table = lua.create_table()?;
    resp_table.set("status", response.status)?;
    resp_table.set("body", lua.create_string(&response.body)?)?;
    resp_table.set("ok", response.ok())?;

    let headers_table = lua.create_table()?;
    for (key, value) in response.headers.iter() {
        headers_table.set(key, value)?;
    }
    resp_table.set("headers", headers_table)?;

    Ok(resp_table)
}

fn parse_method(s: &str) -> Result<Method, String> {
    match s.to_uppercase().as_str() {
        "GET" => Ok(Method::Get),
        "POST" => Ok(Method::Post),
        "PUT" => Ok(Method::Put),
        "PATCH" => Ok(Method::Patch),
        "DELETE" => Ok(Method::Delete),
        "HEAD" => Ok(Method::Head),
        "OPTIONS" => Ok(Method::Options),
        _ => Err(format!("http: unsupported method '{}'", s)),
    }
}
