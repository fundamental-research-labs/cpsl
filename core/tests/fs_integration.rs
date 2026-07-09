#![cfg(feature = "mod-fs")]

use cpsl_core::{MountTable, Sandbox};
use std::fs;
use tempfile::TempDir;

fn sandbox_with_dir(dir: &TempDir, virtual_path: &str, permission: &str) -> Sandbox {
    let mut mounts = MountTable::new();
    let spec = format!("{}:{}:{}", dir.path().display(), virtual_path, permission);
    mounts.parse_and_add(&spec).unwrap();
    Sandbox::with_mounts(mounts).unwrap()
}

#[test]
fn test_fs_read_mounted_file() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("hello.txt"), "world").unwrap();

    let sandbox = sandbox_with_dir(&dir, "/data", "rw");
    let result = sandbox.exec("return fs.read('/data/hello.txt')").unwrap();
    assert_eq!(result, "world");
}

#[test]
fn test_fs_write_to_mounted_file() {
    let dir = TempDir::new().unwrap();

    let sandbox = sandbox_with_dir(&dir, "/data", "rw");
    sandbox
        .exec("fs.write('/data/output.txt', 'written from luau')")
        .unwrap();

    let content = fs::read_to_string(dir.path().join("output.txt")).unwrap();
    assert_eq!(content, "written from luau");
}

#[test]
fn test_fs_read_outside_mount_fails() {
    let dir = TempDir::new().unwrap();
    let sandbox = sandbox_with_dir(&dir, "/data", "rw");

    let result = sandbox.exec("return fs.read('/etc/passwd')");
    assert!(result.is_err(), "reading outside mount should fail");
    // Error should say "No such file or directory", not mention mounts
    let err = result.unwrap_err().to_string();
    assert!(err.contains("No such file or directory"), "got: {}", err);
}

#[test]
fn test_fs_path_traversal_fails() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("file.txt"), "safe").unwrap();

    let sandbox = sandbox_with_dir(&dir, "/data", "rw");
    let result = sandbox.exec("return fs.read('/data/../../etc/passwd')");
    assert!(result.is_err(), "path traversal should be rejected");
}

#[test]
fn test_fs_list_mounted_directory() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("a.txt"), "").unwrap();
    fs::write(dir.path().join("b.txt"), "").unwrap();
    fs::create_dir(dir.path().join("subdir")).unwrap();

    let sandbox = sandbox_with_dir(&dir, "/data", "rw");
    let result = sandbox
        .exec(
            r#"
        local entries = fs.list('/data')
        table.sort(entries)
        return table.concat(entries, ',')
    "#,
        )
        .unwrap();
    assert_eq!(result, "a.txt,b.txt,subdir");
}

#[test]
fn test_fs_exists() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("exists.txt"), "hi").unwrap();

    let sandbox = sandbox_with_dir(&dir, "/data", "rw");
    let result = sandbox
        .exec("return fs.exists('/data/exists.txt')")
        .unwrap();
    assert_eq!(result, "true");

    let result = sandbox.exec("return fs.exists('/data/nope.txt')").unwrap();
    assert_eq!(result, "false");

    // Outside mount should return false, not error
    let result = sandbox.exec("return fs.exists('/etc/passwd')").unwrap();
    assert_eq!(result, "false");
}

#[test]
fn test_fs_mkdir() {
    let dir = TempDir::new().unwrap();

    let sandbox = sandbox_with_dir(&dir, "/data", "rw");
    sandbox.exec("fs.mkdir('/data/newdir')").unwrap();

    assert!(dir.path().join("newdir").is_dir());
}

#[test]
fn test_fs_write_to_readonly_mount_fails() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("file.txt"), "content").unwrap();

    let sandbox = sandbox_with_dir(&dir, "/data", "ro");

    // Read should work
    let result = sandbox.exec("return fs.read('/data/file.txt')").unwrap();
    assert_eq!(result, "content");

    // Write should fail
    let result = sandbox.exec("fs.write('/data/file.txt', 'hacked')");
    assert!(result.is_err(), "writing to read-only mount should fail");
    let err = result.unwrap_err().to_string();
    assert!(err.contains("Read-only file system"), "got: {}", err);
}

#[test]
fn test_dangerous_globals_removed() {
    let sandbox = Sandbox::new().unwrap();

    let result = sandbox.exec("return io").unwrap();
    assert_eq!(result, "nil");

    // os table is entirely removed
    let result = sandbox.exec("return os").unwrap();
    assert_eq!(result, "nil");

    let result = sandbox.exec("return loadfile").unwrap();
    assert_eq!(result, "nil");

    let result = sandbox.exec("return dofile").unwrap();
    assert_eq!(result, "nil");
}

// --- Virtual directory tests ---

#[test]
fn test_fs_list_root_always_works() {
    // Even with no mounts, `/` should be listable (has synthetic dirs like /dev)
    let sandbox = Sandbox::new().unwrap();
    let result = sandbox
        .exec(
            r#"
        local entries = fs.list('/')
        return #entries > 0 and "has_entries" or "empty"
    "#,
        )
        .unwrap();
    // /dev is always present as a synthetic dir
    assert_eq!(result, "has_entries");
}

#[test]
fn test_fs_list_root_shows_mount_toplevel() {
    let dir = TempDir::new().unwrap();
    let sandbox = sandbox_with_dir(&dir, "/data", "rw");

    let result = sandbox
        .exec(
            r#"
        local entries = fs.list('/')
        -- Check that "data" mount is visible at root
        local has_data = false
        for _, e in ipairs(entries) do
            if e == "data" then has_data = true end
        end
        return has_data and "yes" or "no"
    "#,
        )
        .unwrap();
    assert_eq!(result, "yes");
}

