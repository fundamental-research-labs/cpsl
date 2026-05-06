#![cfg(feature = "mod-fs")]

//! Comprehensive FS test suite covering every operation against a realistic mount layout:
//!   /           → ephemeral root (rw)
//!   /workspace  → workspace dir (rw)
//!   /attachments → attachments dir (ro)
//!   /artifacts  → artifacts dir (rw)
//!
//! IMPORTANT: TempDirs must be bound to `_name` (not bare `_`) to prevent
//! premature drop. Bare `_` drops immediately in Rust destructuring.

use cpsl_core::{MountTable, Sandbox};
use std::fs;
use tempfile::TempDir;

/// Build a sandbox with the standard 4-mount layout.
/// Returns (sandbox, root_dir, workspace_dir, attachments_dir, artifacts_dir).
fn standard_sandbox() -> (Sandbox, TempDir, TempDir, TempDir, TempDir) {
    let root = TempDir::new().unwrap();
    let workspace = TempDir::new().unwrap();
    let attachments = TempDir::new().unwrap();
    let artifacts = TempDir::new().unwrap();

    let mut mounts = MountTable::new();
    mounts
        .parse_and_add(&format!("{}:/", root.path().display()))
        .unwrap();
    mounts
        .parse_and_add(&format!("{}:/workspace", workspace.path().display()))
        .unwrap();
    mounts
        .parse_and_add(&format!("{}:/attachments:ro", attachments.path().display()))
        .unwrap();
    mounts
        .parse_and_add(&format!("{}:/artifacts", artifacts.path().display()))
        .unwrap();

    let sandbox = Sandbox::with_mounts(mounts).unwrap();
    (sandbox, root, workspace, attachments, artifacts)
}

// =============================================================================
// Basic CRUD
// =============================================================================

#[test]
fn write_file_read_back() {
    let (sb, _root, _ws, _att, _art) = standard_sandbox();
    sb.exec("fs.write('/workspace/hello.txt', 'world')")
        .unwrap();
    let result = sb.exec("return fs.read('/workspace/hello.txt')").unwrap();
    assert_eq!(result, "world");
}

#[test]
fn write_file_shows_in_list() {
    let (sb, _root, _ws, _att, _art) = standard_sandbox();
    sb.exec("fs.write('/workspace/file.txt', 'data')").unwrap();
    let result = sb
        .exec(
            r#"
        local entries = fs.list('/workspace')
        table.sort(entries)
        return table.concat(entries, ',')
    "#,
        )
        .unwrap();
    assert!(result.contains("file.txt"), "got: {}", result);
}

#[test]
fn mkdir_then_list() {
    let (sb, _root, _ws, _att, _art) = standard_sandbox();
    sb.exec("fs.mkdir('/workspace/subdir')").unwrap();
    let result = sb
        .exec("local e = fs.list('/workspace'); return table.concat(e, ',')")
        .unwrap();
    assert_eq!(result, "subdir");
}

#[test]
fn mkdir_under_root_then_list() {
    let (sb, _root, _ws, _att, _art) = standard_sandbox();
    // This is the original bug: mkdir under root mount, then list
    sb.exec("fs.mkdir('/test')").unwrap();
    let entries = sb.exec("local e = fs.list('/test'); return #e").unwrap();
    assert_eq!(entries, "0", "newly created dir should be empty");
}

#[test]
fn deep_mkdir_then_write_read() {
    let (sb, _root, _ws, _att, _art) = standard_sandbox();
    sb.exec("fs.mkdir('/workspace/a/b/c')").unwrap();
    sb.exec("fs.write('/workspace/a/b/c/file.txt', 'deep')")
        .unwrap();
    let result = sb
        .exec("return fs.read('/workspace/a/b/c/file.txt')")
        .unwrap();
    assert_eq!(result, "deep");
}

