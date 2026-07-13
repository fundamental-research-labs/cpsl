//! CSV module for the Luau sandbox.
//!
//! Exposes `csv.parse`, `csv.parseFile`, `csv.stringify`, `csv.writeFile` as globals.
//! Uses the `csv` crate (BurntSushi) for parsing and writing.

use crate::mount::MountTable;
use crate::pyrt_compat::unwrap_py_seq;
use crate::sandbox::{
    arg_error, wrap_module_with_help_hints, FieldDoc, FnDoc, ModuleDoc, Param, ParamType,
    ReturnType,
};
use mlua::{Lua, MultiValue, Value};
use std::sync::Arc;

const CSV_WRITE_OPTS: &[FieldDoc] = &[
    FieldDoc {
        name: "delimiter",
        typ: "string",
        required: false,
        description: "Field delimiter (default \",\")",
    },
    FieldDoc {
        name: "headers",
        typ: "table",
        required: false,
        description: "Column headers (list of strings)",
    },
];

const CSV_PARSE_OPTS: &[FieldDoc] = &[
    FieldDoc {
        name: "delimiter",
        typ: "string",
        required: false,
        description: "Field delimiter (default \",\")",
    },
    FieldDoc {
        name: "quote",
        typ: "string",
        required: false,
        description: "Quote character (default '\"')",
    },
    FieldDoc {
        name: "header",
        typ: "boolean",
        required: false,
        description: "If true, first row is headers → returns keyed records",
    },
    FieldDoc {
        name: "skipRows",
        typ: "number",
        required: false,
        description: "Skip N rows before parsing",
    },
];

pub(crate) static CSV_DOC: ModuleDoc = ModuleDoc {
    name: "csv",
    summary: "CSV parse & write",
    functions: &[
        FnDoc {
            name: "parse",
            description:
                "Parse a CSV string. Returns rows as lists, or as keyed records if header=true.",
            params: &[
                Param {
                    name: "text",
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
                    fields: Some(CSV_PARSE_OPTS),
                },
            ],
            returns: ReturnType::Table,
            example: Some(r#"local rows = csv.parse(text, {header = true})"#),
        },
        FnDoc {
            name: "parseFile",
            description:
                "Parse a CSV file. Same options as csv.parse.",
            params: &[
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
                    fields: Some(CSV_PARSE_OPTS),
                },
            ],
            returns: ReturnType::Table,
            example: None,
        },
        FnDoc {
            name: "stringify",
            description:
                "Convert rows to a CSV string. Accepts lists of lists, or keyed records with headers opt.",
            params: &[
                Param {
                    name: "rows",
                    short: None,
                    typ: ParamType::Table,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "opts",
                    short: None,
                    typ: ParamType::Table,
                    required: false,
                    fields: Some(CSV_WRITE_OPTS),
                },
            ],
            returns: ReturnType::String,
            example: None,
        },
        FnDoc {
            name: "writeFile",
            description:
                "Write rows to a CSV file. Same options as csv.stringify.",
            params: &[
                Param {
                    name: "path",
                    short: None,
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "rows",
                    short: None,
                    typ: ParamType::Table,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "opts",
                    short: None,
                    typ: ParamType::Table,
                    required: false,
                    fields: Some(CSV_WRITE_OPTS),
                },
            ],
            returns: ReturnType::Void,
            example: Some(r#"csv.writeFile({path="/artifacts/out.csv", rows=data, delimiter="\t"})"#),
        },
    ],
};

/// Extract CSV options from a Lua table.
struct CsvOpts {
    delimiter: u8,
    quote: u8,
    header: bool,
    skip_rows: usize,
}

impl CsvOpts {
    fn from_lua(opts: &Option<mlua::Table>) -> Result<Self, mlua::Error> {
        let mut o = CsvOpts {
            delimiter: b',',
            quote: b'"',
            header: false,
            skip_rows: 0,
        };
        if let Some(t) = opts {
            Self::apply_table(&mut o, t)?;
        }
        Ok(o)
    }

    /// Apply options from a Lua table, supporting short aliases.
    fn apply_table(o: &mut Self, t: &mlua::Table) -> Result<(), mlua::Error> {
        // delimiter / d
        if let Ok(d) = t
            .get::<String>("delimiter")
            .or_else(|_| t.get::<String>("d"))
        {
            if let Some(b) = d.as_bytes().first() {
                o.delimiter = *b;
            }
        }
        // quote / q
        if let Ok(q) = t.get::<String>("quote").or_else(|_| t.get::<String>("q")) {
            if let Some(b) = q.as_bytes().first() {
                o.quote = *b;
            }
        }
        // header / h (bool or string "true"/"false")
        // Note: can't use get::<bool> with or_else because nil coerces to false in mlua,
        // which would prevent the short alias from being checked.
        let header_val: Value = t.get("header").unwrap_or(Value::Nil);
        let header_val = if matches!(header_val, Value::Nil) {
            t.get("h").unwrap_or(Value::Nil)
        } else {
            header_val
        };
        match &header_val {
            Value::Boolean(b) => o.header = *b,
            Value::String(s) => {
                let s = s.to_string_lossy();
                o.header = s == "true" || s == "1" || s == "yes";
            }
            _ => {}
        }
        // skipRows
        if let Ok(s) = t.get::<i32>("skipRows") {
            o.skip_rows = s.max(0) as usize;
        }
        Ok(())
    }
}

fn build_reader<'a>(data: &'a [u8], opts: &CsvOpts) -> csv::Reader<&'a [u8]> {
    csv::ReaderBuilder::new()
        .delimiter(opts.delimiter)
        .quote(opts.quote)
        .has_headers(opts.header)
        .from_reader(data)
}