#[test]
fn test_fs_list_virtual_parent() {
    let dir = TempDir::new().unwrap();
    let mut mounts = MountTable::new();
    mounts
        .parse_and_add(&format!("{}:/workspace/src", dir.path().display()))
        .unwrap();
    let sandbox = Sandbox::with_mounts(mounts).unwrap();

    // `/` should contain "workspace" (and possibly "dev" from synthetic dirs)
    let result = sandbox
        .exec(
            r#"
        local e = fs.list('/')
        local has_ws = false
        for _, v in ipairs(e) do if v == "workspace" then has_ws = true end end
        return has_ws and "yes" or "no"
    "#,
        )
        .unwrap();
    assert_eq!(result, "yes");

    // `/workspace` should show "src"
    let result = sandbox
        .exec("local e = fs.list('/workspace'); return table.concat(e, ',')")
        .unwrap();
    assert_eq!(result, "src");
}

#[test]
fn test_fs_exists_virtual_dirs() {
    let dir = TempDir::new().unwrap();
    let mut mounts = MountTable::new();
    mounts
        .parse_and_add(&format!("{}:/foo/bar", dir.path().display()))
        .unwrap();
    let sandbox = Sandbox::with_mounts(mounts).unwrap();

    let result = sandbox.exec("return fs.exists('/')").unwrap();
    assert_eq!(result, "true");

    let result = sandbox.exec("return fs.exists('/foo')").unwrap();
    assert_eq!(result, "true");

    let result = sandbox.exec("return fs.exists('/foo/bar')").unwrap();
    assert_eq!(result, "true");

    let result = sandbox.exec("return fs.exists('/nope')").unwrap();
    assert_eq!(result, "false");
}

#[test]
fn test_fs_list_nonexistent_errors() {
    let sandbox = Sandbox::new().unwrap();
    let result = sandbox.exec("fs.list('/nope')");
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("No such file or directory"), "got: {}", err);
}

#[test]
fn test_fs_read_directory_errors() {
    let dir = TempDir::new().unwrap();
    let sandbox = sandbox_with_dir(&dir, "/data", "rw");

    // Reading `/` (virtual dir) should give "Is a directory"
    let result = sandbox.exec("fs.read('/')");
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("Is a directory"), "got: {}", err);
}

// --- Partial read (offset/limit) tests ---

fn write_numbered_lines(dir: &TempDir, filename: &str, n: usize) {
    let content: String = (1..=n).map(|i| format!("line {}\n", i)).collect();
    fs::write(dir.path().join(filename), content).unwrap();
}

#[test]
fn test_fs_read_positional_still_works() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("file.txt"), "hello world").unwrap();

    let sandbox = sandbox_with_dir(&dir, "/data", "rw");
    let result = sandbox.exec(r#"return fs.read("/data/file.txt")"#).unwrap();
    assert_eq!(result, "hello world");
}

#[test]
fn test_fs_read_table_form_full() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("file.txt"), "hello world").unwrap();

    let sandbox = sandbox_with_dir(&dir, "/data", "rw");
    let result = sandbox
        .exec(r#"return fs.read({path="/data/file.txt"})"#)
        .unwrap();
    assert_eq!(result, "hello world");
}