#[test]
fn deep_mkdir_under_root_then_write_read() {
    let (sb, _root, _ws, _att, _art) = standard_sandbox();
    sb.exec("fs.mkdir('/tmp/a/b')").unwrap();
    sb.exec("fs.write('/tmp/a/b/file.txt', 'hello')").unwrap();
    let result = sb.exec("return fs.read('/tmp/a/b/file.txt')").unwrap();
    assert_eq!(result, "hello");
}

#[test]
fn exists_file() {
    let (sb, _root, _ws, _att, _art) = standard_sandbox();
    sb.exec("fs.write('/workspace/f.txt', '')").unwrap();
    assert_eq!(
        sb.exec("return fs.exists('/workspace/f.txt')").unwrap(),
        "true"
    );
}

#[test]
fn exists_directory() {
    let (sb, _root, _ws, _att, _art) = standard_sandbox();
    sb.exec("fs.mkdir('/workspace/dir')").unwrap();
    assert_eq!(
        sb.exec("return fs.exists('/workspace/dir')").unwrap(),
        "true"
    );
}

#[test]
fn exists_virtual_dir() {
    let (sb, _root, _ws, _att, _art) = standard_sandbox();
    assert_eq!(sb.exec("return fs.exists('/')").unwrap(), "true");
}

#[test]
fn exists_nonexistent() {
    let (sb, _root, _ws, _att, _art) = standard_sandbox();
    assert_eq!(
        sb.exec("return fs.exists('/workspace/nope')").unwrap(),
        "false"
    );
}

// =============================================================================
// Read-only enforcement (/attachments)
// =============================================================================

#[test]
fn write_to_attachments_fails() {
    let (sb, _root, _ws, _att, _art) = standard_sandbox();
    let result = sb.exec("fs.write('/attachments/hack.txt', 'data')");
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Read-only file system"));
}

#[test]
fn mkdir_under_attachments_fails() {
    let (sb, _root, _ws, _att, _art) = standard_sandbox();
    let result = sb.exec("fs.mkdir('/attachments/subdir')");
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Read-only file system"));
}

#[test]
fn write_under_synthetic_dir_fails_even_with_writable_root() {
    let (sb, _root, _ws, _att, _art) = standard_sandbox();
    let result = sb.exec("fs.write('/proc/newfile', 'data')");
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("/proc/newfile: Read-only file system"),
        "expected synthetic dir read-only error, got: {}",
        err
    );
}

#[test]
fn mkdir_under_synthetic_dir_fails_even_with_writable_root() {
    let (sb, _root, _ws, _att, _art) = standard_sandbox();
    let result = sb.exec("fs.mkdir('/proc/newdir')");
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("/proc/newdir: Read-only file system"),
        "expected synthetic dir read-only error, got: {}",
        err
    );
}

#[test]
fn rename_into_attachments_fails() {
    let (sb, _root, _ws, _att, _art) = standard_sandbox();
    sb.exec("fs.write('/artifacts/file.txt', 'data')").unwrap();
    let result = sb.exec("fs.rename('/artifacts/file.txt', '/attachments/file.txt')");
    assert!(result.is_err());
}

#[test]
fn rename_out_of_attachments_fails() {
    let (sb, _root, _ws, att, _art) = standard_sandbox();
    fs::write(att.path().join("file.txt"), "data").unwrap();
    let result = sb.exec("fs.rename('/attachments/file.txt', '/artifacts/file.txt')");
    assert!(result.is_err());
}

#[test]
fn remove_from_attachments_fails() {
    let (sb, _root, _ws, att, _art) = standard_sandbox();
    fs::write(att.path().join("file.txt"), "data").unwrap();
    let result = sb.exec("fs.remove('/attachments/file.txt')");
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Read-only file system"));
}

#[test]
fn read_from_attachments_works() {
    let (sb, _root, _ws, att, _art) = standard_sandbox();
    fs::write(att.path().join("doc.txt"), "attached content").unwrap();
    let result = sb.exec("return fs.read('/attachments/doc.txt')").unwrap();
    assert_eq!(result, "attached content");
}