/// Parse CSV bytes into a Lua table.
fn parse_csv(lua: &Lua, data: &[u8], opts: &CsvOpts) -> Result<Value, mlua::Error> {
    let mut rdr = build_reader(data, opts);
    let result = lua.create_table()?;
    let mut row_idx = 0usize;

    if opts.header {
        let headers: Vec<String> = rdr
            .headers()
            .map_err(mlua::Error::external)?
            .iter()
            .map(|s: &str| s.to_string())
            .collect();

        for record in rdr.records().skip(opts.skip_rows) {
            let record: csv::StringRecord = record.map_err(mlua::Error::external)?;
            let row = lua.create_table()?;
            for (i, field) in record.iter().enumerate() {
                if let Some(header) = headers.get(i) {
                    row.set(header.as_str(), lua.create_string(field)?)?;
                }
            }
            row_idx += 1;
            result.set(row_idx, row)?;
        }
    } else {
        for record in rdr.records().skip(opts.skip_rows) {
            let record: csv::StringRecord = record.map_err(mlua::Error::external)?;
            let row = lua.create_table()?;
            for (i, field) in record.iter().enumerate() {
                row.set(i + 1, lua.create_string(field)?)?;
            }
            row_idx += 1;
            result.set(row_idx, row)?;
        }
    }

    Ok(Value::Table(result))
}

/// Extract CSV stringify options.
struct StringifyOpts {
    delimiter: u8,
    headers: Option<Vec<String>>,
}

impl StringifyOpts {
    fn from_lua(opts: &Option<mlua::Table>) -> Result<Self, mlua::Error> {
        let mut o = StringifyOpts {
            delimiter: b',',
            headers: None,
        };
        if let Some(t) = opts {
            Self::apply_table(&mut o, t)?;
        }
        Ok(o)
    }

    fn apply_table(o: &mut Self, t: &mlua::Table) -> Result<(), mlua::Error> {
        // delimiter / d
        if let Ok(d) = t
            .get::<String>("delimiter")
            .or_else(|_| t.get::<String>("d"))
        {
            if let Some(b) = d.as_bytes().first() {
                o.delimiter = *b;
            }
        }
        // headers
        if let Ok(h) = t.get::<mlua::Table>("headers") {
            let h = unwrap_py_seq(&h)?;
            let mut hdrs = Vec::new();
            for i in 1..=h.raw_len() {
                hdrs.push(h.get::<String>(i)?);
            }
            o.headers = Some(hdrs);
        }
        Ok(())
    }
}

/// Stringify a Lua table to CSV bytes.
fn stringify_csv(rows: &mlua::Table, opts: &StringifyOpts) -> Result<Vec<u8>, mlua::Error> {
    let mut wtr = csv::WriterBuilder::new()
        .delimiter(opts.delimiter)
        .from_writer(Vec::new());

    // Write headers if provided
    if let Some(ref hdrs) = opts.headers {
        wtr.write_record(hdrs).map_err(mlua::Error::external)?;
    }

    let rows = unwrap_py_seq(rows)?;
    let len = rows.raw_len();
    for i in 1..=len {
        let row: mlua::Table = rows.get(i)?;
        let row = unwrap_py_seq(&row)?;

        // Check if this is a dict (has string keys from headers) or an array
        if let Some(ref hdrs) = opts.headers {
            // Try dict access first
            let mut fields = Vec::new();
            for h in hdrs {
                let val: Value = row.get(h.as_str())?;
                fields.push(lua_value_to_string(&val));
            }
            wtr.write_record(&fields).map_err(mlua::Error::external)?;
        } else {
            // Array of arrays
            let mut fields = Vec::new();
            let row_len = row.raw_len();
            for j in 1..=row_len {
                let val: Value = row.get(j)?;
                fields.push(lua_value_to_string(&val));
            }
            wtr.write_record(&fields).map_err(mlua::Error::external)?;
        }
    }

    wtr.into_inner().map_err(mlua::Error::external)
}