#[test]
fn test_fs_read_offset_and_limit() {
    let dir = TempDir::new().unwrap();
    write_numbered_lines(&dir, "lines.txt", 10);

    let sandbox = sandbox_with_dir(&dir, "/data", "rw");
    // Read 3 lines starting at line 3
    let result = sandbox
        .exec(r#"return fs.read({path="/data/lines.txt", offset=3, limit=3})"#)
        .unwrap();
    assert_eq!(result, "line 3\nline 4\nline 5");
}

#[test]
fn test_fs_read_offset_only() {
    let dir = TempDir::new().unwrap();
    write_numbered_lines(&dir, "lines.txt", 5);

    let sandbox = sandbox_with_dir(&dir, "/data", "rw");
    // Read from line 3 to end
    let result = sandbox
        .exec(r#"return fs.read({path="/data/lines.txt", offset=3})"#)
        .unwrap();
    assert_eq!(result, "line 3\nline 4\nline 5\n");
}

#[test]
fn test_fs_read_limit_only() {
    let dir = TempDir::new().unwrap();
    write_numbered_lines(&dir, "lines.txt", 5);

    let sandbox = sandbox_with_dir(&dir, "/data", "rw");
    // Read first 2 lines
    let result = sandbox
        .exec(r#"return fs.read({path="/data/lines.txt", limit=2})"#)
        .unwrap();
    assert_eq!(result, "line 1\nline 2");
}

#[test]
fn test_fs_read_offset_beyond_eof() {
    let dir = TempDir::new().unwrap();
    write_numbered_lines(&dir, "lines.txt", 3);

    let sandbox = sandbox_with_dir(&dir, "/data", "rw");
    let result = sandbox
        .exec(r#"return fs.read({path="/data/lines.txt", offset=100})"#)
        .unwrap();
    assert_eq!(result, "");
}

#[test]
fn test_fs_read_limit_zero() {
    let dir = TempDir::new().unwrap();
    write_numbered_lines(&dir, "lines.txt", 5);

    let sandbox = sandbox_with_dir(&dir, "/data", "rw");
    let result = sandbox
        .exec(r#"return fs.read({path="/data/lines.txt", limit=0})"#)
        .unwrap();
    assert_eq!(result, "");
}

#[test]
fn test_fs_read_limit_exceeds_remaining_lines() {
    let dir = TempDir::new().unwrap();
    write_numbered_lines(&dir, "lines.txt", 5);

    let sandbox = sandbox_with_dir(&dir, "/data", "rw");
    // offset=4, limit=100 — only 2 lines left
    let result = sandbox
        .exec(r#"return fs.read({path="/data/lines.txt", offset=4, limit=100})"#)
        .unwrap();
    assert_eq!(result, "line 4\nline 5");
}

#[test]
fn test_fs_read_offset_last_line() {
    let dir = TempDir::new().unwrap();
    write_numbered_lines(&dir, "lines.txt", 5);

    let sandbox = sandbox_with_dir(&dir, "/data", "rw");
    let result = sandbox
        .exec(r#"return fs.read({path="/data/lines.txt", offset=5, limit=1})"#)
        .unwrap();
    assert_eq!(result, "line 5");
}

#[test]
fn test_fs_read_positional_with_offset_limit() {
    let dir = TempDir::new().unwrap();
    write_numbered_lines(&dir, "lines.txt", 10);

    let sandbox = sandbox_with_dir(&dir, "/data", "rw");
    // Positional form: fs.read(path, offset, limit)
    let result = sandbox
        .exec(r#"return fs.read("/data/lines.txt", 2, 3)"#)
        .unwrap();
    assert_eq!(result, "line 2\nline 3\nline 4");
}

#[test]
fn test_fs_read_invalid_utf8_suggests_binary_modes() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("blob.bin"), [0xff, 0xfe, 0xfd]).unwrap();

    let sandbox = sandbox_with_dir(&dir, "/data", "rw");
    let err = sandbox
        .exec(r#"return fs.read("/data/blob.bin")"#)
        .unwrap_err();
    assert!(
        err.message.contains("not valid UTF-8"),
        "got: {}",
        err.message
    );
    assert!(
        err.message.contains(r#"mode="binary""#) && err.message.contains(r#"mode="base64""#),
        "got: {}",
        err.message
    );
}

#[test]
fn test_fs_read_base64_mode_handles_binary_bytes() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("blob.bin"), [0, 1, 2, 255]).unwrap();

    let sandbox = sandbox_with_dir(&dir, "/data", "rw");
    let result = sandbox
        .exec(r#"return fs.read({path="/data/blob.bin", mode="base64"})"#)
        .unwrap();
    assert_eq!(result, "AAEC/w==");
}

#[test]
fn test_fs_read_binary_mode_returns_raw_luau_string() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("blob.bin"), [0, 1, 2, 255]).unwrap();

    let sandbox = sandbox_with_dir(&dir, "/data", "rw");
    let result = sandbox
        .exec(
            r#"
            local data = fs.read({path="/data/blob.bin", mode="binary"})
            fs.write("/data/copy.bin", data)
            return #data, string.byte(data, 1), string.byte(data, 4)
            "#,
        )
        .unwrap();
    assert_eq!(result, "4\t0\t255");
    assert_eq!(
        fs::read(dir.path().join("copy.bin")).unwrap(),
        [0, 1, 2, 255]
    );
}

#[test]
fn test_fs_read_base64_mode_supports_byte_slice() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("blob.bin"), [0, 1, 2, 255]).unwrap();

    let sandbox = sandbox_with_dir(&dir, "/data", "rw");
    let result = sandbox
        .exec(
            r#"return fs.read({path="/data/blob.bin", mode="base64", byte_offset=1, byte_limit=2})"#,
        )
        .unwrap();
    assert_eq!(result, "AQI=");
}

#[test]
fn test_fs_read_binary_mode_rejects_line_slice_options() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("blob.bin"), [0, 1, 2, 255]).unwrap();

    let sandbox = sandbox_with_dir(&dir, "/data", "rw");
    let err = sandbox
        .exec(r#"fs.read({path="/data/blob.bin", mode="binary", limit=1})"#)
        .unwrap_err();
    assert!(
        err.message.contains("offset/limit are line-based"),
        "got: {}",
        err.message
    );
}

// --- fs.grep tests ---

#[cfg(feature = "mod-ripgrep")]
#[test]
fn test_grep_single_file() {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("code.rs"),
        "fn main() {\n    println!(\"hello\");\n    // TODO: fix\n}\n",
    )
    .unwrap();

    let sandbox = sandbox_with_dir(&dir, "/data", "rw");
    let result = sandbox
        .exec(
            r#"
        local matches = fs.grep({pattern="TODO", path="/data/code.rs"})
        return #matches .. ":" .. matches[1].line_number .. ":" .. matches[1].match_text
    "#,
        )
        .unwrap();
    assert_eq!(result, "1:3:TODO");
}

#[cfg(feature = "mod-ripgrep")]
#[test]
fn test_grep_single_file_line_content() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("notes.txt"), "alpha\nbeta\ngamma\n").unwrap();

    let sandbox = sandbox_with_dir(&dir, "/data", "rw");
    let result = sandbox
        .exec(
            r#"
        local matches = fs.grep({pattern="beta", path="/data/notes.txt"})
        return matches[1].line
    "#,
        )
        .unwrap();
    assert_eq!(result, "beta");
}

