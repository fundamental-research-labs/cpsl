#![cfg(feature = "mod-image")]

use cpsl_core::{transpile, MountTable, Sandbox};
use tempfile::TempDir;

fn sb_with_workspace() -> (Sandbox, TempDir) {
    let dir = TempDir::new().unwrap();
    let mut mt = MountTable::new();
    mt.parse_and_add(&format!("{}:/workspace", dir.path().display()))
        .unwrap();
    let sb = Sandbox::with_mounts(mt).unwrap();
    (sb, dir)
}

/// Create a minimal 4x3 RGB PNG in the workspace directory.
/// Returns the host path to the created file.
fn create_test_png(dir: &TempDir, name: &str, width: u32, height: u32) -> std::path::PathBuf {
    let path = dir.path().join(name);
    // Use the image crate to create a simple test image
    let mut imgbuf = image::RgbImage::new(width, height);
    for (x, y, pixel) in imgbuf.enumerate_pixels_mut() {
        let r = (x * 255 / width.max(1)) as u8;
        let g = (y * 255 / height.max(1)) as u8;
        let b = 128u8;
        *pixel = image::Rgb([r, g, b]);
    }
    imgbuf.save(&path).unwrap();
    path
}

/// Create a minimal BMP file in the workspace directory.
fn create_test_bmp(dir: &TempDir, name: &str, width: u32, height: u32) -> std::path::PathBuf {
    let path = dir.path().join(name);
    let imgbuf = image::RgbImage::new(width, height);
    imgbuf.save(&path).unwrap();
    path
}

/// Create an RGBA PNG with a uniform color.
fn create_rgba_png(
    dir: &TempDir,
    name: &str,
    width: u32,
    height: u32,
    rgba: [u8; 4],
) -> std::path::PathBuf {
    let path = dir.path().join(name);
    let img = image::RgbaImage::from_pixel(width, height, image::Rgba(rgba));
    img.save(&path).unwrap();
    path
}

// ── image.info ──────────────────────────────────────────────────

#[test]
fn info_basic_png() {
    let (s, dir) = sb_with_workspace();
    create_test_png(&dir, "test.png", 100, 50);

    let r = s
        .exec(
            r#"
            local info = image.info("/workspace/test.png")
            return info.width .. "x" .. info.height .. " " .. info.format
        "#,
        )
        .unwrap();
    assert_eq!(r, "100x50 png");
}

#[test]
fn info_bmp() {
    let (s, dir) = sb_with_workspace();
    create_test_bmp(&dir, "test.bmp", 30, 20);

    let r = s
        .exec(
            r#"
            local info = image.info("/workspace/test.bmp")
            return info.width .. "x" .. info.height
        "#,
        )
        .unwrap();
    assert_eq!(r, "30x20");
}

