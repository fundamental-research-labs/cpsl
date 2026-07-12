//! Image processing module for the Luau sandbox.
//!
//! Exposes `image.info`, `image.resize`, `image.crop`, `image.rotate`,
//! `image.flip`, `image.convert`, `image.thumbnail`, `image.grayscale`,
//! `image.brightness`, `image.contrast` as globals.
//!
//! All heavy lifting is done by the `image` crate (Rust).
//! File I/O is sandboxed through MountTable — no direct host filesystem access.

use crate::lua_util::register_help_functions;
use crate::mount::MountTable;
use crate::sandbox::{
    arg_error, wrap_module_with_help_hints, FieldDoc, FnDoc, ModuleDoc, Param, ParamType,
    ReturnType,
};
use ab_glyph::{Font as AbGlyphFont, FontArc, PxScale, ScaleFont};
use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView, ImageFormat, Rgba, RgbaImage};
use imageproc::drawing::{
    draw_filled_circle_mut, draw_filled_rect_mut, draw_hollow_circle_mut, draw_hollow_rect_mut,
    draw_line_segment_mut, draw_text_mut,
};
use imageproc::geometric_transformations::{rotate_about_center, Interpolation};
use imageproc::rect::Rect;
use mlua::{Lua, MultiValue, Value};
use std::sync::Arc;

const IMAGE_RESIZE_OPTS_FIELDS: &[FieldDoc] = &[
    FieldDoc {
        name: "width",
        typ: "number",
        required: false,
        description: "Target width in pixels",
    },
    FieldDoc {
        name: "height",
        typ: "number",
        required: false,
        description: "Target height in pixels",
    },
    FieldDoc {
        name: "filter",
        typ: "string",
        required: false,
        description: "Resampling filter: nearest, bilinear, bicubic, lanczos3 (default)",
    },
];

const IMAGE_CROP_OPTS_FIELDS: &[FieldDoc] = &[
    FieldDoc {
        name: "x",
        typ: "number",
        required: true,
        description: "Left edge (pixels from left)",
    },
    FieldDoc {
        name: "y",
        typ: "number",
        required: true,
        description: "Top edge (pixels from top)",
    },
    FieldDoc {
        name: "width",
        typ: "number",
        required: true,
        description: "Crop width in pixels",
    },
    FieldDoc {
        name: "height",
        typ: "number",
        required: true,
        description: "Crop height in pixels",
    },
];

const IMAGE_NEW_OPTS_FIELDS: &[FieldDoc] = &[
    FieldDoc {
        name: "width",
        typ: "number",
        required: true,
        description: "Image width in pixels",
    },
    FieldDoc {
        name: "height",
        typ: "number",
        required: true,
        description: "Image height in pixels",
    },
    FieldDoc {
        name: "color",
        typ: "table",
        required: false,
        description: "{r, g, b} or {r, g, b, a} (default: white)",
    },
];

const IMAGE_TEXT_OPTS_FIELDS: &[FieldDoc] = &[
    FieldDoc {
        name: "text",
        typ: "string",
        required: true,
        description: "Text to draw",
    },
    FieldDoc {
        name: "x",
        typ: "number",
        required: true,
        description: "X anchor position",
    },
    FieldDoc {
        name: "y",
        typ: "number",
        required: true,
        description: "Y anchor position",
    },
    FieldDoc {
        name: "size",
        typ: "number",
        required: true,
        description: "Font height in pixels",
    },
    FieldDoc {
        name: "color",
        typ: "table",
        required: true,
        description: "{r, g, b} or {r, g, b, a}",
    },
    FieldDoc {
        name: "font",
        typ: "string",
        required: false,
        description: "System font name or path to .ttf/.otf",
    },
    FieldDoc {
        name: "align",
        typ: "string",
        required: false,
        description: "left (default), center, right",
    },
    FieldDoc {
        name: "valign",
        typ: "string",
        required: false,
        description: "top (default), center, bottom",
    },
];

const IMAGE_COMPOSITE_OPTS_FIELDS: &[FieldDoc] = &[
    FieldDoc { name: "x", typ: "number", required: false, description: "X offset for overlay (default 0)" },
    FieldDoc { name: "y", typ: "number", required: false, description: "Y offset for overlay (default 0)" },
    FieldDoc { name: "mode", typ: "string", required: false, description: "Blend mode: over (default), multiply, screen, overlay, darken, lighten, difference, add" },
];