fn lua_value_to_string(val: &Value) -> String {
    match val {
        Value::Nil => String::new(),
        Value::Boolean(b) => b.to_string(),
        Value::Integer(i) => i.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.to_string_lossy().to_string(),
        _ => String::new(),
    }
}

/// Register `csv.*` globals in the Lua VM.
pub fn register_csv_globals(lua: &Lua, mounts: Arc<MountTable>) -> Result<(), mlua::Error> {
    let csv_table = lua.create_table()?;

    // csv.parse(text, opts?) -> table
    // Dual signature: csv.parse("text", {opts}) OR csv.parse({[1]="text", header=true, ...})
    csv_table.set(
        "parse",
        lua.create_function(|lua, args: MultiValue| {
            if args.is_empty() {
                return Err(arg_error("csv.parse", CSV_DOC.params("parse")));
            }
            let first = &args[0];
            let opts_arg = args.get(1).and_then(|v| match v {
                Value::Table(t) => Some(t.clone()),
                _ => None,
            });
            let (text, csv_opts) = match first {
                Value::String(s) => (s.as_bytes().to_vec(), CsvOpts::from_lua(&opts_arg)?),
                Value::Table(t) => {
                    let s: mlua::LuaString = t.get::<mlua::LuaString>(1).map_err(|_| {
                        mlua::Error::external(
                            "csv.parse: missing required argument 'text' (string)",
                        )
                    })?;
                    let mut csv_opts = CsvOpts {
                        delimiter: b',',
                        quote: b'"',
                        header: false,
                        skip_rows: 0,
                    };
                    CsvOpts::apply_table(&mut csv_opts, t)?;
                    (s.as_bytes().to_vec(), csv_opts)
                }
                _ => {
                    return Err(mlua::Error::external(
                        "csv.parse: argument 'text' expected string, got ".to_string()
                            + first.type_name(),
                    ));
                }
            };
            parse_csv(lua, &text, &csv_opts)
        })?,
    )?;

    // csv.parseFile(path, opts?) -> table
    // Dual signature: csv.parseFile("path", {opts}) OR csv.parseFile({[1]="path", header=true, ...})
    {
        let m = mounts.clone();
        csv_table.set(
            "parseFile",
            lua.create_function(move |lua, args: MultiValue| {
                if args.is_empty() {
                    return Err(arg_error("csv.parseFile", CSV_DOC.params("parseFile")));
                }
                let first = &args[0];
                let opts_arg = args.get(1).and_then(|v| match v {
                    Value::Table(t) => Some(t.clone()),
                    _ => None,
                });
                let (path, csv_opts) = match first {
                    Value::String(s) => (
                        s.to_string_lossy().to_string(),
                        CsvOpts::from_lua(&opts_arg)?,
                    ),
                    Value::Table(t) => {
                        let p: String = t
                            .get::<String>(1)
                            .or_else(|_| t.get::<String>("path"))
                            .map_err(|_| {
                                mlua::Error::external(
                                    "csv.parseFile: missing required argument 'path' (string)",
                                )
                            })?;
                        let mut csv_opts = CsvOpts {
                            delimiter: b',',
                            quote: b'"',
                            header: false,
                            skip_rows: 0,
                        };
                        CsvOpts::apply_table(&mut csv_opts, t)?;
                        (p, csv_opts)
                    }
                    _ => {
                        return Err(mlua::Error::external(
                            "csv.parseFile: argument 'path' expected string, got ".to_string()
                                + first.type_name(),
                        ));
                    }
                };
                let host_path = m.resolve_read(&path).map_err(mlua::Error::external)?;
                let data = std::fs::read(&host_path).map_err(mlua::Error::external)?;
                parse_csv(lua, &data, &csv_opts)
            })?,
        )?;
    }

    // csv.stringify(rows, opts?) -> string
    // Dual signature: csv.stringify(rows, {opts}) OR csv.stringify({[1]=rows, delimiter=","})
    csv_table.set(
        "stringify",
        lua.create_function(|lua, args: MultiValue| {
            if args.is_empty() {
                return Err(arg_error("csv.stringify", CSV_DOC.params("stringify")));
            }
            let first = match &args[0] {
                Value::Table(t) => t.clone(),
                _ => {
                    return Err(mlua::Error::external(
                        "csv.stringify: argument 'rows' expected table, got ".to_string()
                            + args[0].type_name(),
                    ))
                }
            };
            let opts = args.get(1).and_then(|v| match v {
                Value::Table(t) => Some(t.clone()),
                _ => None,
            });
            // If opts is provided, first is always the rows (ordered calling convention)
            if opts.is_some() {
                let str_opts = StringifyOpts::from_lua(&opts)?;
                let bytes = stringify_csv(&first, &str_opts)?;
                let s = String::from_utf8(bytes).map_err(mlua::Error::external)?;
                return lua.create_string(&s);
            }
            // No opts: check if [1] is a table (named-params: {[1]=rows, delimiter=","})
            // vs rows directly (ordered: csv.stringify(rows))
            let maybe_inner: Result<mlua::Table, _> = first.get(1);
            if let Ok(inner) = maybe_inner {
                // [1] is a table — check if it's a nested array (data) or a flat row
                let maybe_nested: Result<mlua::Table, _> = inner.get(1);
                if maybe_nested.is_ok() || inner.raw_len() == 0 {
                    // Named-params mode: {[1]=rows_table, delimiter=...}
                    let rows: mlua::Table = first.get(1)?;
                    let mut str_opts = StringifyOpts {
                        delimiter: b',',
                        headers: None,
                    };
                    StringifyOpts::apply_table(&mut str_opts, &first)?;
                    let bytes = stringify_csv(&rows, &str_opts)?;
                    let s = String::from_utf8(bytes).map_err(mlua::Error::external)?;
                    return lua.create_string(&s);
                }
            }
            // Default: first is the rows table directly
            let str_opts = StringifyOpts {
                delimiter: b',',
                headers: None,
            };
            let bytes = stringify_csv(&first, &str_opts)?;
            let s = String::from_utf8(bytes).map_err(mlua::Error::external)?;
            lua.create_string(&s)
        })?,
    )?;

    // csv.writeFile(path, rows, opts?)
    // Dual signature: csv.writeFile("path", rows, {opts}) OR csv.writeFile({[1]="path", [2]=rows, delimiter=","})
    {
        let m = mounts.clone();
        csv_table.set(
            "writeFile",
            lua.create_function(move |_, args: MultiValue| {
                if args.is_empty() {
                    return Err(arg_error("csv.writeFile", CSV_DOC.params("writeFile")));
                }
                let first = &args[0];
                let rows_opt = args.get(1).and_then(|v| match v {
                    Value::Table(t) => Some(t.clone()),
                    _ => None,
                });
                let opts = args.get(2).and_then(|v| match v {
                    Value::Table(t) => Some(t.clone()),
                    _ => None,
                });
                let (path, rows, str_opts) = match first {
                    Value::String(s) => {
                        let p = s.to_string_lossy().to_string();
                        let r = rows_opt.ok_or_else(|| {
                            mlua::Error::external(
                                "csv.writeFile: missing required argument 'rows' (table)",
                            )
                        })?;
                        let so = StringifyOpts::from_lua(&opts)?;
                        (p, r, so)
                    }
                    Value::Table(t) => {
                        let p: String = t
                            .get::<String>(1)
                            .or_else(|_| t.get::<String>("path"))
                            .map_err(|_| {
                                mlua::Error::external(
                                    "csv.writeFile: missing required argument 'path' (string)",
                                )
                            })?;
                        let r: mlua::Table = t
                            .get::<mlua::Table>(2)
                            .or_else(|_| t.get::<mlua::Table>("rows"))
                            .map_err(|_| {
                                mlua::Error::external(
                                    "csv.writeFile: missing required argument 'rows' (table)",
                                )
                            })?;
                        let mut so = StringifyOpts {
                            delimiter: b',',
                            headers: None,
                        };
                        StringifyOpts::apply_table(&mut so, t)?;
                        (p, r, so)
                    }
                    _ => {
                        return Err(mlua::Error::external(
                            "csv.writeFile: argument 'path' expected string, got ".to_string()
                                + first.type_name(),
                        ));
                    }
                };
                let host_path = m.resolve_write(&path).map_err(mlua::Error::external)?;
                if let Some(parent) = host_path.parent() {
                    std::fs::create_dir_all(parent).map_err(mlua::Error::external)?;
                }
                let bytes = stringify_csv(&rows, &str_opts)?;
                std::fs::write(&host_path, &bytes).map_err(mlua::Error::external)?;
                Ok(())
            })?,
        )?;
    }

    crate::lua_util::register_help_functions(lua, &csv_table, &CSV_DOC)?;

    lua.globals().set("csv", csv_table)?;
    wrap_module_with_help_hints(lua, "csv")?;

    Ok(())
}
