//! Mount table and path resolution for sandboxed filesystem access.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MountPermission {
    ReadOnly,
    ReadWrite,
}

#[derive(Debug, Clone)]
pub struct MountEntry {
    pub host_path: PathBuf,
    pub permission: MountPermission,
}

#[derive(Debug, Error)]
pub enum MountError {
    #[error("invalid mount spec '{0}': expected host:virtual or host:virtual:ro")]
    InvalidSpec(String),
    #[error("host path does not exist: {0}")]
    HostPathNotFound(PathBuf),
    #[error("virtual path must be absolute: {0}")]
    VirtualPathNotAbsolute(String),
    #[error("{0}: No such file or directory")]
    NotFound(String),
    #[error("{0}: Path traversal denied")]
    PathTraversal(String),
    #[error("{0}: Read-only file system")]
    ReadOnly(String),
    #[error("{0}: Is a directory")]
    IsDirectory(String),
    #[error("{0} is a system directory and cannot be removed or renamed")]
    MountRoot(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, Default)]
pub struct MountTable {
    mounts: HashMap<String, MountEntry>,
    /// Synthetic directories: virtual paths that exist without any host backing.
    /// Maps a normalized virtual dir path to a list of child entry names.
    /// Example: "/dev" → ["null", "zero", "urandom", "stdin", "stdout", "stderr"]
    synthetic_dirs: HashMap<String, Vec<String>>,
}

impl MountTable {
    pub fn new() -> Self {
        Self::default()
    }

    /// Parse a mount spec in the format `host:virtual` or `host:virtual:ro`.
    /// Default permission is read-write.
    pub fn parse_and_add(&mut self, spec: &str) -> Result<(), MountError> {
        // On Windows, host paths start with a drive letter like "C:\..." which
        // contains a colon. Skip past the drive-letter colon before splitting.
        let (drive_prefix, rest) = if spec.len() >= 2
            && spec.as_bytes()[0].is_ascii_alphabetic()
            && spec.as_bytes()[1] == b':'
        {
            (&spec[..2], &spec[2..])
        } else {
            ("", spec)
        };

        let parts: Vec<&str> = rest.splitn(3, ':').collect();
        let (host_tail, virtual_str, permission) = match parts.len() {
            2 => (parts[0], parts[1], MountPermission::ReadWrite),
            3 => {
                let perm = match parts[2] {
                    "ro" => MountPermission::ReadOnly,
                    "rw" => MountPermission::ReadWrite,
                    _ => return Err(MountError::InvalidSpec(spec.to_string())),
                };
                (parts[0], parts[1], perm)
            }
            _ => return Err(MountError::InvalidSpec(spec.to_string())),
        };

        let host_path = PathBuf::from(format!("{}{}", drive_prefix, host_tail));
        if !host_path.exists() {
            return Err(MountError::HostPathNotFound(host_path));
        }

        if !virtual_str.starts_with('/') {
            return Err(MountError::VirtualPathNotAbsolute(virtual_str.to_string()));
        }

        let host_path = host_path.canonicalize()?;
        let virtual_path = normalize_virtual(virtual_str);

        self.mounts.insert(
            virtual_path,
            MountEntry {
                host_path,
                permission,
            },
        );

        Ok(())
    }

    /// Access the raw mounts map for direct insertion (used by SandboxBuilder for auto-/tmp).
    pub fn mounts_mut(&mut self) -> &mut HashMap<String, MountEntry> {
        &mut self.mounts
    }

    /// Register a synthetic directory with known child entries.
    /// Synthetic dirs exist in the virtual namespace without any host-backed storage.
    /// They are listable and pass existence checks, but reads/writes to children
    /// must be handled separately (e.g. special-path logic in fs globals).
    pub fn add_synthetic_dir(&mut self, virtual_path: &str, children: Vec<String>) {
        let normalized = normalize_virtual(virtual_path);
        self.synthetic_dirs.insert(normalized, children);
    }

    /// Returns true if the normalized virtual path is an exact mount key.
    pub fn is_mount_root(&self, path: &str) -> bool {
        let normalized = normalize_virtual(path);
        self.mounts.contains_key(&normalized)
    }