#[cfg(feature = "mod-ripgrep")]
#[test]
fn test_grep_recursive_directory() {
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(dir.path().join("src/a.rs"), "fn main() {}\n").unwrap();
    fs::write(
        dir.path().join("src/b.rs"),
        "fn helper() {}\nfn main() {}\n",
    )
    .unwrap();
    fs::write(dir.path().join("readme.md"), "no functions here\n").unwrap();

    let sandbox = sandbox_with_dir(&dir, "/data", "rw");
    let result = sandbox
        .exec(
            r#"
        local matches = fs.grep({pattern="fn main", path="/data"})
        return tostring(#matches)
    "#,
        )
        .unwrap();
    // Should find "fn main" in both a.rs and b.rs
    assert_eq!(result, "2");
}

#[cfg(feature = "mod-ripgrep")]
#[test]
fn test_grep_glob_filter() {
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(dir.path().join("src/code.rs"), "// TODO: fix this\n").unwrap();
    fs::write(dir.path().join("src/notes.txt"), "TODO: remember\n").unwrap();

    let sandbox = sandbox_with_dir(&dir, "/data", "rw");
    let result = sandbox
        .exec(
            r#"
        local matches = fs.grep({pattern="TODO", path="/data", glob="*.rs"})
        return #matches .. ":" .. matches[1].file
    "#,
        )
        .unwrap();
    assert!(result.starts_with("1:"), "got: {}", result);
    assert!(result.contains("code.rs"), "got: {}", result);
}

#[cfg(feature = "mod-ripgrep")]
#[test]
fn test_grep_files_only() {
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(
        dir.path().join("src/a.rs"),
        "// TODO: first\n// TODO: second\n",
    )
    .unwrap();
    fs::write(dir.path().join("src/b.rs"), "// TODO: third\n").unwrap();

    let sandbox = sandbox_with_dir(&dir, "/data", "rw");
    let result = sandbox
        .exec(
            r#"
        local files = fs.grep({pattern="TODO", path="/data", files_only=true})
        table.sort(files)
        return #files .. ":" .. table.concat(files, ",")
    "#,
        )
        .unwrap();
    // Should return 2 unique file paths despite 3 total matches
    assert!(result.starts_with("2:"), "got: {}", result);
}

#[cfg(feature = "mod-ripgrep")]
#[test]
fn test_grep_max_count() {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("many.txt"),
        "line1 match\nline2 match\nline3 match\nline4 match\nline5 match\n",
    )
    .unwrap();

    let sandbox = sandbox_with_dir(&dir, "/data", "rw");
    let result = sandbox
        .exec(
            r#"
        local matches = fs.grep({pattern="match", path="/data/many.txt", max_count=3})
        return tostring(#matches)
    "#,
        )
        .unwrap();
    assert_eq!(result, "3");
}

#[cfg(feature = "mod-ripgrep")]
#[test]
fn test_grep_invalid_pattern() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("file.txt"), "content\n").unwrap();

    let sandbox = sandbox_with_dir(&dir, "/data", "rw");
    let result = sandbox.exec(r#"fs.grep({pattern="[invalid", path="/data/file.txt"})"#);
    assert!(result.is_err(), "invalid regex should error");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("invalid pattern"),
        "error should mention invalid pattern, got: {}",
        err
    );
}

#[cfg(feature = "mod-ripgrep")]
#[test]
fn test_grep_outside_mount_fails() {
    let dir = TempDir::new().unwrap();
    let sandbox = sandbox_with_dir(&dir, "/data", "rw");

    let result = sandbox.exec(r#"fs.grep({pattern="test", path="/etc/passwd"})"#);
    assert!(result.is_err(), "grep outside mount should fail");
    let err = result.unwrap_err().to_string();
    assert!(err.contains("No such file or directory"), "got: {}", err);
}

#[cfg(feature = "mod-ripgrep")]
#[test]
fn test_grep_no_matches_returns_empty_table() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("file.txt"), "nothing interesting\n").unwrap();

    let sandbox = sandbox_with_dir(&dir, "/data", "rw");
    let result = sandbox
        .exec(
            r#"
        local matches = fs.grep({pattern="ZZZZZ_NONEXISTENT", path="/data/file.txt"})
        return tostring(#matches)
    "#,
        )
        .unwrap();
    assert_eq!(result, "0");
}

#[cfg(feature = "mod-ripgrep")]
#[test]
fn test_grep_plain_mode_treats_pattern_literally() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("status.txt"), "DONE\nTODO|DONE literal\n").unwrap();

    let sandbox = sandbox_with_dir(&dir, "/data", "rw");
    let result = sandbox
        .exec(
            r#"
        local matches = fs.grep({pattern="TODO|DONE", path="/data/status.txt", mode="plain"})
        return #matches .. ":" .. matches[1].line .. ":" .. matches[1].match_text
    "#,
        )
        .unwrap();
    assert_eq!(result, "1:TODO|DONE literal:TODO|DONE");
}

#[cfg(feature = "mod-ripgrep")]
#[test]
fn test_grep_invalid_mode_fails() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("status.txt"), "TODO\n").unwrap();

    let sandbox = sandbox_with_dir(&dir, "/data", "rw");
    let result =
        sandbox.exec(r#"fs.grep({pattern="TODO", path="/data/status.txt", mode="fuzzy"})"#);
    assert!(result.is_err(), "invalid mode should error");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("unsupported mode 'fuzzy'"),
        "error should mention unsupported mode, got: {}",
        err
    );
}

#[cfg(feature = "mod-ripgrep")]
#[test]
fn test_grep_file_paths_are_virtual() {
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join("sub")).unwrap();
    fs::write(dir.path().join("sub/target.rs"), "fn hello() {}\n").unwrap();

    let sandbox = sandbox_with_dir(&dir, "/workspace", "rw");
    let result = sandbox
        .exec(
            r#"
        local matches = fs.grep({pattern="hello", path="/workspace"})
        return matches[1].file
    "#,
        )
        .unwrap();
    // File path should be virtual (sandbox path), not host filesystem path
    assert!(
        result.starts_with("/workspace/"),
        "should be virtual path, got: {}",
        result
    );
    assert!(result.contains("target.rs"), "got: {}", result);
}

#[cfg(all(feature = "mod-ripgrep", feature = "mod-fff"))]
#[test]
fn test_fs_grep_prefers_regex_provider_when_both_enabled() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("status.txt"), "DONE\nTODO|DONE literal\n").unwrap();

    let sandbox = sandbox_with_dir(&dir, "/data", "rw");
    let result = sandbox
        .exec(
            r#"
        local matches = fs.grep({pattern="TODO|DONE", path="/data/status.txt", max_count=1})
        return #matches .. ":" .. matches[1].line .. ":" .. matches[1].match_text
    "#,
        )
        .unwrap();
    assert_eq!(result, "1:DONE:DONE");
}

#[cfg(all(feature = "mod-fff", not(feature = "mod-ripgrep")))]
#[test]
fn test_fff_only_fs_grep_single_file_common_shape() {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("code.rs"),
        "fn main() {\n    // TODO: fix\n}\n",
    )
    .unwrap();

    let sandbox = sandbox_with_dir(&dir, "/data", "rw");
    let result = sandbox
        .exec(
            r#"
        local matches = fs.grep({pattern="TODO", path="/data/code.rs"})
        return #matches .. ":" .. matches[1].line_number .. ":" .. matches[1].match_text .. ":" .. tostring(matches[1].column)
    "#,
        )
        .unwrap();
    assert_eq!(result, "1:2:TODO:nil");
}