#[test]
fn list_attachments_works() {
    let (sb, _root, _ws, att, _art) = standard_sandbox();
    fs::write(att.path().join("a.txt"), "").unwrap();
    fs::write(att.path().join("b.txt"), "").unwrap();
    let result = sb
        .exec("local e = fs.list('/attachments'); table.sort(e); return table.concat(e, ',')")
        .unwrap();
    assert_eq!(result, "a.txt,b.txt");
}

// =============================================================================
// Root mount behavior
// =============================================================================

#[test]
fn write_to_tmp_goes_to_ephemeral_root() {
    let (sb, root, _ws, _att, _art) = standard_sandbox();
    sb.exec("fs.mkdir('/tmp')").unwrap();
    sb.exec("fs.write('/tmp/file.txt', 'ephemeral')").unwrap();
    let content = fs::read_to_string(root.path().join("tmp/file.txt")).unwrap();
    assert_eq!(content, "ephemeral");
}

#[test]
fn mkdir_home_user_goes_to_ephemeral_root() {
    let (sb, root, _ws, _att, _art) = standard_sandbox();
    sb.exec("fs.mkdir('/home/user')").unwrap();
    assert!(root.path().join("home/user").is_dir());
}

#[test]
fn list_tmp_after_writing_file() {
    let (sb, _root, _ws, _att, _art) = standard_sandbox();
    sb.exec("fs.mkdir('/tmp')").unwrap();
    sb.exec("fs.write('/tmp/a.txt', '')").unwrap();
    let result = sb
        .exec("local e = fs.list('/tmp'); return table.concat(e, ',')")
        .unwrap();
    assert_eq!(result, "a.txt");
}

#[test]
fn workspace_mount_wins_over_root() {
    let (sb, _root, ws, _att, _art) = standard_sandbox();
    sb.exec("fs.write('/workspace/test.txt', 'from sandbox')")
        .unwrap();
    let content = fs::read_to_string(ws.path().join("test.txt")).unwrap();
    assert_eq!(content, "from sandbox");
}

// =============================================================================
// Mount root protection
// =============================================================================

#[test]
fn rename_workspace_root_fails() {
    let (sb, _root, _ws, _att, _art) = standard_sandbox();
    let result = sb.exec("fs.rename('/workspace', '/workspace2')");
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("is a system directory"));
}

#[test]
fn rename_attachments_root_fails() {
    let (sb, _root, _ws, _att, _art) = standard_sandbox();
    let result = sb.exec("fs.rename('/attachments', '/attachments2')");
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("is a system directory"));
}

#[test]
fn remove_workspace_root_fails() {
    let (sb, _root, _ws, _att, _art) = standard_sandbox();
    let result = sb.exec("fs.remove('/workspace')");
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("is a system directory"));
}

#[test]
fn remove_attachments_root_fails() {
    let (sb, _root, _ws, _att, _art) = standard_sandbox();
    let result = sb.exec("fs.remove('/attachments')");
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("is a system directory"));
}

#[test]
fn rename_within_workspace_works() {
    let (sb, _root, _ws, _att, _art) = standard_sandbox();
    sb.exec("fs.write('/workspace/old.txt', 'content')")
        .unwrap();
    sb.exec("fs.rename('/workspace/old.txt', '/workspace/new.txt')")
        .unwrap();
    let result = sb.exec("return fs.read('/workspace/new.txt')").unwrap();
    assert_eq!(result, "content");
    assert_eq!(
        sb.exec("return fs.exists('/workspace/old.txt')").unwrap(),
        "false"
    );
}

// =============================================================================
// Cross-mount operations
// =============================================================================