pub(crate) static IMAGE_DOC: ModuleDoc = ModuleDoc {
    name: "image",
    summary: "Image processing (resize, crop, rotate, convert, adjust, composite, create, draw, text, blur, sharpen)",
    functions: &[
        FnDoc {
            name: "info",
            description:
                "Get image metadata without full decode. Returns {width, height, format}.",
            params: &[Param {
                name: "path",
                short: Some('p'),
                typ: ParamType::String,
                required: true,
                    fields: None,
            }],
            returns: ReturnType::Table,
            example: None,
        },
        FnDoc {
            name: "resize",
            description: "Resize an image. At least one of width or height is required.",
            params: &[
                Param {
                    name: "input",
                    short: Some('i'),
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
                    required: true,
                    fields: Some(IMAGE_RESIZE_OPTS_FIELDS),
                },
            ],
            returns: ReturnType::String,
            example: Some(r#"image.resize({input="/workspace/photo.jpg", output="/artifacts/thumb.jpg", width=200})"#),
        },
        FnDoc {
            name: "crop",
            description: "Crop an image.",
            params: &[
                Param {
                    name: "input",
                    short: Some('i'),
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
                    required: true,
                    fields: Some(IMAGE_CROP_OPTS_FIELDS),
                },
            ],
            returns: ReturnType::String,
            example: Some(r#"image.crop({input="photo.jpg", output="cropped.jpg", x=10, y=10, width=200, height=200})"#),
        },
        FnDoc {
            name: "rotate",
            description: "Rotate an image by 90, 180, or 270 degrees.",
            params: &[
                Param {
                    name: "input",
                    short: Some('i'),
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
                    name: "degrees",
                    short: Some('d'),
                    typ: ParamType::Number,
                    required: true,
                    fields: None,
                },
            ],
            returns: ReturnType::String,
            example: Some(r#"image.rotate({input="photo.jpg", output="rotated.jpg", degrees=90})"#),
        },
        FnDoc {
            name: "flip",
            description:
                "Flip an image. Direction: \"horizontal\" or \"vertical\".",
            params: &[
                Param {
                    name: "input",
                    short: Some('i'),
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
                    name: "direction",
                    short: Some('d'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
            ],
            returns: ReturnType::String,
            example: Some(r#"image.flip({input="photo.jpg", output="flipped.jpg", direction="horizontal"})"#),
        },
        FnDoc {
            name: "convert",
            description:
                "Convert image format. Output format is inferred from the file extension (PNG, JPEG, GIF, WebP, BMP, TIFF).",
            params: &[
                Param {
                    name: "input",
                    short: Some('i'),
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
            ],
            returns: ReturnType::String,
            example: None,
        },
        FnDoc {
            name: "thumbnail",
            description:
                "Create a thumbnail that fits within maxSize, preserving aspect ratio.",
            params: &[
                Param {
                    name: "input",
                    short: Some('i'),
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
                    name: "maxSize",
                    short: Some('s'),
                    typ: ParamType::Number,
                    required: true,
                    fields: None,
                },
            ],
            returns: ReturnType::String,
            example: Some(r#"image.thumbnail({input="photo.jpg", output="thumb.jpg", maxSize=150})"#),
        },
        FnDoc {
            name: "grayscale",
            description: "Convert an image to grayscale.",
            params: &[
                Param {
                    name: "input",
                    short: Some('i'),
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
            ],
            returns: ReturnType::String,
            example: None,
        },
        FnDoc {
            name: "brightness",
            description:
                "Adjust image brightness. Factor: -255 to 255 (negative = darker, positive = brighter).",
            params: &[
                Param {
                    name: "input",
                    short: Some('i'),
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
                    name: "factor",
                    short: Some('f'),
                    typ: ParamType::Number,
                    required: true,
                    fields: None,
                },
            ],
            returns: ReturnType::String,
            example: Some(r#"image.brightness({input="photo.jpg", output="bright.jpg", factor=50})"#),
        },
        FnDoc {
            name: "contrast",
            description:
                "Adjust image contrast. Factor: negative = less contrast, positive = more contrast.",
            params: &[
                Param {
                    name: "input",
                    short: Some('i'),
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
                    name: "factor",
                    short: Some('f'),
                    typ: ParamType::Number,
                    required: true,
                    fields: None,
                },
            ],
            returns: ReturnType::String,
            example: Some(r#"image.contrast({input="photo.jpg", output="contrast.jpg", factor=30})"#),
        },
        FnDoc {
            name: "composite",
            description: "Overlay one image on another.",
            params: &[
                Param {
                    name: "base",
                    short: Some('b'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "overlay",
                    short: None,
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
                    fields: Some(IMAGE_COMPOSITE_OPTS_FIELDS),
                },
            ],
            returns: ReturnType::String,
            example: Some(r#"image.composite({base="bg.png", overlay="logo.png", output="/artifacts/out.png", opts={x=10, y=10}})"#),
        },
        FnDoc {
            name: "new",
            description: "Create a blank image with a solid color.",
            params: &[
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
                    required: true,
                    fields: Some(IMAGE_NEW_OPTS_FIELDS),
                },
            ],
            returns: ReturnType::String,
            example: None,
        },
        FnDoc {
            name: "draw",
            description:
                "Draw shapes on an image. Shapes is a list of shape tables. Color is {r,g,b} or {r,g,b,a} (0-255, a defaults to 255). Coords: origin (0,0) is top-left; x increases right, y increases down. Rect: {type='rect', x, y, width, height, color, fill?} — x,y is top-left corner; fill=true (default) solid, false outline. Circle: {type='circle', x, y, radius, color, fill?} — x,y is center; fill=true (default) solid, false outline. Line: {type='line', x1, y1, x2, y2, color} — 1px line between two points.",
            params: &[
                Param {
                    name: "input",
                    short: Some('i'),
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
                    name: "shapes",
                    short: None,
                    typ: ParamType::Table,
                    required: true,
                    fields: None,
                },
            ],
            returns: ReturnType::String,
            example: Some(r#"image.draw({input="bg.png", output="out.png", shapes={{type="rect", x=10, y=10, width=100, height=50, color={r=255,g=0,b=0}}}})"#),
        },
        FnDoc {
            name: "text",
            description:
                "Draw text on an image. (x,y) is the anchor point. Multi-line: splits on \\n with 1.2x line spacing. Use image.measure_text() to get exact dimensions before drawing.",
            params: &[
                Param {
                    name: "input",
                    short: Some('i'),
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
                    required: true,
                    fields: Some(IMAGE_TEXT_OPTS_FIELDS),
                },
            ],
            returns: ReturnType::String,
            example: Some(r#"image.text({input="bg.png", output="out.png", text="Hello", x=50, y=50, size=24, color={r=255,g=255,b=255}})"#),
        },
        FnDoc {
            name: "fonts",
            description:
                "List available system fonts. Returns a list of {name, path, style} tables. Searches macOS and Linux system font directories.",
            params: &[],
            returns: ReturnType::Table,
            example: None,
        },
        FnDoc {
            name: "measure_text",
            description:
                "Measure text dimensions without rendering. Opts: {text, size, font?}. Returns {width, height, line_height}. width = widest line in pixels. height = total height of all lines with 1.2x line spacing. line_height = single line height. Same font/size as image.text(). Use before image.text() to compute exact positions for centering or layout.",
            params: &[Param {
                name: "opts",
                short: None,
                typ: ParamType::Table,
                required: true,
                    fields: None,
            }],
            returns: ReturnType::Table,
            example: None,
        },
        FnDoc {
            name: "blur",
            description:
                "Apply Gaussian blur to an image. Sigma controls blur strength (higher = more blur, must be > 0).",
            params: &[
                Param {
                    name: "input",
                    short: Some('i'),
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
                    name: "sigma",
                    short: Some('s'),
                    typ: ParamType::Number,
                    required: true,
                    fields: None,
                },
            ],
            returns: ReturnType::String,
            example: Some(r#"image.blur({input="photo.jpg", output="blurred.jpg", sigma=2.5})"#),
        },
        FnDoc {
            name: "sharpen",
            description:
                "Sharpen an image using unsharp mask. Sigma controls blur radius, amount controls intensity (default: sigma=1.0, amount=1.0).",
            params: &[
                Param {
                    name: "input",
                    short: Some('i'),
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
                    name: "sigma",
                    short: Some('s'),
                    typ: ParamType::Number,
                    required: false,
                    fields: None,
                },
                Param {
                    name: "amount",
                    short: Some('a'),
                    typ: ParamType::Number,
                    required: false,
                    fields: None,
                },
            ],
            returns: ReturnType::String,
            example: Some(r#"image.sharpen({input="photo.jpg", output="sharp.jpg", sigma=1.5, amount=1.2})"#),
        },
        FnDoc {
            name: "rotate_exact",
            description:
                "Rotate an image by an arbitrary angle in degrees (clockwise). Unlike rotate() which only does 90/180/270, this handles any angle. Background pixels default to transparent.",
            params: &[
                Param {
                    name: "input",
                    short: Some('i'),
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
                    name: "degrees",
                    short: Some('d'),
                    typ: ParamType::Number,
                    required: true,
                    fields: None,
                },
            ],
            returns: ReturnType::String,
            example: Some(r#"image.rotate_exact({input="photo.png", output="tilted.png", degrees=15})"#),
        },
    ],
};

/// Parse a filter type string into an image::imageops::FilterType.
fn parse_filter(s: &str) -> Result<FilterType, mlua::Error> {
    match s.to_lowercase().as_str() {
        "nearest" => Ok(FilterType::Nearest),
        "bilinear" | "triangle" => Ok(FilterType::Triangle),
        "bicubic" | "catmullrom" => Ok(FilterType::CatmullRom),
        "lanczos3" | "lanczos" => Ok(FilterType::Lanczos3),
        _ => Err(mlua::Error::external(format!(
            "image: unknown filter '{s}'. Use: nearest, bilinear, bicubic, lanczos3"
        ))),
    }
}

/// Guess output format from file extension.
fn format_from_ext(path: &str) -> Result<ImageFormat, mlua::Error> {
    let ext = path.rsplit('.').next().unwrap_or("").to_lowercase();
    match ext.as_str() {
        "png" => Ok(ImageFormat::Png),
        "jpg" | "jpeg" => Ok(ImageFormat::Jpeg),
        "gif" => Ok(ImageFormat::Gif),
        "webp" => Ok(ImageFormat::WebP),
        "bmp" => Ok(ImageFormat::Bmp),
        "tiff" | "tif" => Ok(ImageFormat::Tiff),
        _ => Err(mlua::Error::external(format!(
            "image: unsupported format '.{ext}'. Use: png, jpg, gif, webp, bmp, tiff"
        ))),
    }
}

/// Format name string for image.info().
fn format_name(fmt: ImageFormat) -> &'static str {
    match fmt {
        ImageFormat::Png => "png",
        ImageFormat::Jpeg => "jpeg",
        ImageFormat::Gif => "gif",
        ImageFormat::WebP => "webp",
        ImageFormat::Bmp => "bmp",
        ImageFormat::Tiff => "tiff",
        _ => "unknown",
    }
}

/// Load an image from a sandboxed path.
fn load_image(mounts: &MountTable, virtual_path: &str) -> Result<DynamicImage, mlua::Error> {
    let host_path = mounts
        .resolve_read(virtual_path)
        .map_err(mlua::Error::external)?;
    image::open(&host_path).map_err(|e| {
        mlua::Error::external(format!("image: failed to open '{}': {}", virtual_path, e))
    })
}

/// Save an image to a sandboxed path, creating parent directories as needed.
fn save_image(
    mounts: &MountTable,
    virtual_path: &str,
    img: &DynamicImage,
) -> Result<(), mlua::Error> {
    let host_path = mounts
        .resolve_write(virtual_path)
        .map_err(mlua::Error::external)?;
    if let Some(parent) = host_path.parent() {
        std::fs::create_dir_all(parent).map_err(mlua::Error::external)?;
    }
    let fmt = format_from_ext(virtual_path)?;
    img.save_with_format(&host_path, fmt).map_err(|e| {
        mlua::Error::external(format!("image: failed to save '{}': {}", virtual_path, e))
    })
}

/// Extract a string arg from ordered args or named table (dual-signature support).
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

/// Extract a number arg from ordered args or named table.
fn extract_number(
    args: &MultiValue,
    idx: usize,
    name: &str,
    func: &str,
) -> Result<f64, mlua::Error> {
    if args.len() > 1 || !matches!(args.get(0), Some(Value::Table(_))) {
        return match args.get(idx) {
            Some(Value::Number(n)) => Ok(*n),
            Some(Value::Integer(i)) => Ok(*i as f64),
            Some(v) => Err(mlua::Error::external(format!(
                "{func}: argument '{name}' expected number, got {}",
                v.type_name()
            ))),
            None => Err(mlua::Error::external(format!(
                "{func}: missing required argument '{name}'"
            ))),
        };
    }
    let t = match args.get(0) {
        Some(Value::Table(t)) => t,
        _ => unreachable!(),
    };
    let val: Value = t.get(idx as i64 + 1).unwrap_or(Value::Nil);
    match val {
        Value::Number(n) => return Ok(n),
        Value::Integer(i) => return Ok(i as f64),
        _ => {}
    }
    let val: Value = t.get(name).unwrap_or(Value::Nil);
    match val {
        Value::Number(n) => Ok(n),
        Value::Integer(i) => Ok(i as f64),
        _ => Err(mlua::Error::external(format!(
            "{func}: missing required argument '{name}' (number)"
        ))),
    }
}

/// Extract opts table from ordered args or named table.
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

/// Extract an RGBA color from an opts table field.
/// Color can be {r, g, b} or {r, g, b, a}. Returns default if field is absent.
fn extract_rgba_color(
    opts: &mlua::Table,
    field: &str,
    default: [u8; 4],
) -> Result<Rgba<u8>, mlua::Error> {
    let color_val: Value = opts.get(field).unwrap_or(Value::Nil);
    match color_val {
        Value::Table(ct) => {
            let r: u8 = ct
                .get::<f64>("r")
                .map_err(|_| mlua::Error::external("color.r is required (number)"))?
                as u8;
            let g: u8 = ct
                .get::<f64>("g")
                .map_err(|_| mlua::Error::external("color.g is required (number)"))?
                as u8;
            let b: u8 = ct
                .get::<f64>("b")
                .map_err(|_| mlua::Error::external("color.b is required (number)"))?
                as u8;
            let a: u8 = ct.get::<f64>("a").map(|n| n as u8).unwrap_or(255);
            Ok(Rgba([r, g, b, a]))
        }
        Value::Nil => Ok(Rgba(default)),
        _ => Err(mlua::Error::external(format!(
            "{field}: expected table {{r, g, b}} or {{r, g, b, a}}"
        ))),
    }
}

/// Blend `overlay` onto `base` at position (x, y) using the given blend mode.
fn blend_images(
    base: &DynamicImage,
    overlay: &DynamicImage,
    x: i64,
    y: i64,
    mode: &str,
) -> Result<RgbaImage, mlua::Error> {
    let mut result = base.to_rgba8();
    let over = overlay.to_rgba8();
    let (bw, bh) = (result.width() as i64, result.height() as i64);
    let (ow, oh) = (over.width() as i64, over.height() as i64);

    for oy in 0..oh {
        let by = y + oy;
        if by < 0 || by >= bh {
            continue;
        }
        for ox in 0..ow {
            let bx = x + ox;
            if bx < 0 || bx >= bw {
                continue;
            }
            let base_px = *result.get_pixel(bx as u32, by as u32);
            let over_px = *over.get_pixel(ox as u32, oy as u32);
            let blended = blend_pixel(base_px, over_px, mode)?;
            result.put_pixel(bx as u32, by as u32, blended);
        }
    }
    Ok(result)
}

/// Blend two RGBA pixels according to a blend mode.
fn blend_pixel(base: Rgba<u8>, over: Rgba<u8>, mode: &str) -> Result<Rgba<u8>, mlua::Error> {
    let [br, bg, bb, ba] = base.0;
    let [or, og, ob, oa] = over.0;

    // Overlay alpha factor (0.0 – 1.0)
    let a_over = oa as f32 / 255.0;
    let a_base = ba as f32 / 255.0;

    if a_over == 0.0 {
        return Ok(base);
    }

    // Compute blended channel value (before alpha compositing)
    let (cr, cg, cb) = match mode {
        "over" => {
            // Standard alpha composite — overlay's color directly
            (or as f32, og as f32, ob as f32)
        }
        "multiply" => {
            let r = (br as f32 * or as f32) / 255.0;
            let g = (bg as f32 * og as f32) / 255.0;
            let b = (bb as f32 * ob as f32) / 255.0;
            (r, g, b)
        }
        "screen" => {
            let r = 255.0 - ((255.0 - br as f32) * (255.0 - or as f32)) / 255.0;
            let g = 255.0 - ((255.0 - bg as f32) * (255.0 - og as f32)) / 255.0;
            let b = 255.0 - ((255.0 - bb as f32) * (255.0 - ob as f32)) / 255.0;
            (r, g, b)
        }
        "overlay" => {
            fn overlay_ch(b: f32, o: f32) -> f32 {
                if b < 128.0 {
                    (2.0 * b * o) / 255.0
                } else {
                    255.0 - (2.0 * (255.0 - b) * (255.0 - o)) / 255.0
                }
            }
            (
                overlay_ch(br as f32, or as f32),
                overlay_ch(bg as f32, og as f32),
                overlay_ch(bb as f32, ob as f32),
            )
        }
        "darken" => (
            (br as f32).min(or as f32),
            (bg as f32).min(og as f32),
            (bb as f32).min(ob as f32),
        ),
        "lighten" => (
            (br as f32).max(or as f32),
            (bg as f32).max(og as f32),
            (bb as f32).max(ob as f32),
        ),
        "difference" => (
            (br as f32 - or as f32).abs(),
            (bg as f32 - og as f32).abs(),
            (bb as f32 - ob as f32).abs(),
        ),
        "add" => (
            (br as f32 + or as f32).min(255.0),
            (bg as f32 + og as f32).min(255.0),
            (bb as f32 + ob as f32).min(255.0),
        ),
        _ => {
            return Err(mlua::Error::external(format!(
                "image.composite: unknown blend mode '{mode}'. Use: over, multiply, screen, overlay, darken, lighten, difference, add"
            )));
        }
    };

    // Alpha composite: result = over * a_over + base * a_base * (1 - a_over)
    let a_out = a_over + a_base * (1.0 - a_over);
    if a_out == 0.0 {
        return Ok(Rgba([0, 0, 0, 0]));
    }
    let inv_a_out = 1.0 / a_out;
    let rr = (cr * a_over + br as f32 * a_base * (1.0 - a_over)) * inv_a_out;
    let rg = (cg * a_over + bg as f32 * a_base * (1.0 - a_over)) * inv_a_out;
    let rb = (cb * a_over + bb as f32 * a_base * (1.0 - a_over)) * inv_a_out;
    let ra = a_out * 255.0;

    Ok(Rgba([
        rr.round().clamp(0.0, 255.0) as u8,
        rg.round().clamp(0.0, 255.0) as u8,
        rb.round().clamp(0.0, 255.0) as u8,
        ra.round().clamp(0.0, 255.0) as u8,
    ]))
}

/// Register `image.*` globals in the Lua VM.
pub fn register_image_globals(lua: &Lua, mounts: Arc<MountTable>) -> Result<(), mlua::Error> {
    let image_table = lua.create_table()?;

    register_image_info_and_transforms(lua, &image_table, mounts.clone())?;

    register_image_composition_and_drawing(lua, &image_table, mounts.clone())?;

    register_image_text_and_fonts(lua, &image_table, mounts.clone())?;

    register_image_filter_effects(lua, &image_table, mounts.clone())?;

    // PIL compatibility aliases
    let composite_fn: mlua::Function = image_table.get("composite")?;
    image_table.set("paste", composite_fn)?;

    register_help_functions(lua, &image_table, &IMAGE_DOC)?;

    lua.globals().set("image", image_table)?;
    wrap_module_with_help_hints(lua, "image")?;

    Ok(())
}

/// A discovered system font.
struct FontInfo {
    name: String,
    path: String,
    style: String,
}

/// Discover system fonts from standard OS directories.
fn discover_system_fonts() -> Vec<FontInfo> {
    let mut fonts = Vec::new();
    let dirs = if cfg!(target_os = "macos") {
        vec![
            "/System/Library/Fonts".to_string(),
            "/Library/Fonts".to_string(),
            std::env::var("HOME")
                .map(|h| format!("{h}/Library/Fonts"))
                .unwrap_or_default(),
        ]
    } else {
        vec![
            "/usr/share/fonts".to_string(),
            "/usr/local/share/fonts".to_string(),
            std::env::var("HOME")
                .map(|h| format!("{h}/.fonts"))
                .unwrap_or_default(),
            std::env::var("HOME")
                .map(|h| format!("{h}/.local/share/fonts"))
                .unwrap_or_default(),
        ]
    };

    for dir in &dirs {
        if dir.is_empty() {
            continue;
        }
        scan_font_dir(dir, &mut fonts);
    }
    fonts.sort_by(|a, b| a.name.cmp(&b.name));
    fonts
}

/// Recursively scan a directory for font files.
fn scan_font_dir(dir: &str, fonts: &mut Vec<FontInfo>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan_font_dir(&path.to_string_lossy(), fonts);
            continue;
        }
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        if !matches!(ext.as_str(), "ttf" | "otf" | "ttc" | "otc") {
            continue;
        }
        let file_stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        // Infer style from filename
        let lower = file_stem.to_lowercase();
        let style = if lower.contains("bolditalic") || lower.contains("bold-italic") {
            "bold-italic"
        } else if lower.contains("bold") {
            "bold"
        } else if lower.contains("italic") || lower.contains("oblique") {
            "italic"
        } else if lower.contains("light") {
            "light"
        } else if lower.contains("thin") {
            "thin"
        } else if lower.contains("medium") {
            "medium"
        } else {
            "regular"
        };

        // Clean name: remove style suffixes
        let name = file_stem
            .replace("-Bold", "")
            .replace("-Italic", "")
            .replace("-Regular", "")
            .replace("-Light", "")
            .replace("-Thin", "")
            .replace("-Medium", "")
            .replace("-BoldItalic", "")
            .replace("-Oblique", "");

        fonts.push(FontInfo {
            name,
            path: path.to_string_lossy().to_string(),
            style: style.to_string(),
        });
    }
}

/// Load a font by name or path. If None, uses first available system sans-serif.
fn load_font(font_spec: Option<&str>) -> Result<FontArc, mlua::Error> {
    if let Some(spec) = font_spec {
        // If it looks like a path (contains / or \), load directly
        if spec.contains('/') || spec.contains('\\') {
            let data = std::fs::read(spec).map_err(|e| {
                mlua::Error::external(format!("image.text: failed to read font '{spec}': {e}"))
            })?;
            return FontArc::try_from_vec(data).map_err(|_| {
                mlua::Error::external(format!("image.text: invalid font file '{spec}'"))
            });
        }

        // Search system fonts by name (case-insensitive)
        let lower = spec.to_lowercase();
        let fonts = discover_system_fonts();
        for font_info in &fonts {
            if font_info.name.to_lowercase() == lower
                || font_info
                    .path
                    .rsplit('/')
                    .next()
                    .unwrap_or("")
                    .to_lowercase()
                    .starts_with(&lower)
            {
                let data = std::fs::read(&font_info.path).map_err(|e| {
                    mlua::Error::external(format!(
                        "image.text: failed to read font '{}': {e}",
                        font_info.path
                    ))
                })?;
                return FontArc::try_from_vec(data).map_err(|_| {
                    mlua::Error::external(format!(
                        "image.text: invalid font file '{}'",
                        font_info.path
                    ))
                });
            }
        }
        return Err(mlua::Error::external(format!(
            "image.text: font '{spec}' not found. Use image.fonts() to list available fonts."
        )));
    }

    // Default: find a sans-serif font
    let preferred = [
        "Helvetica",
        "HelveticaNeue",
        "Arial",
        "SF-Pro",
        "SFPro",
        "DejaVuSans",
        "LiberationSans",
        "NotoSans",
    ];
    let fonts = discover_system_fonts();
    for pref in &preferred {
        let lower = pref.to_lowercase();
        if let Some(found) = fonts
            .iter()
            .find(|f| f.name.to_lowercase() == lower && f.style == "regular")
        {
            let data = std::fs::read(&found.path).ok();
            if let Some(data) = data {
                if let Ok(font) = FontArc::try_from_vec(data) {
                    return Ok(font);
                }
            }
        }
    }
    // Fallback: use the first regular font we find
    for font_info in &fonts {
        if font_info.style == "regular" {
            let data = std::fs::read(&font_info.path).ok();
            if let Some(data) = data {
                if let Ok(font) = FontArc::try_from_vec(data) {
                    return Ok(font);
                }
            }
        }
    }
    // Last resort: any font at all
    for font_info in &fonts {
        let data = std::fs::read(&font_info.path).ok();
        if let Some(data) = data {
            if let Ok(font) = FontArc::try_from_vec(data) {
                return Ok(font);
            }
        }
    }
    Err(mlua::Error::external(
        "image.text: no system fonts found. Use the font parameter with a path to a .ttf/.otf file.",
    ))
}

fn register_image_info_and_transforms(
    lua: &Lua,
    image_table: &mlua::Table,
    mounts: Arc<MountTable>,
) -> Result<(), mlua::Error> {
    // image.info(path) -> {width, height, format}
    {
        let m = mounts.clone();
        image_table.set(
            "info",
            lua.create_function(move |lua, args: MultiValue| {
                if args.is_empty() {
                    return Err(arg_error("image.info", IMAGE_DOC.params("info")));
                }
                let path = extract_string(&args, 0, "path", "image.info")?;
                let host_path = m.resolve_read(&path).map_err(mlua::Error::external)?;

                // Try to read dimensions without full decode using image::image_dimensions
                let (width, height) = image::image_dimensions(&host_path).map_err(|e| {
                    mlua::Error::external(format!("image.info: failed to read '{}': {}", path, e))
                })?;

                let fmt = ImageFormat::from_path(&host_path).unwrap_or(ImageFormat::Png);

                let result = lua.create_table()?;
                result.set("width", width)?;
                result.set("height", height)?;
                result.set("format", format_name(fmt))?;
                Ok(Value::Table(result))
            })?,
        )?;
    }

    // image.resize(input, output, {width?, height?, filter?})
    {
        let m = mounts.clone();
        image_table.set(
            "resize",
            lua.create_function(move |_, args: MultiValue| {
                if args.is_empty() {
                    return Err(arg_error("image.resize", IMAGE_DOC.params("resize")));
                }
                let input = extract_string(&args, 0, "input", "image.resize")?;
                let output = extract_string(&args, 1, "output", "image.resize")?;

                let opts = extract_opts(&args, 2);

                let new_width: Option<u32> = opts
                    .as_ref()
                    .and_then(|t| t.get::<f64>("width").ok().map(|n| n as u32));
                let new_height: Option<u32> = opts
                    .as_ref()
                    .and_then(|t| t.get::<f64>("height").ok().map(|n| n as u32));
                let filter_str: String = opts
                    .as_ref()
                    .and_then(|t| t.get::<String>("filter").ok())
                    .unwrap_or_else(|| "lanczos3".to_string());
                let filter = parse_filter(&filter_str)?;

                let img = load_image(&m, &input)?;
                let (orig_w, orig_h) = img.dimensions();

                let (w, h) = match (new_width, new_height) {
                    (Some(w), Some(h)) => (w, h),
                    (Some(w), None) => {
                        let h = ((w as f64 / orig_w as f64) * orig_h as f64).round() as u32;
                        (w, h.max(1))
                    }
                    (None, Some(h)) => {
                        let w = ((h as f64 / orig_h as f64) * orig_w as f64).round() as u32;
                        (w.max(1), h)
                    }
                    (None, None) => {
                        return Err(mlua::Error::external(
                            "image.resize: opts must specify at least 'width' or 'height'",
                        ));
                    }
                };

                let resized = img.resize_exact(w, h, filter);
                save_image(&m, &output, &resized)?;
                Ok(output)
            })?,
        )?;
    }

    // image.crop(input, output, {x, y, width, height})
    {
        let m = mounts.clone();
        image_table.set(
            "crop",
            lua.create_function(move |_, args: MultiValue| {
                if args.is_empty() {
                    return Err(arg_error("image.crop", IMAGE_DOC.params("crop")));
                }
                let input = extract_string(&args, 0, "input", "image.crop")?;
                let output = extract_string(&args, 1, "output", "image.crop")?;

                let opts = extract_opts(&args, 2).ok_or_else(|| {
                    mlua::Error::external("image.crop: missing opts table {x, y, width, height}")
                })?;

                let x: u32 = opts
                    .get::<f64>("x")
                    .map_err(|_| mlua::Error::external("image.crop: opts.x is required (number)"))?
                    as u32;
                let y: u32 = opts
                    .get::<f64>("y")
                    .map_err(|_| mlua::Error::external("image.crop: opts.y is required (number)"))?
                    as u32;
                let w: u32 = opts.get::<f64>("width").map_err(|_| {
                    mlua::Error::external("image.crop: opts.width is required (number)")
                })? as u32;
                let h: u32 = opts.get::<f64>("height").map_err(|_| {
                    mlua::Error::external("image.crop: opts.height is required (number)")
                })? as u32;

                let mut img = load_image(&m, &input)?;
                let cropped = img.crop(x, y, w, h);
                save_image(&m, &output, &cropped)?;
                Ok(output)
            })?,
        )?;
    }

    // image.rotate(input, output, degrees)
    {
        let m = mounts.clone();
        image_table.set(
            "rotate",
            lua.create_function(move |_, args: MultiValue| {
                if args.is_empty() {
                    return Err(arg_error("image.rotate", IMAGE_DOC.params("rotate")));
                }
                let input = extract_string(&args, 0, "input", "image.rotate")?;
                let output = extract_string(&args, 1, "output", "image.rotate")?;
                let degrees = extract_number(&args, 2, "degrees", "image.rotate")?;

                let img = load_image(&m, &input)?;
                let rotated = match degrees as i32 {
                    90 => img.rotate90(),
                    180 => img.rotate180(),
                    270 | -90 => img.rotate270(),
                    _ => {
                        return Err(mlua::Error::external(
                            "image.rotate: degrees must be 90, 180, or 270",
                        ));
                    }
                };
                save_image(&m, &output, &rotated)?;
                Ok(output)
            })?,
        )?;
    }

    // image.flip(input, output, direction)
    {
        let m = mounts.clone();
        image_table.set(
            "flip",
            lua.create_function(move |_, args: MultiValue| {
                if args.is_empty() {
                    return Err(arg_error("image.flip", IMAGE_DOC.params("flip")));
                }
                let input = extract_string(&args, 0, "input", "image.flip")?;
                let output = extract_string(&args, 1, "output", "image.flip")?;
                let direction = extract_string(&args, 2, "direction", "image.flip")?;

                let img = load_image(&m, &input)?;
                let flipped = match direction.to_lowercase().as_str() {
                    "horizontal" | "h" => img.fliph(),
                    "vertical" | "v" => img.flipv(),
                    _ => {
                        return Err(mlua::Error::external(
                            "image.flip: direction must be 'horizontal' or 'vertical'",
                        ));
                    }
                };
                save_image(&m, &output, &flipped)?;
                Ok(output)
            })?,
        )?;
    }

    // image.convert(input, output)
    {
        let m = mounts.clone();
        image_table.set(
            "convert",
            lua.create_function(move |_, args: MultiValue| {
                if args.is_empty() {
                    return Err(arg_error("image.convert", IMAGE_DOC.params("convert")));
                }
                let input = extract_string(&args, 0, "input", "image.convert")?;
                let output = extract_string(&args, 1, "output", "image.convert")?;

                let img = load_image(&m, &input)?;
                save_image(&m, &output, &img)?;
                Ok(output)
            })?,
        )?;
    }

    // image.thumbnail(input, output, maxSize)
    {
        let m = mounts.clone();
        image_table.set(
            "thumbnail",
            lua.create_function(move |_, args: MultiValue| {
                if args.is_empty() {
                    return Err(arg_error("image.thumbnail", IMAGE_DOC.params("thumbnail")));
                }
                let input = extract_string(&args, 0, "input", "image.thumbnail")?;
                let output = extract_string(&args, 1, "output", "image.thumbnail")?;
                let max_size = extract_number(&args, 2, "maxSize", "image.thumbnail")? as u32;

                let img = load_image(&m, &input)?;
                let thumb = img.thumbnail(max_size, max_size);
                save_image(&m, &output, &thumb)?;
                Ok(output)
            })?,
        )?;
    }

    // image.grayscale(input, output)
    {
        let m = mounts.clone();
        image_table.set(
            "grayscale",
            lua.create_function(move |_, args: MultiValue| {
                if args.is_empty() {
                    return Err(arg_error("image.grayscale", IMAGE_DOC.params("grayscale")));
                }
                let input = extract_string(&args, 0, "input", "image.grayscale")?;
                let output = extract_string(&args, 1, "output", "image.grayscale")?;

                let img = load_image(&m, &input)?;
                let gray = img.grayscale();
                save_image(&m, &output, &gray)?;
                Ok(output)
            })?,
        )?;
    }

    // image.brightness(input, output, factor)
    {
        let m = mounts.clone();
        image_table.set(
            "brightness",
            lua.create_function(move |_, args: MultiValue| {
                if args.is_empty() {
                    return Err(arg_error(
                        "image.brightness",
                        IMAGE_DOC.params("brightness"),
                    ));
                }
                let input = extract_string(&args, 0, "input", "image.brightness")?;
                let output = extract_string(&args, 1, "output", "image.brightness")?;
                let factor = extract_number(&args, 2, "factor", "image.brightness")? as i32;

                let img = load_image(&m, &input)?;
                let adjusted = img.brighten(factor);
                save_image(&m, &output, &adjusted)?;
                Ok(output)
            })?,
        )?;
    }

    // image.contrast(input, output, factor)
    {
        let m = mounts.clone();
        image_table.set(
            "contrast",
            lua.create_function(move |_, args: MultiValue| {
                if args.is_empty() {
                    return Err(arg_error("image.contrast", IMAGE_DOC.params("contrast")));
                }
                let input = extract_string(&args, 0, "input", "image.contrast")?;
                let output = extract_string(&args, 1, "output", "image.contrast")?;
                let factor = extract_number(&args, 2, "factor", "image.contrast")? as f32;

                let img = load_image(&m, &input)?;
                let adjusted = img.adjust_contrast(factor);
                save_image(&m, &output, &adjusted)?;
                Ok(output)
            })?,
        )?;
    }

    Ok(())
}

fn register_image_composition_and_drawing(
    lua: &Lua,
    image_table: &mlua::Table,
    mounts: Arc<MountTable>,
) -> Result<(), mlua::Error> {
    // image.composite(base, overlay, output, opts?)
    {
        let m = mounts.clone();
        image_table.set(
            "composite",
            lua.create_function(move |_, args: MultiValue| {
                if args.is_empty() {
                    return Err(arg_error("image.composite", IMAGE_DOC.params("composite")));
                }
                let base_path = extract_string(&args, 0, "base", "image.composite")?;
                let overlay_path = extract_string(&args, 1, "overlay", "image.composite")?;
                let output_path = extract_string(&args, 2, "output", "image.composite")?;

                let opts = extract_opts(&args, 3);
                let x: i64 = opts
                    .as_ref()
                    .and_then(|t| t.get::<f64>("x").ok().map(|n| n as i64))
                    .unwrap_or(0);
                let y: i64 = opts
                    .as_ref()
                    .and_then(|t| t.get::<f64>("y").ok().map(|n| n as i64))
                    .unwrap_or(0);
                let mode: String = opts
                    .as_ref()
                    .and_then(|t| t.get::<String>("mode").ok())
                    .unwrap_or_else(|| "over".to_string());

                let base_img = load_image(&m, &base_path)?;
                let overlay_img = load_image(&m, &overlay_path)?;

                let result = blend_images(&base_img, &overlay_img, x, y, &mode)?;
                save_image(&m, &output_path, &DynamicImage::ImageRgba8(result))?;
                Ok(output_path)
            })?,
        )?;
    }

    // image.new(output, opts)
    {
        let m = mounts.clone();
        image_table.set(
            "new",
            lua.create_function(move |_, args: MultiValue| {
                if args.is_empty() {
                    return Err(arg_error("image.new", IMAGE_DOC.params("new")));
                }
                let output_path = extract_string(&args, 0, "output", "image.new")?;

                let opts = extract_opts(&args, 1).ok_or_else(|| {
                    mlua::Error::external("image.new: missing opts table {width, height, color?}")
                })?;

                let width: u32 = opts.get::<f64>("width").map_err(|_| {
                    mlua::Error::external("image.new: opts.width is required (number)")
                })? as u32;
                let height: u32 = opts.get::<f64>("height").map_err(|_| {
                    mlua::Error::external("image.new: opts.height is required (number)")
                })? as u32;

                let color = extract_rgba_color(&opts, "color", [255, 255, 255, 255])?;

                let img = RgbaImage::from_pixel(width, height, color);
                save_image(&m, &output_path, &DynamicImage::ImageRgba8(img))?;
                Ok(output_path)
            })?,
        )?;
    }

    // image.draw(input, output, shapes)
    {
        let m = mounts.clone();
        image_table.set(
            "draw",
            lua.create_function(move |_, args: MultiValue| {
                if args.is_empty() {
                    return Err(arg_error("image.draw", IMAGE_DOC.params("draw")));
                }
                let input = extract_string(&args, 0, "input", "image.draw")?;
                let output = extract_string(&args, 1, "output", "image.draw")?;

                // Get shapes table (arg index 2)
                let shapes_table = match if args.len() > 2 {
                    args.get(2).cloned()
                } else {
                    // Single-table form
                    args.get(0).and_then(|v| {
                        if let Value::Table(t) = v {
                            t.get::<Value>("shapes").ok()
                        } else {
                            None
                        }
                    })
                } {
                    Some(Value::Table(t)) => t,
                    _ => return Err(mlua::Error::external("image.draw: missing shapes list")),
                };

                let img = load_image(&m, &input)?;
                let mut canvas = img.to_rgba8();

                // Iterate over shapes
                let mut shape_idx = 0usize;
                for result in shapes_table.sequence_values::<mlua::Table>() {
                    shape_idx += 1;
                    let i = shape_idx;
                    let shape = result.map_err(|_| {
                        mlua::Error::external(format!("image.draw: shape[{i}] is not a table"))
                    })?;

                    let shape_type: String = shape.get("type").map_err(|_| {
                        mlua::Error::external(format!(
                            "image.draw: shape[{i}].type is required (string)"
                        ))
                    })?;

                    let color = extract_rgba_color(&shape, "color", [0, 0, 0, 255])?;
                    let fill: bool = match shape.get::<Value>("fill") {
                        Ok(Value::Boolean(b)) => b,
                        _ => true, // default: filled
                    };

                    match shape_type.as_str() {
                        "rect" => {
                            let x: i32 = shape.get::<f64>("x").map_err(|_| {
                                mlua::Error::external(format!(
                                    "image.draw: shape[{i}] rect requires x"
                                ))
                            })? as i32;
                            let y: i32 = shape.get::<f64>("y").map_err(|_| {
                                mlua::Error::external(format!(
                                    "image.draw: shape[{i}] rect requires y"
                                ))
                            })? as i32;
                            let w: u32 = shape.get::<f64>("width").map_err(|_| {
                                mlua::Error::external(format!(
                                    "image.draw: shape[{i}] rect requires width"
                                ))
                            })? as u32;
                            let h: u32 = shape.get::<f64>("height").map_err(|_| {
                                mlua::Error::external(format!(
                                    "image.draw: shape[{i}] rect requires height"
                                ))
                            })? as u32;

                            let rect = Rect::at(x, y).of_size(w, h);
                            if fill {
                                draw_filled_rect_mut(&mut canvas, rect, color);
                            } else {
                                draw_hollow_rect_mut(&mut canvas, rect, color);
                            }
                        }
                        "circle" => {
                            let cx: i32 = shape.get::<f64>("x").map_err(|_| {
                                mlua::Error::external(format!(
                                    "image.draw: shape[{i}] circle requires x"
                                ))
                            })? as i32;
                            let cy: i32 = shape.get::<f64>("y").map_err(|_| {
                                mlua::Error::external(format!(
                                    "image.draw: shape[{i}] circle requires y"
                                ))
                            })? as i32;
                            let radius: i32 = shape.get::<f64>("radius").map_err(|_| {
                                mlua::Error::external(format!(
                                    "image.draw: shape[{i}] circle requires radius"
                                ))
                            })? as i32;

                            if fill {
                                draw_filled_circle_mut(&mut canvas, (cx, cy), radius, color);
                            } else {
                                draw_hollow_circle_mut(&mut canvas, (cx, cy), radius, color);
                            }
                        }
                        "line" => {
                            let x1: f32 = shape.get::<f64>("x1").map_err(|_| {
                                mlua::Error::external(format!(
                                    "image.draw: shape[{i}] line requires x1"
                                ))
                            })? as f32;
                            let y1: f32 = shape.get::<f64>("y1").map_err(|_| {
                                mlua::Error::external(format!(
                                    "image.draw: shape[{i}] line requires y1"
                                ))
                            })? as f32;
                            let x2: f32 = shape.get::<f64>("x2").map_err(|_| {
                                mlua::Error::external(format!(
                                    "image.draw: shape[{i}] line requires x2"
                                ))
                            })? as f32;
                            let y2: f32 = shape.get::<f64>("y2").map_err(|_| {
                                mlua::Error::external(format!(
                                    "image.draw: shape[{i}] line requires y2"
                                ))
                            })? as f32;

                            draw_line_segment_mut(&mut canvas, (x1, y1), (x2, y2), color);
                        }
                        other => {
                            return Err(mlua::Error::external(format!(
                                "image.draw: unknown shape type '{other}'. Use: rect, circle, line"
                            )));
                        }
                    }
                }

                save_image(&m, &output, &DynamicImage::ImageRgba8(canvas))?;
                Ok(output)
            })?,
        )?;
    }

    Ok(())
}

fn register_image_text_and_fonts(
    lua: &Lua,
    image_table: &mlua::Table,
    mounts: Arc<MountTable>,
) -> Result<(), mlua::Error> {
    // image.text(input, output, opts)
    {
        let m = mounts.clone();
        image_table.set(
        "text",
        lua.create_function(move |_, args: MultiValue| {
            if args.is_empty() {
                return Err(arg_error("image.text", IMAGE_DOC.params("text")));
            }
            let input = extract_string(&args, 0, "input", "image.text")?;
            let output_path = extract_string(&args, 1, "output", "image.text")?;

            let opts = extract_opts(&args, 2).ok_or_else(|| {
                mlua::Error::external(
                    "image.text: missing opts table {text, x, y, size, color, font?, align?, valign?}",
                )
            })?;

            let text: String = opts.get("text").map_err(|_| {
                mlua::Error::external("image.text: opts.text is required (string)")
            })?;
            let x: i32 = opts.get::<f64>("x").map_err(|_| {
                mlua::Error::external("image.text: opts.x is required (number)")
            })? as i32;
            let y: i32 = opts.get::<f64>("y").map_err(|_| {
                mlua::Error::external("image.text: opts.y is required (number)")
            })? as i32;
            let size: f32 = opts.get::<f64>("size").map_err(|_| {
                mlua::Error::external("image.text: opts.size is required (number)")
            })? as f32;
            let color = extract_rgba_color(&opts, "color", [0, 0, 0, 255])?;

            // Optional alignment fields
            let align: String = opts
                .get::<String>("align")
                .unwrap_or_else(|_| "left".to_string());
            let valign: String = opts
                .get::<String>("valign")
                .unwrap_or_else(|_| "top".to_string());

            if !matches!(align.as_str(), "left" | "center" | "right") {
                return Err(mlua::Error::external(
                    "image.text: opts.align must be 'left', 'center', or 'right'",
                ));
            }
            if !matches!(valign.as_str(), "top" | "center" | "bottom") {
                return Err(mlua::Error::external(
                    "image.text: opts.valign must be 'top', 'center', or 'bottom'",
                ));
            }

            // Load font
            let font_spec: Option<String> = opts.get("font").ok();
            let font = load_font(font_spec.as_deref())?;

            let img = load_image(&m, &input)?;
            let mut canvas = img.to_rgba8();

            let scale = PxScale::from(size);
            let scaled_font = font.as_scaled(scale);

            let lines: Vec<&str> = text.split('\n').collect();
            let line_spacing = size * 1.2;
            let total_height = lines.len() as f32 * line_spacing;

            // Compute vertical offset based on valign
            let base_y = match valign.as_str() {
                "center" => y - (total_height / 2.0) as i32,
                "bottom" => y - total_height as i32,
                _ => y, // "top"
            };

            for (line_idx, line) in lines.iter().enumerate() {
                let line_y = base_y + (line_idx as f32 * line_spacing) as i32;

                // Compute horizontal offset based on align
                let draw_x = match align.as_str() {
                    "center" | "right" => {
                        let line_width: f32 = line
                            .chars()
                            .map(|c| scaled_font.h_advance(font.glyph_id(c)))
                            .sum();
                        if align == "center" {
                            x - (line_width / 2.0) as i32
                        } else {
                            x - line_width as i32
                        }
                    }
                    _ => x, // "left"
                };

                draw_text_mut(&mut canvas, color, draw_x, line_y, scale, &font, line);
            }

            save_image(&m, &output_path, &DynamicImage::ImageRgba8(canvas))?;
            Ok(output_path)
        })?,
    )?;
    }

    // image.fonts()
    {
        image_table.set(
            "fonts",
            lua.create_function(|lua, _: MultiValue| {
                let fonts = discover_system_fonts();
                let result = lua.create_table()?;
                for (i, font_info) in fonts.iter().enumerate() {
                    let entry = lua.create_table()?;
                    entry.set("name", font_info.name.as_str())?;
                    entry.set("path", font_info.path.as_str())?;
                    entry.set("style", font_info.style.as_str())?;
                    result.set(i + 1, entry)?;
                }
                Ok(Value::Table(result))
            })?,
        )?;
    }

    // image.measure_text(opts)
    {
        image_table.set(
            "measure_text",
            lua.create_function(|lua, args: MultiValue| {
                if args.is_empty() {
                    return Err(arg_error(
                        "image.measure_text",
                        IMAGE_DOC.params("measure_text"),
                    ));
                }

                let opts = extract_opts(&args, 0).ok_or_else(|| {
                    mlua::Error::external(
                        "image.measure_text: expected opts table {text, size, font?}",
                    )
                })?;

                let text: String = opts.get("text").map_err(|_| {
                    mlua::Error::external("image.measure_text: opts.text is required (string)")
                })?;
                let size: f32 = opts.get::<f64>("size").map_err(|_| {
                    mlua::Error::external("image.measure_text: opts.size is required (number)")
                })? as f32;

                let font_spec: Option<String> = opts.get("font").ok();
                let font = load_font(font_spec.as_deref())?;

                let scale = PxScale::from(size);
                let scaled_font = font.as_scaled(scale);

                let line_height = scaled_font.height();
                let line_spacing = size * 1.2;

                let lines: Vec<&str> = text.split('\n').collect();
                let mut max_width: f32 = 0.0;

                for line in &lines {
                    let width: f32 = line
                        .chars()
                        .map(|c| scaled_font.h_advance(font.glyph_id(c)))
                        .sum();
                    if width > max_width {
                        max_width = width;
                    }
                }

                let total_height = if lines.is_empty() {
                    0.0
                } else {
                    // Last line uses actual line_height, previous lines use line_spacing
                    line_height + (lines.len() as f32 - 1.0) * line_spacing
                };

                let result = lua.create_table()?;
                result.set("width", max_width)?;
                result.set("height", total_height)?;
                result.set("line_height", line_height)?;
                Ok(Value::Table(result))
            })?,
        )?;
    }

    Ok(())
}

fn register_image_filter_effects(
    lua: &Lua,
    image_table: &mlua::Table,
    mounts: Arc<MountTable>,
) -> Result<(), mlua::Error> {
    // image.blur(input, output, sigma)
    {
        let m = mounts.clone();
        image_table.set(
            "blur",
            lua.create_function(move |_, args: MultiValue| {
                if args.is_empty() {
                    return Err(arg_error("image.blur", IMAGE_DOC.params("blur")));
                }
                let input = extract_string(&args, 0, "input", "image.blur")?;
                let output = extract_string(&args, 1, "output", "image.blur")?;
                let sigma = extract_number(&args, 2, "sigma", "image.blur")? as f32;

                if sigma <= 0.0 {
                    return Err(mlua::Error::external("image.blur: sigma must be > 0"));
                }

                let img = load_image(&m, &input)?;
                let blurred = img.blur(sigma);
                save_image(&m, &output, &blurred)?;
                Ok(output)
            })?,
        )?;
    }

    // image.sharpen(input, output, sigma?, amount?)
    {
        let m = mounts.clone();
        image_table.set(
            "sharpen",
            lua.create_function(move |_, args: MultiValue| {
                if args.is_empty() {
                    return Err(arg_error("image.sharpen", IMAGE_DOC.params("sharpen")));
                }
                let input = extract_string(&args, 0, "input", "image.sharpen")?;
                let output_path = extract_string(&args, 1, "output", "image.sharpen")?;

                // Optional sigma and amount — default both to 1.0
                let sigma = match extract_number(&args, 2, "sigma", "image.sharpen") {
                    Ok(n) => n as f32,
                    Err(_) => 1.0,
                };
                let amount = match extract_number(&args, 3, "amount", "image.sharpen") {
                    Ok(n) => n as f32,
                    Err(_) => 1.0,
                };

                let img = load_image(&m, &input)?;
                let result = img.unsharpen(sigma, amount as i32);
                save_image(&m, &output_path, &result)?;
                Ok(output_path)
            })?,
        )?;
    }

    // image.rotate_exact(input, output, degrees)
    {
        let m = mounts.clone();
        image_table.set(
            "rotate_exact",
            lua.create_function(move |_, args: MultiValue| {
                if args.is_empty() {
                    return Err(arg_error(
                        "image.rotate_exact",
                        IMAGE_DOC.params("rotate_exact"),
                    ));
                }
                let input = extract_string(&args, 0, "input", "image.rotate_exact")?;
                let output_path = extract_string(&args, 1, "output", "image.rotate_exact")?;
                let degrees = extract_number(&args, 2, "degrees", "image.rotate_exact")? as f32;

                let img = load_image(&m, &input)?;
                let rgba = img.to_rgba8();
                let theta = degrees.to_radians();
                let default_pixel = Rgba([0, 0, 0, 0]); // transparent background
                let rotated =
                    rotate_about_center(&rgba, theta, Interpolation::Bilinear, default_pixel);
                save_image(&m, &output_path, &DynamicImage::ImageRgba8(rotated))?;
                Ok(output_path)
            })?,
        )?;
    }

    Ok(())
}