#[cfg(all(feature = "mod-fff", not(feature = "mod-ripgrep")))]
#[test]
fn test_fff_only_fs_grep_recursive_directory() {
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(dir.path().join("src/a.txt"), "needle\n").unwrap();
    fs::write(dir.path().join("src/b.txt"), "needle again\n").unwrap();
    fs::write(dir.path().join("readme.md"), "no match\n").unwrap();

    let sandbox = sandbox_with_dir(&dir, "/data", "rw");
    let result = sandbox
        .exec(
            r#"
        local matches = fs.grep({pattern="needle", path="/data"})
        return tostring(#matches)
    "#,
        )
        .unwrap();
    assert_eq!(result, "2");
}

#[cfg(all(feature = "mod-fff", not(feature = "mod-ripgrep")))]
#[test]
fn test_fff_only_fs_grep_default_regex_mode() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("status.txt"), "DONE\nTODO|DONE literal\n").unwrap();

    let sandbox = sandbox_with_dir(&dir, "/data", "rw");
    let result = sandbox
        .exec(
            r#"
        local matches = fs.grep({pattern="TODO|DONE", path="/data/status.txt", max_count=1})
        return #matches .. ":" .. matches[1].line .. ":" .. matches[1].match_text
    "#,
        )
        .unwrap();
    assert_eq!(result, "1:DONE:DONE");
}

#[cfg(all(feature = "mod-fff", not(feature = "mod-ripgrep")))]
#[test]
fn test_fff_only_fs_grep_plain_mode_treats_pattern_literally() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("status.txt"), "DONE\nTODO|DONE literal\n").unwrap();

    let sandbox = sandbox_with_dir(&dir, "/data", "rw");
    let result = sandbox
        .exec(
            r#"
        local matches = fs.grep({pattern="TODO|DONE", path="/data/status.txt", mode="plain"})
        return #matches .. ":" .. matches[1].line .. ":" .. matches[1].match_text
    "#,
        )
        .unwrap();
    assert_eq!(result, "1:TODO|DONE literal:TODO|DONE");
}

#[cfg(all(feature = "mod-fff", not(feature = "mod-ripgrep")))]
#[test]
fn test_fff_only_fs_grep_invalid_pattern() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("file.txt"), "content\n").unwrap();

    let sandbox = sandbox_with_dir(&dir, "/data", "rw");
    let result = sandbox.exec(r#"fs.grep({pattern="[invalid", path="/data/file.txt"})"#);
    assert!(result.is_err(), "invalid regex should error");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("invalid pattern"),
        "error should mention invalid pattern, got: {}",
        err
    );
}

#[cfg(all(feature = "mod-fff", not(feature = "mod-ripgrep")))]
#[test]
fn test_fff_only_fs_grep_invalid_mode_fails() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("status.txt"), "TODO\n").unwrap();

    let sandbox = sandbox_with_dir(&dir, "/data", "rw");
    let result =
        sandbox.exec(r#"fs.grep({pattern="TODO", path="/data/status.txt", mode="fuzzy"})"#);
    assert!(result.is_err(), "invalid mode should error");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("unsupported mode 'fuzzy'"),
        "error should mention unsupported mode, got: {}",
        err
    );
}

#[cfg(all(feature = "mod-fff", not(feature = "mod-ripgrep")))]
#[test]
fn test_fff_only_fs_grep_glob_and_max_count() {
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(dir.path().join("src/code.rs"), "TODO: one\nTODO: two\n").unwrap();
    fs::write(dir.path().join("src/notes.txt"), "TODO: note\n").unwrap();

    let sandbox = sandbox_with_dir(&dir, "/data", "rw");
    let result = sandbox
        .exec(
            r#"
        local matches = fs.grep({pattern="TODO", path="/data", glob="*.rs", max_count=1})
        return #matches .. ":" .. matches[1].file .. ":" .. matches[1].line
    "#,
        )
        .unwrap();
    assert_eq!(result, "1:/data/src/code.rs:TODO: one");
}

#[cfg(all(feature = "mod-fff", not(feature = "mod-ripgrep")))]
#[test]
fn test_fff_only_fs_grep_files_only() {
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(dir.path().join("src/a.txt"), "TODO: one\nTODO: two\n").unwrap();
    fs::write(dir.path().join("src/b.txt"), "TODO: three\n").unwrap();

    let sandbox = sandbox_with_dir(&dir, "/data", "rw");
    let result = sandbox
        .exec(
            r#"
        local files = fs.grep({pattern="TODO", path="/data", files_only=true})
        table.sort(files)
        return #files .. ":" .. table.concat(files, ",")
    "#,
        )
        .unwrap();
    assert_eq!(result, "2:/data/src/a.txt,/data/src/b.txt");
}

#[cfg(all(feature = "mod-fff", not(feature = "mod-ripgrep")))]
#[test]
fn test_fff_only_fs_grep_filters_invalid_utf8_per_matched_line() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("nul.txt"), b"TODO\0\n".as_slice()).unwrap();
    fs::write(
        dir.path().join("mixed.bin"),
        b"TODO invalid \xff\nTODO valid\n".as_slice(),
    )
    .unwrap();
    fs::write(
        dir.path().join("invalid_only.bin"),
        b"TODO invalid \xff\n".as_slice(),
    )
    .unwrap();

    let sandbox = sandbox_with_dir(&dir, "/data", "rw");
    let result = sandbox
        .exec(
            r#"
        local nul = fs.grep({pattern="TODO", path="/data/nul.txt"})
        local mixed = fs.grep({pattern="TODO", path="/data/mixed.bin"})
        local capped = fs.grep({pattern="TODO", path="/data/mixed.bin", max_count=1})
        local mixed_files = fs.grep({pattern="TODO", path="/data/mixed.bin", files_only=true})
        local invalid = fs.grep({pattern="TODO", path="/data/invalid_only.bin"})
        local invalid_files = fs.grep({pattern="TODO", path="/data/invalid_only.bin", files_only=true})
        return #nul
            .. ":" .. string.byte(nul[1].line, 5)
            .. ":" .. #mixed
            .. ":" .. mixed[1].line_number
            .. ":" .. mixed[1].line
            .. ":" .. #capped
            .. ":" .. capped[1].line_number
            .. ":" .. #mixed_files
            .. ":" .. #invalid
            .. ":" .. #invalid_files
    "#,
        )
        .unwrap();
    assert_eq!(result, "1:0:1:2:TODO valid:1:2:1:0:0");
}