#[test]
fn rename_across_mounts_works() {
    let (sb, _root, _ws, _att, _art) = standard_sandbox();
    sb.exec("fs.write('/workspace/a.txt', 'data')").unwrap();
    sb.exec("fs.rename('/workspace/a.txt', '/artifacts/a.txt')")
        .unwrap();
    let result = sb.exec("return fs.read('/artifacts/a.txt')").unwrap();
    assert_eq!(result, "data");
    assert_eq!(
        sb.exec("return fs.exists('/workspace/a.txt')").unwrap(),
        "false"
    );
}

#[test]
fn rename_within_same_mount_works() {
    let (sb, _root, _ws, _att, _art) = standard_sandbox();
    sb.exec("fs.write('/artifacts/old.txt', 'data')").unwrap();
    sb.exec("fs.rename('/artifacts/old.txt', '/artifacts/new.txt')")
        .unwrap();
    let result = sb.exec("return fs.read('/artifacts/new.txt')").unwrap();
    assert_eq!(result, "data");
}

// =============================================================================
// isdir / isfile
// =============================================================================

#[test]
fn isdir_root() {
    let (sb, _root, _ws, _att, _art) = standard_sandbox();
    assert_eq!(sb.exec("return fs.isdir('/')").unwrap(), "true");
}

#[test]
fn isdir_virtual_dir() {
    let dir = TempDir::new().unwrap();
    let mut mounts = MountTable::new();
    mounts
        .parse_and_add(&format!("{}:/foo/bar", dir.path().display()))
        .unwrap();
    let sb = Sandbox::with_mounts(mounts).unwrap();
    assert_eq!(sb.exec("return fs.isdir('/foo')").unwrap(), "true");
}

#[test]
fn isdir_real_dir() {
    let (sb, _root, _ws, _att, _art) = standard_sandbox();
    sb.exec("fs.mkdir('/workspace/mydir')").unwrap();
    assert_eq!(
        sb.exec("return fs.isdir('/workspace/mydir')").unwrap(),
        "true"
    );
}

#[test]
fn isdir_on_file() {
    let (sb, _root, _ws, _att, _art) = standard_sandbox();
    sb.exec("fs.write('/workspace/file.txt', '')").unwrap();
    assert_eq!(
        sb.exec("return fs.isdir('/workspace/file.txt')").unwrap(),
        "false"
    );
}

#[test]
fn isfile_on_file() {
    let (sb, _root, _ws, _att, _art) = standard_sandbox();
    sb.exec("fs.write('/workspace/file.txt', '')").unwrap();
    assert_eq!(
        sb.exec("return fs.isfile('/workspace/file.txt')").unwrap(),
        "true"
    );
}

#[test]
fn isfile_on_dir() {
    let (sb, _root, _ws, _att, _art) = standard_sandbox();
    sb.exec("fs.mkdir('/workspace/dir')").unwrap();
    assert_eq!(
        sb.exec("return fs.isfile('/workspace/dir')").unwrap(),
        "false"
    );
}

#[test]
fn isfile_on_virtual_dir() {
    let (sb, _root, _ws, _att, _art) = standard_sandbox();
    assert_eq!(sb.exec("return fs.isfile('/')").unwrap(), "false");
}

#[test]
fn isdir_on_mount_root() {
    let (sb, _root, _ws, _att, _art) = standard_sandbox();
    assert_eq!(sb.exec("return fs.isdir('/workspace')").unwrap(), "true");
    assert_eq!(sb.exec("return fs.isdir('/attachments')").unwrap(), "true");
    assert_eq!(sb.exec("return fs.isdir('/artifacts')").unwrap(), "true");
}

#[test]
fn isfile_on_nonexistent() {
    let (sb, _root, _ws, _att, _art) = standard_sandbox();
    assert_eq!(
        sb.exec("return fs.isfile('/workspace/nope')").unwrap(),
        "false"
    );
}

#[test]
fn isdir_on_nonexistent() {
    let (sb, _root, _ws, _att, _art) = standard_sandbox();
    assert_eq!(
        sb.exec("return fs.isdir('/workspace/nope')").unwrap(),
        "false"
    );
}

// =============================================================================
// Path traversal
// =============================================================================