    /// Return the mount key (longest-prefix mount) that a virtual path falls under.
    pub fn mount_key_for(&self, virtual_path: &str) -> Option<String> {
        let normalized = normalize_virtual(virtual_path);
        let mut best: Option<&str> = None;
        for mount_virtual in self.mounts.keys() {
            if is_under_mount(&normalized, mount_virtual) {
                if best.is_none() || mount_virtual.len() > best.unwrap().len() {
                    best = Some(mount_virtual);
                }
            }
        }
        best.map(|s| s.to_string())
    }

    /// Check if a virtual path is a synthesized directory (an ancestor of a mount point,
    /// or a registered synthetic directory).
    /// `/` is always a virtual directory. If `/foo/bar` is mounted, `/foo` is also a virtual dir.
    pub fn is_virtual_dir(&self, virtual_path: &str) -> bool {
        let normalized = normalize_virtual(virtual_path);
        if normalized == "/" {
            return true;
        }
        // Check synthetic dirs
        if self.synthetic_dirs.contains_key(&normalized) {
            return true;
        }
        let prefix = format!("{}/", normalized);
        // Check mount-implied virtual dirs
        if self.mounts.keys().any(|m| m.starts_with(&prefix)) {
            return true;
        }
        // Check if this is an ancestor of a synthetic dir
        self.synthetic_dirs.keys().any(|s| s.starts_with(&prefix))
    }

    /// List entries in a virtual directory. Returns the immediate children.
    /// For `/` with mounts `/data` and `/workspace/src`, returns `["data", "workspace"]`.
    /// For `/workspace`, returns `["src"]`.
    /// If the path is a real mounted directory, lists the host directory contents too.
    /// Paths that resolve through a parent mount (e.g. `/test` under a `/` mount)
    /// are also listed correctly.
    pub fn list_virtual_dir(&self, virtual_path: &str) -> Option<Vec<String>> {
        let normalized = normalize_virtual(virtual_path);

        let mut entries = HashSet::new();
        let mut found = false;
        let prefix = if normalized == "/" {
            "/".to_string()
        } else {
            format!("{}/", normalized)
        };

        // Collect immediate children from mount paths
        for mount_virtual in self.mounts.keys() {
            if let Some(rest) = mount_virtual.strip_prefix(&prefix) {
                // rest is like "data" or "workspace/src" — take the first component.
                // Skip empty rest (happens when "/" mount is listed with prefix "/").
                let child = rest.split('/').next().unwrap();
                if !child.is_empty() {
                    entries.insert(child.to_string());
                    found = true;
                }
            }
        }

        // If this path is itself a mount point, list the host directory
        if let Some(entry) = self.mounts.get(&normalized) {
            found = true;
            if entry.host_path.is_dir() {
                if let Ok(read_dir) = std::fs::read_dir(&entry.host_path) {
                    for e in read_dir.filter_map(|e| e.ok()) {
                        entries.insert(e.file_name().to_string_lossy().to_string());
                    }
                }
            }
        } else if let Ok((host_path, _)) = self.resolve(&normalized) {
            // Not an exact mount key, but resolves through a parent mount
            if host_path.is_dir() {
                found = true;
                if let Ok(read_dir) = std::fs::read_dir(&host_path) {
                    for e in read_dir.filter_map(|e| e.ok()) {
                        entries.insert(e.file_name().to_string_lossy().to_string());
                    }
                }
            }
        }

        // If this exact path is a synthetic dir, add its children
        if let Some(children) = self.synthetic_dirs.get(&normalized) {
            found = true;
            for child in children {
                entries.insert(child.clone());
            }
        }

        // Collect immediate children from synthetic dir paths
        // (e.g. if /dev is synthetic, listing "/" should show "dev")
        for synth_path in self.synthetic_dirs.keys() {
            if let Some(rest) = synth_path.strip_prefix(&prefix) {
                let child = rest.split('/').next().unwrap();
                if !child.is_empty() {
                    entries.insert(child.to_string());
                    found = true;
                }
            }
        }

        // Virtual dir (ancestor of a mount or synthetic) is always valid
        if self.is_virtual_dir(virtual_path) {
            found = true;
        }

        if !found {
            return None;
        }

        let mut result: Vec<String> = entries.into_iter().collect();
        result.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
        Some(result)
    }

    /// Check if a virtual path exists — either as a real mounted path, a virtual directory,
    /// or a child entry of a synthetic directory.
    pub fn exists(&self, virtual_path: &str) -> bool {
        if self.is_virtual_dir(virtual_path) {
            return true;
        }
        // Check if this is a child of a synthetic dir
        if self.is_synthetic_entry(virtual_path) {
            return true;
        }
        match self.resolve(virtual_path) {
            Ok((host_path, _)) => host_path.exists(),
            Err(_) => false,
        }
    }

