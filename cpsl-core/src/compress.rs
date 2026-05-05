//! Compression module for the Luau sandbox.
//!
//! Exposes cross-platform archive & compression operations as `compress.*` globals.
//! All paths are virtual and resolved through the mount table.
//! Supported formats: ZIP, tar, tar.gz, bzip2, xz/LZMA, 7z.

use crate::mount::MountTable;
use crate::pyrt_compat::unwrap_py_seq;
use crate::sandbox::{arg_error, validate_args, wrap_module_with_help_hints, FnDoc, ModuleDoc, Param, ParamType, ReturnType};
use mlua::MultiValue;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::Arc;

pub(crate) static COMPRESS_DOC: ModuleDoc = ModuleDoc {
    name: "compress",
    summary: "archive & compression (zip, tar, bzip2, xz, 7z)",
    functions: &[
        FnDoc {
            name: "zip",
            description: "Create a zip archive. source can be a path or list of paths.",
            params: &[
                Param { name: "source", short: Some('s'), typ: ParamType::Value, required: true, fields: None },
                Param { name: "archive", short: Some('a'), typ: ParamType::String, required: true, fields: None },
            ],
            returns: ReturnType::Void,
            example: Some(r#"compress.zip({source={"/workspace/a.txt", "/workspace/b.txt"}, archive="/artifacts/out.zip"})"#),
        },
        FnDoc {
            name: "unzip",
            description: "Extract a zip archive to a directory.",
            params: &[
                Param { name: "archive", short: Some('a'), typ: ParamType::String, required: true, fields: None },
                Param { name: "dest", short: Some('d'), typ: ParamType::String, required: true, fields: None },
            ],
            returns: ReturnType::Void,
            example: None,
        },
        FnDoc {
            name: "tar",
            description: "Create a tar archive. source can be a path or list of paths.",
            params: &[
                Param { name: "source", short: Some('s'), typ: ParamType::Value, required: true, fields: None },
                Param { name: "archive", short: Some('a'), typ: ParamType::String, required: true, fields: None },
            ],
            returns: ReturnType::Void,
            example: Some(r#"compress.tar({source="/workspace/data", archive="/artifacts/data.tar"})"#),
        },
        FnDoc {
            name: "untar",
            description: "Extract a tar archive to a directory.",
            params: &[
                Param { name: "archive", short: Some('a'), typ: ParamType::String, required: true, fields: None },
                Param { name: "dest", short: Some('d'), typ: ParamType::String, required: true, fields: None },
            ],
            returns: ReturnType::Void,
            example: None,
        },
        FnDoc {
            name: "targz",
            description: "Create a gzip-compressed tar archive (.tar.gz). source can be a path or list of paths.",
            params: &[
                Param { name: "source", short: None, typ: ParamType::Value, required: true, fields: None },
                Param { name: "archive", short: None, typ: ParamType::String, required: true, fields: None },
            ],
            returns: ReturnType::Void,
            example: Some(r#"compress.targz({source="/workspace/data", archive="/artifacts/data.tar.gz"})"#),
        },
        FnDoc {
            name: "untargz",
            description: "Extract a gzip-compressed tar archive.",
            params: &[
                Param { name: "archive", short: None, typ: ParamType::String, required: true, fields: None },
                Param { name: "dest", short: None, typ: ParamType::String, required: true, fields: None },
            ],
            returns: ReturnType::Void,
            example: None,
        },
        FnDoc {
            name: "bzip2",
            description: "Compress a file with bzip2.",
            params: &[
                Param { name: "input", short: None, typ: ParamType::String, required: true, fields: None },
                Param { name: "output", short: None, typ: ParamType::String, required: true, fields: None },
            ],
            returns: ReturnType::Void,
            example: None,
        },
        FnDoc {
            name: "bunzip2",
            description: "Decompress a bzip2 file.",
            params: &[
                Param { name: "input", short: None, typ: ParamType::String, required: true, fields: None },
                Param { name: "output", short: None, typ: ParamType::String, required: true, fields: None },
            ],
            returns: ReturnType::Void,
            example: None,
        },
        FnDoc {
            name: "xz",
            description: "Compress a file with xz/LZMA.",
            params: &[
                Param { name: "input", short: None, typ: ParamType::String, required: true, fields: None },
                Param { name: "output", short: None, typ: ParamType::String, required: true, fields: None },
            ],
            returns: ReturnType::Void,
            example: None,
        },
        FnDoc {
            name: "unxz",
            description: "Decompress an xz file.",
            params: &[
                Param { name: "input", short: None, typ: ParamType::String, required: true, fields: None },
                Param { name: "output", short: None, typ: ParamType::String, required: true, fields: None },
            ],
            returns: ReturnType::Void,
            example: None,
        },
        FnDoc {
            name: "un7z",
            description: "Extract a 7z archive to a directory.",
            params: &[
                Param { name: "archive", short: None, typ: ParamType::String, required: true, fields: None },
                Param { name: "dest", short: None, typ: ParamType::String, required: true, fields: None },
            ],
            returns: ReturnType::Void,
            example: None,
        },
    ],
};

/// Register `compress.*` globals in the Lua VM.
pub fn register_compress_globals(
    lua: &mlua::Lua,
    mounts: Arc<MountTable>,
) -> Result<(), mlua::Error> {
    let compress = lua.create_table()?;

    // compress.zip(source, archive)
    // source can be a string or {string} list
    {
        let m = mounts.clone();
        compress.set(
            "zip",
            lua.create_function(
                move |lua, (first, archive_opt): (mlua::Value, Option<String>)| {
                    if matches!(first, mlua::Value::Nil) {
                        return Err(arg_error("compress.zip", COMPRESS_DOC.params("zip")));
                    }
                    // If second arg present → positional mode (source, archive)
                    // If absent and first is table with [2] → named-param table
                    let (source_val, archive) = if let Some(a) = archive_opt {
                        (first, a)
                    } else if let mlua::Value::Table(ref t) = first {
                        let arc = t.get::<String>(2)
                            .or_else(|_| t.get::<String>("archive"))
                            .map_err(|_| mlua::Error::external("compress.zip: missing archive argument"))?;
                        let src_val = if let Ok(s) = t.get::<String>(1) {
                            mlua::Value::String(lua.create_string(&s)?)
                        } else {
                            t.get::<mlua::Value>("source")
                                .map_err(|_| mlua::Error::external("compress.zip: missing source argument"))?
                        };
                        (src_val, arc)
                    } else {
                        return Err(mlua::Error::external("compress.zip: missing archive argument"));
                    };
                    let archive_host =
                        m.resolve_write(&archive).map_err(mlua::Error::external)?;
                    if let Some(parent) = archive_host.parent() {
                        std::fs::create_dir_all(parent).map_err(mlua::Error::external)?;
                    }
                    let sources = resolve_source_paths(&m, &source_val, "compress.zip")?;
                    if sources.len() == 1 {
                        create_zip(&sources[0], &archive_host)
                            .map_err(mlua::Error::RuntimeError)?;
                    } else {
                        create_zip_multi(&sources, &archive_host)
                            .map_err(mlua::Error::RuntimeError)?;
                    }
                    Ok(())
                },
            )?,
        )?;
    }

    // compress.unzip(archive, dest)
    {
        let m = mounts.clone();
        compress.set(
            "unzip",
            lua.create_function(move |_, args: MultiValue| {
                let validated = validate_args(&args, COMPRESS_DOC.params("unzip"), "compress.unzip")?;
                let archive = match &validated[0] { mlua::Value::String(s) => s.to_string_lossy().to_string(), _ => unreachable!() };
                let dest = match &validated[1] { mlua::Value::String(s) => s.to_string_lossy().to_string(), _ => unreachable!() };
                let archive_host =
                    m.resolve_read(&archive).map_err(mlua::Error::external)?;
                let dest_host =
                    m.resolve_write_deep(&dest).map_err(mlua::Error::external)?;
                std::fs::create_dir_all(&dest_host).map_err(mlua::Error::external)?;
                extract_zip(&archive_host, &dest_host)
                    .map_err(mlua::Error::RuntimeError)?;
                Ok(())
            })?,
        )?;
    }

    // compress.bzip2(input, output)
    {
        let m = mounts.clone();
        compress.set(
            "bzip2",
            lua.create_function(move |_, args: MultiValue| {
                let validated = validate_args(&args, COMPRESS_DOC.params("bzip2"), "compress.bzip2")?;
                let input = match &validated[0] { mlua::Value::String(s) => s.to_string_lossy().to_string(), _ => unreachable!() };
                let output = match &validated[1] { mlua::Value::String(s) => s.to_string_lossy().to_string(), _ => unreachable!() };
                let input_host = m.resolve_read(&input).map_err(mlua::Error::external)?;
                let output_host = m.resolve_write(&output).map_err(mlua::Error::external)?;
                if let Some(parent) = output_host.parent() {
                    std::fs::create_dir_all(parent).map_err(mlua::Error::external)?;
                }
                bzip2_compress(&input_host, &output_host)
                    .map_err(mlua::Error::RuntimeError)?;
                Ok(())
            })?,
        )?;
    }

    // compress.bunzip2(input, output)
    {
        let m = mounts.clone();
        compress.set(
            "bunzip2",
            lua.create_function(move |_, args: MultiValue| {
                let validated = validate_args(&args, COMPRESS_DOC.params("bunzip2"), "compress.bunzip2")?;
                let input = match &validated[0] { mlua::Value::String(s) => s.to_string_lossy().to_string(), _ => unreachable!() };
                let output = match &validated[1] { mlua::Value::String(s) => s.to_string_lossy().to_string(), _ => unreachable!() };
                let input_host = m.resolve_read(&input).map_err(mlua::Error::external)?;
                let output_host = m.resolve_write(&output).map_err(mlua::Error::external)?;
                if let Some(parent) = output_host.parent() {
                    std::fs::create_dir_all(parent).map_err(mlua::Error::external)?;
                }
                bzip2_decompress(&input_host, &output_host)
                    .map_err(mlua::Error::RuntimeError)?;
                Ok(())
            })?,
        )?;
    }

    // compress.xz(input, output)
    {
        let m = mounts.clone();
        compress.set(
            "xz",
            lua.create_function(move |_, args: MultiValue| {
                let validated = validate_args(&args, COMPRESS_DOC.params("xz"), "compress.xz")?;
                let input = match &validated[0] { mlua::Value::String(s) => s.to_string_lossy().to_string(), _ => unreachable!() };
                let output = match &validated[1] { mlua::Value::String(s) => s.to_string_lossy().to_string(), _ => unreachable!() };
                let input_host = m.resolve_read(&input).map_err(mlua::Error::external)?;
                let output_host = m.resolve_write(&output).map_err(mlua::Error::external)?;
                if let Some(parent) = output_host.parent() {
                    std::fs::create_dir_all(parent).map_err(mlua::Error::external)?;
                }
                xz_compress(&input_host, &output_host)
                    .map_err(mlua::Error::RuntimeError)?;
                Ok(())
            })?,
        )?;
    }

    // compress.unxz(input, output)
    {
        let m = mounts.clone();
        compress.set(
            "unxz",
            lua.create_function(move |_, args: MultiValue| {
                let validated = validate_args(&args, COMPRESS_DOC.params("unxz"), "compress.unxz")?;
                let input = match &validated[0] { mlua::Value::String(s) => s.to_string_lossy().to_string(), _ => unreachable!() };
                let output = match &validated[1] { mlua::Value::String(s) => s.to_string_lossy().to_string(), _ => unreachable!() };
                let input_host = m.resolve_read(&input).map_err(mlua::Error::external)?;
                let output_host = m.resolve_write(&output).map_err(mlua::Error::external)?;
                if let Some(parent) = output_host.parent() {
                    std::fs::create_dir_all(parent).map_err(mlua::Error::external)?;
                }
                xz_decompress(&input_host, &output_host)
                    .map_err(mlua::Error::RuntimeError)?;
                Ok(())
            })?,
        )?;
    }

    // compress.un7z(archive, dest)
    {
        let m = mounts.clone();
        compress.set(
            "un7z",
            lua.create_function(move |_, args: MultiValue| {
                let validated = validate_args(&args, COMPRESS_DOC.params("un7z"), "compress.un7z")?;
                let archive = match &validated[0] { mlua::Value::String(s) => s.to_string_lossy().to_string(), _ => unreachable!() };
                let dest = match &validated[1] { mlua::Value::String(s) => s.to_string_lossy().to_string(), _ => unreachable!() };
                let archive_host =
                    m.resolve_read(&archive).map_err(mlua::Error::external)?;
                let dest_host =
                    m.resolve_write_deep(&dest).map_err(mlua::Error::external)?;
                std::fs::create_dir_all(&dest_host).map_err(mlua::Error::external)?;
                extract_7z(&archive_host, &dest_host)
                    .map_err(mlua::Error::RuntimeError)?;
                Ok(())
            })?,
        )?;
    }

    // compress.tar(source, archive)
    {
        let m = mounts.clone();
        compress.set(
            "tar",
            lua.create_function(
                move |lua, (first, archive_opt): (mlua::Value, Option<String>)| {
                    if matches!(first, mlua::Value::Nil) {
                        return Err(arg_error("compress.tar", COMPRESS_DOC.params("tar")));
                    }
                    let (source_val, archive) = if let Some(a) = archive_opt {
                        (first, a)
                    } else if let mlua::Value::Table(ref t) = first {
                        let arc = t.get::<String>(2)
                            .or_else(|_| t.get::<String>("archive"))
                            .map_err(|_| mlua::Error::external("compress.tar: missing archive argument"))?;
                        let src_val = if let Ok(s) = t.get::<String>(1) {
                            mlua::Value::String(lua.create_string(&s)?)
                        } else {
                            t.get::<mlua::Value>("source")
                                .map_err(|_| mlua::Error::external("compress.tar: missing source argument"))?
                        };
                        (src_val, arc)
                    } else {
                        return Err(mlua::Error::external("compress.tar: missing archive argument"));
                    };
                    let archive_host =
                        m.resolve_write(&archive).map_err(mlua::Error::external)?;
                    if let Some(parent) = archive_host.parent() {
                        std::fs::create_dir_all(parent).map_err(mlua::Error::external)?;
                    }
                    let sources = resolve_source_paths(&m, &source_val, "compress.tar")?;
                    create_tar(&sources, &archive_host)
                        .map_err(mlua::Error::RuntimeError)?;
                    Ok(())
                },
            )?,
        )?;
    }

    // compress.untar(archive, dest)
    {
        let m = mounts.clone();
        compress.set(
            "untar",
            lua.create_function(move |_, args: MultiValue| {
                let validated = validate_args(&args, COMPRESS_DOC.params("untar"), "compress.untar")?;
                let archive = match &validated[0] { mlua::Value::String(s) => s.to_string_lossy().to_string(), _ => unreachable!() };
                let dest = match &validated[1] { mlua::Value::String(s) => s.to_string_lossy().to_string(), _ => unreachable!() };
                let archive_host =
                    m.resolve_read(&archive).map_err(mlua::Error::external)?;
                let dest_host =
                    m.resolve_write_deep(&dest).map_err(mlua::Error::external)?;
                std::fs::create_dir_all(&dest_host).map_err(mlua::Error::external)?;
                extract_tar(&archive_host, &dest_host)
                    .map_err(mlua::Error::RuntimeError)?;
                Ok(())
            })?,
        )?;
    }

    // compress.targz(source, archive)
    {
        let m = mounts.clone();
        compress.set(
            "targz",
            lua.create_function(
                move |lua, (first, archive_opt): (mlua::Value, Option<String>)| {
                    if matches!(first, mlua::Value::Nil) {
                        return Err(arg_error("compress.targz", COMPRESS_DOC.params("targz")));
                    }
                    let (source_val, archive) = if let Some(a) = archive_opt {
                        (first, a)
                    } else if let mlua::Value::Table(ref t) = first {
                        let arc = t.get::<String>(2)
                            .or_else(|_| t.get::<String>("archive"))
                            .map_err(|_| mlua::Error::external("compress.targz: missing archive argument"))?;
                        let src_val = if let Ok(s) = t.get::<String>(1) {
                            mlua::Value::String(lua.create_string(&s)?)
                        } else {
                            t.get::<mlua::Value>("source")
                                .map_err(|_| mlua::Error::external("compress.targz: missing source argument"))?
                        };
                        (src_val, arc)
                    } else {
                        return Err(mlua::Error::external("compress.targz: missing archive argument"));
                    };
                    let archive_host =
                        m.resolve_write(&archive).map_err(mlua::Error::external)?;
                    if let Some(parent) = archive_host.parent() {
                        std::fs::create_dir_all(parent).map_err(mlua::Error::external)?;
                    }
                    let sources = resolve_source_paths(&m, &source_val, "compress.targz")?;
                    create_tar_gz(&sources, &archive_host)
                        .map_err(mlua::Error::RuntimeError)?;
                    Ok(())
                },
            )?,
        )?;
    }

    // compress.untargz(archive, dest)
    {
        let m = mounts.clone();
        compress.set(
            "untargz",
            lua.create_function(move |_, args: MultiValue| {
                let validated = validate_args(&args, COMPRESS_DOC.params("untargz"), "compress.untargz")?;
                let archive = match &validated[0] { mlua::Value::String(s) => s.to_string_lossy().to_string(), _ => unreachable!() };
                let dest = match &validated[1] { mlua::Value::String(s) => s.to_string_lossy().to_string(), _ => unreachable!() };
                let archive_host =
                    m.resolve_read(&archive).map_err(mlua::Error::external)?;
                let dest_host =
                    m.resolve_write_deep(&dest).map_err(mlua::Error::external)?;
                std::fs::create_dir_all(&dest_host).map_err(mlua::Error::external)?;
                extract_tar_gz(&archive_host, &dest_host)
                    .map_err(mlua::Error::RuntimeError)?;
                Ok(())
            })?,
        )?;
    }

    crate::lua_util::register_help_functions(lua, &compress, &COMPRESS_DOC)?;

    lua.globals().set("compress", compress)?;
    wrap_module_with_help_hints(lua, "compress")?;

    Ok(())
}

// --- Helpers ---

/// Resolve a `source` parameter that can be a string or table of strings into host paths.
fn resolve_source_paths(
    mounts: &MountTable,
    source: &mlua::Value,
    fn_name: &str,
) -> Result<Vec<std::path::PathBuf>, mlua::Error> {
    match source {
        mlua::Value::String(s) => {
            let s = s.to_str().map_err(mlua::Error::external)?;
            let host = mounts.resolve_read(&s).map_err(mlua::Error::external)?;
            Ok(vec![host])
        }
        mlua::Value::Table(t) => {
            let t = unwrap_py_seq(t)?;
            let paths: Vec<std::path::PathBuf> = t
                .sequence_values::<String>()
                .map(|r| {
                    let p = r?;
                    mounts.resolve_read(&p).map_err(mlua::Error::external)
                })
                .collect::<Result<Vec<_>, _>>()?;
            if paths.is_empty() {
                return Err(mlua::Error::RuntimeError(
                    format!("{}: source list is empty", fn_name),
                ));
            }
            Ok(paths)
        }
        _ => Err(mlua::Error::RuntimeError(
            format!("{}: source must be a string or table of strings", fn_name),
        )),
    }
}

// --- ZIP ---

fn create_zip(source: &Path, dest: &Path) -> Result<(), String> {
    let file = File::create(dest).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    if source.is_dir() {
        add_dir_to_zip(&mut zip, source, source, options, None)?;
    } else {
        let name = source
            .file_name()
            .ok_or_else(|| "source has no file name".to_string())?
            .to_string_lossy();
        zip.start_file(name, options).map_err(|e| e.to_string())?;
        let mut f = File::open(source).map_err(|e| e.to_string())?;
        std::io::copy(&mut f, &mut zip).map_err(|e| e.to_string())?;
    }

    zip.finish().map_err(|e| e.to_string())?;
    Ok(())
}

fn create_zip_multi(sources: &[std::path::PathBuf], dest: &Path) -> Result<(), String> {
    let file = File::create(dest).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    for source in sources {
        let name = source
            .file_name()
            .ok_or_else(|| format!("source has no file name: {}", source.display()))?
            .to_string_lossy()
            .to_string();

        if source.is_dir() {
            zip.add_directory(format!("{}/", name), options)
                .map_err(|e| e.to_string())?;
            add_dir_to_zip(&mut zip, source, source, options, Some(&name))?;
        } else {
            zip.start_file(&name, options).map_err(|e| e.to_string())?;
            let mut f = File::open(source).map_err(|e| e.to_string())?;
            std::io::copy(&mut f, &mut zip).map_err(|e| e.to_string())?;
        }
    }

    zip.finish().map_err(|e| e.to_string())?;
    Ok(())
}

fn add_dir_to_zip<W: Write + std::io::Seek>(
    zip: &mut zip::ZipWriter<W>,
    dir: &Path,
    base: &Path,
    options: zip::write::SimpleFileOptions,
    prefix: Option<&str>,
) -> Result<(), String> {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok())
        .collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let path = entry.path();
        let relative = path
            .strip_prefix(base)
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .to_string();
        let entry_name = match prefix {
            Some(p) => format!("{}/{}", p, relative),
            None => relative,
        };

        if path.is_dir() {
            zip.add_directory(format!("{}/", entry_name), options)
                .map_err(|e| e.to_string())?;
            add_dir_to_zip(zip, &path, base, options, prefix)?;
        } else {
            zip.start_file(&entry_name, options)
                .map_err(|e| e.to_string())?;
            let mut f = File::open(&path).map_err(|e| e.to_string())?;
            std::io::copy(&mut f, zip).map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}

fn extract_zip(archive: &Path, dest: &Path) -> Result<(), String> {
    let file = File::open(archive).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    let dest_canonical = dest.canonicalize().map_err(|e| e.to_string())?;

    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;

        // Reject symlinks — they could point outside the mount boundary
        if entry.is_symlink() {
            return Err(format!(
                "zip entry '{}' is a symlink — rejected for sandbox safety",
                entry.name()
            ));
        }

        let out_path = dest.join(
            entry
                .enclosed_name()
                .ok_or_else(|| format!("zip entry '{}' has unsafe path", entry.name()))?,
        );

        // Verify resolved path stays within destination
        let out_canonical = if out_path.exists() {
            out_path.canonicalize().map_err(|e| e.to_string())?
        } else {
            let parent = out_path
                .parent()
                .ok_or_else(|| format!("invalid entry path: {}", entry.name()))?;
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            parent
                .canonicalize()
                .map_err(|e| e.to_string())?
                .join(out_path.file_name().unwrap())
        };

        if !out_canonical.starts_with(&dest_canonical) {
            return Err(format!(
                "zip entry '{}' escapes destination — rejected",
                entry.name()
            ));
        }

        if entry.is_dir() {
            std::fs::create_dir_all(&out_canonical).map_err(|e| e.to_string())?;
        } else {
            let mut outfile = File::create(&out_canonical).map_err(|e| e.to_string())?;
            std::io::copy(&mut entry, &mut outfile).map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}

// --- TAR ---

fn create_tar(sources: &[std::path::PathBuf], dest: &Path) -> Result<(), String> {
    let file = File::create(dest).map_err(|e| e.to_string())?;
    let mut builder = tar::Builder::new(file);
    append_sources_to_tar(&mut builder, sources)?;
    builder.finish().map_err(|e| e.to_string())?;
    Ok(())
}

fn create_tar_gz(sources: &[std::path::PathBuf], dest: &Path) -> Result<(), String> {
    let file = File::create(dest).map_err(|e| e.to_string())?;
    let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
    let mut builder = tar::Builder::new(encoder);
    append_sources_to_tar(&mut builder, sources)?;
    builder.into_inner().map_err(|e| e.to_string())?
        .finish().map_err(|e| e.to_string())?;
    Ok(())
}

fn append_sources_to_tar<W: Write>(
    builder: &mut tar::Builder<W>,
    sources: &[std::path::PathBuf],
) -> Result<(), String> {
    for source in sources {
        let name = source
            .file_name()
            .ok_or_else(|| format!("source has no file name: {}", source.display()))?
            .to_string_lossy()
            .to_string();

        if source.is_dir() {
            builder
                .append_dir_all(&name, source)
                .map_err(|e| e.to_string())?;
        } else {
            let mut f = File::open(source).map_err(|e| e.to_string())?;
            builder
                .append_file(&name, &mut f)
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn extract_tar(archive: &Path, dest: &Path) -> Result<(), String> {
    let file = File::open(archive).map_err(|e| e.to_string())?;
    extract_tar_from_reader(file, dest)
}

fn extract_tar_gz(archive: &Path, dest: &Path) -> Result<(), String> {
    let file = File::open(archive).map_err(|e| e.to_string())?;
    let decoder = flate2::read::GzDecoder::new(file);
    extract_tar_from_reader(decoder, dest)
}

fn extract_tar_from_reader<R: Read>(reader: R, dest: &Path) -> Result<(), String> {
    let dest_canonical = dest.canonicalize().map_err(|e| e.to_string())?;
    let mut archive = tar::Archive::new(reader);

    for entry_result in archive.entries().map_err(|e| e.to_string())? {
        let mut entry = entry_result.map_err(|e| e.to_string())?;
        let entry_type = entry.header().entry_type();

        // Reject symlinks — they could point outside the mount boundary
        if entry_type == tar::EntryType::Symlink || entry_type == tar::EntryType::Link {
            let path_display = entry
                .path()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| "<unknown>".into());
            return Err(format!(
                "tar entry '{}' is a symlink/hardlink — rejected for sandbox safety",
                path_display
            ));
        }

        let entry_path = entry.path().map_err(|e| e.to_string())?.into_owned();

        // Reject entries with parent traversal components
        for component in entry_path.components() {
            if matches!(component, std::path::Component::ParentDir) {
                return Err(format!(
                    "tar entry '{}' contains '..' — rejected for sandbox safety",
                    entry_path.display()
                ));
            }
        }

        let out_path = dest.join(&entry_path);

        // Verify resolved path stays within destination
        let out_canonical = if out_path.exists() {
            out_path.canonicalize().map_err(|e| e.to_string())?
        } else {
            let parent = out_path
                .parent()
                .ok_or_else(|| format!("invalid entry path: {}", entry_path.display()))?;
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            parent
                .canonicalize()
                .map_err(|e| e.to_string())?
                .join(out_path.file_name().unwrap())
        };

        if !out_canonical.starts_with(&dest_canonical) {
            return Err(format!(
                "tar entry '{}' escapes destination — rejected",
                entry_path.display()
            ));
        }

        if entry_type == tar::EntryType::Directory {
            std::fs::create_dir_all(&out_canonical).map_err(|e| e.to_string())?;
        } else {
            if let Some(parent) = out_canonical.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let mut outfile = File::create(&out_canonical).map_err(|e| e.to_string())?;
            std::io::copy(&mut entry, &mut outfile).map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}

// --- BZIP2 ---

fn bzip2_compress(input: &Path, output: &Path) -> Result<(), String> {
    let data = std::fs::read(input).map_err(|e| e.to_string())?;
    let mut encoder =
        bzip2::write::BzEncoder::new(Vec::new(), bzip2::Compression::default());
    encoder.write_all(&data).map_err(|e| e.to_string())?;
    let compressed = encoder.finish().map_err(|e| e.to_string())?;
    std::fs::write(output, compressed).map_err(|e| e.to_string())?;
    Ok(())
}

fn bzip2_decompress(input: &Path, output: &Path) -> Result<(), String> {
    let data = std::fs::read(input).map_err(|e| e.to_string())?;
    let mut decoder = bzip2::read::BzDecoder::new(&data[..]);
    let mut decompressed = Vec::new();
    decoder
        .read_to_end(&mut decompressed)
        .map_err(|e| e.to_string())?;
    std::fs::write(output, decompressed).map_err(|e| e.to_string())?;
    Ok(())
}

// --- XZ ---

fn xz_compress(input: &Path, output: &Path) -> Result<(), String> {
    let data = std::fs::read(input).map_err(|e| e.to_string())?;
    let mut encoder = xz2::write::XzEncoder::new(Vec::new(), 6);
    encoder.write_all(&data).map_err(|e| e.to_string())?;
    let compressed = encoder.finish().map_err(|e| e.to_string())?;
    std::fs::write(output, compressed).map_err(|e| e.to_string())?;
    Ok(())
}

fn xz_decompress(input: &Path, output: &Path) -> Result<(), String> {
    let data = std::fs::read(input).map_err(|e| e.to_string())?;
    let mut decoder = xz2::read::XzDecoder::new(&data[..]);
    let mut decompressed = Vec::new();
    decoder
        .read_to_end(&mut decompressed)
        .map_err(|e| e.to_string())?;
    std::fs::write(output, decompressed).map_err(|e| e.to_string())?;
    Ok(())
}

// --- 7Z ---

fn extract_7z(archive: &Path, dest: &Path) -> Result<(), String> {
    let dest_canonical = dest.canonicalize().map_err(|e| e.to_string())?;

    sevenz_rust::decompress_file_with_extract_fn(archive, dest, |entry, reader, out_path| {
        // Verify resolved path stays within destination
        let out_canonical = if out_path.exists() {
            out_path.canonicalize()?
        } else {
            let parent = out_path.parent().ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("invalid entry path: {}", entry.name()),
                )
            })?;
            std::fs::create_dir_all(parent)?;
            parent.canonicalize()?.join(out_path.file_name().unwrap())
        };

        if !out_canonical.starts_with(&dest_canonical) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                format!(
                    "7z entry '{}' escapes destination — rejected",
                    entry.name()
                ),
            )
            .into());
        }

        if entry.is_directory() {
            std::fs::create_dir_all(&out_canonical)?;
        } else {
            if let Some(parent) = out_canonical.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut outfile = File::create(&out_canonical)?;
            std::io::copy(reader, &mut outfile)?;
        }

        Ok(true)
    })
    .map_err(|e| format!("7z extraction failed: {}", e))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{MountTable, Sandbox};
    use std::fs;
    use tempfile::TempDir;

    fn sandbox_with_dir() -> (TempDir, Sandbox) {
        let dir = TempDir::new().unwrap();
        let mut table = MountTable::new();
        table
            .parse_and_add(&format!("{}:/workspace", dir.path().display()))
            .unwrap();
        let sandbox = Sandbox::with_mounts(table).unwrap();
        (dir, sandbox)
    }

    #[test]
    fn test_compress_help() {
        let sandbox = Sandbox::new().unwrap();
        let result = sandbox.exec("return compress.help()").unwrap();
        assert!(result.contains("compress — archive & compression"));
        assert!(result.contains("compress.zip"));
        assert!(result.contains("compress.unzip"));
        assert!(result.contains("compress.bzip2"));
        assert!(result.contains("compress.bunzip2"));
        assert!(result.contains("compress.xz"));
        assert!(result.contains("compress.unxz"));
        assert!(result.contains("compress.un7z"));
    }

    #[test]
    fn test_compress_help_bare_call() {
        let sandbox = Sandbox::new().unwrap();
        let result = sandbox.exec("compress.help()").unwrap();
        assert!(result.contains("compress — archive & compression"));
    }

    #[test]
    fn test_compress_nonexistent_fn_hint() {
        let sandbox = Sandbox::new().unwrap();
        let err = sandbox.exec("compress.gzip()").unwrap_err().to_string();
        assert!(err.contains("compress.gzip does not exist"));
        assert!(err.contains("hint: call compress.help() for usage"));
    }

    #[test]
    fn test_zip_roundtrip_file() {
        let (dir, sandbox) = sandbox_with_dir();
        fs::write(dir.path().join("hello.txt"), "hello world").unwrap();

        sandbox
            .exec("compress.zip('/workspace/hello.txt', '/workspace/out.zip')")
            .unwrap();
        assert!(dir.path().join("out.zip").exists());

        fs::create_dir(dir.path().join("extracted")).unwrap();
        sandbox
            .exec("compress.unzip('/workspace/out.zip', '/workspace/extracted')")
            .unwrap();

        let content =
            fs::read_to_string(dir.path().join("extracted/hello.txt")).unwrap();
        assert_eq!(content, "hello world");
    }

    #[test]
    fn test_zip_roundtrip_directory() {
        let (dir, sandbox) = sandbox_with_dir();
        fs::create_dir(dir.path().join("mydir")).unwrap();
        fs::write(dir.path().join("mydir/a.txt"), "aaa").unwrap();
        fs::write(dir.path().join("mydir/b.txt"), "bbb").unwrap();

        sandbox
            .exec("compress.zip('/workspace/mydir', '/workspace/mydir.zip')")
            .unwrap();
        assert!(dir.path().join("mydir.zip").exists());

        fs::create_dir(dir.path().join("out")).unwrap();
        sandbox
            .exec("compress.unzip('/workspace/mydir.zip', '/workspace/out')")
            .unwrap();

        assert_eq!(
            fs::read_to_string(dir.path().join("out/a.txt")).unwrap(),
            "aaa"
        );
        assert_eq!(
            fs::read_to_string(dir.path().join("out/b.txt")).unwrap(),
            "bbb"
        );
    }

    #[test]
    fn test_zip_multiple_files() {
        let (dir, sandbox) = sandbox_with_dir();
        fs::write(dir.path().join("a.txt"), "aaa").unwrap();
        fs::write(dir.path().join("b.txt"), "bbb").unwrap();
        fs::write(dir.path().join("c.txt"), "ccc").unwrap();

        sandbox
            .exec("compress.zip({'/workspace/a.txt', '/workspace/c.txt'}, '/workspace/out.zip')")
            .unwrap();
        assert!(dir.path().join("out.zip").exists());

        sandbox
            .exec("compress.unzip('/workspace/out.zip', '/workspace/extracted')")
            .unwrap();

        assert_eq!(
            fs::read_to_string(dir.path().join("extracted/a.txt")).unwrap(),
            "aaa"
        );
        assert_eq!(
            fs::read_to_string(dir.path().join("extracted/c.txt")).unwrap(),
            "ccc"
        );
        // b.txt should NOT be in the archive
        assert!(!dir.path().join("extracted/b.txt").exists());
    }

    #[test]
    fn test_zip_mixed_files_and_dirs() {
        let (dir, sandbox) = sandbox_with_dir();
        fs::write(dir.path().join("solo.txt"), "solo").unwrap();
        fs::create_dir(dir.path().join("subdir")).unwrap();
        fs::write(dir.path().join("subdir/x.txt"), "xxx").unwrap();
        fs::write(dir.path().join("subdir/y.txt"), "yyy").unwrap();

        sandbox
            .exec("compress.zip({'/workspace/solo.txt', '/workspace/subdir'}, '/workspace/mix.zip')")
            .unwrap();

        sandbox
            .exec("compress.unzip('/workspace/mix.zip', '/workspace/out')")
            .unwrap();

        assert_eq!(
            fs::read_to_string(dir.path().join("out/solo.txt")).unwrap(),
            "solo"
        );
        assert_eq!(
            fs::read_to_string(dir.path().join("out/subdir/x.txt")).unwrap(),
            "xxx"
        );
        assert_eq!(
            fs::read_to_string(dir.path().join("out/subdir/y.txt")).unwrap(),
            "yyy"
        );
    }

    #[test]
    fn test_zip_empty_table_errors() {
        let (_dir, sandbox) = sandbox_with_dir();
        let err = sandbox
            .exec("compress.zip({}, '/workspace/out.zip')")
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("empty"),
            "expected 'empty' in error, got: {}",
            err
        );
    }

    #[test]
    fn test_bzip2_roundtrip() {
        let (dir, sandbox) = sandbox_with_dir();
        let original = "The quick brown fox jumps over the lazy dog.";
        fs::write(dir.path().join("data.txt"), original).unwrap();

        sandbox
            .exec("compress.bzip2('/workspace/data.txt', '/workspace/data.txt.bz2')")
            .unwrap();
        assert!(dir.path().join("data.txt.bz2").exists());

        sandbox
            .exec("compress.bunzip2('/workspace/data.txt.bz2', '/workspace/restored.txt')")
            .unwrap();

        let restored =
            fs::read_to_string(dir.path().join("restored.txt")).unwrap();
        assert_eq!(restored, original);
    }

    #[test]
    fn test_xz_roundtrip() {
        let (dir, sandbox) = sandbox_with_dir();
        let original = "Compression test data for xz format.";
        fs::write(dir.path().join("data.txt"), original).unwrap();

        sandbox
            .exec("compress.xz('/workspace/data.txt', '/workspace/data.txt.xz')")
            .unwrap();
        assert!(dir.path().join("data.txt.xz").exists());

        sandbox
            .exec("compress.unxz('/workspace/data.txt.xz', '/workspace/restored.txt')")
            .unwrap();

        let restored =
            fs::read_to_string(dir.path().join("restored.txt")).unwrap();
        assert_eq!(restored, original);
    }

    #[test]
    fn test_unmounted_path_rejected() {
        let sandbox = Sandbox::new().unwrap();
        let err = sandbox
            .exec("compress.zip('/etc/passwd', '/tmp/out.zip')")
            .unwrap_err()
            .to_string();
        assert!(err.contains("No such file"));
    }

    #[test]
    fn test_readonly_mount_write_rejected() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("file.txt"), "content").unwrap();
        let mut table = MountTable::new();
        table
            .parse_and_add(&format!("{}:/data:ro", dir.path().display()))
            .unwrap();
        let sandbox = Sandbox::with_mounts(table).unwrap();

        let err = sandbox
            .exec("compress.bzip2('/data/file.txt', '/data/file.txt.bz2')")
            .unwrap_err()
            .to_string();
        assert!(err.contains("Read-only"));
    }

    #[test]
    fn test_unzip_creates_dest_dir() {
        let (dir, sandbox) = sandbox_with_dir();

        // Create a zip file first
        fs::write(dir.path().join("test.txt"), "test content").unwrap();
        sandbox
            .exec("compress.zip('/workspace/test.txt', '/workspace/archive.zip')")
            .unwrap();

        // Extract to a non-existent directory (one level deep)
        sandbox
            .exec("compress.unzip('/workspace/archive.zip', '/workspace/newdir')")
            .unwrap();
        assert!(dir.path().join("newdir/test.txt").exists());
    }

    #[test]
    fn test_global_help_mentions_compress() {
        let sandbox = Sandbox::new().unwrap();
        let result = sandbox.exec("return help()").unwrap();
        assert!(
            result.contains("compress"),
            "global help should mention compress module: {}",
            result
        );
    }

    /// Create a malicious zip with an arbitrary entry name for testing path traversal.
    /// Uses `start_file` with a raw string to bypass any path sanitization.
    fn create_zip_slip_archive(archive_path: &std::path::Path, entry_name: &str) {
        use std::io::Write;
        let file = std::fs::File::create(archive_path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file(entry_name, options).unwrap();
        zip.write_all(b"escaped content").unwrap();
        zip.finish().unwrap();
    }

    #[test]
    fn test_unzip_rejects_path_traversal() {
        let dir = TempDir::new().unwrap();
        let extract_dir = dir.path().join("extract");
        fs::create_dir(&extract_dir).unwrap();

        // Create a zip with a "../escape.txt" entry
        let archive = dir.path().join("evil.zip");
        create_zip_slip_archive(&archive, "../escape.txt");

        // extract_zip should reject the traversal attempt
        let result = super::extract_zip(&archive, &extract_dir);
        assert!(result.is_err(), "zip slip should be rejected");
        let err = result.unwrap_err();
        assert!(
            err.contains("unsafe path") || err.contains("escapes destination"),
            "error should mention safety: {}",
            err
        );

        // The escaped file must NOT exist outside the extraction directory
        assert!(
            !dir.path().join("escape.txt").exists(),
            "file must not escape extraction directory"
        );
    }

    #[test]
    fn test_unzip_rejects_absolute_path_entry() {
        let dir = TempDir::new().unwrap();
        let extract_dir = dir.path().join("extract");
        fs::create_dir(&extract_dir).unwrap();

        let archive = dir.path().join("evil.zip");
        create_zip_slip_archive(&archive, "/tmp/pwned.txt");

        let result = super::extract_zip(&archive, &extract_dir);
        assert!(result.is_err(), "absolute path entry should be rejected");

        assert!(
            !std::path::Path::new("/tmp/pwned.txt").exists(),
            "file must not be written to absolute path"
        );
    }

    #[test]
    fn test_unzip_via_sandbox_rejects_traversal() {
        let (dir, sandbox) = sandbox_with_dir();

        // Create the malicious zip directly on the host filesystem
        let extract_dir = dir.path().join("output");
        fs::create_dir(&extract_dir).unwrap();
        create_zip_slip_archive(&dir.path().join("evil.zip"), "../escape.txt");

        let result = sandbox.exec("compress.unzip('/workspace/evil.zip', '/workspace/output')");
        assert!(result.is_err(), "sandbox unzip should reject zip slip");

        // Must not escape
        assert!(
            !dir.path().join("escape.txt").exists(),
            "file must not escape mount boundary"
        );
    }

    // --- TAR tests ---

    #[test]
    fn test_tar_roundtrip_file() {
        let (dir, sandbox) = sandbox_with_dir();
        fs::write(dir.path().join("hello.txt"), "hello tar").unwrap();

        sandbox
            .exec("compress.tar('/workspace/hello.txt', '/workspace/out.tar')")
            .unwrap();
        assert!(dir.path().join("out.tar").exists());

        sandbox
            .exec("compress.untar('/workspace/out.tar', '/workspace/extracted')")
            .unwrap();

        let content = fs::read_to_string(dir.path().join("extracted/hello.txt")).unwrap();
        assert_eq!(content, "hello tar");
    }

    #[test]
    fn test_tar_roundtrip_directory() {
        let (dir, sandbox) = sandbox_with_dir();
        fs::create_dir(dir.path().join("mydir")).unwrap();
        fs::write(dir.path().join("mydir/a.txt"), "aaa").unwrap();
        fs::write(dir.path().join("mydir/b.txt"), "bbb").unwrap();

        sandbox
            .exec("compress.tar('/workspace/mydir', '/workspace/mydir.tar')")
            .unwrap();
        assert!(dir.path().join("mydir.tar").exists());

        sandbox
            .exec("compress.untar('/workspace/mydir.tar', '/workspace/out')")
            .unwrap();

        assert_eq!(
            fs::read_to_string(dir.path().join("out/mydir/a.txt")).unwrap(),
            "aaa"
        );
        assert_eq!(
            fs::read_to_string(dir.path().join("out/mydir/b.txt")).unwrap(),
            "bbb"
        );
    }

    #[test]
    fn test_tar_multi_source() {
        let (dir, sandbox) = sandbox_with_dir();
        fs::write(dir.path().join("a.txt"), "aaa").unwrap();
        fs::write(dir.path().join("b.txt"), "bbb").unwrap();
        fs::create_dir(dir.path().join("sub")).unwrap();
        fs::write(dir.path().join("sub/c.txt"), "ccc").unwrap();

        sandbox
            .exec("compress.tar({'/workspace/a.txt', '/workspace/sub'}, '/workspace/multi.tar')")
            .unwrap();

        sandbox
            .exec("compress.untar('/workspace/multi.tar', '/workspace/out')")
            .unwrap();

        assert_eq!(
            fs::read_to_string(dir.path().join("out/a.txt")).unwrap(),
            "aaa"
        );
        assert_eq!(
            fs::read_to_string(dir.path().join("out/sub/c.txt")).unwrap(),
            "ccc"
        );
        // b.txt was not included
        assert!(!dir.path().join("out/b.txt").exists());
    }

    #[test]
    fn test_targz_roundtrip_file() {
        let (dir, sandbox) = sandbox_with_dir();
        fs::write(dir.path().join("hello.txt"), "hello targz").unwrap();

        sandbox
            .exec("compress.targz('/workspace/hello.txt', '/workspace/out.tar.gz')")
            .unwrap();
        assert!(dir.path().join("out.tar.gz").exists());

        sandbox
            .exec("compress.untargz('/workspace/out.tar.gz', '/workspace/extracted')")
            .unwrap();

        let content = fs::read_to_string(dir.path().join("extracted/hello.txt")).unwrap();
        assert_eq!(content, "hello targz");
    }

    #[test]
    fn test_targz_roundtrip_directory() {
        let (dir, sandbox) = sandbox_with_dir();
        fs::create_dir(dir.path().join("mydir")).unwrap();
        fs::write(dir.path().join("mydir/x.txt"), "xxx").unwrap();
        fs::write(dir.path().join("mydir/y.txt"), "yyy").unwrap();

        sandbox
            .exec("compress.targz('/workspace/mydir', '/workspace/mydir.tar.gz')")
            .unwrap();

        sandbox
            .exec("compress.untargz('/workspace/mydir.tar.gz', '/workspace/out')")
            .unwrap();

        assert_eq!(
            fs::read_to_string(dir.path().join("out/mydir/x.txt")).unwrap(),
            "xxx"
        );
        assert_eq!(
            fs::read_to_string(dir.path().join("out/mydir/y.txt")).unwrap(),
            "yyy"
        );
    }

    #[test]
    fn test_targz_multi_source() {
        let (dir, sandbox) = sandbox_with_dir();
        fs::write(dir.path().join("a.txt"), "aaa").unwrap();
        fs::create_dir(dir.path().join("sub")).unwrap();
        fs::write(dir.path().join("sub/b.txt"), "bbb").unwrap();

        sandbox
            .exec(
                "compress.targz({'/workspace/a.txt', '/workspace/sub'}, '/workspace/multi.tar.gz')",
            )
            .unwrap();

        sandbox
            .exec("compress.untargz('/workspace/multi.tar.gz', '/workspace/out')")
            .unwrap();

        assert_eq!(
            fs::read_to_string(dir.path().join("out/a.txt")).unwrap(),
            "aaa"
        );
        assert_eq!(
            fs::read_to_string(dir.path().join("out/sub/b.txt")).unwrap(),
            "bbb"
        );
    }

    // --- TAR security tests ---

    /// Create a malicious tar archive with an arbitrary entry path.
    /// Built at the byte level because the `tar` crate rejects `..` in paths.
    fn create_tar_slip_archive(archive_path: &std::path::Path, entry_name: &str) {
        use std::io::Write;
        let data = b"escaped content";

        // Build a 512-byte USTAR header
        let mut header = [0u8; 512];
        // name field: bytes 0..100
        let name_bytes = entry_name.as_bytes();
        header[..name_bytes.len()].copy_from_slice(name_bytes);
        // mode: bytes 100..108
        header[100..107].copy_from_slice(b"0000644");
        // uid: bytes 108..116
        header[108..115].copy_from_slice(b"0001000");
        // gid: bytes 116..124
        header[116..123].copy_from_slice(b"0001000");
        // size: bytes 124..136 (octal)
        let size_str = format!("{:011o}", data.len());
        header[124..135].copy_from_slice(size_str.as_bytes());
        // mtime: bytes 136..148
        header[136..147].copy_from_slice(b"14717570000");
        // typeflag: byte 156 ('0' = regular file)
        header[156] = b'0';
        // magic: bytes 257..263
        header[257..263].copy_from_slice(b"ustar\0");
        // version: bytes 263..265
        header[263..265].copy_from_slice(b"00");

        // Compute checksum: sum of all bytes, treating checksum field (148..156) as spaces
        header[148..156].copy_from_slice(b"        ");
        let cksum: u32 = header.iter().map(|&b| b as u32).sum();
        let cksum_str = format!("{:06o}\0 ", cksum);
        header[148..156].copy_from_slice(cksum_str.as_bytes());

        let mut file = std::fs::File::create(archive_path).unwrap();
        file.write_all(&header).unwrap();
        // Data block (padded to 512 bytes)
        let mut data_block = [0u8; 512];
        data_block[..data.len()].copy_from_slice(data);
        file.write_all(&data_block).unwrap();
        // Two zero blocks to terminate
        file.write_all(&[0u8; 1024]).unwrap();
    }

    /// Create a tar archive containing a symlink entry.
    fn create_tar_symlink_archive(archive_path: &std::path::Path) {
        let file = std::fs::File::create(archive_path).unwrap();
        let mut builder = tar::Builder::new(file);
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(tar::EntryType::Symlink);
        header.set_size(0);
        header.set_mode(0o777);
        header.set_cksum();
        builder
            .append_link(&mut header, "evil_link", "/etc/passwd")
            .unwrap();
        builder.finish().unwrap();
    }

    #[test]
    fn test_untar_rejects_path_traversal() {
        let dir = TempDir::new().unwrap();
        let extract_dir = dir.path().join("extract");
        fs::create_dir(&extract_dir).unwrap();

        let archive = dir.path().join("evil.tar");
        create_tar_slip_archive(&archive, "../escape.txt");

        let result = super::extract_tar(&archive, &extract_dir);
        assert!(result.is_err(), "tar slip should be rejected");
        let err = result.unwrap_err();
        assert!(
            err.contains("..") || err.contains("escapes destination"),
            "error should mention safety: {}",
            err
        );

        assert!(
            !dir.path().join("escape.txt").exists(),
            "file must not escape extraction directory"
        );
    }

    #[test]
    fn test_untar_rejects_symlink() {
        let dir = TempDir::new().unwrap();
        let extract_dir = dir.path().join("extract");
        fs::create_dir(&extract_dir).unwrap();

        let archive = dir.path().join("symlink.tar");
        create_tar_symlink_archive(&archive);

        let result = super::extract_tar(&archive, &extract_dir);
        assert!(result.is_err(), "symlink entry should be rejected");
        let err = result.unwrap_err();
        assert!(
            err.contains("symlink") || err.contains("hardlink"),
            "error should mention symlink: {}",
            err
        );
    }

    #[test]
    fn test_untar_via_sandbox_rejects_traversal() {
        let (dir, sandbox) = sandbox_with_dir();

        let extract_dir = dir.path().join("output");
        fs::create_dir(&extract_dir).unwrap();
        create_tar_slip_archive(&dir.path().join("evil.tar"), "../escape.txt");

        let result = sandbox.exec("compress.untar('/workspace/evil.tar', '/workspace/output')");
        assert!(result.is_err(), "sandbox untar should reject tar slip");

        assert!(
            !dir.path().join("escape.txt").exists(),
            "file must not escape mount boundary"
        );
    }

    // --- TAR edge case tests ---

    #[test]
    fn test_tar_empty_table_errors() {
        let (_dir, sandbox) = sandbox_with_dir();
        let err = sandbox
            .exec("compress.tar({}, '/workspace/out.tar')")
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("empty"),
            "expected 'empty' in error, got: {}",
            err
        );
    }

    #[test]
    fn test_targz_empty_table_errors() {
        let (_dir, sandbox) = sandbox_with_dir();
        let err = sandbox
            .exec("compress.targz({}, '/workspace/out.tar.gz')")
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("empty"),
            "expected 'empty' in error, got: {}",
            err
        );
    }

    #[test]
    fn test_untar_creates_dest_dir() {
        let (dir, sandbox) = sandbox_with_dir();
        fs::write(dir.path().join("test.txt"), "test content").unwrap();
        sandbox
            .exec("compress.tar('/workspace/test.txt', '/workspace/archive.tar')")
            .unwrap();

        // Extract to a non-existent directory
        sandbox
            .exec("compress.untar('/workspace/archive.tar', '/workspace/newdir')")
            .unwrap();
        assert!(dir.path().join("newdir/test.txt").exists());
    }

    #[test]
    fn test_compress_help_mentions_tar() {
        let sandbox = Sandbox::new().unwrap();
        let result = sandbox.exec("return compress.help()").unwrap();
        assert!(result.contains("compress.tar"));
        assert!(result.contains("compress.untar"));
        assert!(result.contains("compress.targz"));
        assert!(result.contains("compress.untargz"));
    }
}