#[test]
fn path_traversal_read() {
    let (sb, _root, _ws, _att, _art) = standard_sandbox();
    let result = sb.exec("return fs.read('/workspace/../../../etc/passwd')");
    assert!(result.is_err());
}

#[test]
fn path_traversal_write() {
    let (sb, _root, _ws, _att, _art) = standard_sandbox();
    let result = sb.exec("fs.write('/workspace/../../../etc/evil', 'hack')");
    assert!(result.is_err());
}

#[test]
fn path_traversal_rename() {
    let (sb, _root, _ws, _att, _art) = standard_sandbox();
    sb.exec("fs.write('/workspace/file.txt', '')").unwrap();
    let result = sb.exec("fs.rename('/workspace/file.txt', '/workspace/../../evil')");
    assert!(result.is_err());
}

#[test]
fn path_traversal_remove() {
    let (sb, _root, _ws, _att, _art) = standard_sandbox();
    let result = sb.exec("fs.remove('/workspace/../../../etc/passwd')");
    assert!(result.is_err());
}

// =============================================================================
// Edge cases
// =============================================================================

#[test]
fn trailing_slashes_in_paths() {
    let (sb, _root, _ws, _att, _art) = standard_sandbox();
    sb.exec("fs.write('/workspace/file.txt', 'data')").unwrap();
    let result = sb
        .exec("local e = fs.list('/workspace/'); return table.concat(e, ',')")
        .unwrap();
    assert!(result.contains("file.txt"));
}

#[test]
fn double_slashes_in_paths() {
    let (sb, _root, _ws, _att, _art) = standard_sandbox();
    sb.exec("fs.write('/workspace/file.txt', 'data')").unwrap();
    let result = sb.exec("return fs.read('/workspace//file.txt')").unwrap();
    assert_eq!(result, "data");
}

#[test]
fn list_on_file_errors() {
    let (sb, _root, _ws, _att, _art) = standard_sandbox();
    sb.exec("fs.write('/workspace/file.txt', '')").unwrap();
    let result = sb.exec("fs.list('/workspace/file.txt')");
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("No such file or directory"));
}

// =============================================================================
// Remove operations
// =============================================================================

#[test]
fn remove_file() {
    let (sb, _root, _ws, _att, _art) = standard_sandbox();
    sb.exec("fs.write('/workspace/bye.txt', 'gone')").unwrap();
    sb.exec("fs.remove('/workspace/bye.txt')").unwrap();
    assert_eq!(
        sb.exec("return fs.exists('/workspace/bye.txt')").unwrap(),
        "false"
    );
}

#[test]
fn remove_directory_recursive() {
    let (sb, _root, _ws, _att, _art) = standard_sandbox();
    sb.exec("fs.mkdir('/workspace/dir/sub')").unwrap();
    sb.exec("fs.write('/workspace/dir/sub/file.txt', '')")
        .unwrap();
    sb.exec("fs.remove('/workspace/dir')").unwrap();
    assert_eq!(
        sb.exec("return fs.exists('/workspace/dir')").unwrap(),
        "false"
    );
}

#[test]
fn remove_nonexistent_fails() {
    let (sb, _root, _ws, _att, _art) = standard_sandbox();
    let result = sb.exec("fs.remove('/workspace/nope')");
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("No such file or directory"));
}

// =============================================================================
// Rename edge cases
// =============================================================================

#[test]
fn rename_directory() {
    let (sb, _root, _ws, _att, _art) = standard_sandbox();
    sb.exec("fs.mkdir('/workspace/olddir')").unwrap();
    sb.exec("fs.write('/workspace/olddir/f.txt', 'hi')")
        .unwrap();
    sb.exec("fs.rename('/workspace/olddir', '/workspace/newdir')")
        .unwrap();
    let result = sb
        .exec("return fs.read('/workspace/newdir/f.txt')")
        .unwrap();
    assert_eq!(result, "hi");
    assert_eq!(
        sb.exec("return fs.exists('/workspace/olddir')").unwrap(),
        "false"
    );
}