    /// Check if a virtual path is a named child entry of a registered synthetic directory.
    /// e.g. "/dev/null" is a synthetic entry if "/dev" is a synthetic dir containing "null".
    pub fn is_synthetic_entry(&self, virtual_path: &str) -> bool {
        let normalized = normalize_virtual(virtual_path);
        // Split into parent + child
        if let Some(slash_pos) = normalized.rfind('/') {
            let parent = if slash_pos == 0 {
                "/"
            } else {
                &normalized[..slash_pos]
            };
            let child = &normalized[slash_pos + 1..];
            if let Some(children) = self.synthetic_dirs.get(parent) {
                return children.iter().any(|c| c == child);
            }
        }
        false
    }

    /// Return the synthetic directory that owns this path, if any.
    ///
    /// This treats both the synthetic directory itself and any descendant path as
    /// covered by that synthetic namespace. Callers use this to keep pseudo
    /// filesystems such as `/proc`, `/etc`, and `/dev` read-only even when `/`
    /// is backed by a writable root mount.
    pub fn synthetic_dir_for(&self, virtual_path: &str) -> Option<String> {
        let normalized = normalize_virtual(virtual_path);
        let mut best: Option<&str> = None;
        for synthetic_path in self.synthetic_dirs.keys() {
            if normalized == *synthetic_path
                || normalized.starts_with(&format!("{}/", synthetic_path))
            {
                if best.is_none() || synthetic_path.len() > best.unwrap().len() {
                    best = Some(synthetic_path);
                }
            }
        }
        best.map(|path| path.to_string())
    }

    /// Resolve a virtual path to a host path, enforcing mount boundaries.
    pub fn resolve(&self, virtual_path: &str) -> Result<(PathBuf, MountPermission), MountError> {
        let normalized = normalize_virtual(virtual_path);

        // Find the longest-prefix mount
        let mut best_mount: Option<(&str, &MountEntry)> = None;
        for (mount_virtual, entry) in &self.mounts {
            if is_under_mount(&normalized, mount_virtual) {
                if best_mount.is_none() || mount_virtual.len() > best_mount.unwrap().0.len() {
                    best_mount = Some((mount_virtual, entry));
                }
            }
        }

        let (mount_virtual, entry) =
            best_mount.ok_or_else(|| MountError::NotFound(normalized.clone()))?;

        // Compute the relative path below the mount point
        let relative = mount_relative(&normalized, mount_virtual);

        let host_resolved = if relative.is_empty() {
            entry.host_path.clone()
        } else {
            entry.host_path.join(relative)
        };

        // Canonicalize if the path exists, otherwise check the parent
        let canonical = if host_resolved.exists() {
            host_resolved.canonicalize()?
        } else {
            // For new files, canonicalize the parent and append the filename
            let parent = host_resolved
                .parent()
                .ok_or_else(|| MountError::PathTraversal(virtual_path.to_string()))?;
            if !parent.exists() {
                return Err(MountError::NotFound(virtual_path.to_string()));
            }
            let canonical_parent = parent.canonicalize()?;
            let file_name = host_resolved
                .file_name()
                .ok_or_else(|| MountError::PathTraversal(virtual_path.to_string()))?;
            canonical_parent.join(file_name)
        };

        // Ensure the resolved path is under the mount's host path
        if !canonical.starts_with(&entry.host_path) {
            return Err(MountError::PathTraversal(virtual_path.to_string()));
        }

        Ok((canonical, entry.permission))
    }

    /// Resolve a virtual path and require write permission.
    /// Rejects virtual directories (they're not real, can't write to them).
    pub fn resolve_write(&self, virtual_path: &str) -> Result<PathBuf, MountError> {
        if self.is_virtual_dir(virtual_path)
            && self.mounts.get(&normalize_virtual(virtual_path)).is_none()
        {
            return Err(MountError::ReadOnly(virtual_path.to_string()));
        }
        let (host_path, perm) = self.resolve(virtual_path)?;
        if perm != MountPermission::ReadWrite {
            return Err(MountError::ReadOnly(virtual_path.to_string()));
        }
        Ok(host_path)
    }