#[cfg(all(feature = "mod-fff", not(feature = "mod-ripgrep")))]
#[test]
fn test_fff_only_fs_grep_virtual_path_and_mount_denial_fail() {
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(dir.path().join("src/a.txt"), "needle\n").unwrap();

    let mut mounts = MountTable::new();
    mounts
        .parse_and_add(&format!("{}:/workspace/src:rw", dir.path().display()))
        .unwrap();
    let sandbox = Sandbox::with_mounts(mounts).unwrap();

    let result = sandbox.exec(r#"fs.grep({pattern="needle", path="/workspace"})"#);
    assert!(result.is_err(), "virtual parent path should fail");
    let err = result.unwrap_err().to_string();
    assert!(err.contains("No such file or directory"), "got: {}", err);

    let result = sandbox.exec(r#"fs.grep({pattern="needle", path="/etc/passwd"})"#);
    assert!(result.is_err(), "outside mount should fail");
    let err = result.unwrap_err().to_string();
    assert!(err.contains("No such file or directory"), "got: {}", err);
}

#[cfg(all(feature = "mod-fff", not(feature = "mod-ripgrep")))]
#[test]
fn test_fff_only_fs_help_includes_grep() {
    let sandbox = Sandbox::new().unwrap();
    let result = sandbox.exec("fs.help()").unwrap();
    assert!(result.contains("fs.grep"), "got: {}", result);
    assert!(result.contains("regex"), "got: {}", result);
    assert!(result.contains("plain"), "got: {}", result);
}

// --- fs.tree tests ---

/// Helper to create a directory structure for tree tests.
fn create_tree_fixture(dir: &TempDir) {
    // dir1/
    //   file1.rs
    //   file2.txt
    // dir2/
    //   nested/
    //     deep.rs
    //   file3.rs
    // root.txt
    fs::create_dir_all(dir.path().join("dir1")).unwrap();
    fs::create_dir_all(dir.path().join("dir2/nested")).unwrap();
    fs::write(dir.path().join("dir1/file1.rs"), "fn main() {}").unwrap();
    fs::write(dir.path().join("dir1/file2.txt"), "hello").unwrap();
    fs::write(dir.path().join("dir2/nested/deep.rs"), "mod deep;").unwrap();
    fs::write(dir.path().join("dir2/file3.rs"), "use std;").unwrap();
    fs::write(dir.path().join("root.txt"), "root").unwrap();
}

#[test]
#[cfg(feature = "mod-ripgrep")]
fn test_fs_tree_basic_output() {
    let dir = TempDir::new().unwrap();
    create_tree_fixture(&dir);

    let sandbox = sandbox_with_dir(&dir, "/workspace", "rw");
    let result = sandbox
        .exec(r#"return fs.tree({path="/workspace"})"#)
        .unwrap();

    // Should start with root name
    assert!(result.starts_with("workspace/\n"), "got: {}", result);
    // Should contain directory entries with trailing /
    assert!(result.contains("dir1/"), "missing dir1/: {}", result);
    assert!(result.contains("dir2/"), "missing dir2/: {}", result);
    // Should contain file entries
    assert!(result.contains("file1.rs"), "missing file1.rs: {}", result);
    assert!(result.contains("root.txt"), "missing root.txt: {}", result);
    // Should contain tree connectors
    assert!(
        result.contains("\u{251c}\u{2500}\u{2500} ")
            || result.contains("\u{2514}\u{2500}\u{2500} "),
        "missing tree connectors: {}",
        result
    );
    // Should end with summary line
    assert!(
        result.contains("directories,"),
        "missing summary: {}",
        result
    );
    assert!(result.contains("files"), "missing summary: {}", result);
}

#[test]
#[cfg(feature = "mod-ripgrep")]
fn test_fs_tree_summary_counts() {
    let dir = TempDir::new().unwrap();
    create_tree_fixture(&dir);

    let sandbox = sandbox_with_dir(&dir, "/workspace", "rw");
    let result = sandbox
        .exec(r#"return fs.tree({path="/workspace", depth=10})"#)
        .unwrap();

    // 3 directories (dir1, dir2, dir2/nested), 5 files
    assert!(
        result.contains("3 directories, 5 files"),
        "expected '3 directories, 5 files', got: {}",
        result
    );
}

#[test]
#[cfg(feature = "mod-ripgrep")]
fn test_fs_tree_default_depth_limit() {
    let dir = TempDir::new().unwrap();
    // Create a deeply nested structure: a/b/c/d/e.txt
    fs::create_dir_all(dir.path().join("a/b/c/d")).unwrap();
    fs::write(dir.path().join("a/b/c/d/e.txt"), "deep").unwrap();
    fs::write(dir.path().join("a/b/shallow.txt"), "shallow").unwrap();

    let sandbox = sandbox_with_dir(&dir, "/workspace", "rw");
    // Default depth is 3 — should show a/, a/b/, a/b/c/, a/b/shallow.txt
    // but NOT a/b/c/d/ or a/b/c/d/e.txt
    let result = sandbox
        .exec(r#"return fs.tree({path="/workspace"})"#)
        .unwrap();

    assert!(
        result.contains("shallow.txt"),
        "missing shallow.txt at depth 2: {}",
        result
    );
    assert!(result.contains("c/"), "missing c/ at depth 3: {}", result);
    assert!(
        !result.contains("e.txt"),
        "e.txt should be hidden at depth 4: {}",
        result
    );
    assert!(
        !result.contains("d/"),
        "d/ should be hidden at depth 4: {}",
        result
    );
}

#[test]
#[cfg(feature = "mod-ripgrep")]
fn test_fs_tree_custom_depth() {
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join("a/b/c/d")).unwrap();
    fs::write(dir.path().join("a/b/c/d/e.txt"), "deep").unwrap();

    let sandbox = sandbox_with_dir(&dir, "/workspace", "rw");
    // depth=5 should show everything
    let result = sandbox
        .exec(r#"return fs.tree({path="/workspace", depth=5})"#)
        .unwrap();
    assert!(
        result.contains("e.txt"),
        "e.txt should be visible at depth 5: {}",
        result
    );

    // depth=1 should only show top-level
    let result = sandbox
        .exec(r#"return fs.tree({path="/workspace", depth=1})"#)
        .unwrap();
    assert!(result.contains("a/"), "missing a/ at depth 1: {}", result);
    assert!(
        !result.contains("b/"),
        "b/ should be hidden at depth 1: {}",
        result
    );
}

#[test]
#[cfg(feature = "mod-ripgrep")]
fn test_fs_tree_dirs_only() {
    let dir = TempDir::new().unwrap();
    create_tree_fixture(&dir);

    let sandbox = sandbox_with_dir(&dir, "/workspace", "rw");
    let result = sandbox
        .exec(r#"return fs.tree({path="/workspace", dirs_only=true, depth=10})"#)
        .unwrap();

    // Should contain directories
    assert!(result.contains("dir1/"), "missing dir1/: {}", result);
    assert!(result.contains("dir2/"), "missing dir2/: {}", result);
    assert!(result.contains("nested/"), "missing nested/: {}", result);
    // Should NOT contain files
    assert!(
        !result.contains("file1.rs"),
        "file1.rs should be hidden: {}",
        result
    );
    assert!(
        !result.contains("root.txt"),
        "root.txt should be hidden: {}",
        result
    );
    // Summary should show 0 files
    assert!(result.contains("3 directories, 0 files"), "got: {}", result);
}

#[test]
#[cfg(feature = "mod-ripgrep")]
fn test_fs_tree_glob_filter() {
    let dir = TempDir::new().unwrap();
    create_tree_fixture(&dir);

    let sandbox = sandbox_with_dir(&dir, "/workspace", "rw");
    let result = sandbox
        .exec(r#"return fs.tree({path="/workspace", glob="*.rs", depth=10})"#)
        .unwrap();

    // Should show .rs files
    assert!(result.contains("file1.rs"), "missing file1.rs: {}", result);
    assert!(result.contains("file3.rs"), "missing file3.rs: {}", result);
    assert!(result.contains("deep.rs"), "missing deep.rs: {}", result);
    // Should NOT show non-rs files
    assert!(
        !result.contains("file2.txt"),
        "file2.txt should be hidden: {}",
        result
    );
    assert!(
        !result.contains("root.txt"),
        "root.txt should be hidden: {}",
        result
    );
    // Should show ancestor directories of .rs files
    assert!(result.contains("dir1/"), "missing dir1/: {}", result);
    assert!(result.contains("dir2/"), "missing dir2/: {}", result);
    // 3 .rs files
    assert!(result.contains("3 files"), "got: {}", result);
}

#[test]
#[cfg(feature = "mod-ripgrep")]
fn test_fs_tree_empty_directory() {
    let dir = TempDir::new().unwrap();
    // Just an empty directory — no files

    let sandbox = sandbox_with_dir(&dir, "/workspace", "rw");
    let result = sandbox
        .exec(r#"return fs.tree({path="/workspace"})"#)
        .unwrap();

    assert!(result.starts_with("workspace/\n"), "got: {}", result);
    assert!(result.contains("0 directories, 0 files"), "got: {}", result);
}

#[test]
#[cfg(feature = "mod-ripgrep")]
fn test_fs_tree_single_file() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("only.txt"), "content").unwrap();

    let sandbox = sandbox_with_dir(&dir, "/workspace", "rw");
    // Point tree at a file, not a directory
    let result = sandbox
        .exec(r#"return fs.tree({path="/workspace/only.txt"})"#)
        .unwrap();

    assert!(result.contains("only.txt"), "got: {}", result);
    assert!(result.contains("0 directories, 1 file"), "got: {}", result);
}

#[test]
#[cfg(feature = "mod-ripgrep")]
fn test_fs_tree_nonexistent_path_errors() {
    let dir = TempDir::new().unwrap();
    let sandbox = sandbox_with_dir(&dir, "/workspace", "rw");

    let result = sandbox.exec(r#"return fs.tree({path="/workspace/nope"})"#);
    assert!(result.is_err(), "tree on nonexistent path should fail");
}

#[test]
#[cfg(feature = "mod-ripgrep")]
fn test_fs_tree_connectors_correct() {
    let dir = TempDir::new().unwrap();
    // Simple structure: two files at root level
    fs::write(dir.path().join("aaa.txt"), "").unwrap();
    fs::write(dir.path().join("zzz.txt"), "").unwrap();

    let sandbox = sandbox_with_dir(&dir, "/data", "rw");
    let result = sandbox.exec(r#"return fs.tree({path="/data"})"#).unwrap();

    let lines: Vec<&str> = result.lines().collect();
    // Line 0: "data/"
    assert_eq!(lines[0], "data/");
    // Line 1: non-last item uses ├──
    assert!(
        lines[1].contains("\u{251c}\u{2500}\u{2500} aaa.txt"),
        "first item should use \u{251c}\u{2500}\u{2500}: got '{}'",
        lines[1]
    );
    // Line 2: last item uses └──
    assert!(
        lines[2].contains("\u{2514}\u{2500}\u{2500} zzz.txt"),
        "last item should use \u{2514}\u{2500}\u{2500}: got '{}'",
        lines[2]
    );
}

#[cfg(feature = "mod-ripgrep")]
#[test]
fn test_fs_tree_root_shows_all_mounts() {
    let dir1 = TempDir::new().unwrap();
    let dir2 = TempDir::new().unwrap();
    fs::write(dir1.path().join("a.txt"), "hello").unwrap();
    fs::write(dir2.path().join("b.txt"), "world").unwrap();

    let mut mounts = MountTable::new();
    mounts
        .parse_and_add(&format!("{}:/workspace:rw", dir1.path().display()))
        .unwrap();
    mounts
        .parse_and_add(&format!("{}:/attachments:ro", dir2.path().display()))
        .unwrap();
    let sandbox = Sandbox::with_mounts(mounts).unwrap();

    let result = sandbox
        .exec(r#"return fs.tree({path="/", depth=2})"#)
        .unwrap();

    // Root should show both mounts and synthetic dirs (dev, etc, proc, tmp, home)
    assert!(
        result.contains("workspace/"),
        "should show workspace mount: {}",
        result
    );
    assert!(
        result.contains("attachments/"),
        "should show attachments mount: {}",
        result
    );
    assert!(
        result.contains("a.txt"),
        "should show files inside workspace: {}",
        result
    );
    assert!(
        result.contains("b.txt"),
        "should show files inside attachments: {}",
        result
    );
    // Virtual dirs like tmp, home should appear
    assert!(
        result.contains("tmp/"),
        "should show tmp virtual dir: {}",
        result
    );
}

#[cfg(feature = "mod-ripgrep")]
#[test]
fn test_fs_tree_root_depth_1_shows_top_level_only() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("file.txt"), "data").unwrap();

    let sandbox = sandbox_with_dir(&dir, "/workspace", "rw");

    let result = sandbox
        .exec(r#"return fs.tree({path="/", depth=1})"#)
        .unwrap();

    // Should show top-level entries but NOT files inside them
    assert!(
        result.contains("workspace/"),
        "should show workspace: {}",
        result
    );
    assert!(
        !result.contains("file.txt"),
        "depth=1 should not show files inside mounts: {}",
        result
    );
}

#[cfg(feature = "mod-ripgrep")]
#[test]
fn test_fs_tree_virtual_dirs_shown() {
    // Even with no mounts at a path, virtual dirs like /dev, /proc should appear
    let dir = TempDir::new().unwrap();
    let sandbox = sandbox_with_dir(&dir, "/workspace", "rw");

    let result = sandbox
        .exec(r#"return fs.tree({path="/", depth=1})"#)
        .unwrap();

    // Synthetic dirs should be listed
    assert!(
        result.contains("dev/"),
        "should show /dev synthetic dir: {}",
        result
    );
    assert!(
        result.contains("proc/"),
        "should show /proc synthetic dir: {}",
        result
    );
    assert!(
        result.contains("etc/"),
        "should show /etc synthetic dir: {}",
        result
    );
}

#[cfg(feature = "mod-ripgrep")]
#[test]
fn test_shell_tree_command() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("hello.txt"), "world").unwrap();
    fs::create_dir(dir.path().join("subdir")).unwrap();
    fs::write(dir.path().join("subdir/inner.txt"), "data").unwrap();

    let sandbox = sandbox_with_dir(&dir, "/workspace", "rw");
    let shrt = include_str!("../../runtime/shrt.luau");
    sandbox.setup_shell_runtime(shrt).unwrap();

    // Transpile bash -> Luau, then execute
    let luau = cpsl_core::sh_transpile::transpile_sh("tree /workspace").unwrap();
    let result = sandbox.exec(&luau.luau_source).unwrap();

    assert!(
        result.contains("workspace"),
        "should show root name: {}",
        result
    );
    assert!(
        result.contains("hello.txt"),
        "should show files: {}",
        result
    );
    assert!(result.contains("subdir/"), "should show dirs: {}", result);
    assert!(
        result.contains("inner.txt"),
        "should show nested files: {}",
        result
    );
}

#[cfg(feature = "mod-ripgrep")]
#[test]
fn test_shell_tree_with_depth_flag() {
    let dir = TempDir::new().unwrap();
    fs::create_dir(dir.path().join("a")).unwrap();
    fs::write(dir.path().join("a/deep.txt"), "data").unwrap();

    let sandbox = sandbox_with_dir(&dir, "/workspace", "rw");
    let shrt = include_str!("../../runtime/shrt.luau");
    sandbox.setup_shell_runtime(shrt).unwrap();

    let luau = cpsl_core::sh_transpile::transpile_sh("tree /workspace -L 1").unwrap();
    let result = sandbox.exec(&luau.luau_source).unwrap();

    assert!(result.contains("a/"), "should show dir: {}", result);
    assert!(
        !result.contains("deep.txt"),
        "depth=1 should hide nested file: {}",
        result
    );
}

#[cfg(feature = "mod-ripgrep")]
#[test]
fn test_shell_tree_bare_shows_cwd() {
    // Bare `tree` (no args) should show cwd ("/"), not "/."
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("file.txt"), "data").unwrap();

    let sandbox = sandbox_with_dir(&dir, "/workspace", "rw");
    let shrt = include_str!("../../runtime/shrt.luau");
    sandbox.setup_shell_runtime(shrt).unwrap();

    let luau = cpsl_core::sh_transpile::transpile_sh("tree").unwrap();
    let result = sandbox.exec(&luau.luau_source).unwrap();

    // Should show root "/" with all mounts, not "/."
    assert!(
        result.starts_with("/\n"),
        "bare tree should show / as root, got: {}",
        result
    );
    assert!(
        result.contains("workspace/"),
        "should show workspace mount: {}",
        result
    );
    assert!(!result.contains("/."), "should not contain /.: {}", result);
}
