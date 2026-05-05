//! PDFium engine wrapper — loads the PDFium shared library and provides
//! a thread-safe handle for all PDF operations.
//!
//! Lifecycle: one `PdfiumEngine` per `Sandbox`, injected via `SandboxBuilder`.
//! NOT a global singleton — follows the same pattern as `HttpGateway`.

use pdfium_render::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Wraps a loaded `Pdfium` instance behind `Arc` for shared access.
///
/// `Pdfium` with the `sync` + `thread_safe` features is `Send + Sync`,
/// so `PdfiumEngine` can be cloned and shared across threads.
#[derive(Clone)]
pub struct PdfiumEngine {
    pdfium: Arc<Pdfium>,
    /// Path to the loaded library (for diagnostics).
    lib_path: PathBuf,
}

impl PdfiumEngine {
    /// Load PDFium from a specific library path.
    ///
    /// The path should point to the directory containing the library file,
    /// or directly to the library file itself.
    pub fn from_path(lib_path: impl AsRef<Path>) -> Result<Self, String> {
        let lib_path = lib_path.as_ref().to_path_buf();

        // Try the directory-based lookup first (handles platform naming),
        // then fall back to treating the path as the exact library file.
        let dir = if lib_path.is_dir() {
            lib_path.clone()
        } else {
            lib_path.parent().unwrap_or(Path::new(".")).to_path_buf()
        };

        let bindings = Pdfium::bind_to_library(
            Pdfium::pdfium_platform_library_name_at_path(&dir),
        )
        .or_else(|_| {
            // Fall back to the exact path if the directory-based lookup failed
            Pdfium::bind_to_library(lib_path.to_str().unwrap_or_default())
        })
        .map_err(|e| format!("failed to load PDFium from {}: {}", lib_path.display(), e))?;

        Ok(Self {
            pdfium: Arc::new(Pdfium::new(bindings)),
            lib_path,
        })
    }

    /// Try to load PDFium from well-known locations:
    /// 1. `PDFIUM_DYNAMIC_LIB_PATH` environment variable
    /// 2. `libs/pdfium/lib/` relative to the given base directory
    /// 3. System library paths (platform-specific)
    pub fn discover(base_dir: Option<&Path>) -> Result<Self, String> {
        // 1. Explicit env var
        if let Ok(path) = std::env::var("PDFIUM_DYNAMIC_LIB_PATH") {
            let p = PathBuf::from(&path);
            if p.exists() {
                return Self::from_path(&p);
            }
        }

        // 2. Relative to base directory (e.g., crate root / app bundle)
        if let Some(base) = base_dir {
            let lib_name = Self::platform_lib_name();
            let candidate = base.join("libs").join("pdfium").join("lib").join(lib_name);
            if candidate.exists() {
                return Self::from_path(candidate.parent().unwrap());
            }
        }

        // 3. System paths via pdfium-render's built-in search
        let bindings = Pdfium::bind_to_system_library()
            .map_err(|e| format!("PDFium not found in system paths: {}", e))?;

        Ok(Self {
            lib_path: PathBuf::from("<system>"),
            pdfium: Arc::new(Pdfium::new(bindings)),
        })
    }

    /// Access the underlying `Pdfium` instance.
    pub fn pdfium(&self) -> &Pdfium {
        &self.pdfium
    }

    /// Path to the loaded library (for diagnostics / logging).
    pub fn lib_path(&self) -> &Path {
        &self.lib_path
    }

    /// Platform-specific library filename.
    fn platform_lib_name() -> &'static str {
        #[cfg(target_os = "macos")]
        { "libpdfium.dylib" }
        #[cfg(target_os = "linux")]
        { "libpdfium.so" }
        #[cfg(target_os = "windows")]
        { "pdfium.dll" }
        #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
        { "libpdfium.so" }
    }
}

impl std::fmt::Debug for PdfiumEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PdfiumEngine")
            .field("lib_path", &self.lib_path)
            .finish()
    }
}