    /// Resolve a virtual path for reading (any permission is fine).
    pub fn resolve_read(&self, virtual_path: &str) -> Result<PathBuf, MountError> {
        let (host_path, _) = self.resolve(virtual_path)?;
        Ok(host_path)
    }

    /// Resolve a virtual path for writing without requiring intermediate directories to exist.
    /// Unlike `resolve_write`, this constructs the host path directly from the mount root
    /// instead of canonicalizing parents. The caller is responsible for creating directories.
    /// Rejects paths containing ".." components for security.
    pub fn resolve_write_deep(&self, virtual_path: &str) -> Result<PathBuf, MountError> {
        let normalized = normalize_virtual(virtual_path);

        // Reject ".." components (can't canonicalize non-existent paths)
        for segment in normalized.split('/') {
            if segment == ".." {
                return Err(MountError::PathTraversal(virtual_path.to_string()));
            }
        }

        // Find the longest-prefix mount
        let mut best_mount: Option<(&str, &MountEntry)> = None;
        for (mount_virtual, entry) in &self.mounts {
            if is_under_mount(&normalized, mount_virtual) {
                if best_mount.is_none() || mount_virtual.len() > best_mount.unwrap().0.len() {
                    best_mount = Some((mount_virtual, entry));
                }
            }
        }

        let (mount_virtual, entry) =
            best_mount.ok_or_else(|| MountError::NotFound(normalized.clone()))?;

        if entry.permission != MountPermission::ReadWrite {
            return Err(MountError::ReadOnly(virtual_path.to_string()));
        }

        let relative = mount_relative(&normalized, mount_virtual);

        let host_path = if relative.is_empty() {
            entry.host_path.clone()
        } else {
            entry.host_path.join(relative)
        };

        Ok(host_path)
    }
}

/// Check if a normalized path is under a mount virtual path.
/// Handles the root mount "/" correctly (format!("{}/", "/") would produce "//"
/// which breaks starts_with matching).
fn is_under_mount(normalized: &str, mount_virtual: &str) -> bool {
    normalized == mount_virtual
        || (mount_virtual == "/" && normalized.starts_with('/'))
        || normalized.starts_with(&format!("{}/", mount_virtual))
}

/// Compute the relative path of `normalized` below `mount_virtual`.
/// Returns an empty string if the path IS the mount point.
/// The returned string never has a leading '/'.
fn mount_relative<'a>(normalized: &'a str, mount_virtual: &str) -> &'a str {
    if normalized == mount_virtual {
        ""
    } else if mount_virtual == "/" {
        // Root mount: "/test/foo" → "test/foo"
        &normalized[1..]
    } else {
        // Non-root mount: "/workspace/foo" → "foo" (skip mount + '/')
        &normalized[mount_virtual.len() + 1..]
    }
}