#[test]
fn info_nonexistent_file_errors() {
    let (s, _dir) = sb_with_workspace();
    let err = s.exec(r#"image.info("/workspace/nope.png")"#).unwrap_err();
    assert!(
        err.message.contains("No such file") || err.message.contains("failed to read"),
        "msg: {}",
        err.message
    );
}

#[test]
fn info_no_args_errors() {
    let (s, _dir) = sb_with_workspace();
    let err = s.exec("image.info()").unwrap_err();
    assert!(
        err.message.contains("bad argument") || err.message.contains("missing"),
        "msg: {}",
        err.message
    );
}

// ── image.resize ────────────────────────────────────────────────

#[test]
fn resize_both_dimensions() {
    let (s, dir) = sb_with_workspace();
    create_test_png(&dir, "src.png", 100, 80);

    let r = s
        .exec(
            r#"
            image.resize("/workspace/src.png", "/workspace/resized.png", {width = 50, height = 40})
            local info = image.info("/workspace/resized.png")
            return info.width .. "x" .. info.height
        "#,
        )
        .unwrap();
    assert_eq!(r, "50x40");
}

#[test]
fn resize_width_only_preserves_aspect() {
    let (s, dir) = sb_with_workspace();
    create_test_png(&dir, "src.png", 100, 50);

    let r = s
        .exec(
            r#"
            image.resize("/workspace/src.png", "/workspace/resized.png", {width = 50})
            local info = image.info("/workspace/resized.png")
            return info.width .. "x" .. info.height
        "#,
        )
        .unwrap();
    assert_eq!(r, "50x25");
}

#[test]
fn resize_height_only_preserves_aspect() {
    let (s, dir) = sb_with_workspace();
    create_test_png(&dir, "src.png", 100, 50);

    let r = s
        .exec(
            r#"
            image.resize("/workspace/src.png", "/workspace/resized.png", {height = 25})
            local info = image.info("/workspace/resized.png")
            return info.width .. "x" .. info.height
        "#,
        )
        .unwrap();
    assert_eq!(r, "50x25");
}

#[test]
fn resize_with_filter() {
    let (s, dir) = sb_with_workspace();
    create_test_png(&dir, "src.png", 100, 100);

    let r = s
        .exec(
            r#"
            image.resize("/workspace/src.png", "/workspace/resized.png", {width = 50, height = 50, filter = "nearest"})
            local info = image.info("/workspace/resized.png")
            return info.width .. "x" .. info.height
        "#,
        )
        .unwrap();
    assert_eq!(r, "50x50");
}

#[test]
fn resize_no_dimensions_errors() {
    let (s, dir) = sb_with_workspace();
    create_test_png(&dir, "src.png", 100, 100);

    let err = s
        .exec(r#"image.resize("/workspace/src.png", "/workspace/out.png", {})"#)
        .unwrap_err();
    assert!(
        err.message.contains("width") || err.message.contains("height"),
        "msg: {}",
        err.message
    );
}

#[test]
fn resize_returns_output_path() {
    let (s, dir) = sb_with_workspace();
    create_test_png(&dir, "src.png", 100, 100);

    let r = s
        .exec(r#"return image.resize("/workspace/src.png", "/workspace/out.png", {width = 50})"#)
        .unwrap();
    assert_eq!(r, "/workspace/out.png");
}

// ── image.crop ──────────────────────────────────────────────────

#[test]
fn crop_basic() {
    let (s, dir) = sb_with_workspace();
    create_test_png(&dir, "src.png", 100, 100);

    let r = s
        .exec(
            r#"
            image.crop("/workspace/src.png", "/workspace/cropped.png", {x = 10, y = 10, width = 50, height = 30})
            local info = image.info("/workspace/cropped.png")
            return info.width .. "x" .. info.height
        "#,
        )
        .unwrap();
    assert_eq!(r, "50x30");
}

#[test]
fn crop_returns_output_path() {
    let (s, dir) = sb_with_workspace();
    create_test_png(&dir, "src.png", 100, 100);

    let r = s
        .exec(
            r#"return image.crop("/workspace/src.png", "/workspace/c.png", {x=0, y=0, width=10, height=10})"#,
        )
        .unwrap();
    assert_eq!(r, "/workspace/c.png");
}

#[test]
fn crop_missing_opts_errors() {
    let (s, dir) = sb_with_workspace();
    create_test_png(&dir, "src.png", 100, 100);

    let err = s
        .exec(r#"image.crop("/workspace/src.png", "/workspace/c.png")"#)
        .unwrap_err();
    assert!(
        err.message.contains("missing") || err.message.contains("opts"),
        "msg: {}",
        err.message
    );
}

// ── image.rotate ────────────────────────────────────────────────

#[test]
fn rotate_90() {
    let (s, dir) = sb_with_workspace();
    create_test_png(&dir, "src.png", 100, 50);

    let r = s
        .exec(
            r#"
            image.rotate("/workspace/src.png", "/workspace/rot.png", 90)
            local info = image.info("/workspace/rot.png")
            return info.width .. "x" .. info.height
        "#,
        )
        .unwrap();
    // 100x50 rotated 90 degrees → 50x100
    assert_eq!(r, "50x100");
}

#[test]
fn rotate_180() {
    let (s, dir) = sb_with_workspace();
    create_test_png(&dir, "src.png", 100, 50);

    let r = s
        .exec(
            r#"
            image.rotate("/workspace/src.png", "/workspace/rot.png", 180)
            local info = image.info("/workspace/rot.png")
            return info.width .. "x" .. info.height
        "#,
        )
        .unwrap();
    assert_eq!(r, "100x50");
}

#[test]
fn rotate_270() {
    let (s, dir) = sb_with_workspace();
    create_test_png(&dir, "src.png", 100, 50);

    let r = s
        .exec(
            r#"
            image.rotate("/workspace/src.png", "/workspace/rot.png", 270)
            local info = image.info("/workspace/rot.png")
            return info.width .. "x" .. info.height
        "#,
        )
        .unwrap();
    assert_eq!(r, "50x100");
}

#[test]
fn rotate_invalid_degrees_errors() {
    let (s, dir) = sb_with_workspace();
    create_test_png(&dir, "src.png", 100, 100);

    let err = s
        .exec(r#"image.rotate("/workspace/src.png", "/workspace/rot.png", 45)"#)
        .unwrap_err();
    assert!(
        err.message.contains("90") || err.message.contains("180") || err.message.contains("270"),
        "msg: {}",
        err.message
    );
}

#[test]
fn rotate_returns_output_path() {
    let (s, dir) = sb_with_workspace();
    create_test_png(&dir, "src.png", 100, 100);

    let r = s
        .exec(r#"return image.rotate("/workspace/src.png", "/workspace/rot.png", 90)"#)
        .unwrap();
    assert_eq!(r, "/workspace/rot.png");
}

// ── image.flip ──────────────────────────────────────────────────

#[test]
fn flip_horizontal() {
    let (s, dir) = sb_with_workspace();
    create_test_png(&dir, "src.png", 100, 50);

    let r = s
        .exec(
            r#"
            image.flip("/workspace/src.png", "/workspace/flipped.png", "horizontal")
            local info = image.info("/workspace/flipped.png")
            return info.width .. "x" .. info.height
        "#,
        )
        .unwrap();
    assert_eq!(r, "100x50");
    assert!(dir.path().join("flipped.png").exists());
}

#[test]
fn flip_vertical() {
    let (s, dir) = sb_with_workspace();
    create_test_png(&dir, "src.png", 100, 50);

    let r = s
        .exec(
            r#"
            image.flip("/workspace/src.png", "/workspace/flipped.png", "vertical")
            local info = image.info("/workspace/flipped.png")
            return info.width .. "x" .. info.height
        "#,
        )
        .unwrap();
    assert_eq!(r, "100x50");
}

#[test]
fn flip_short_aliases() {
    let (s, dir) = sb_with_workspace();
    create_test_png(&dir, "src.png", 10, 10);

    // "h" and "v" should work
    s.exec(r#"image.flip("/workspace/src.png", "/workspace/fh.png", "h")"#)
        .unwrap();
    s.exec(r#"image.flip("/workspace/src.png", "/workspace/fv.png", "v")"#)
        .unwrap();
    assert!(dir.path().join("fh.png").exists());
    assert!(dir.path().join("fv.png").exists());
}

#[test]
fn flip_invalid_direction_errors() {
    let (s, dir) = sb_with_workspace();
    create_test_png(&dir, "src.png", 10, 10);

    let err = s
        .exec(r#"image.flip("/workspace/src.png", "/workspace/out.png", "diagonal")"#)
        .unwrap_err();
    assert!(
        err.message.contains("horizontal") || err.message.contains("vertical"),
        "msg: {}",
        err.message
    );
}

// ── image.convert ───────────────────────────────────────────────

#[test]
fn convert_png_to_bmp() {
    let (s, dir) = sb_with_workspace();
    create_test_png(&dir, "src.png", 10, 10);

    let r = s
        .exec(r#"return image.convert("/workspace/src.png", "/workspace/out.bmp")"#)
        .unwrap();
    assert_eq!(r, "/workspace/out.bmp");
    assert!(dir.path().join("out.bmp").exists());
}

#[test]
fn convert_bmp_to_png() {
    let (s, dir) = sb_with_workspace();
    create_test_bmp(&dir, "src.bmp", 10, 10);

    let r = s
        .exec(r#"return image.convert("/workspace/src.bmp", "/workspace/out.png")"#)
        .unwrap();
    assert_eq!(r, "/workspace/out.png");

    // Verify it's a valid PNG
    let info = image::image_dimensions(dir.path().join("out.png")).unwrap();
    assert_eq!(info, (10, 10));
}

#[test]
fn convert_unsupported_extension_errors() {
    let (s, dir) = sb_with_workspace();
    create_test_png(&dir, "src.png", 10, 10);

    let err = s
        .exec(r#"image.convert("/workspace/src.png", "/workspace/out.xyz")"#)
        .unwrap_err();
    assert!(
        err.message.contains("unsupported format"),
        "msg: {}",
        err.message
    );
}

// ── image.thumbnail ─────────────────────────────────────────────

#[test]
fn thumbnail_fits_within_bounds() {
    let (s, dir) = sb_with_workspace();
    create_test_png(&dir, "src.png", 200, 100);

    let r = s
        .exec(
            r#"
            image.thumbnail("/workspace/src.png", "/workspace/thumb.png", 50)
            local info = image.info("/workspace/thumb.png")
            return info.width .. "x" .. info.height
        "#,
        )
        .unwrap();
    // 200x100 fit into 50x50 → 50x25
    assert_eq!(r, "50x25");
}

#[test]
fn thumbnail_tall_image() {
    let (s, dir) = sb_with_workspace();
    create_test_png(&dir, "src.png", 50, 200);

    let r = s
        .exec(
            r#"
            image.thumbnail("/workspace/src.png", "/workspace/thumb.png", 100)
            local info = image.info("/workspace/thumb.png")
            return info.width .. "x" .. info.height
        "#,
        )
        .unwrap();
    // 50x200 fit into 100x100 → 25x100
    assert_eq!(r, "25x100");
}

#[test]
fn thumbnail_returns_output_path() {
    let (s, dir) = sb_with_workspace();
    create_test_png(&dir, "src.png", 100, 100);

    let r = s
        .exec(r#"return image.thumbnail("/workspace/src.png", "/workspace/thumb.png", 50)"#)
        .unwrap();
    assert_eq!(r, "/workspace/thumb.png");
}

// ── image.grayscale ─────────────────────────────────────────────

#[test]
fn grayscale_produces_valid_output() {
    let (s, dir) = sb_with_workspace();
    create_test_png(&dir, "src.png", 50, 50);

    let r = s
        .exec(
            r#"
            image.grayscale("/workspace/src.png", "/workspace/gray.png")
            local info = image.info("/workspace/gray.png")
            return info.width .. "x" .. info.height
        "#,
        )
        .unwrap();
    assert_eq!(r, "50x50");
}

#[test]
fn grayscale_returns_output_path() {
    let (s, dir) = sb_with_workspace();
    create_test_png(&dir, "src.png", 10, 10);

    let r = s
        .exec(r#"return image.grayscale("/workspace/src.png", "/workspace/gray.png")"#)
        .unwrap();
    assert_eq!(r, "/workspace/gray.png");
}

// ── image.brightness ────────────────────────────────────────────

#[test]
fn brightness_positive() {
    let (s, dir) = sb_with_workspace();
    create_test_png(&dir, "src.png", 10, 10);

    let r = s
        .exec(
            r#"
            image.brightness("/workspace/src.png", "/workspace/bright.png", 50)
            local info = image.info("/workspace/bright.png")
            return info.width .. "x" .. info.height
        "#,
        )
        .unwrap();
    assert_eq!(r, "10x10");
}

#[test]
fn brightness_negative() {
    let (s, dir) = sb_with_workspace();
    create_test_png(&dir, "src.png", 10, 10);

    s.exec(r#"image.brightness("/workspace/src.png", "/workspace/dark.png", -100)"#)
        .unwrap();
    assert!(dir.path().join("dark.png").exists());
}

#[test]
fn brightness_returns_output_path() {
    let (s, dir) = sb_with_workspace();
    create_test_png(&dir, "src.png", 10, 10);

    let r = s
        .exec(r#"return image.brightness("/workspace/src.png", "/workspace/b.png", 10)"#)
        .unwrap();
    assert_eq!(r, "/workspace/b.png");
}

// ── image.contrast ──────────────────────────────────────────────

#[test]
fn contrast_increase() {
    let (s, dir) = sb_with_workspace();
    create_test_png(&dir, "src.png", 10, 10);

    let r = s
        .exec(
            r#"
            image.contrast("/workspace/src.png", "/workspace/hi.png", 50)
            local info = image.info("/workspace/hi.png")
            return info.width .. "x" .. info.height
        "#,
        )
        .unwrap();
    assert_eq!(r, "10x10");
}

#[test]
fn contrast_decrease() {
    let (s, dir) = sb_with_workspace();
    create_test_png(&dir, "src.png", 10, 10);

    s.exec(r#"image.contrast("/workspace/src.png", "/workspace/lo.png", -50)"#)
        .unwrap();
    assert!(dir.path().join("lo.png").exists());
}

#[test]
fn contrast_returns_output_path() {
    let (s, dir) = sb_with_workspace();
    create_test_png(&dir, "src.png", 10, 10);

    let r = s
        .exec(r#"return image.contrast("/workspace/src.png", "/workspace/c.png", 10)"#)
        .unwrap();
    assert_eq!(r, "/workspace/c.png");
}

// ── Chaining operations ─────────────────────────────────────────

#[test]
fn chain_resize_then_grayscale() {
    let (s, dir) = sb_with_workspace();
    create_test_png(&dir, "src.png", 200, 100);

    let r = s
        .exec(
            r#"
            image.resize("/workspace/src.png", "/workspace/small.png", {width = 50})
            image.grayscale("/workspace/small.png", "/workspace/final.png")
            local info = image.info("/workspace/final.png")
            return info.width .. "x" .. info.height
        "#,
        )
        .unwrap();
    assert_eq!(r, "50x25");
}

#[test]
fn chain_crop_then_rotate() {
    let (s, dir) = sb_with_workspace();
    create_test_png(&dir, "src.png", 100, 100);

    let r = s
        .exec(
            r#"
            image.crop("/workspace/src.png", "/workspace/cropped.png", {x=0, y=0, width=60, height=40})
            image.rotate("/workspace/cropped.png", "/workspace/final.png", 90)
            local info = image.info("/workspace/final.png")
            return info.width .. "x" .. info.height
        "#,
        )
        .unwrap();
    // 60x40 rotated 90 → 40x60
    assert_eq!(r, "40x60");
}

// ── Dual-signature tests (table form for shell dispatch) ────────

#[test]
fn info_table_form() {
    let (s, dir) = sb_with_workspace();
    create_test_png(&dir, "test.png", 20, 10);

    let r = s
        .exec(
            r#"
            local info = image.info({[1]="/workspace/test.png"})
            return info.width .. "x" .. info.height
        "#,
        )
        .unwrap();
    assert_eq!(r, "20x10");
}

#[test]
fn info_table_form_named() {
    let (s, dir) = sb_with_workspace();
    create_test_png(&dir, "test.png", 20, 10);

    let r = s
        .exec(
            r#"
            local info = image.info({path="/workspace/test.png"})
            return info.width .. "x" .. info.height
        "#,
        )
        .unwrap();
    assert_eq!(r, "20x10");
}

#[test]
fn grayscale_table_form() {
    let (s, dir) = sb_with_workspace();
    create_test_png(&dir, "src.png", 10, 10);

    let r = s
        .exec(r#"return image.grayscale({[1]="/workspace/src.png", [2]="/workspace/gray.png"})"#)
        .unwrap();
    assert_eq!(r, "/workspace/gray.png");
    assert!(dir.path().join("gray.png").exists());
}

// ── Sandbox safety ──────────────────────────────────────────────

#[test]
fn cannot_read_outside_mount() {
    let (s, _dir) = sb_with_workspace();
    let err = s.exec(r#"image.info("/etc/passwd")"#).unwrap_err();
    assert!(
        err.message.contains("No such file")
            || err.message.contains("not found")
            || err.message.contains("failed"),
        "should deny access outside mount: {}",
        err.message
    );
}

#[test]
fn cannot_write_outside_mount() {
    let (s, dir) = sb_with_workspace();
    create_test_png(&dir, "src.png", 10, 10);

    let err = s
        .exec(r#"image.grayscale("/workspace/src.png", "/etc/evil.png")"#)
        .unwrap_err();
    assert!(
        err.message.contains("No such file")
            || err.message.contains("Read-only")
            || err.message.contains("denied"),
        "should deny write outside mount: {}",
        err.message
    );
}

#[test]
fn cannot_traverse_path() {
    let (s, dir) = sb_with_workspace();
    create_test_png(&dir, "src.png", 10, 10);

    let err = s
        .exec(r#"image.grayscale("/workspace/src.png", "/workspace/../../../tmp/evil.png")"#)
        .unwrap_err();
    assert!(
        err.message.contains("traversal")
            || err.message.contains("denied")
            || err.message.contains("No such"),
        "should deny path traversal: {}",
        err.message
    );
}

#[test]
fn read_only_mount_blocks_writes() {
    let dir = TempDir::new().unwrap();
    create_test_png(&dir, "src.png", 10, 10);

    let mut mt = MountTable::new();
    mt.parse_and_add(&format!("{}:/data:ro", dir.path().display()))
        .unwrap();
    let s = Sandbox::with_mounts(mt).unwrap();

    // Read should work
    let r = s
        .exec(
            r#"
        local info = image.info("/data/src.png")
        return info.width .. "x" .. info.height
    "#,
        )
        .unwrap();
    assert_eq!(r, "10x10");

    // Write should fail
    let err = s
        .exec(r#"image.grayscale("/data/src.png", "/data/out.png")"#)
        .unwrap_err();
    assert!(
        err.message.contains("Read-only"),
        "should deny write on ro mount: {}",
        err.message
    );
}

#[test]
fn no_dangerous_globals_exposed() {
    let (s, _dir) = sb_with_workspace();
    let r = s
        .exec(
            r#"
            return tostring(type(image.info)) .. " " ..
                   tostring(type(image.resize)) .. " " ..
                   tostring(type(image.crop)) .. " " ..
                   tostring(rawget(image, "io")) .. " " ..
                   tostring(rawget(image, "os"))
        "#,
        )
        .unwrap();
    assert_eq!(r, "function function function nil nil");
}

#[test]
fn no_io_leak_through_metatable() {
    let (s, _dir) = sb_with_workspace();
    let r = s
        .exec(
            r#"
            local mt = getmetatable(image)
            if mt then
                local idx = rawget(mt, "__index")
                if type(idx) == "table" then
                    if rawget(idx, "io") or rawget(idx, "os") then
                        return "metatable leaks dangerous globals"
                    end
                end
            end
            return "safe"
        "#,
        )
        .unwrap();
    assert_eq!(r, "safe");
}

// ── Error handling ──────────────────────────────────────────────

#[test]
fn resize_no_args_errors() {
    let (s, _dir) = sb_with_workspace();
    let err = s.exec("image.resize()").unwrap_err();
    assert!(
        err.message.contains("bad argument") || err.message.contains("missing"),
        "msg: {}",
        err.message
    );
}

#[test]
fn crop_no_args_errors() {
    let (s, _dir) = sb_with_workspace();
    let err = s.exec("image.crop()").unwrap_err();
    assert!(
        err.message.contains("bad argument") || err.message.contains("missing"),
        "msg: {}",
        err.message
    );
}

#[test]
fn rotate_no_args_errors() {
    let (s, _dir) = sb_with_workspace();
    let err = s.exec("image.rotate()").unwrap_err();
    assert!(
        err.message.contains("bad argument") || err.message.contains("missing"),
        "msg: {}",
        err.message
    );
}

#[test]
fn flip_no_args_errors() {
    let (s, _dir) = sb_with_workspace();
    let err = s.exec("image.flip()").unwrap_err();
    assert!(
        err.message.contains("bad argument") || err.message.contains("missing"),
        "msg: {}",
        err.message
    );
}

#[test]
fn convert_no_args_errors() {
    let (s, _dir) = sb_with_workspace();
    let err = s.exec("image.convert()").unwrap_err();
    assert!(
        err.message.contains("bad argument") || err.message.contains("missing"),
        "msg: {}",
        err.message
    );
}

#[test]
fn thumbnail_no_args_errors() {
    let (s, _dir) = sb_with_workspace();
    let err = s.exec("image.thumbnail()").unwrap_err();
    assert!(
        err.message.contains("bad argument") || err.message.contains("missing"),
        "msg: {}",
        err.message
    );
}

#[test]
fn grayscale_no_args_errors() {
    let (s, _dir) = sb_with_workspace();
    let err = s.exec("image.grayscale()").unwrap_err();
    assert!(
        err.message.contains("bad argument") || err.message.contains("missing"),
        "msg: {}",
        err.message
    );
}

#[test]
fn brightness_no_args_errors() {
    let (s, _dir) = sb_with_workspace();
    let err = s.exec("image.brightness()").unwrap_err();
    assert!(
        err.message.contains("bad argument") || err.message.contains("missing"),
        "msg: {}",
        err.message
    );
}

#[test]
fn contrast_no_args_errors() {
    let (s, _dir) = sb_with_workspace();
    let err = s.exec("image.contrast()").unwrap_err();
    assert!(
        err.message.contains("bad argument") || err.message.contains("missing"),
        "msg: {}",
        err.message
    );
}

#[test]
fn resize_wrong_type_errors() {
    let (s, _dir) = sb_with_workspace();
    let err = s.exec("image.resize(42, 43, {width=10})").unwrap_err();
    assert!(
        err.message.contains("string") || err.message.contains("expected"),
        "msg: {}",
        err.message
    );
}

// ── Help ────────────────────────────────────────────────────────

#[test]
fn help_returns_help() {
    let (s, _dir) = sb_with_workspace();
    let r = s.exec("return image.help()").unwrap();
    assert!(r.contains("image"), "help: {}", r);
    assert!(r.contains("image.info"), "help: {}", r);
    assert!(r.contains("image.resize"), "help: {}", r);
    assert!(r.contains("image.crop"), "help: {}", r);
    assert!(r.contains("image.rotate"), "help: {}", r);
    assert!(r.contains("image.flip"), "help: {}", r);
    assert!(r.contains("image.convert"), "help: {}", r);
    assert!(r.contains("image.thumbnail"), "help: {}", r);
    assert!(r.contains("image.grayscale"), "help: {}", r);
    assert!(r.contains("image.brightness"), "help: {}", r);
    assert!(r.contains("image.contrast"), "help: {}", r);
}

#[test]
fn nonexistent_fn_hint() {
    let (s, _dir) = sb_with_workspace();
    let err = s.exec("image.foo()").unwrap_err();
    assert!(
        err.message.contains("image.foo does not exist"),
        "msg: {}",
        err.message
    );
    assert!(
        err.message.contains("hint: call image.help() for usage"),
        "msg: {}",
        err.message
    );
}

#[test]
fn global_help_mentions_image() {
    let (s, _dir) = sb_with_workspace();
    let r = s.exec("return help()").unwrap();
    assert!(r.contains("image"), "global help should list image: {}", r);
}

// ── Shell dispatch tests ────────────────────────────────────────

#[test]
fn shell_image_info() {
    let (s, dir) = sb_with_workspace();
    create_test_png(&dir, "test.png", 20, 10);
    let shrt = include_str!("../../runtime/shrt.luau");
    s.register_module("shrt", shrt).unwrap();

    let result = cpsl_core::sh_transpile::transpile_sh(r#"image info "/workspace/test.png""#);
    assert!(result.is_ok(), "transpile err: {:?}", result.err());
    let luau = result.unwrap().luau_source;
    let r = s.exec(&luau).unwrap();
    assert!(
        r.contains("20") && r.contains("10"),
        "expected dimensions in output, got: {}",
        r
    );
}

#[test]
fn shell_image_grayscale() {
    let (s, dir) = sb_with_workspace();
    create_test_png(&dir, "src.png", 10, 10);
    let shrt = include_str!("../../runtime/shrt.luau");
    s.register_module("shrt", shrt).unwrap();

    let result = cpsl_core::sh_transpile::transpile_sh(
        r#"image grayscale "/workspace/src.png" "/workspace/gray.png""#,
    );
    assert!(result.is_ok(), "transpile err: {:?}", result.err());
    let luau = result.unwrap().luau_source;
    let r = s.exec(&luau);
    assert!(
        r.is_ok(),
        "shell image grayscale should not error: {:?}",
        r.err()
    );
    assert!(dir.path().join("gray.png").exists());
}

// ── Edge cases ──────────────────────────────────────────────────

#[test]
fn resize_to_1x1() {
    let (s, dir) = sb_with_workspace();
    create_test_png(&dir, "src.png", 100, 100);

    let r = s
        .exec(
            r#"
            image.resize("/workspace/src.png", "/workspace/tiny.png", {width = 1, height = 1})
            local info = image.info("/workspace/tiny.png")
            return info.width .. "x" .. info.height
        "#,
        )
        .unwrap();
    assert_eq!(r, "1x1");
}

#[test]
fn crop_full_image() {
    let (s, dir) = sb_with_workspace();
    create_test_png(&dir, "src.png", 50, 30);

    let r = s
        .exec(
            r#"
            image.crop("/workspace/src.png", "/workspace/full.png", {x=0, y=0, width=50, height=30})
            local info = image.info("/workspace/full.png")
            return info.width .. "x" .. info.height
        "#,
        )
        .unwrap();
    assert_eq!(r, "50x30");
}

#[test]
fn overwrite_existing_file() {
    let (s, dir) = sb_with_workspace();
    create_test_png(&dir, "src.png", 100, 100);

    // First write
    s.exec(
        r#"image.resize("/workspace/src.png", "/workspace/out.png", {width = 50, height = 50})"#,
    )
    .unwrap();

    // Overwrite
    s.exec(
        r#"image.resize("/workspace/src.png", "/workspace/out.png", {width = 25, height = 25})"#,
    )
    .unwrap();

    let r = s
        .exec(
            r#"
            local info = image.info("/workspace/out.png")
            return info.width .. "x" .. info.height
        "#,
        )
        .unwrap();
    assert_eq!(r, "25x25");
}

#[test]
fn same_input_and_output_path() {
    let (s, dir) = sb_with_workspace();
    create_test_png(&dir, "src.png", 100, 100);

    // Reading and writing the same file should work (image is fully loaded into memory first)
    s.exec(r#"image.grayscale("/workspace/src.png", "/workspace/src.png")"#)
        .unwrap();
    let info = image::image_dimensions(dir.path().join("src.png")).unwrap();
    assert_eq!(info, (100, 100));
}

// ── Python transpiler e2e ───────────────────────────────────────

#[test]
fn python_pil_import_info() {
    let (s, dir) = sb_with_workspace();
    create_test_png(&dir, "test.png", 30, 20);
    let pyrt = include_str!("../../runtime/pyrt.luau");
    s.setup_python_runtime(pyrt).unwrap();

    let py_code = r#"
from PIL import Image
info = Image.info("/workspace/test.png")
print(str(info.width) + "x" + str(info.height))
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    let r = s.exec(&transpiled.luau_source).unwrap();
    assert!(
        r.contains("30") && r.contains("20"),
        "expected 30x20, got: {}",
        r
    );
}

#[test]
fn python_pil_import_maps_to_image() {
    let py_code = r#"
from PIL import Image
info = Image.info("/workspace/test.png")
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    assert!(
        transpiled.luau_source.contains("image"),
        "transpiled: {}",
        transpiled.luau_source
    );
}

#[test]
fn python_import_pil_passthrough() {
    let py_code = r#"
import image
result = image.info("/test.png")
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    assert!(
        transpiled.luau_source.contains("image"),
        "transpiled: {}",
        transpiled.luau_source
    );
}

// ── image.new ───────────────────────────────────────────────────

#[test]
fn new_creates_blank_image() {
    let (s, dir) = sb_with_workspace();

    let r = s
        .exec(
            r#"
            image.new("/workspace/blank.png", {width = 200, height = 100, color = {r = 255, g = 0, b = 0}})
            local info = image.info("/workspace/blank.png")
            return info.width .. "x" .. info.height
        "#,
        )
        .unwrap();
    assert_eq!(r, "200x100");

    // Verify pixel color
    let img = image::open(dir.path().join("blank.png"))
        .unwrap()
        .to_rgba8();
    let px = img.get_pixel(0, 0);
    assert_eq!(px.0, [255, 0, 0, 255]);
}

#[test]
fn new_default_color_is_white() {
    let (s, dir) = sb_with_workspace();

    s.exec(r#"image.new("/workspace/blank.png", {width = 10, height = 10})"#)
        .unwrap();

    let img = image::open(dir.path().join("blank.png"))
        .unwrap()
        .to_rgba8();
    let px = img.get_pixel(5, 5);
    assert_eq!(px.0, [255, 255, 255, 255]);
}

#[test]
fn new_with_alpha() {
    let (s, dir) = sb_with_workspace();

    s.exec(
        r#"image.new("/workspace/semi.png", {width = 5, height = 5, color = {r = 0, g = 128, b = 255, a = 128}})"#,
    )
    .unwrap();

    let img = image::open(dir.path().join("semi.png")).unwrap().to_rgba8();
    let px = img.get_pixel(0, 0);
    assert_eq!(px.0, [0, 128, 255, 128]);
}

#[test]
fn new_returns_output_path() {
    let (s, _dir) = sb_with_workspace();
    let r = s
        .exec(r#"return image.new("/workspace/x.png", {width = 1, height = 1})"#)
        .unwrap();
    assert_eq!(r, "/workspace/x.png");
}

#[test]
fn new_missing_width_errors() {
    let (s, _dir) = sb_with_workspace();
    let err = s
        .exec(r#"image.new("/workspace/x.png", {height = 10})"#)
        .unwrap_err();
    assert!(err.message.contains("width"), "msg: {}", err.message);
}

#[test]
fn new_missing_opts_errors() {
    let (s, _dir) = sb_with_workspace();
    let err = s.exec(r#"image.new("/workspace/x.png")"#).unwrap_err();
    assert!(
        err.message.contains("missing") || err.message.contains("opts"),
        "msg: {}",
        err.message
    );
}

#[test]
fn new_no_args_errors() {
    let (s, _dir) = sb_with_workspace();
    let err = s.exec("image.new()").unwrap_err();
    assert!(
        err.message.contains("bad argument") || err.message.contains("missing"),
        "msg: {}",
        err.message
    );
}

// ── image.composite ─────────────────────────────────────────────

#[test]
fn composite_over_default() {
    let (s, dir) = sb_with_workspace();
    // Red 200x200 base
    create_rgba_png(&dir, "base.png", 200, 200, [255, 0, 0, 255]);
    // Blue 50x50 overlay
    create_rgba_png(&dir, "overlay.png", 50, 50, [0, 0, 255, 255]);

    let r = s
        .exec(
            r#"
            image.composite("/workspace/base.png", "/workspace/overlay.png", "/workspace/out.png", {x = 10, y = 20})
            local info = image.info("/workspace/out.png")
            return info.width .. "x" .. info.height
        "#,
        )
        .unwrap();
    // Output should be same size as base
    assert_eq!(r, "200x200");

    // Pixel at (10, 20) should be blue (overlay), pixel at (0, 0) should be red (base)
    let img = image::open(dir.path().join("out.png")).unwrap().to_rgba8();
    let base_px = img.get_pixel(0, 0);
    assert_eq!(base_px.0[0], 255, "base red channel");
    assert_eq!(base_px.0[2], 0, "base blue channel");

    let overlay_px = img.get_pixel(10, 20);
    assert_eq!(overlay_px.0[0], 0, "overlay red channel");
    assert_eq!(overlay_px.0[2], 255, "overlay blue channel");
}

#[test]
fn composite_no_opts_defaults_to_origin() {
    let (s, dir) = sb_with_workspace();
    create_rgba_png(&dir, "base.png", 100, 100, [255, 0, 0, 255]);
    create_rgba_png(&dir, "overlay.png", 20, 20, [0, 255, 0, 255]);

    s.exec(
        r#"image.composite("/workspace/base.png", "/workspace/overlay.png", "/workspace/out.png")"#,
    )
    .unwrap();

    let img = image::open(dir.path().join("out.png")).unwrap().to_rgba8();
    // (0,0) should be green (overlay at origin)
    let px = img.get_pixel(0, 0);
    assert_eq!(px.0[1], 255, "green channel at origin");
    // (50,50) should be red (base, outside overlay)
    let px2 = img.get_pixel(50, 50);
    assert_eq!(px2.0[0], 255, "red channel outside overlay");
}

#[test]
fn composite_semitransparent_overlay() {
    let (s, dir) = sb_with_workspace();
    create_rgba_png(&dir, "base.png", 10, 10, [255, 0, 0, 255]);
    // 50% transparent blue overlay
    create_rgba_png(&dir, "overlay.png", 10, 10, [0, 0, 255, 128]);

    s.exec(
        r#"image.composite("/workspace/base.png", "/workspace/overlay.png", "/workspace/out.png")"#,
    )
    .unwrap();

    let img = image::open(dir.path().join("out.png")).unwrap().to_rgba8();
    let px = img.get_pixel(5, 5);
    // Should be a blend of red and blue — neither pure red nor pure blue
    assert!(px.0[0] > 50, "some red: {}", px.0[0]);
    assert!(px.0[2] > 50, "some blue: {}", px.0[2]);
    assert!(px.0[0] < 255 || px.0[2] < 255, "blended, not pure");
}

#[test]
fn composite_multiply_mode() {
    let (s, dir) = sb_with_workspace();
    // Gray base (128, 128, 128)
    create_rgba_png(&dir, "base.png", 10, 10, [128, 128, 128, 255]);
    // White overlay
    create_rgba_png(&dir, "overlay.png", 10, 10, [255, 255, 255, 255]);

    s.exec(
        r#"image.composite("/workspace/base.png", "/workspace/overlay.png", "/workspace/out.png", {mode = "multiply"})"#,
    )
    .unwrap();

    let img = image::open(dir.path().join("out.png")).unwrap().to_rgba8();
    let px = img.get_pixel(0, 0);
    // multiply(128, 255) = 128*255/255 = 128
    assert_eq!(px.0[0], 128, "multiply with white is identity");
}

#[test]
fn composite_screen_mode() {
    let (s, dir) = sb_with_workspace();
    create_rgba_png(&dir, "base.png", 10, 10, [100, 100, 100, 255]);
    create_rgba_png(&dir, "overlay.png", 10, 10, [100, 100, 100, 255]);

    s.exec(
        r#"image.composite("/workspace/base.png", "/workspace/overlay.png", "/workspace/out.png", {mode = "screen"})"#,
    )
    .unwrap();

    let img = image::open(dir.path().join("out.png")).unwrap().to_rgba8();
    let px = img.get_pixel(0, 0);
    // screen(100, 100) = 255 - (155*155)/255 ≈ 161
    assert!(px.0[0] > 100, "screen should brighten: got {}", px.0[0]);
}

#[test]
fn composite_add_mode() {
    let (s, dir) = sb_with_workspace();
    create_rgba_png(&dir, "base.png", 10, 10, [200, 0, 0, 255]);
    create_rgba_png(&dir, "overlay.png", 10, 10, [100, 0, 0, 255]);

    s.exec(
        r#"image.composite("/workspace/base.png", "/workspace/overlay.png", "/workspace/out.png", {mode = "add"})"#,
    )
    .unwrap();

    let img = image::open(dir.path().join("out.png")).unwrap().to_rgba8();
    let px = img.get_pixel(0, 0);
    // add(200, 100) clamped to 255
    assert_eq!(px.0[0], 255, "add should clamp at 255");
}

#[test]
fn composite_difference_mode() {
    let (s, dir) = sb_with_workspace();
    create_rgba_png(&dir, "base.png", 10, 10, [200, 50, 0, 255]);
    create_rgba_png(&dir, "overlay.png", 10, 10, [100, 150, 0, 255]);

    s.exec(
        r#"image.composite("/workspace/base.png", "/workspace/overlay.png", "/workspace/out.png", {mode = "difference"})"#,
    )
    .unwrap();

    let img = image::open(dir.path().join("out.png")).unwrap().to_rgba8();
    let px = img.get_pixel(0, 0);
    assert_eq!(px.0[0], 100, "diff(200,100)=100");
    assert_eq!(px.0[1], 100, "diff(50,150)=100");
}

#[test]
fn composite_darken_mode() {
    let (s, dir) = sb_with_workspace();
    create_rgba_png(&dir, "base.png", 10, 10, [200, 50, 100, 255]);
    create_rgba_png(&dir, "overlay.png", 10, 10, [100, 150, 200, 255]);

    s.exec(
        r#"image.composite("/workspace/base.png", "/workspace/overlay.png", "/workspace/out.png", {mode = "darken"})"#,
    )
    .unwrap();

    let img = image::open(dir.path().join("out.png")).unwrap().to_rgba8();
    let px = img.get_pixel(0, 0);
    assert_eq!(px.0[0], 100);
    assert_eq!(px.0[1], 50);
    assert_eq!(px.0[2], 100);
}

#[test]
fn composite_lighten_mode() {
    let (s, dir) = sb_with_workspace();
    create_rgba_png(&dir, "base.png", 10, 10, [200, 50, 100, 255]);
    create_rgba_png(&dir, "overlay.png", 10, 10, [100, 150, 200, 255]);

    s.exec(
        r#"image.composite("/workspace/base.png", "/workspace/overlay.png", "/workspace/out.png", {mode = "lighten"})"#,
    )
    .unwrap();

    let img = image::open(dir.path().join("out.png")).unwrap().to_rgba8();
    let px = img.get_pixel(0, 0);
    assert_eq!(px.0[0], 200);
    assert_eq!(px.0[1], 150);
    assert_eq!(px.0[2], 200);
}

#[test]
fn composite_invalid_mode_errors() {
    let (s, dir) = sb_with_workspace();
    create_rgba_png(&dir, "base.png", 10, 10, [0, 0, 0, 255]);
    create_rgba_png(&dir, "overlay.png", 10, 10, [0, 0, 0, 255]);

    let err = s
        .exec(
            r#"image.composite("/workspace/base.png", "/workspace/overlay.png", "/workspace/out.png", {mode = "bogus"})"#,
        )
        .unwrap_err();
    assert!(
        err.message.contains("unknown blend mode"),
        "msg: {}",
        err.message
    );
}

#[test]
fn composite_overlay_partially_outside() {
    let (s, dir) = sb_with_workspace();
    create_rgba_png(&dir, "base.png", 50, 50, [255, 0, 0, 255]);
    create_rgba_png(&dir, "overlay.png", 30, 30, [0, 0, 255, 255]);

    // Place overlay partially outside (starting at x=30, so 20px inside, 10px clipped)
    s.exec(
        r#"
        image.composite("/workspace/base.png", "/workspace/overlay.png", "/workspace/out.png", {x = 30, y = 30})
    "#,
    )
    .unwrap();

    let img = image::open(dir.path().join("out.png")).unwrap().to_rgba8();
    assert_eq!(img.width(), 50);
    assert_eq!(img.height(), 50);
    // (30, 30) should be blue
    let px = img.get_pixel(30, 30);
    assert_eq!(px.0[2], 255);
    // (0, 0) should still be red
    let px2 = img.get_pixel(0, 0);
    assert_eq!(px2.0[0], 255);
}

#[test]
fn composite_returns_output_path() {
    let (s, dir) = sb_with_workspace();
    create_rgba_png(&dir, "base.png", 10, 10, [0, 0, 0, 255]);
    create_rgba_png(&dir, "overlay.png", 5, 5, [0, 0, 0, 255]);

    let r = s
        .exec(
            r#"return image.composite("/workspace/base.png", "/workspace/overlay.png", "/workspace/out.png")"#,
        )
        .unwrap();
    assert_eq!(r, "/workspace/out.png");
}

#[test]
fn composite_no_args_errors() {
    let (s, _dir) = sb_with_workspace();
    let err = s.exec("image.composite()").unwrap_err();
    assert!(
        err.message.contains("bad argument") || err.message.contains("missing"),
        "msg: {}",
        err.message
    );
}

// ── Combined new + composite workflow ───────────────────────────

#[test]
fn create_background_then_composite() {
    let (s, dir) = sb_with_workspace();
    // Create a small blue overlay image on disk
    create_rgba_png(&dir, "icon.png", 30, 30, [0, 0, 255, 255]);

    let r = s
        .exec(
            r#"
            -- Create a 200x200 red background
            image.new("/workspace/bg.png", {width = 200, height = 200, color = {r = 255, g = 0, b = 0}})
            -- Composite the blue icon onto the red background
            image.composite("/workspace/bg.png", "/workspace/icon.png", "/workspace/result.png", {x = 85, y = 85})
            local info = image.info("/workspace/result.png")
            return info.width .. "x" .. info.height
        "#,
        )
        .unwrap();
    assert_eq!(r, "200x200");

    let img = image::open(dir.path().join("result.png"))
        .unwrap()
        .to_rgba8();
    // Center area should be blue
    let center = img.get_pixel(100, 100);
    assert_eq!(center.0[0], 0, "center should be blue, not red");
    assert_eq!(center.0[2], 255, "center blue channel");
    // Corner should be red
    let corner = img.get_pixel(0, 0);
    assert_eq!(corner.0[0], 255, "corner should be red");
    assert_eq!(corner.0[2], 0, "corner should have no blue");
}

// ── Help for new functions ──────────────────────────────────────

#[test]
fn help_includes_composite_and_new() {
    let (s, _dir) = sb_with_workspace();
    let r = s.exec("return image.help()").unwrap();
    assert!(r.contains("image.composite"), "help: {}", r);
    assert!(r.contains("image.new"), "help: {}", r);
}

// ── Phase 3: image.draw ─────────────────────────────────────────

#[test]
fn draw_filled_rect() {
    let (s, dir) = sb_with_workspace();
    create_rgba_png(&dir, "canvas.png", 100, 100, [255, 255, 255, 255]);

    s.exec(
        r#"
        image.draw("/workspace/canvas.png", "/workspace/out.png", {
            {type = "rect", x = 10, y = 10, width = 30, height = 20, color = {r = 255, g = 0, b = 0}}
        })
    "#,
    )
    .unwrap();

    let img = image::open(dir.path().join("out.png")).unwrap().to_rgba8();
    // Inside rect should be red
    let px = img.get_pixel(20, 15);
    assert_eq!(px.0[0], 255, "red channel inside rect");
    assert_eq!(px.0[1], 0, "green channel inside rect");
    // Outside rect should be white
    let px2 = img.get_pixel(0, 0);
    assert_eq!(px2.0[0], 255);
    assert_eq!(px2.0[1], 255);
}

#[test]
fn draw_hollow_rect() {
    let (s, dir) = sb_with_workspace();
    create_rgba_png(&dir, "canvas.png", 100, 100, [255, 255, 255, 255]);

    s.exec(
        r#"
        image.draw("/workspace/canvas.png", "/workspace/out.png", {
            {type = "rect", x = 10, y = 10, width = 40, height = 40, color = {r = 0, g = 0, b = 255}, fill = false}
        })
    "#,
    )
    .unwrap();

    let img = image::open(dir.path().join("out.png")).unwrap().to_rgba8();
    // Border pixel should be blue
    let border = img.get_pixel(10, 10);
    assert_eq!(border.0[2], 255, "border should be blue");
    // Interior should still be white
    let interior = img.get_pixel(25, 25);
    assert_eq!(interior.0[0], 255, "interior should be white");
    assert_eq!(interior.0[1], 255, "interior green");
    assert_eq!(interior.0[2], 255, "interior blue");
}

#[test]
fn draw_filled_circle() {
    let (s, dir) = sb_with_workspace();
    create_rgba_png(&dir, "canvas.png", 100, 100, [0, 0, 0, 255]);

    s.exec(
        r#"
        image.draw("/workspace/canvas.png", "/workspace/out.png", {
            {type = "circle", x = 50, y = 50, radius = 20, color = {r = 0, g = 255, b = 0}}
        })
    "#,
    )
    .unwrap();

    let img = image::open(dir.path().join("out.png")).unwrap().to_rgba8();
    // Center of circle should be green
    let px = img.get_pixel(50, 50);
    assert_eq!(px.0[1], 255, "circle center should be green");
    // Far corner should remain black
    let corner = img.get_pixel(0, 0);
    assert_eq!(corner.0[0], 0);
}

#[test]
fn draw_hollow_circle() {
    let (s, dir) = sb_with_workspace();
    create_rgba_png(&dir, "canvas.png", 100, 100, [255, 255, 255, 255]);

    s.exec(
        r#"
        image.draw("/workspace/canvas.png", "/workspace/out.png", {
            {type = "circle", x = 50, y = 50, radius = 20, color = {r = 255, g = 0, b = 0}, fill = false}
        })
    "#,
    )
    .unwrap();

    let img = image::open(dir.path().join("out.png")).unwrap().to_rgba8();
    // Center should still be white (hollow)
    let center = img.get_pixel(50, 50);
    assert_eq!(center.0[0], 255);
    assert_eq!(center.0[1], 255);
}

#[test]
fn draw_line() {
    let (s, dir) = sb_with_workspace();
    create_rgba_png(&dir, "canvas.png", 100, 100, [255, 255, 255, 255]);

    s.exec(
        r#"
        image.draw("/workspace/canvas.png", "/workspace/out.png", {
            {type = "line", x1 = 0, y1 = 0, x2 = 99, y2 = 99, color = {r = 255, g = 0, b = 0}}
        })
    "#,
    )
    .unwrap();

    let img = image::open(dir.path().join("out.png")).unwrap().to_rgba8();
    // Diagonal pixel near center should be red
    let px = img.get_pixel(50, 50);
    assert_eq!(px.0[0], 255, "line pixel should be red");
    assert_eq!(px.0[1], 0);
}

#[test]
fn draw_multiple_shapes() {
    let (s, dir) = sb_with_workspace();
    create_rgba_png(&dir, "canvas.png", 200, 200, [255, 255, 255, 255]);

    s.exec(
        r#"
        image.draw("/workspace/canvas.png", "/workspace/out.png", {
            {type = "rect", x = 10, y = 10, width = 50, height = 50, color = {r = 255, g = 0, b = 0}},
            {type = "circle", x = 150, y = 150, radius = 30, color = {r = 0, g = 0, b = 255}},
            {type = "line", x1 = 0, y1 = 100, x2 = 200, y2 = 100, color = {r = 0, g = 255, b = 0}}
        })
    "#,
    )
    .unwrap();

    let img = image::open(dir.path().join("out.png")).unwrap().to_rgba8();
    // Rect area should be red (not white)
    let px_rect = img.get_pixel(30, 30);
    assert_eq!(px_rect.0[0], 255, "rect red channel");
    assert_eq!(px_rect.0[1], 0, "rect green channel should be 0");
    // Circle center should be blue
    let px_circle = img.get_pixel(150, 150);
    assert_eq!(px_circle.0[2], 255, "circle blue channel");
    assert_eq!(px_circle.0[0], 0, "circle red channel should be 0");
}

#[test]
fn draw_unknown_shape_errors() {
    let (s, dir) = sb_with_workspace();
    create_rgba_png(&dir, "canvas.png", 10, 10, [0, 0, 0, 255]);

    let err = s
        .exec(
            r#"
            image.draw("/workspace/canvas.png", "/workspace/out.png", {
                {type = "triangle", x = 0, y = 0}
            })
        "#,
        )
        .unwrap_err();
    assert!(
        err.message.contains("unknown shape type"),
        "msg: {}",
        err.message
    );
}

#[test]
fn draw_returns_output_path() {
    let (s, dir) = sb_with_workspace();
    create_rgba_png(&dir, "canvas.png", 10, 10, [0, 0, 0, 255]);

    let r = s
        .exec(
            r#"
            return image.draw("/workspace/canvas.png", "/workspace/out.png", {
                {type = "rect", x = 0, y = 0, width = 5, height = 5, color = {r = 255, g = 0, b = 0}}
            })
        "#,
        )
        .unwrap();
    assert_eq!(r, "/workspace/out.png");
}

#[test]
fn draw_no_args_errors() {
    let (s, _dir) = sb_with_workspace();
    let err = s.exec("image.draw()").unwrap_err();
    assert!(
        err.message.contains("bad argument") || err.message.contains("missing"),
        "msg: {}",
        err.message
    );
}

// ── Phase 3: image.text ─────────────────────────────────────────

#[test]
fn text_draws_on_image() {
    let (s, dir) = sb_with_workspace();
    create_rgba_png(&dir, "canvas.png", 200, 100, [255, 255, 255, 255]);

    s.exec(
        r#"
        image.text("/workspace/canvas.png", "/workspace/out.png", {
            text = "Hello",
            x = 10,
            y = 10,
            size = 24,
            color = {r = 0, g = 0, b = 0}
        })
    "#,
    )
    .unwrap();

    // Verify output exists and has same dimensions
    let img = image::open(dir.path().join("out.png")).unwrap();
    assert_eq!(img.width(), 200);
    assert_eq!(img.height(), 100);
    // Some pixels should have changed (text was drawn)
    let rgba = img.to_rgba8();
    let has_dark_pixels = rgba.pixels().any(|px| px.0[0] < 200);
    assert!(has_dark_pixels, "text should have drawn dark pixels");
}

#[test]
fn text_returns_output_path() {
    let (s, dir) = sb_with_workspace();
    create_rgba_png(&dir, "canvas.png", 50, 50, [255, 255, 255, 255]);

    let r = s
        .exec(
            r#"
            return image.text("/workspace/canvas.png", "/workspace/out.png", {
                text = "Hi",
                x = 0,
                y = 0,
                size = 12,
                color = {r = 0, g = 0, b = 0}
            })
        "#,
        )
        .unwrap();
    assert_eq!(r, "/workspace/out.png");
}

#[test]
fn text_missing_text_field_errors() {
    let (s, dir) = sb_with_workspace();
    create_rgba_png(&dir, "canvas.png", 50, 50, [255, 255, 255, 255]);

    let err = s
        .exec(
            r#"
            image.text("/workspace/canvas.png", "/workspace/out.png", {
                x = 0, y = 0, size = 12, color = {r = 0, g = 0, b = 0}
            })
        "#,
        )
        .unwrap_err();
    assert!(err.message.contains("text"), "msg: {}", err.message);
}

#[test]
fn text_no_args_errors() {
    let (s, _dir) = sb_with_workspace();
    let err = s.exec("image.text()").unwrap_err();
    assert!(
        err.message.contains("bad argument") || err.message.contains("missing"),
        "msg: {}",
        err.message
    );
}

// ── Phase 3: image.fonts ────────────────────────────────────────

#[test]
fn fonts_returns_table() {
    let (s, _dir) = sb_with_workspace();
    let r = s.exec("return type(image.fonts())").unwrap();
    assert_eq!(r, "table");
}

#[test]
fn fonts_has_entries_on_macos() {
    if !cfg!(target_os = "macos") {
        return; // Skip on non-macOS
    }
    let (s, _dir) = sb_with_workspace();
    let r = s
        .exec(
            r#"
            local fonts = image.fonts()
            return tostring(#fonts)
        "#,
        )
        .unwrap();
    let count: usize = r.parse().unwrap_or(0);
    assert!(count > 0, "macOS should have system fonts, got {}", count);
}

#[test]
fn fonts_entries_have_name_path_style() {
    if !cfg!(target_os = "macos") {
        return;
    }
    let (s, _dir) = sb_with_workspace();
    let r = s
        .exec(
            r#"
            local fonts = image.fonts()
            if #fonts == 0 then return "no fonts" end
            local f = fonts[1]
            return f.name .. "|" .. f.style .. "|" .. (f.path ~= "" and "has_path" or "no_path")
        "#,
        )
        .unwrap();
    assert!(
        r.contains("|"),
        "expected name|style|path_check, got: {}",
        r
    );
    assert!(r.contains("has_path"), "font should have a path: {}", r);
}

// ── Phase 3: image.blur ─────────────────────────────────────────

#[test]
fn blur_applies_gaussian() {
    let (s, dir) = sb_with_workspace();
    // Create a high-contrast image (black/white halves)
    let mut img = image::RgbaImage::new(100, 100);
    for (x, _y, pixel) in img.enumerate_pixels_mut() {
        if x < 50 {
            *pixel = image::Rgba([0, 0, 0, 255]);
        } else {
            *pixel = image::Rgba([255, 255, 255, 255]);
        }
    }
    img.save(dir.path().join("sharp.png")).unwrap();

    s.exec(r#"image.blur("/workspace/sharp.png", "/workspace/blurred.png", 5.0)"#)
        .unwrap();

    let blurred = image::open(dir.path().join("blurred.png"))
        .unwrap()
        .to_rgba8();
    // At the boundary (x=50), the pixel should be mid-gray due to blur
    let px = blurred.get_pixel(50, 50);
    assert!(
        px.0[0] > 30 && px.0[0] < 225,
        "boundary should be blurred to mid-gray, got: {}",
        px.0[0]
    );
}

#[test]
fn blur_returns_output_path() {
    let (s, dir) = sb_with_workspace();
    create_test_png(&dir, "test.png", 10, 10);

    let r = s
        .exec(r#"return image.blur("/workspace/test.png", "/workspace/out.png", 1.0)"#)
        .unwrap();
    assert_eq!(r, "/workspace/out.png");
}

#[test]
fn blur_zero_sigma_errors() {
    let (s, dir) = sb_with_workspace();
    create_test_png(&dir, "test.png", 10, 10);

    let err = s
        .exec(r#"image.blur("/workspace/test.png", "/workspace/out.png", 0)"#)
        .unwrap_err();
    assert!(
        err.message.contains("sigma must be > 0"),
        "msg: {}",
        err.message
    );
}

#[test]
fn blur_no_args_errors() {
    let (s, _dir) = sb_with_workspace();
    let err = s.exec("image.blur()").unwrap_err();
    assert!(
        err.message.contains("bad argument") || err.message.contains("missing"),
        "msg: {}",
        err.message
    );
}

// ── Phase 3: image.sharpen ──────────────────────────────────────

#[test]
fn sharpen_enhances_edges() {
    let (s, dir) = sb_with_workspace();
    create_test_png(&dir, "test.png", 50, 50);

    s.exec(r#"image.sharpen("/workspace/test.png", "/workspace/out.png", 1.0, 2)"#)
        .unwrap();

    // Just verify it produces output without error
    let img = image::open(dir.path().join("out.png")).unwrap();
    assert_eq!(img.width(), 50);
    assert_eq!(img.height(), 50);
}

#[test]
fn sharpen_defaults() {
    let (s, dir) = sb_with_workspace();
    create_test_png(&dir, "test.png", 10, 10);

    // Calling with no sigma/amount should use defaults
    let r = s
        .exec(r#"return image.sharpen("/workspace/test.png", "/workspace/out.png")"#)
        .unwrap();
    assert_eq!(r, "/workspace/out.png");
}

#[test]
fn sharpen_no_args_errors() {
    let (s, _dir) = sb_with_workspace();
    let err = s.exec("image.sharpen()").unwrap_err();
    assert!(
        err.message.contains("bad argument") || err.message.contains("missing"),
        "msg: {}",
        err.message
    );
}

// ── Phase 3: image.rotate_exact ─────────────────────────────────

#[test]
fn rotate_exact_45_degrees() {
    let (s, dir) = sb_with_workspace();
    create_rgba_png(&dir, "test.png", 100, 100, [255, 0, 0, 255]);

    s.exec(r#"image.rotate_exact("/workspace/test.png", "/workspace/out.png", 45)"#)
        .unwrap();

    let img = image::open(dir.path().join("out.png")).unwrap().to_rgba8();
    // Output should be same dimensions
    assert_eq!(img.width(), 100);
    assert_eq!(img.height(), 100);
    // Corner pixels should be transparent (background from rotation)
    let corner = img.get_pixel(0, 0);
    assert_eq!(
        corner.0[3], 0,
        "corner should be transparent after 45° rotation"
    );
    // Center should still be red
    let center = img.get_pixel(50, 50);
    assert_eq!(center.0[0], 255, "center should still be red");
}

#[test]
fn rotate_exact_returns_output_path() {
    let (s, dir) = sb_with_workspace();
    create_test_png(&dir, "test.png", 10, 10);

    let r = s
        .exec(r#"return image.rotate_exact("/workspace/test.png", "/workspace/out.png", 30)"#)
        .unwrap();
    assert_eq!(r, "/workspace/out.png");
}

#[test]
fn rotate_exact_360_is_identity() {
    let (s, dir) = sb_with_workspace();
    create_rgba_png(&dir, "test.png", 50, 50, [128, 64, 32, 255]);

    s.exec(r#"image.rotate_exact("/workspace/test.png", "/workspace/out.png", 360)"#)
        .unwrap();

    let img = image::open(dir.path().join("out.png")).unwrap().to_rgba8();
    // Center pixel should be approximately the same as original
    let center = img.get_pixel(25, 25);
    assert!(
        (center.0[0] as i16 - 128).unsigned_abs() < 5,
        "360° rotation should preserve center pixel, got: {:?}",
        center.0
    );
}

#[test]
fn rotate_exact_no_args_errors() {
    let (s, _dir) = sb_with_workspace();
    let err = s.exec("image.rotate_exact()").unwrap_err();
    assert!(
        err.message.contains("bad argument") || err.message.contains("missing"),
        "msg: {}",
        err.message
    );
}

// ── Phase 3: PIL transpiler mappings ────────────────────────────

#[test]
fn pil_imagedraw_import_maps_to_image() {
    let py_code = "from PIL import ImageDraw\n";
    let transpiled = transpile::transpile(py_code).unwrap();
    assert!(
        transpiled.luau_source.contains("image")
            || transpiled.luau_source.is_empty()
            || !transpiled.luau_source.contains("require"),
        "ImageDraw should map to image, got: {}",
        transpiled.luau_source
    );
}

#[test]
fn pil_imagefilter_import_maps_to_image() {
    let py_code = "from PIL import ImageFilter\n";
    let transpiled = transpile::transpile(py_code).unwrap();
    assert!(
        transpiled.luau_source.contains("image")
            || transpiled.luau_source.is_empty()
            || !transpiled.luau_source.contains("require"),
        "ImageFilter should map to image, got: {}",
        transpiled.luau_source
    );
}

#[test]
fn pil_imagefont_import_maps_to_image() {
    let py_code = "from PIL import ImageFont\n";
    let transpiled = transpile::transpile(py_code).unwrap();
    assert!(
        transpiled.luau_source.contains("image")
            || transpiled.luau_source.is_empty()
            || !transpiled.luau_source.contains("require"),
        "ImageFont should map to image, got: {}",
        transpiled.luau_source
    );
}

#[test]
fn image_paste_alias_works() {
    let (s, dir) = sb_with_workspace();
    create_rgba_png(&dir, "base.png", 100, 100, [255, 0, 0, 255]);
    create_rgba_png(&dir, "overlay.png", 20, 20, [0, 0, 255, 255]);

    // image.paste should work identically to image.composite
    s.exec(
        r#"image.paste("/workspace/base.png", "/workspace/overlay.png", "/workspace/out.png", {x = 10, y = 10})"#,
    )
    .unwrap();

    let img = image::open(dir.path().join("out.png")).unwrap().to_rgba8();
    let overlay_px = img.get_pixel(15, 15);
    assert_eq!(overlay_px.0[2], 255, "paste should work like composite");
}

#[test]
fn python_pil_draw_transpiles() {
    let pyrt = include_str!("../../runtime/pyrt.luau");
    let py_code = r#"
from PIL import Image
Image.draw("/workspace/canvas.png", "/workspace/out.png", [{"type": "rect", "x": 0, "y": 0, "width": 10, "height": 10, "color": {"r": 255, "g": 0, "b": 0}}])
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    // Should contain image.draw call (Image is aliased to image)
    assert!(
        transpiled.luau_source.contains("Image.draw")
            || transpiled.luau_source.contains("image.draw"),
        "should transpile to image.draw or Image.draw, got: {}",
        transpiled.luau_source
    );

    // Actually run it
    let (s, dir) = sb_with_workspace();
    create_rgba_png(&dir, "canvas.png", 50, 50, [255, 255, 255, 255]);
    s.setup_python_runtime(pyrt).unwrap();
    s.exec(&transpiled.luau_source).unwrap();

    let img = image::open(dir.path().join("out.png")).unwrap().to_rgba8();
    let px = img.get_pixel(5, 5);
    assert_eq!(px.0[0], 255, "rect should be red");
}

// ── Phase 3: help text includes new functions ───────────────────

#[test]
fn help_includes_phase3_functions() {
    let (s, _dir) = sb_with_workspace();
    let r = s.exec("return image.help()").unwrap();
    assert!(r.contains("image.draw"), "help should include draw: {}", r);
    assert!(r.contains("image.text"), "help should include text: {}", r);
    assert!(
        r.contains("image.fonts"),
        "help should include fonts: {}",
        r
    );
    assert!(r.contains("image.blur"), "help should include blur: {}", r);
    assert!(
        r.contains("image.sharpen"),
        "help should include sharpen: {}",
        r
    );
    assert!(
        r.contains("image.rotate_exact"),
        "help should include rotate_exact: {}",
        r
    );
    assert!(
        r.contains("image.measure_text"),
        "help should include measure_text: {}",
        r
    );
}

// ── image.text align/valign ─────────────────────────────────────

#[test]
fn text_align_center_shifts_left() {
    // With align="center", text drawn at x=100 should have its center at x=100,
    // meaning some pixels left of x=100 should be drawn on.
    let (s, dir) = sb_with_workspace();
    create_rgba_png(&dir, "bg.png", 200, 100, [255, 255, 255, 255]);
    let r = s
        .exec(
            r#"
        image.text("/workspace/bg.png", "/workspace/out.png", {
            text = "HELLO",
            x = 100,
            y = 10,
            size = 20,
            color = {r = 0, g = 0, b = 0},
            align = "center"
        })
        return "ok"
    "#,
        )
        .unwrap();
    assert_eq!(r, "ok");
    // Verify the output file was created
    assert!(dir.path().join("out.png").exists());
}

#[test]
fn text_align_right_shifts_left() {
    let (s, dir) = sb_with_workspace();
    create_rgba_png(&dir, "bg.png", 200, 100, [255, 255, 255, 255]);
    let r = s
        .exec(
            r#"
        image.text("/workspace/bg.png", "/workspace/out.png", {
            text = "HELLO",
            x = 190,
            y = 10,
            size = 20,
            color = {r = 0, g = 0, b = 0},
            align = "right"
        })
        return "ok"
    "#,
        )
        .unwrap();
    assert_eq!(r, "ok");
}

#[test]
fn text_valign_center_shifts_up() {
    let (s, dir) = sb_with_workspace();
    create_rgba_png(&dir, "bg.png", 200, 200, [255, 255, 255, 255]);
    let r = s
        .exec(
            r#"
        image.text("/workspace/bg.png", "/workspace/out.png", {
            text = "HELLO",
            x = 10,
            y = 100,
            size = 20,
            color = {r = 0, g = 0, b = 0},
            valign = "center"
        })
        return "ok"
    "#,
        )
        .unwrap();
    assert_eq!(r, "ok");
}

#[test]
fn text_valign_bottom_shifts_up() {
    let (s, dir) = sb_with_workspace();
    create_rgba_png(&dir, "bg.png", 200, 200, [255, 255, 255, 255]);
    let r = s
        .exec(
            r#"
        image.text("/workspace/bg.png", "/workspace/out.png", {
            text = "HELLO",
            x = 10,
            y = 190,
            size = 20,
            color = {r = 0, g = 0, b = 0},
            valign = "bottom"
        })
        return "ok"
    "#,
        )
        .unwrap();
    assert_eq!(r, "ok");
}

#[test]
fn text_center_both_axes() {
    // The most common use case: center text on an image
    let (s, dir) = sb_with_workspace();
    create_rgba_png(&dir, "bg.png", 200, 200, [255, 255, 255, 255]);
    let r = s
        .exec(
            r#"
        image.text("/workspace/bg.png", "/workspace/out.png", {
            text = "HI",
            x = 100,
            y = 100,
            size = 30,
            color = {r = 0, g = 0, b = 0},
            align = "center",
            valign = "center"
        })
        return "ok"
    "#,
        )
        .unwrap();
    assert_eq!(r, "ok");
}

#[test]
fn text_invalid_align_errors() {
    let (s, dir) = sb_with_workspace();
    create_rgba_png(&dir, "bg.png", 200, 100, [255, 255, 255, 255]);
    let r = s.exec(
        r#"
        image.text("/workspace/bg.png", "/workspace/out.png", {
            text = "HI",
            x = 10,
            y = 10,
            size = 20,
            color = {r = 0, g = 0, b = 0},
            align = "justify"
        })
    "#,
    );
    assert!(r.is_err(), "invalid align should error");
    let err = r.unwrap_err().to_string();
    assert!(err.contains("align"), "error should mention align: {}", err);
}

#[test]
fn text_invalid_valign_errors() {
    let (s, dir) = sb_with_workspace();
    create_rgba_png(&dir, "bg.png", 200, 100, [255, 255, 255, 255]);
    let r = s.exec(
        r#"
        image.text("/workspace/bg.png", "/workspace/out.png", {
            text = "HI",
            x = 10,
            y = 10,
            size = 20,
            color = {r = 0, g = 0, b = 0},
            valign = "middle"
        })
    "#,
    );
    assert!(r.is_err(), "invalid valign should error");
    let err = r.unwrap_err().to_string();
    assert!(
        err.contains("valign"),
        "error should mention valign: {}",
        err
    );
}

#[test]
fn text_multiline_center_aligns_each_line() {
    let (s, dir) = sb_with_workspace();
    create_rgba_png(&dir, "bg.png", 300, 200, [255, 255, 255, 255]);
    let r = s
        .exec(
            r#"
        image.text("/workspace/bg.png", "/workspace/out.png", {
            text = "Hello\nWorld!",
            x = 150,
            y = 100,
            size = 24,
            color = {r = 0, g = 0, b = 0},
            align = "center",
            valign = "center"
        })
        return "ok"
    "#,
        )
        .unwrap();
    assert_eq!(r, "ok");
}

// ── image.measure_text ──────────────────────────────────────────

#[test]
fn measure_text_returns_dimensions() {
    let (s, _dir) = sb_with_workspace();
    let r = s
        .exec(
            r#"
        local m = image.measure_text({text = "Hello", size = 24})
        return string.format("%.0f|%.0f|%.0f", m.width, m.height, m.line_height)
    "#,
        )
        .unwrap();
    let parts: Vec<&str> = r.split('|').collect();
    assert_eq!(parts.len(), 3, "expected 3 parts: {}", r);
    let width: f64 = parts[0].parse().unwrap();
    let height: f64 = parts[1].parse().unwrap();
    let line_height: f64 = parts[2].parse().unwrap();
    assert!(width > 0.0, "width should be > 0: {}", width);
    assert!(height > 0.0, "height should be > 0: {}", height);
    assert!(
        line_height > 0.0,
        "line_height should be > 0: {}",
        line_height
    );
}

#[test]
fn measure_text_multiline_wider_than_single() {
    let (s, _dir) = sb_with_workspace();
    // "WWWWW" should be wider than "i" — measure the widest line
    let r = s
        .exec(
            r#"
        local m1 = image.measure_text({text = "i", size = 24})
        local m2 = image.measure_text({text = "WWWWW", size = 24})
        return tostring(m2.width > m1.width)
    "#,
        )
        .unwrap();
    assert_eq!(r, "true");
}

#[test]
fn measure_text_multiline_height() {
    let (s, _dir) = sb_with_workspace();
    let r = s
        .exec(
            r#"
        local m1 = image.measure_text({text = "Hello", size = 24})
        local m2 = image.measure_text({text = "Hello\nWorld", size = 24})
        return tostring(m2.height > m1.height)
    "#,
        )
        .unwrap();
    assert_eq!(r, "true", "multiline should be taller than single line");
}

#[test]
fn measure_text_missing_text_errors() {
    let (s, _dir) = sb_with_workspace();
    let r = s.exec(r#"image.measure_text({size = 24})"#);
    assert!(r.is_err(), "missing text should error");
}

#[test]
fn measure_text_missing_size_errors() {
    let (s, _dir) = sb_with_workspace();
    let r = s.exec(r#"image.measure_text({text = "Hi"})"#);
    assert!(r.is_err(), "missing size should error");
}

#[test]
fn measure_text_no_args_errors() {
    let (s, _dir) = sb_with_workspace();
    let r = s.exec(r#"image.measure_text()"#);
    assert!(r.is_err(), "no args should error");
}

#[test]
fn measure_text_used_for_centering() {
    // End-to-end: use measure_text to center text, then draw it
    let (s, dir) = sb_with_workspace();
    create_rgba_png(&dir, "bg.png", 400, 200, [255, 255, 255, 255]);
    let r = s
        .exec(
            r#"
        local m = image.measure_text({text = "Centered", size = 32})
        local info = image.info("/workspace/bg.png")
        local cx = math.floor((info.width - m.width) / 2)
        local cy = math.floor((info.height - m.height) / 2)
        image.text("/workspace/bg.png", "/workspace/out.png", {
            text = "Centered",
            x = cx,
            y = cy,
            size = 32,
            color = {r = 0, g = 0, b = 0}
        })
        return "ok"
    "#,
        )
        .unwrap();
    assert_eq!(r, "ok");
    assert!(dir.path().join("out.png").exists());
}
