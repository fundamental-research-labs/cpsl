//! QR code generation module for the Luau sandbox.
//!
//! Exposes `qr.generate` and `qr.to_string` as globals.
//! Uses the `qrcode` crate (pure Rust) for encoding and rendering.
//! File I/O is sandboxed through MountTable — no direct host filesystem access.

use crate::lua_util::register_help_functions;
use crate::mount::MountTable;
use crate::sandbox::{
    validate_args, wrap_module_with_help_hints, FnDoc, ModuleDoc, Param, ParamType, ReturnType,
};
use mlua::{Lua, MultiValue, Value};
use qrcode::render::svg;
use qrcode::QrCode;
use std::sync::Arc;

pub(crate) static QR_DOC: ModuleDoc = ModuleDoc {
    name: "qr",
    summary: "QR code generation (PNG, SVG, ASCII/Unicode art)",
    functions: &[
        FnDoc {
            name: "generate",
            description: "Generate a QR code image file (PNG or SVG). Returns the output path on success.",
            params: &[
                Param {
                    name: "data",
                    short: Some('d'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "output",
                    short: Some('o'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "opts",
                    short: None,
                    typ: ParamType::Table,
                    required: false,
                    fields: None,
                },
            ],
            returns: ReturnType::String,
            example: Some(r#"qr.generate({data="https://example.com", output="/artifacts/qr.svg"})"#),
        },
        FnDoc {
            name: "to_string",
            description:
                "Render a QR code as ASCII or Unicode art for terminal/text display. Returns the string.",
            params: &[
                Param {
                    name: "data",
                    short: Some('d'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "opts",
                    short: None,
                    typ: ParamType::Table,
                    required: false,
                    fields: None,
                },
            ],
            returns: ReturnType::String,
            example: None,
        },
    ],
};

/// Extract a string argument from positional or table-form args.
fn extract_string(
    args: &MultiValue,
    idx: usize,
    name: &str,
    func: &str,
) -> Result<String, mlua::Error> {
    // Ordered args: fn(arg1, arg2, ...)
    if args.len() > 1 || !matches!(args.get(0), Some(Value::Table(_))) {
        return match args.get(idx) {
            Some(Value::String(s)) => Ok(s.to_string_lossy().to_string()),
            Some(v) => Err(mlua::Error::external(format!(
                "{func}: argument '{name}' expected string, got {}",
                v.type_name()
            ))),
            None => Err(mlua::Error::external(format!(
                "{func}: missing required argument '{name}'"
            ))),
        };
    }
    // Single table form: fn({[1]=arg1, name=arg1, ...})
    let t = match args.get(0) {
        Some(Value::Table(t)) => t,
        _ => unreachable!(),
    };
    // Try positional first, then named
    let val: Value = t.get(idx as i64 + 1).unwrap_or(Value::Nil);
    if let Value::String(s) = val {
        return Ok(s.to_string_lossy().to_string());
    }
    let val: Value = t.get(name).unwrap_or(Value::Nil);
    if let Value::String(s) = val {
        return Ok(s.to_string_lossy().to_string());
    }
    Err(mlua::Error::external(format!(
        "{func}: missing required argument '{name}' (string)"
    )))
}

/// Extract opts table from ordered args or table-form args.
fn extract_opts(args: &MultiValue, idx: usize) -> Option<mlua::Table> {
    if args.len() > 1 || !matches!(args.get(0), Some(Value::Table(_))) {
        return match args.get(idx) {
            Some(Value::Table(t)) => Some(t.clone()),
            _ => None,
        };
    }
    // In single-table form, the table itself contains the opts
    match args.get(0) {
        Some(Value::Table(t)) => Some(t.clone()),
        _ => None,
    }
}

/// Helper: get a string option from opts table, trying both long and short names.
fn opt_string(opts: &Option<mlua::Table>, long: &str, short: &str) -> Option<String> {
    opts.as_ref().and_then(|t| {
        t.get::<String>(long)
            .ok()
            .or_else(|| t.get::<String>(short).ok())
    })
}

/// Helper: get a numeric option from opts table, trying both long and short names.
fn opt_number(opts: &Option<mlua::Table>, long: &str, short: &str) -> Option<f64> {
    opts.as_ref().and_then(|t| {
        t.get::<f64>(long)
            .ok()
            .or_else(|| t.get::<f64>(short).ok())
            .or_else(|| t.get::<i64>(long).ok().map(|i| i as f64))
            .or_else(|| t.get::<i64>(short).ok().map(|i| i as f64))
    })
}

/// Parse a hex color string like "#FF0000" into (r, g, b).
fn parse_hex_color(hex: &str, func: &str, param: &str) -> Result<(u8, u8, u8), mlua::Error> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return Err(mlua::Error::external(format!(
            "{func}: {param} must be a 6-digit hex color (e.g. \"#FF0000\"), got \"#{hex}\""
        )));
    }
    let r = u8::from_str_radix(&hex[0..2], 16).map_err(|_| {
        mlua::Error::external(format!("{func}: invalid hex color for {param}: \"#{hex}\""))
    })?;
    let g = u8::from_str_radix(&hex[2..4], 16).map_err(|_| {
        mlua::Error::external(format!("{func}: invalid hex color for {param}: \"#{hex}\""))
    })?;
    let b = u8::from_str_radix(&hex[4..6], 16).map_err(|_| {
        mlua::Error::external(format!("{func}: invalid hex color for {param}: \"#{hex}\""))
    })?;
    Ok((r, g, b))
}

/// Register `qr.*` globals in the Lua VM.
pub fn register_qr_globals(lua: &Lua, mounts: Arc<MountTable>) -> Result<(), mlua::Error> {
    let qr_table = lua.create_table()?;

    // qr.generate(data, output, opts?) -> output path
    {
        let m = mounts.clone();
        qr_table.set(
            "generate",
            lua.create_function(move |_, args: MultiValue| {
                if args.is_empty() {
                    return Err(mlua::Error::external(
                        "qr.generate: missing required arguments 'data' (string) and 'output' (string)",
                    ));
                }
                let data = extract_string(&args, 0, "data", "qr.generate")?;
                let output = extract_string(&args, 1, "output", "qr.generate")?;
                let opts = extract_opts(&args, 2);

                // Parse options with short aliases
                let size = opt_number(&opts, "size", "s").unwrap_or(10.0) as u32;
                let margin = opt_number(&opts, "margin", "m").unwrap_or(4.0) as u32;
                let format = opt_string(&opts, "format", "f").unwrap_or_else(|| "png".to_string());
                let color_hex =
                    opt_string(&opts, "color", "c").unwrap_or_else(|| "#000000".to_string());
                let bg_hex = opt_string(&opts, "bg", "").unwrap_or_else(|| "#FFFFFF".to_string());

                // Encode QR code
                let code = QrCode::new(data.as_bytes()).map_err(|e| {
                    mlua::Error::external(format!("qr.generate: failed to encode data: {e}"))
                })?;

                // Resolve sandboxed output path (deep: creates intermediate dirs)
                let host_path = m.resolve_write_deep(&output).map_err(mlua::Error::external)?;
                if let Some(parent) = host_path.parent() {
                    std::fs::create_dir_all(parent).map_err(mlua::Error::external)?;
                }

                match format.to_lowercase().as_str() {
                    "svg" => {
                        let (cr, cg, cb) = parse_hex_color(&color_hex, "qr.generate", "color")?;
                        let (br, bg_r, bb) = parse_hex_color(&bg_hex, "qr.generate", "bg")?;
                        let dark_str = format!("rgb({cr},{cg},{cb})");
                        let light_str = format!("rgb({br},{bg_r},{bb})");
                        let svg_str = code
                            .render::<svg::Color<'_>>()
                            .dark_color(svg::Color(&dark_str))
                            .light_color(svg::Color(&light_str))
                            .quiet_zone(margin > 0)
                            .module_dimensions(size, size)
                            .build();
                        std::fs::write(&host_path, svg_str).map_err(|e| {
                            mlua::Error::external(format!(
                                "qr.generate: failed to write SVG to '{}': {e}",
                                output
                            ))
                        })?;
                    }
                    "png" => {
                        let (cr, cg, cb) = parse_hex_color(&color_hex, "qr.generate", "color")?;
                        let (br, bg_r, bb) = parse_hex_color(&bg_hex, "qr.generate", "bg")?;

                        // Get the QR matrix
                        let matrix = code.to_colors();
                        let modules = code.width() as u32;

                        // Calculate image dimensions
                        let img_size = modules * size + 2 * margin * size;
                        let mut imgbuf = vec![0u8; (img_size * img_size * 3) as usize];

                        // Fill background
                        for pixel in imgbuf.chunks_exact_mut(3) {
                            pixel[0] = br;
                            pixel[1] = bg_r;
                            pixel[2] = bb;
                        }

                        // Draw modules
                        for (y, row) in matrix.chunks(modules as usize).enumerate() {
                            for (x, &color) in row.iter().enumerate() {
                                if color == qrcode::Color::Dark {
                                    let px = (margin + x as u32) * size;
                                    let py = (margin + y as u32) * size;
                                    for dy in 0..size {
                                        for dx in 0..size {
                                            let ix = ((py + dy) * img_size + (px + dx)) as usize
                                                * 3;
                                            if ix + 2 < imgbuf.len() {
                                                imgbuf[ix] = cr;
                                                imgbuf[ix + 1] = cg;
                                                imgbuf[ix + 2] = cb;
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // Encode as PNG
                        let file = std::fs::File::create(&host_path).map_err(|e| {
                            mlua::Error::external(format!(
                                "qr.generate: failed to create '{}': {e}",
                                output
                            ))
                        })?;
                        let w = std::io::BufWriter::new(file);
                        let mut encoder =
                            png::Encoder::new(w, img_size, img_size);
                        encoder.set_color(png::ColorType::Rgb);
                        encoder.set_depth(png::BitDepth::Eight);
                        let mut writer = encoder.write_header().map_err(|e| {
                            mlua::Error::external(format!(
                                "qr.generate: failed to write PNG header: {e}"
                            ))
                        })?;
                        writer.write_image_data(&imgbuf).map_err(|e| {
                            mlua::Error::external(format!(
                                "qr.generate: failed to write PNG data: {e}"
                            ))
                        })?;
                    }
                    other => {
                        return Err(mlua::Error::external(format!(
                            "qr.generate: unsupported format '{other}', expected \"png\" or \"svg\""
                        )));
                    }
                }

                Ok(output)
            })?,
        )?;
    }

    // qr.to_string(data, opts?) -> string
    qr_table.set(
        "to_string",
        lua.create_function(|_, args: MultiValue| {
            let validated =
                validate_args(&args, QR_DOC.params("to_string"), "qr.to_string")?;
            let data = match &validated[0] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };

            let opts_table = match &validated[1] {
                Value::Table(t) => Some(t.clone()),
                _ => None,
            };

            let style = opts_table
                .as_ref()
                .and_then(|t| t.get::<String>("style").ok())
                .unwrap_or_else(|| "ascii".to_string());

            let code = QrCode::new(data.as_bytes()).map_err(|e| {
                mlua::Error::external(format!("qr.to_string: failed to encode data: {e}"))
            })?;

            let result = match style.as_str() {
                "ascii" => code
                    .render()
                    .dark_color("##")
                    .light_color("  ")
                    .quiet_zone(true)
                    .build(),
                "unicode" => {
                    // Use unicode half-block rendering (▀▄█ )
                    // Each character represents 2 vertical pixels
                    let matrix = code.to_colors();
                    let width = code.width() as usize;
                    let height = matrix.len() / width;
                    let mut lines = Vec::new();

                    // Process 2 rows at a time
                    let mut y = 0;
                    while y < height {
                        let mut line = String::new();
                        for x in 0..width {
                            let top = matrix[y * width + x] == qrcode::Color::Dark;
                            let bottom = if y + 1 < height {
                                matrix[(y + 1) * width + x] == qrcode::Color::Dark
                            } else {
                                false
                            };
                            let ch = match (top, bottom) {
                                (true, true) => '\u{2588}',   // █ (full block)
                                (true, false) => '\u{2580}',  // ▀ (upper half)
                                (false, true) => '\u{2584}',  // ▄ (lower half)
                                (false, false) => ' ',
                            };
                            line.push(ch);
                        }
                        lines.push(line);
                        y += 2;
                    }
                    lines.join("\n")
                }
                other => {
                    return Err(mlua::Error::external(format!(
                        "qr.to_string: unsupported style '{other}', expected \"ascii\" or \"unicode\""
                    )));
                }
            };

            Ok(result)
        })?,
    )?;

    register_help_functions(lua, &qr_table, &QR_DOC)?;

    lua.globals().set("qr", qr_table)?;
    wrap_module_with_help_hints(lua, "qr")?;

    Ok(())
}