#[test]
fn rename_nonexistent_src_fails() {
    let (sb, _root, _ws, _att, _art) = standard_sandbox();
    let result = sb.exec("fs.rename('/workspace/nope', '/workspace/dst')");
    assert!(result.is_err());
}

// =============================================================================
// Copy operations
// =============================================================================

#[test]
fn copy_file_within_same_mount() {
    let (sb, _root, _ws, _att, _art) = standard_sandbox();
    sb.exec("fs.write('/workspace/src.txt', 'original')")
        .unwrap();
    sb.exec("fs.copy('/workspace/src.txt', '/workspace/dst.txt')")
        .unwrap();
    // Both files should exist with same content
    assert_eq!(
        sb.exec("return fs.read('/workspace/dst.txt')").unwrap(),
        "original"
    );
    assert_eq!(
        sb.exec("return fs.read('/workspace/src.txt')").unwrap(),
        "original"
    );
}

#[test]
fn copy_file_across_mounts() {
    let (sb, _root, _ws, _att, _art) = standard_sandbox();
    sb.exec("fs.write('/workspace/data.txt', 'cross-mount')")
        .unwrap();
    sb.exec("fs.copy('/workspace/data.txt', '/artifacts/data.txt')")
        .unwrap();
    assert_eq!(
        sb.exec("return fs.read('/artifacts/data.txt')").unwrap(),
        "cross-mount"
    );
    // Source still exists
    assert_eq!(
        sb.exec("return fs.exists('/workspace/data.txt')").unwrap(),
        "true"
    );
}

#[test]
fn copy_from_readonly_source_works() {
    let (sb, _root, _ws, att, _art) = standard_sandbox();
    fs::write(att.path().join("doc.txt"), "readonly content").unwrap();
    sb.exec("fs.copy('/attachments/doc.txt', '/artifacts/doc.txt')")
        .unwrap();
    assert_eq!(
        sb.exec("return fs.read('/artifacts/doc.txt')").unwrap(),
        "readonly content"
    );
}

#[test]
fn copy_to_readonly_dest_fails() {
    let (sb, _root, _ws, _att, _art) = standard_sandbox();
    sb.exec("fs.write('/workspace/file.txt', 'data')").unwrap();
    let result = sb.exec("fs.copy('/workspace/file.txt', '/attachments/file.txt')");
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Read-only file system"));
}

#[test]
fn copy_nonexistent_src_fails() {
    let (sb, _root, _ws, _att, _art) = standard_sandbox();
    let result = sb.exec("fs.copy('/workspace/nope.txt', '/artifacts/nope.txt')");
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("No such file or directory"));
}

#[test]
fn copy_directory_fails() {
    let (sb, _root, _ws, _att, _art) = standard_sandbox();
    sb.exec("fs.mkdir('/workspace/mydir')").unwrap();
    let result = sb.exec("fs.copy('/workspace/mydir', '/artifacts/mydir')");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Is a directory"));
}

#[test]
fn copy_overwrites_existing_destination() {
    let (sb, _root, _ws, _att, _art) = standard_sandbox();
    sb.exec("fs.write('/workspace/src.txt', 'new content')")
        .unwrap();
    sb.exec("fs.write('/workspace/dst.txt', 'old content')")
        .unwrap();
    sb.exec("fs.copy('/workspace/src.txt', '/workspace/dst.txt')")
        .unwrap();
    assert_eq!(
        sb.exec("return fs.read('/workspace/dst.txt')").unwrap(),
        "new content"
    );
}

#[test]
fn copy_path_traversal_fails() {
    let (sb, _root, _ws, _att, _art) = standard_sandbox();
    sb.exec("fs.write('/workspace/file.txt', '')").unwrap();
    let result = sb.exec("fs.copy('/workspace/file.txt', '/workspace/../../evil')");
    assert!(result.is_err());
}