/// Normalize a virtual path: remove trailing slashes, collapse consecutive slashes.
fn normalize_virtual(path: &str) -> String {
    let mut result = String::with_capacity(path.len());
    let mut prev_slash = false;
    for ch in path.chars() {
        if ch == '/' {
            if !prev_slash {
                result.push('/');
            }
            prev_slash = true;
        } else {
            result.push(ch);
            prev_slash = false;
        }
    }
    // Remove trailing slash unless it's the root "/"
    if result.len() > 1 && result.ends_with('/') {
        result.pop();
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn setup_mount() -> (TempDir, MountTable) {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("hello.txt"), "world").unwrap();
        fs::create_dir(dir.path().join("subdir")).unwrap();
        fs::write(dir.path().join("subdir/nested.txt"), "nested").unwrap();

        let mut table = MountTable::new();
        let spec = format!("{}:/data", dir.path().display());
        table.parse_and_add(&spec).unwrap();
        (dir, table)
    }

    #[test]
    fn test_resolve_file() {
        let (_dir, table) = setup_mount();
        let (path, perm) = table.resolve("/data/hello.txt").unwrap();
        assert!(path.exists());
        assert_eq!(perm, MountPermission::ReadWrite);
        assert_eq!(fs::read_to_string(&path).unwrap(), "world");
    }

    #[test]
    fn test_resolve_nested() {
        let (_dir, table) = setup_mount();
        let path = table.resolve_read("/data/subdir/nested.txt").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "nested");
    }

    #[test]
    fn test_not_found() {
        let (_dir, table) = setup_mount();
        let result = table.resolve("/other/file.txt");
        assert!(matches!(result, Err(MountError::NotFound(_))));
    }

    #[test]
    fn test_path_traversal() {
        let (_dir, table) = setup_mount();
        let result = table.resolve("/data/../../../etc/passwd");
        assert!(result.is_err());
    }

    #[test]
    fn test_read_only_mount() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("file.txt"), "content").unwrap();

        let mut table = MountTable::new();
        let spec = format!("{}:/ro-data:ro", dir.path().display());
        table.parse_and_add(&spec).unwrap();

        // Read should work
        let path = table.resolve_read("/ro-data/file.txt").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "content");

        // Write should fail
        let result = table.resolve_write("/ro-data/file.txt");
        assert!(matches!(result, Err(MountError::ReadOnly(_))));
    }

    #[test]
    fn test_invalid_spec() {
        let mut table = MountTable::new();
        assert!(table.parse_and_add("no-colon").is_err());
    }

    #[test]
    fn test_windows_drive_letter_mount() {
        // Simulate a Windows-style path with drive letter.
        // We use a temp dir and construct a spec that looks like "X:\path:/virtual"
        // by rewriting the temp path to include a drive letter prefix.
        let dir = TempDir::new().unwrap();
        let canonical = dir.path().canonicalize().unwrap();
        let display = canonical.display().to_string();

        // On Windows, the canonical path already has a drive letter (e.g. C:\...).
        // On Unix, skip this test since paths don't have drive letters.
        if display.len() >= 2
            && display.as_bytes()[0].is_ascii_alphabetic()
            && display.as_bytes()[1] == b':'
        {
            let mut table = MountTable::new();
            let spec = format!("{}:/data", display);
            table.parse_and_add(&spec).unwrap();

            // Also test with :ro suffix
            let mut table2 = MountTable::new();
            let spec_ro = format!("{}:/rodata:ro", display);
            table2.parse_and_add(&spec_ro).unwrap();
        }
    }

    #[test]
    fn test_virtual_path_must_be_absolute() {
        let dir = TempDir::new().unwrap();
        let mut table = MountTable::new();
        let spec = format!("{}:relative", dir.path().display());
        let result = table.parse_and_add(&spec);
        assert!(matches!(result, Err(MountError::VirtualPathNotAbsolute(_))));
    }

    #[test]
    fn test_resolve_new_file() {
        let (dir, table) = setup_mount();
        // Resolve a file that doesn't exist yet (for writing)
        let path = table.resolve_write("/data/newfile.txt").unwrap();
        assert_eq!(path, dir.path().canonicalize().unwrap().join("newfile.txt"));
    }

    #[test]
    fn test_root_is_always_virtual_dir() {
        let table = MountTable::new();
        assert!(table.is_virtual_dir("/"));
    }

    #[test]
    fn test_list_root_shows_mount_children() {
        let dir = TempDir::new().unwrap();
        let mut table = MountTable::new();
        table
            .parse_and_add(&format!("{}:/data", dir.path().display()))
            .unwrap();
        table
            .parse_and_add(&format!("{}:/workspace/src", dir.path().display()))
            .unwrap();

        let entries = table.list_virtual_dir("/").unwrap();
        assert_eq!(entries, vec!["data", "workspace"]);
    }

    #[test]
    fn test_list_virtual_parent() {
        let dir = TempDir::new().unwrap();
        let mut table = MountTable::new();
        table
            .parse_and_add(&format!("{}:/workspace/src", dir.path().display()))
            .unwrap();

        assert!(table.is_virtual_dir("/workspace"));
        let entries = table.list_virtual_dir("/workspace").unwrap();
        assert_eq!(entries, vec!["src"]);
    }

    #[test]
    fn test_list_empty_root() {
        let table = MountTable::new();
        let entries = table.list_virtual_dir("/").unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_list_nonexistent_path() {
        let table = MountTable::new();
        assert!(table.list_virtual_dir("/nope").is_none());
    }

    #[test]
    fn test_exists_virtual_dir() {
        let dir = TempDir::new().unwrap();
        let mut table = MountTable::new();
        table
            .parse_and_add(&format!("{}:/foo/bar", dir.path().display()))
            .unwrap();

        assert!(table.exists("/"));
        assert!(table.exists("/foo"));
        assert!(table.exists("/foo/bar"));
        assert!(!table.exists("/baz"));
    }

    #[test]
    fn test_list_mounted_dir_includes_host_contents() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("a.txt"), "").unwrap();
        fs::write(dir.path().join("b.txt"), "").unwrap();

        let mut table = MountTable::new();
        table
            .parse_and_add(&format!("{}:/data", dir.path().display()))
            .unwrap();

        let entries = table.list_virtual_dir("/data").unwrap();
        assert!(entries.contains(&"a.txt".to_string()));
        assert!(entries.contains(&"b.txt".to_string()));
    }

    // -- Root mount tests --

    #[test]
    fn test_root_mount_resolve_child() {
        let root_dir = TempDir::new().unwrap();
        fs::create_dir(root_dir.path().join("test")).unwrap();

        let mut table = MountTable::new();
        table
            .parse_and_add(&format!("{}:/", root_dir.path().display()))
            .unwrap();

        // /test should resolve to {root_dir}/test
        let (path, perm) = table.resolve("/test").unwrap();
        assert_eq!(perm, MountPermission::ReadWrite);
        assert_eq!(path, root_dir.path().canonicalize().unwrap().join("test"));
    }

    #[test]
    fn test_root_mount_resolve_nested() {
        let root_dir = TempDir::new().unwrap();
        fs::create_dir_all(root_dir.path().join("a/b")).unwrap();
        fs::write(root_dir.path().join("a/b/c.txt"), "content").unwrap();

        let mut table = MountTable::new();
        table
            .parse_and_add(&format!("{}:/", root_dir.path().display()))
            .unwrap();

        let path = table.resolve_read("/a/b/c.txt").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "content");
    }

    #[test]
    fn test_root_mount_write_new_file() {
        let root_dir = TempDir::new().unwrap();

        let mut table = MountTable::new();
        table
            .parse_and_add(&format!("{}:/", root_dir.path().display()))
            .unwrap();

        // Writing a new file under / should resolve to the root dir
        let path = table.resolve_write("/newfile.txt").unwrap();
        assert_eq!(
            path,
            root_dir.path().canonicalize().unwrap().join("newfile.txt")
        );
    }

    #[test]
    fn test_root_mount_mkdir() {
        let root_dir = TempDir::new().unwrap();

        let mut table = MountTable::new();
        table
            .parse_and_add(&format!("{}:/", root_dir.path().display()))
            .unwrap();

        // mkdir /test should resolve to {root_dir}/test
        let path = table.resolve_write("/test").unwrap();
        assert_eq!(path, root_dir.path().canonicalize().unwrap().join("test"));
    }

    #[test]
    fn test_root_mount_write_deep() {
        let root_dir = TempDir::new().unwrap();

        let mut table = MountTable::new();
        table
            .parse_and_add(&format!("{}:/", root_dir.path().display()))
            .unwrap();

        // Deep write should resolve correctly
        let path = table.resolve_write_deep("/tmp/scratch/file.txt").unwrap();
        assert_eq!(
            path,
            root_dir
                .path()
                .canonicalize()
                .unwrap()
                .join("tmp/scratch/file.txt")
        );
    }

    #[test]
    fn test_root_mount_specific_mount_wins() {
        // When both / and /workspace are mounted, /workspace/foo should
        // resolve to the workspace mount, not the root mount.
        let root_dir = TempDir::new().unwrap();
        let ws_dir = TempDir::new().unwrap();
        fs::write(ws_dir.path().join("foo.txt"), "ws").unwrap();

        let mut table = MountTable::new();
        table
            .parse_and_add(&format!("{}:/", root_dir.path().display()))
            .unwrap();
        table
            .parse_and_add(&format!("{}:/workspace", ws_dir.path().display()))
            .unwrap();

        // /workspace/foo.txt should resolve to ws_dir, not root_dir
        let path = table.resolve_read("/workspace/foo.txt").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "ws");

        // /tmp should resolve to root_dir
        let path = table.resolve_write("/tmp").unwrap();
        assert_eq!(path, root_dir.path().canonicalize().unwrap().join("tmp"));
    }

    #[test]
    fn test_root_mount_list_merges_mounts_and_host() {
        let root_dir = TempDir::new().unwrap();
        let ws_dir = TempDir::new().unwrap();
        // Create a physical dir in root
        fs::create_dir(root_dir.path().join("tmp")).unwrap();

        let mut table = MountTable::new();
        table
            .parse_and_add(&format!("{}:/", root_dir.path().display()))
            .unwrap();
        table
            .parse_and_add(&format!("{}:/workspace", ws_dir.path().display()))
            .unwrap();

        let entries = table.list_virtual_dir("/").unwrap();
        // Should include both the physical "tmp" dir and the synthesized "workspace" mount
        assert!(entries.contains(&"tmp".to_string()));
        assert!(entries.contains(&"workspace".to_string()));
    }

    #[test]
    fn test_root_mount_list_no_empty_entry() {
        // When "/" is mounted, list_virtual_dir("/") must not include an
        // empty-string entry (the root mount key stripped of prefix "/").
        let root_dir = TempDir::new().unwrap();
        let ws_dir = TempDir::new().unwrap();
        fs::create_dir(root_dir.path().join("tmp")).unwrap();

        let mut table = MountTable::new();
        table
            .parse_and_add(&format!("{}:/", root_dir.path().display()))
            .unwrap();
        table
            .parse_and_add(&format!("{}:/workspace", ws_dir.path().display()))
            .unwrap();

        let entries = table.list_virtual_dir("/").unwrap();
        assert!(
            !entries.contains(&String::new()),
            "root listing must not contain an empty-string entry, got: {:?}",
            entries
        );
    }

    // -- Synthetic directory tests --

    #[test]
    fn test_synthetic_dir_is_virtual_dir() {
        let mut table = MountTable::new();
        table.add_synthetic_dir("/dev", vec!["null".into(), "zero".into()]);

        assert!(table.is_virtual_dir("/dev"));
        assert!(!table.is_virtual_dir("/dev/null")); // child is a file, not a dir
    }

    #[test]
    fn test_synthetic_dir_exists() {
        let mut table = MountTable::new();
        table.add_synthetic_dir("/dev", vec!["null".into(), "zero".into()]);

        assert!(table.exists("/dev"));
        assert!(table.exists("/dev/null"));
        assert!(table.exists("/dev/zero"));
        assert!(!table.exists("/dev/random")); // not in children list
    }

    #[test]
    fn test_synthetic_dir_list() {
        let mut table = MountTable::new();
        table.add_synthetic_dir("/dev", vec!["null".into(), "zero".into(), "urandom".into()]);

        let entries = table.list_virtual_dir("/dev").unwrap();
        assert_eq!(entries, vec!["null", "urandom", "zero"]); // sorted
    }

    #[test]
    fn test_synthetic_dir_shows_in_parent_listing() {
        let mut table = MountTable::new();
        table.add_synthetic_dir("/dev", vec!["null".into()]);

        let entries = table.list_virtual_dir("/").unwrap();
        assert!(entries.contains(&"dev".to_string()));
    }

    #[test]
    fn test_synthetic_dir_coexists_with_mounts() {
        let dir = TempDir::new().unwrap();
        let mut table = MountTable::new();
        table
            .parse_and_add(&format!("{}:/data", dir.path().display()))
            .unwrap();
        table.add_synthetic_dir("/dev", vec!["null".into()]);

        let entries = table.list_virtual_dir("/").unwrap();
        assert!(entries.contains(&"data".to_string()));
        assert!(entries.contains(&"dev".to_string()));
    }

    #[test]
    fn test_is_synthetic_entry() {
        let mut table = MountTable::new();
        table.add_synthetic_dir("/dev", vec!["null".into(), "zero".into()]);

        assert!(table.is_synthetic_entry("/dev/null"));
        assert!(table.is_synthetic_entry("/dev/zero"));
        assert!(!table.is_synthetic_entry("/dev/random"));
        assert!(!table.is_synthetic_entry("/dev"));
    }

    #[test]
    fn test_synthetic_dir_for() {
        let mut table = MountTable::new();
        table.add_synthetic_dir("/proc", vec!["version".into()]);
        table.add_synthetic_dir("/proc/sys", vec!["kernel".into()]);

        assert_eq!(table.synthetic_dir_for("/proc"), Some("/proc".to_string()));
        assert_eq!(
            table.synthetic_dir_for("/proc/version"),
            Some("/proc".to_string())
        );
        assert_eq!(
            table.synthetic_dir_for("/proc/new/file"),
            Some("/proc".to_string())
        );
        assert_eq!(
            table.synthetic_dir_for("/proc/sys/kernel"),
            Some("/proc/sys".to_string())
        );
        assert_eq!(table.synthetic_dir_for("/tmp/proc"), None);
    }
}
