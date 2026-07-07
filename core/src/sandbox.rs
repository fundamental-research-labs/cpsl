//! Luau sandbox construction, execution, and global module registration.

use crate::mount::MountTable;
#[cfg(feature = "mod-apple-calendar")]
use apple_calendar::AppleCalendarGateway;
use mlua::{Lua, MultiValue, Value};
#[cfg(feature = "mod-http")]
use native_http::HttpGateway;
#[cfg(cpsl_experimental_sfae)]
use sfae_core::store::SecretStore;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

mod doc;

#[allow(unused_imports)]
pub(crate) use doc::validate_args;
#[cfg(feature = "mod-fs")]
pub(crate) use doc::FS_DOC;
pub(crate) use doc::{
    arg_error, FieldDoc, FnDoc, HelpMode, ModuleDoc, Param, ParamType, ReturnType,
};

mod errors;

pub use errors::{clean_lua_error, humanize_error, ExecError, SandboxError};

pub struct Sandbox {
    lua: Lua,
    /// Buffer that captures print() output. Shared with the Lua print function.
    print_buf: Arc<Mutex<String>>,
    /// Whether the next print/__write needs a newline separator before content.
    /// Shared with print/__write Lua functions. Reset on each exec() call.
    needs_newline: Arc<Mutex<bool>>,
    /// Auto-created tmpdir backing `/tmp` mount. Cleaned up on drop.
    _tmpdir: Option<std::path::PathBuf>,
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        if let Some(ref dir) = self._tmpdir {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}

impl Sandbox {
    /// Create a new sandbox with no mounts (no FS access, no networking).
    pub fn new() -> Result<Self, SandboxError> {
        Self::builder().build()
    }

    /// Create a new sandbox with the given mount table for FS access.
    pub fn with_mounts(mounts: MountTable) -> Result<Self, SandboxError> {
        Self::builder().mounts(mounts).build()
    }

    pub fn builder() -> SandboxBuilder {
        SandboxBuilder::default()
    }

    /// Execute Luau code and return the result as a string.
    ///
    /// Output from `print()` calls is captured and included in the result.
    /// If the code has both print output and a return value, they are
    /// concatenated with a newline separator.
    ///
    /// Returns a clean [`ExecError`] on failure with the Luau/mlua noise
    /// stripped and optional source location extracted.
    pub fn exec(&self, code: &str) -> Result<String, ExecError> {
        // Clear the print buffer and reset newline state before execution.
        // On poisoned lock, clear by replacing with a new value.
        match self.print_buf.lock() {
            Ok(mut b) => b.clear(),
            Err(e) => *e.into_inner() = String::new(),
        }
        match self.needs_newline.lock() {
            Ok(mut nl) => *nl = false,
            Err(e) => *e.into_inner() = false,
        }

        let result: MultiValue = self
            .lua
            .load(code)
            .set_name("input")
            .eval()
            .map_err(|e| clean_lua_error(&e))?;
        let return_val = format_multi_value(&result);

        // Drain captured print output
        let printed = match self.print_buf.lock() {
            Ok(b) => b.clone(),
            Err(e) => e.into_inner().clone(),
        };

        // Combine: print output first, then return value (if any)
        let mut output = printed;
        if !return_val.is_empty() {
            if !output.is_empty() {
                output.push('\n');
            }
            output.push_str(&return_val);
        }
        Ok(output)
    }

    /// Execute Luau code and return captured stdout-style output.
    ///
    /// `exec()` preserves the historical test-oriented output shape without a
    /// trailing print newline. FFI callers need process-like stdout, so this
    /// method restores the final newline when the last write came from print().
    pub fn exec_stdout(&self, code: &str) -> Result<String, ExecError> {
        let mut output = self.exec(code)?;
        let ended_with_print_newline = match self.needs_newline.lock() {
            Ok(needs_newline) => *needs_newline,
            Err(poisoned) => *poisoned.into_inner(),
        };
        if ended_with_print_newline && !output.is_empty() {
            output.push('\n');
        }
        Ok(output)
    }

    /// Load the pyrt (Python runtime) module into the sandbox.
    /// After this, transpiled Python→Luau code can `require("pyrt")`.
    pub fn load_pyrt(&self, pyrt_source: &str) -> Result<(), SandboxError> {
        // Compile and cache pyrt as a module accessible via require("pyrt")
        // We load it into a global, since Luau sandboxed mode restricts require()
        let chunk = format!(
            r#"
            do
                local pyrt_loader = function()
                    {}
                end
                -- Make it available globally so `local py = require("pyrt")` works
                -- In sandboxed mode, we override require to return our module
                _G.__pyrt_module = pyrt_loader()
            end
            "#,
            pyrt_source
        );
        // We need to run this before sandbox mode is enabled.
        // Since sandbox is already enabled, we temporarily work around it
        // by setting the module directly.
        self.lua
            .load(&chunk)
            .set_name("pyrt_loader")
            .exec()
            .map_err(SandboxError::Lua)?;
        Ok(())
    }

    /// Load pyrt and set up the custom require system for Python modules.
    pub fn setup_python_runtime(&self, pyrt_source: &str) -> Result<(), SandboxError> {
        // Load pyrt as a chunk that returns the module table
        let val: mlua::Value = self
            .lua
            .load(pyrt_source)
            .set_name("pyrt")
            .eval()
            .map_err(SandboxError::Lua)?;

        // Register pyrt via the helper installed in build()
        let register: mlua::Function = self
            .lua
            .globals()
            .get("__register_module")
            .map_err(SandboxError::Lua)?;
        register
            .call::<()>(("pyrt", val))
            .map_err(SandboxError::Lua)?;

        Ok(())
    }

    /// Register a Luau module so `require(name)` returns it.
    /// The source must be a Luau chunk that returns a table.
    /// Also ensures the custom require override is installed so `require(name)` works.
    pub fn register_module(&self, name: &str, source: &str) -> Result<(), SandboxError> {
        let val: mlua::Value = self
            .lua
            .load(source)
            .set_name(name)
            .eval()
            .map_err(SandboxError::Lua)?;

        // Register via the helper installed in build()
        let register: mlua::Function = self
            .lua
            .globals()
            .get("__register_module")
            .map_err(SandboxError::Lua)?;
        register
            .call::<()>((name, val))
            .map_err(SandboxError::Lua)?;

        Ok(())
    }

    /// Load shrt.luau shell runtime and register it so `require("shrt")` works.
    pub fn setup_shell_runtime(&self, shrt_source: &str) -> Result<(), SandboxError> {
        self.register_module("shrt", shrt_source)
    }

    /// Access the underlying Lua VM.
    pub fn lua(&self) -> &Lua {
        &self.lua
    }
}

/// Host hardware and OS information collected at sandbox build time.
/// Used to populate synthetic files (/proc/cpuinfo, etc.) and shell builtins (uname, etc.)
/// with real values from the host machine.
#[derive(Debug, Clone)]
pub(crate) struct HostInfo {
    pub os: String,        // "macos", "linux", "windows"
    pub arch: String,      // "aarch64", "x86_64"
    pub cpu_brand: String, // e.g. "Apple M4 Max"
    pub cpu_cores: u32,
    pub mem_total_bytes: u64,
    pub os_version: String, // e.g. "15.3" on macOS
}

impl HostInfo {
    /// Collect real host system information using platform-specific methods.
    pub fn collect() -> Self {
        let os = std::env::consts::OS.to_string();
        let arch = std::env::consts::ARCH.to_string();

        let (cpu_brand, cpu_cores, mem_total_bytes, os_version) = if cfg!(target_os = "macos") {
            Self::collect_macos()
        } else if cfg!(target_os = "linux") {
            Self::collect_linux()
        } else {
            ("Unknown CPU".to_string(), 1, 0u64, String::new())
        };

        HostInfo {
            os,
            arch,
            cpu_brand,
            cpu_cores,
            mem_total_bytes,
            os_version,
        }
    }

    #[cfg(target_os = "macos")]
    fn collect_macos() -> (String, u32, u64, String) {
        use std::process::Command;

        let sysctl = |key: &str| -> Option<String> {
            Command::new("sysctl")
                .args(["-n", key])
                .output()
                .ok()
                .and_then(|o| {
                    if o.status.success() {
                        Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
                    } else {
                        None
                    }
                })
        };

        let cpu_brand =
            sysctl("machdep.cpu.brand_string").unwrap_or_else(|| "Unknown CPU".to_string());
        let cpu_cores = sysctl("hw.ncpu")
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(1);
        let mem_total = sysctl("hw.memsize")
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);
        let os_version = sysctl("kern.osproductversion").unwrap_or_default();

        (cpu_brand, cpu_cores, mem_total, os_version)
    }

    #[cfg(not(target_os = "macos"))]
    fn collect_macos() -> (String, u32, u64, String) {
        ("Unknown CPU".to_string(), 1, 0, String::new())
    }

    #[cfg(target_os = "linux")]
    fn collect_linux() -> (String, u32, u64, String) {
        let mut cpu_brand = "Unknown CPU".to_string();
        let mut cpu_cores: u32 = 1;
        let mut mem_total: u64 = 0;
        let mut os_version = String::new();

        if let Ok(cpuinfo) = std::fs::read_to_string("/proc/cpuinfo") {
            let mut core_count = 0u32;
            for line in cpuinfo.lines() {
                if line.starts_with("model name") {
                    if let Some(val) = line.split(':').nth(1) {
                        cpu_brand = val.trim().to_string();
                    }
                }
                if line.starts_with("processor") {
                    core_count += 1;
                }
            }
            if core_count > 0 {
                cpu_cores = core_count;
            }
        }

        if let Ok(meminfo) = std::fs::read_to_string("/proc/meminfo") {
            for line in meminfo.lines() {
                if line.starts_with("MemTotal:") {
                    if let Some(val) = line.split_whitespace().nth(1) {
                        if let Ok(kb) = val.parse::<u64>() {
                            mem_total = kb * 1024;
                        }
                    }
                    break;
                }
            }
        }

        if let Ok(release) = std::fs::read_to_string("/etc/os-release") {
            for line in release.lines() {
                if line.starts_with("VERSION_ID=") {
                    os_version = line
                        .trim_start_matches("VERSION_ID=")
                        .trim_matches('"')
                        .to_string();
                    break;
                }
            }
        }

        (cpu_brand, cpu_cores, mem_total, os_version)
    }

    #[cfg(not(target_os = "linux"))]
    fn collect_linux() -> (String, u32, u64, String) {
        ("Unknown CPU".to_string(), 1, 0, String::new())
    }
}

/// Callback invoked when the sandbox performs a file operation.
/// Arguments: (virtual_path, activity_type) where activity_type is "read", "write", "remove", "rename", or "mkdir".
pub type FileActivityCallback = Arc<dyn Fn(&str, &str) + Send + Sync>;

/// Called by doc module before local extraction. Gets first crack at reading any file.
/// Vision callback — only called when mode resolves to "vision".
/// The sandbox owns the routing decision; the callback just extracts or fails.
///
/// Arguments: (file_bytes, filename, query)
/// Returns: Ok(text) on success, Err(msg) on failure.
pub type VisionCallback = Arc<dyn Fn(&[u8], &str, &str) -> Result<String, String> + Send + Sync>;

/// Legacy alias for backward compatibility with external callers (Tauri).
pub type DocReadCallback = VisionCallback;

/// A deferred read awaiting batch resolution via `doc.readAsync()`.
#[cfg(feature = "mod-doc")]
pub struct PendingRead {
    pub data: Vec<u8>,
    pub filename: String,
    pub format: crate::doc_reader::DocFormat,
    pub query: String,
    pub read_opts: crate::doc_reader::ReadOptions,
    pub cache_key: String,
    pub result_slot: Arc<Mutex<Option<Result<String, String>>>>,
}

/// Shared queue of deferred reads, drained on first `:await()`.
#[cfg(feature = "mod-doc")]
pub type PendingReads = Arc<Mutex<Vec<PendingRead>>>;

pub struct SandboxBuilder {
    mounts: Option<MountTable>,
    auto_tmp: bool,
    #[cfg(feature = "mod-http")]
    http_gateway: Option<Arc<HttpGateway>>,
    #[cfg(feature = "mod-apple-calendar")]
    calendar_gateway: Option<Arc<AppleCalendarGateway>>,
    #[cfg(cpsl_experimental_sfae)]
    sfae_store: Option<Arc<Mutex<dyn SecretStore + Send>>>,
    #[cfg(cpsl_experimental_sfae)]
    sfae_prompt: Option<Arc<dyn crate::sfae::CredentialPrompt>>,
    #[cfg(cpsl_experimental_sfae)]
    sfae_browser_opener: Option<Arc<dyn crate::sfae::BrowserOpener>>,
    file_activity_callback: Option<FileActivityCallback>,
    vision_callback: Option<VisionCallback>,
    doc_cache_dir: Option<PathBuf>,
    #[cfg(feature = "pdfium-render")]
    pdfium_engine: Option<Arc<crate::pdfium_engine::PdfiumEngine>>,
}

impl Default for SandboxBuilder {
    fn default() -> Self {
        Self {
            mounts: None,
            auto_tmp: true,
            #[cfg(feature = "mod-http")]
            http_gateway: None,
            #[cfg(feature = "mod-apple-calendar")]
            calendar_gateway: None,
            #[cfg(cpsl_experimental_sfae)]
            sfae_store: None,
            #[cfg(cpsl_experimental_sfae)]
            sfae_prompt: None,
            #[cfg(cpsl_experimental_sfae)]
            sfae_browser_opener: None,
            file_activity_callback: None,
            vision_callback: None,
            doc_cache_dir: None,
            #[cfg(feature = "pdfium-render")]
            pdfium_engine: None,
        }
    }
}

impl SandboxBuilder {
    pub fn mounts(mut self, mounts: MountTable) -> Self {
        self.mounts = Some(mounts);
        self
    }

    pub fn auto_tmp(mut self, enabled: bool) -> Self {
        self.auto_tmp = enabled;
        self
    }

    #[cfg(feature = "mod-http")]
    pub fn http_gateway(mut self, gateway: Arc<HttpGateway>) -> Self {
        self.http_gateway = Some(gateway);
        self
    }

    #[cfg(feature = "mod-apple-calendar")]
    pub fn calendar_gateway(mut self, gateway: Arc<AppleCalendarGateway>) -> Self {
        self.calendar_gateway = Some(gateway);
        self
    }

    #[cfg(cpsl_experimental_sfae)]
    pub fn sfae_store(mut self, store: Arc<Mutex<dyn SecretStore + Send>>) -> Self {
        self.sfae_store = Some(store);
        self
    }

    #[cfg(cpsl_experimental_sfae)]
    pub fn sfae_prompt(mut self, prompt: Arc<dyn crate::sfae::CredentialPrompt>) -> Self {
        self.sfae_prompt = Some(prompt);
        self
    }

    #[cfg(cpsl_experimental_sfae)]
    pub fn sfae_browser_opener(mut self, opener: Arc<dyn crate::sfae::BrowserOpener>) -> Self {
        self.sfae_browser_opener = Some(opener);
        self
    }

    /// Set a callback that is invoked on every file operation (read, write, remove, etc.).
    pub fn file_activity_callback(mut self, cb: FileActivityCallback) -> Self {
        self.file_activity_callback = Some(cb);
        self
    }

    /// Set a callback for vision-powered document reading (images, PDFs via Gemini).
    pub fn vision_callback(mut self, cb: VisionCallback) -> Self {
        self.vision_callback = Some(cb);
        self
    }

    /// Legacy alias — use `vision_callback()` for new code.
    pub fn doc_read_callback(self, cb: VisionCallback) -> Self {
        self.vision_callback(cb)
    }

    /// Set the PDFium engine for PDF operations (structural extraction, editing).
    #[cfg(feature = "pdfium-render")]
    pub fn pdfium_engine(mut self, engine: Arc<crate::pdfium_engine::PdfiumEngine>) -> Self {
        self.pdfium_engine = Some(engine);
        self
    }

    /// Set the disk cache directory for doc read results.
    pub fn doc_cache_dir(mut self, dir: PathBuf) -> Self {
        self.doc_cache_dir = Some(dir);
        self
    }

    pub fn build(self) -> Result<Sandbox, SandboxError> {
        let lua = Lua::new();
        let mut mount_table = self.mounts.unwrap_or_default();

        // Collect host hardware info and build synthetic file content
        let host_info = HostInfo::collect();

        // Register synthetic directories for common Linux paths
        mount_table.add_synthetic_dir(
            "/dev",
            vec![
                "null".into(),
                "zero".into(),
                "urandom".into(),
                "stdin".into(),
                "stdout".into(),
                "stderr".into(),
            ],
        );
        mount_table.add_synthetic_dir(
            "/proc",
            vec!["version".into(), "cpuinfo".into(), "meminfo".into()],
        );
        mount_table.add_synthetic_dir("/etc", vec!["hostname".into(), "os-release".into()]);

        #[cfg(feature = "mod-fs")]
        let synthetic_files = Arc::new(build_synthetic_files(&host_info));

        // Auto-create a writable /tmp directory if no user mount covers it
        let tmpdir = if self.auto_tmp && mount_table.mount_key_for("/tmp").is_none() {
            use std::sync::atomic::{AtomicU64, Ordering};
            static COUNTER: AtomicU64 = AtomicU64::new(0);
            let id = COUNTER.fetch_add(1, Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!("sandbox-{}-{}", std::process::id(), id));
            std::fs::create_dir_all(&dir)
                .map_err(|e| SandboxError::Lua(mlua::Error::external(e)))?;
            // Canonicalize to resolve symlinks (e.g. /tmp → /private/tmp on macOS)
            let canonical = dir
                .canonicalize()
                .map_err(|e| SandboxError::Lua(mlua::Error::external(e)))?;
            // Mount it at /tmp as read-write
            mount_table.mounts_mut().insert(
                "/tmp".to_string(),
                crate::mount::MountEntry {
                    host_path: canonical,
                    permission: crate::mount::MountPermission::ReadWrite,
                },
            );
            Some(dir)
        } else {
            // /tmp is covered by a parent mount (e.g. "/").  Ensure the host-side
            // `tmp` directory actually exists so writes to /tmp/… don't fail.
            if !mount_table.is_mount_root("/tmp") {
                if let Ok(host_tmp) = mount_table.resolve_write_deep("/tmp") {
                    let _ = std::fs::create_dir_all(&host_tmp);
                }
            }
            None
        };

        let mounts = Arc::new(mount_table);
        let print_buf = Arc::new(Mutex::new(String::new()));

        // Register custom globals BEFORE enabling sandbox mode (which makes globals read-only).
        // Each module is gated on its Cargo feature — only compiled-in modules are registered.
        #[cfg(feature = "mod-fs")]
        register_fs_globals(
            &lua,
            mounts.clone(),
            synthetic_files,
            self.file_activity_callback.clone(),
        )?;
        #[cfg(feature = "mod-compress")]
        crate::compress::register_compress_globals(&lua, mounts.clone())?;
        #[cfg(feature = "mod-json")]
        crate::json::register_json_globals(&lua)?;
        #[cfg(feature = "mod-csv")]
        crate::csv_mod::register_csv_globals(&lua, mounts.clone())?;
        #[cfg(feature = "mod-doc")]
        crate::doc::register_doc_globals(
            &lua,
            mounts.clone(),
            self.vision_callback,
            self.doc_cache_dir,
            #[cfg(feature = "pdfium-render")]
            self.pdfium_engine,
        )?;
        #[cfg(feature = "mod-plot")]
        crate::plot::register_plot_globals(&lua, mounts.clone())?;
        #[cfg(feature = "mod-yaml")]
        crate::yaml::register_yaml_globals(&lua, mounts.clone())?;
        #[cfg(feature = "mod-xml")]
        crate::xml::register_xml_globals(&lua, mounts.clone())?;
        #[cfg(feature = "mod-numpy")]
        {
            crate::numpy::register_numpy_globals(&lua)?;
            crate::numpy::register_numpy_linalg(&lua)?;
            crate::numpy::register_numpy_random(&lua)?;
        }
        #[cfg(feature = "mod-fuzzy")]
        crate::fuzzy::register_fuzzy_globals(&lua)?;
        #[cfg(feature = "mod-phone")]
        crate::phone::register_phone_globals(&lua)?;
        #[cfg(feature = "mod-email")]
        crate::email::register_email_globals(&lua)?;
        #[cfg(feature = "mod-country")]
        crate::country::register_country_globals(&lua)?;
        #[cfg(feature = "mod-datetime")]
        crate::datetime::register_datetime_globals(&lua)?;
        #[cfg(feature = "mod-image")]
        crate::image::register_image_globals(&lua, mounts.clone())?;
        #[cfg(feature = "mod-random")]
        crate::random::register_random_globals(&lua)?;
        #[cfg(feature = "mod-base64")]
        crate::base64::register_base64_globals(&lua)?;
        #[cfg(feature = "mod-fin")]
        crate::fin::register_fin_globals(&lua)?;
        #[cfg(feature = "mod-crypto")]
        crate::crypto::register_crypto_globals(&lua)?;
        #[cfg(feature = "mod-regex")]
        crate::regex_mod::register_regex_globals(&lua)?;
        #[cfg(feature = "mod-html")]
        crate::html_mod::register_html_globals(&lua)?;
        #[cfg(feature = "mod-url")]
        crate::url_mod::register_url_globals(&lua)?;
        #[cfg(feature = "mod-qr")]
        crate::qr::register_qr_globals(&lua, mounts.clone())?;
        #[cfg(feature = "mod-http")]
        if let Some(ref gw) = self.http_gateway {
            crate::http::register_http_globals(&lua, gw.clone())?;
            #[cfg(feature = "mod-yfinance")]
            crate::yfinance::register_yfinance_globals(&lua, gw.clone())?;
            #[cfg(feature = "mod-edgar")]
            crate::edgar::register_edgar_globals(&lua, gw.clone())?;
        }
        #[cfg(all(
            feature = "mod-apple-calendar",
            any(target_os = "macos", target_os = "ios")
        ))]
        {
            let gateway = self
                .calendar_gateway
                .unwrap_or_else(AppleCalendarGateway::shared_platform_default);
            crate::calendar::register_calendar_globals(&lua, gateway)?;
        }
        #[cfg(all(
            feature = "mod-apple-calendar",
            not(any(target_os = "macos", target_os = "ios"))
        ))]
        if let Some(ref gateway) = self.calendar_gateway {
            crate::calendar::register_calendar_globals(&lua, gateway.clone())?;
        }
        #[cfg(cpsl_experimental_sfae)]
        if let (Some(ref store), Some(ref prompt)) = (&self.sfae_store, &self.sfae_prompt) {
            crate::sfae::register_sfae_globals(
                &lua,
                store.clone(),
                prompt.clone(),
                self.sfae_browser_opener.clone(),
            )?;
        }

        let needs_newline = Arc::new(Mutex::new(false));
        register_print(&lua, print_buf.clone(), needs_newline.clone())?;
        register_global_help(&lua)?;
        remove_dangerous_globals(&lua)?;

        // Inject __sysinfo table so shell builtins (uname, hostname, etc.) can read real host values
        {
            let sysinfo = lua.create_table().map_err(SandboxError::Lua)?;
            sysinfo
                .set("sysname", "Sandbox")
                .map_err(SandboxError::Lua)?;
            sysinfo.set("release", "1.0").map_err(SandboxError::Lua)?;
            sysinfo
                .set("machine", host_info.arch.as_str())
                .map_err(SandboxError::Lua)?;
            sysinfo
                .set("hostname", "sandbox")
                .map_err(SandboxError::Lua)?;
            sysinfo
                .set("os", host_info.os.as_str())
                .map_err(SandboxError::Lua)?;
            sysinfo
                .set("os_version", host_info.os_version.as_str())
                .map_err(SandboxError::Lua)?;
            sysinfo
                .set("cpu_brand", host_info.cpu_brand.as_str())
                .map_err(SandboxError::Lua)?;
            sysinfo
                .set("cpu_cores", host_info.cpu_cores)
                .map_err(SandboxError::Lua)?;
            sysinfo
                .set("mem_total_bytes", host_info.mem_total_bytes)
                .map_err(SandboxError::Lua)?;
            lua.globals()
                .set("__sysinfo", sysinfo)
                .map_err(SandboxError::Lua)?;
        }

        // Install custom require that checks __modules then falls back to globals.
        // This lets the LLM write `local doc = require("doc")` for any global module.
        // Must happen before lua.sandbox(true) so we can reassign `require`.
        // NOTE: __modules is created as an upvalue-only table (not set on _G) so it
        // won't be frozen by sandbox mode. A __register_module helper is exposed as a
        // global so that register_module() / setup_python_runtime() can add entries
        // after sandbox mode is enabled.
        lua.load(
            r#"
            local modules = {}
            local orig_require = require
            require = function(name)
                local m = modules[name]
                if m ~= nil then return m end
                local g = rawget(_G, name)
                if type(g) == "table" then return g end
                return orig_require(name)
            end
            __register_module = function(name, val)
                modules[name] = val
            end
            "#,
        )
        .exec()
        .map_err(SandboxError::Lua)?;

        lua.sandbox(true)?;
        Ok(Sandbox {
            lua,
            print_buf,
            needs_newline,
            _tmpdir: tmpdir,
        })
    }
}

/// Build the static synthetic file content map from host info.
/// These are virtual files with fixed content (computed once at sandbox build time).
#[cfg(feature = "mod-fs")]
fn build_synthetic_files(info: &HostInfo) -> std::collections::HashMap<String, String> {
    use std::collections::HashMap;
    let mut map = HashMap::new();

    // /proc/version — branded as "Sandbox" with real host details
    map.insert(
        "/proc/version".to_string(),
        format!(
            "Sandbox 1.0 ({} {} {})",
            info.os, info.os_version, info.arch
        ),
    );

    // /proc/cpuinfo — simplified Linux-style format
    let mut cpuinfo = String::new();
    for i in 0..info.cpu_cores {
        if i > 0 {
            cpuinfo.push('\n');
        }
        cpuinfo.push_str(&format!(
            "processor\t: {}\nmodel name\t: {}\n",
            i, info.cpu_brand
        ));
    }
    map.insert("/proc/cpuinfo".to_string(), cpuinfo);

    // /proc/meminfo — MemTotal line (bytes → kB for Linux convention)
    let mem_kb = info.mem_total_bytes / 1024;
    map.insert(
        "/proc/meminfo".to_string(),
        format!("MemTotal:       {} kB", mem_kb),
    );

    // /etc/hostname
    map.insert("/etc/hostname".to_string(), "sandbox".to_string());

    // /etc/os-release
    let pretty = format!("Sandbox 1.0 ({} {})", info.os, info.os_version);
    map.insert(
        "/etc/os-release".to_string(),
        format!(
            "NAME=\"Sandbox\"\nPRETTY_NAME=\"{}\"\nID=sandbox\nVERSION=\"1.0\"\nHOST_OS=\"{}\"\nHOST_OS_VERSION=\"{}\"\nHOST_ARCH=\"{}\"",
            pretty, info.os, info.os_version, info.arch
        ),
    );

    map
}

/// Return the content of a synthetic virtual file, or None if not a known synthetic file.
/// Checks the pre-computed content map first, then handles special /dev/* files.
#[cfg(feature = "mod-fs")]
fn synthetic_file_content(
    path: &str,
    static_files: &std::collections::HashMap<String, String>,
) -> Option<String> {
    // Check pre-computed static files
    if let Some(content) = static_files.get(path) {
        return Some(content.clone());
    }
    // Dynamic/special files
    match path {
        "/dev/null" | "/dev/zero" => Some(String::new()),
        "/dev/urandom" => {
            let bytes: Vec<u8> = (0..16).map(|_| rand::random::<u8>()).collect();
            Some(bytes.iter().map(|b| format!("{:02x}", b)).collect())
        }
        _ => None,
    }
}

/// Slice a string by line numbers (1-based offset, optional limit).
/// - No offset, no limit -> return entire content
/// - offset only -> lines from offset to end
/// - limit only -> first `limit` lines
/// - both -> `limit` lines starting at `offset`
/// - offset beyond EOF -> empty string
/// - limit=0 -> empty string
#[cfg(feature = "mod-fs")]
fn slice_lines(content: &str, offset: Option<usize>, limit: Option<usize>) -> String {
    if offset.is_none() && limit.is_none() {
        return content.to_string();
    }

    let lines: Vec<&str> = content.lines().collect();
    let total = lines.len();

    // offset is 1-based; convert to 0-based index
    let start = match offset {
        Some(o) if o == 0 => 0, // treat 0 same as 1 (first line)
        Some(o) => o.saturating_sub(1),
        None => 0,
    };

    if start >= total {
        return String::new();
    }

    let end = match limit {
        Some(0) => return String::new(),
        Some(l) => std::cmp::min(start.saturating_add(l), total),
        None => total,
    };

    let selected = &lines[start..end];
    let mut result = selected.join("\n");
    // Preserve trailing newline if the original content had one and we're reading
    // to the natural end (no explicit limit that clips the output)
    if end == total && limit.is_none() && content.ends_with('\n') {
        result.push('\n');
    }
    result
}

#[cfg(feature = "mod-fs")]
fn register_fs_globals(
    lua: &Lua,
    mounts: Arc<MountTable>,
    synthetic_files: Arc<std::collections::HashMap<String, String>>,
    file_activity_cb: Option<FileActivityCallback>,
) -> Result<(), mlua::Error> {
    let fs = lua.create_table()?;

    register_fs_read_and_metadata(
        lua,
        &fs,
        mounts.clone(),
        synthetic_files.clone(),
        file_activity_cb.clone(),
    )?;

    register_fs_mutations(lua, &fs, mounts.clone(), file_activity_cb.clone())?;

    register_fs_search(lua, &fs, mounts.clone())?;

    register_fs_tree(lua, &fs, mounts.clone())?;

    crate::lua_util::register_help_functions(lua, &fs, &FS_DOC)?;

    lua.globals().set("fs", fs)?;

    // Wrap all functions (except help) to append hint on argument errors
    wrap_module_with_help_hints(lua, "fs")?;

    Ok(())
}

/// Wrap every function in a module table (except `help`) so that errors
/// include the caller's line number and inline usage help for argument errors.
///
/// ## Module error convention
///
/// Errors are read by both humans and AI agents. They must be:
/// - **Located**: always include the caller's source line (via `error(msg, 2)`)
/// - **Short**: one line of context, no stack traces or internal paths
/// - **Descriptive**: say what went wrong, not Luau/Rust internals
/// - **Self-correcting**: usage errors include the function's signature and
///   example inline so the caller (human or AI) can fix the call immediately
///   without a round-trip to `module.help()`
///
/// Error format after processing by `clean_lua_error()`:
/// ```text
/// 1: /attachments/test: Read-only file system     ← runtime error (no help)
/// 1: plot.bar: expected table of numbers           ← usage error with inline help
///   Usage: plot.bar(labels: table, values: table, opts: table) -> string
///   Example: plot.bar({x={"Q1","Q2","Q3"}, y={100,200,150}, output="/artifacts/sales.svg"})
/// ```
pub(crate) fn wrap_module_with_help_hints(lua: &Lua, module_name: &str) -> Result<(), mlua::Error> {
    lua.load(format!(
        r#"
        local mod = {module_name}
        local fn_help = rawget(mod, "__fn_help") or {{}}
        for k, v in pairs(mod) do
            if type(v) == "function" and k ~= "help" then
                local raw = v
                local usage = fn_help[k]
                mod[k] = function(...)
                    local results = table.pack(pcall(raw, ...))
                    if results[1] then
                        return table.unpack(results, 2, results.n)
                    end
                    local msg = tostring(results[2])
                    local trace = string.find(msg, "\nstack traceback:")
                    local clean = trace and string.sub(msg, 1, trace - 1) or msg
                    if string.find(clean, "bad argument")
                        or string.find(clean, "missing required argument")
                        or string.find(clean, "expected ")
                    then
                        if usage then
                            error(clean .. "\n" .. usage, 2)
                        else
                            error(clean .. "\n  hint: call {module_name}.help() for usage", 2)
                        end
                    else
                        error(clean, 2)
                    end
                end
            end
        end
        setmetatable(mod, {{
            __index = function(_, key)
                local k = tostring(key)
                if k:sub(1, 2) == "__" then return nil end
                error("{module_name}." .. k .. " does not exist\n  hint: call {module_name}.help() for usage", 2)
            end
        }})
        "#
    ))
    .exec()?;
    Ok(())
}

fn register_global_help(lua: &Lua) -> Result<(), mlua::Error> {
    // help() only prints — exec() picks up the print buffer.
    // No return, so there's no duplication when exec() combines print+return.
    //
    // The module listing is built dynamically at runtime: each module table has
    // a __summary field (set by register_help_functions). help() probes known
    // global names and includes only those that are actually registered.
    let code = r#"
        function help()
            local known = {"base64","calendar","compress","country","crypto","csv","currency","datetime","doc","edgar","email","fin","fs","fuzzy","html","http","image","json","numx","phone","plot","qr","random","regex","sfae","url","xml","yaml","yfinance"}
            local lines = {}
            for _, name in ipairs(known) do
                local m = rawget(_G, name)
                if type(m) == "table" then
                    local summary = rawget(m, "__summary") or ""
                    local pad = string.rep(" ", math.max(1, 13 - #name))
                    table.insert(lines, "  " .. name .. pad .. summary)
                end
            end
            local modules = #lines > 0 and table.concat(lines, "\n") or "  (none)"

            local text = "Sandbox — available modules and globals\n"
                .. "\n"
                .. "All modules are available as globals (no require needed).\n"
                .. "Run <module>.help() for detailed usage — recommended to discover useful options.\n"
                .. "\n"
                .. "Modules:\n"
                .. modules .. "\n"
                .. "\n"
                .. "Globals:\n"
                .. "  print(...)   Output values (captured and returned as result)\n"
                .. "  require(m)   Load a registered module\n"
                .. "  help()       Show this help message\n"
                .. "\n"
                .. "Standard libraries: string, table, math, bit32, buffer, vector, coroutine, utf8\n"
                .. "\n"
                .. "Removed (sandboxed): io, os, loadfile, dofile, string.dump\n"
            print(text)
        end
    "#;
    lua.load(code).exec()?;
    Ok(())
}

fn register_print(
    lua: &Lua,
    buf: Arc<Mutex<String>>,
    needs_newline: Arc<Mutex<bool>>,
) -> Result<(), mlua::Error> {
    let needs_nl_print = needs_newline.clone();
    let needs_nl_write = needs_newline;
    let buf2 = buf.clone();

    let print_fn = lua.create_function(move |_, values: MultiValue| {
        let line: Vec<String> = values.iter().map(format_value).collect();
        let Ok(mut b) = buf.lock() else { return Ok(()) };
        let Ok(mut nl) = needs_nl_print.lock() else {
            return Ok(());
        };
        if *nl {
            b.push('\n');
        }
        b.push_str(&line.join("\t"));
        *nl = true; // After print, next output needs a newline separator
        Ok(())
    })?;
    lua.globals().set("print", print_fn)?;

    // __write: append text to the output buffer without a trailing newline.
    // Used by shrt.luau's echo -n support.
    let write_fn = lua.create_function(move |_, text: String| {
        let Ok(mut b) = buf2.lock() else {
            return Ok(());
        };
        let Ok(mut nl) = needs_nl_write.lock() else {
            return Ok(());
        };
        if *nl {
            b.push('\n');
        }
        b.push_str(&text);
        *nl = false; // After __write, next output continues on same line
        Ok(())
    })?;
    lua.globals().set("__write", write_fn)?;

    Ok(())
}

fn remove_dangerous_globals(lua: &Lua) -> Result<(), mlua::Error> {
    let globals = lua.globals();

    // Remove io library entirely
    globals.set("io", mlua::Value::Nil)?;

    // Remove os library entirely — scripts should use fs module for file ops
    // and the sandbox doesn't need os.clock/os.time/os.difftime
    globals.set("os", mlua::Value::Nil)?;

    // Remove file loading functions
    globals.set("loadfile", mlua::Value::Nil)?;
    globals.set("dofile", mlua::Value::Nil)?;

    Ok(())
}

fn format_multi_value(values: &MultiValue) -> String {
    if values.is_empty() {
        return String::new();
    }
    values
        .iter()
        .map(format_value)
        .collect::<Vec<_>>()
        .join("\t")
}

fn format_value(value: &Value) -> String {
    match value {
        Value::Nil => "nil".to_string(),
        Value::Boolean(b) => b.to_string(),
        Value::Integer(i) => i.to_string(),
        Value::Number(n) => {
            if *n == (*n as i64) as f64 {
                format!("{}", *n as i64)
            } else {
                n.to_string()
            }
        }
        Value::String(s) => s.to_string_lossy().to_string(),
        Value::Table(_) => "table".to_string(),
        Value::Function(_) => "function".to_string(),
        _ => format!("{:?}", value),
    }
}

#[cfg(test)]
mod tests;

#[cfg(feature = "mod-fs")]
fn register_fs_read_and_metadata(
    lua: &Lua,
    fs: &mlua::Table,
    mounts: Arc<MountTable>,
    synthetic_files: Arc<std::collections::HashMap<String, String>>,
    file_activity_cb: Option<FileActivityCallback>,
) -> Result<(), mlua::Error> {
    // fs.read(path [, offset, limit]) -> string
    {
        let m = mounts.clone();
        let sf = synthetic_files.clone();
        let cb = file_activity_cb.clone();
        fs.set(
            "read",
            lua.create_function(move |_, args: MultiValue| {
                let validated = validate_args(&args, FS_DOC.params("read"), "fs.read")?;
                let path = match &validated[0] {
                    Value::String(s) => s.to_string_lossy().to_string(),
                    _ => unreachable!("validate_args ensures string"),
                };

                // Extract optional offset/limit (1-based line numbers)
                // Clamp negatives to 0 to avoid wrapping on the i64→usize cast.
                let offset: Option<usize> = match &validated[1] {
                    Value::Integer(n) => Some((*n).max(0) as usize),
                    Value::Number(n) => Some((*n).max(0.0) as usize),
                    _ => None,
                };
                let limit: Option<usize> = match &validated[2] {
                    Value::Integer(n) => Some((*n).max(0) as usize),
                    Value::Number(n) => Some((*n).max(0.0) as usize),
                    _ => None,
                };

                // Handle synthetic special files
                if let Some(content) = synthetic_file_content(&path, &sf) {
                    return Ok(slice_lines(&content, offset, limit));
                }
                if path.starts_with("/dev/") {
                    return match path.as_str() {
                        "/dev/stdin" | "/dev/stdout" | "/dev/stderr" => Err(mlua::Error::external(
                            format!("{}: not a regular file in sandbox", path),
                        )),
                        _ if m.is_synthetic_entry(&path) => Err(mlua::Error::external(format!(
                            "{}: not readable in sandbox",
                            path
                        ))),
                        _ => Err(mlua::Error::external(crate::MountError::NotFound(path))),
                    };
                }
                if m.is_virtual_dir(&path) {
                    return Err(mlua::Error::external(crate::MountError::IsDirectory(path)));
                }
                let host_path = m.resolve_read(&path).map_err(mlua::Error::external)?;
                let result = std::fs::read_to_string(&host_path).map_err(mlua::Error::external)?;
                if let Some(ref cb) = cb {
                    cb(&path, "read");
                }
                Ok(slice_lines(&result, offset, limit))
            })?,
        )?;
    }

    Ok(())
}

#[cfg(feature = "mod-fs")]
fn register_fs_mutations(
    lua: &Lua,
    fs: &mlua::Table,
    mounts: Arc<MountTable>,
    file_activity_cb: Option<FileActivityCallback>,
) -> Result<(), mlua::Error> {
    // fs.write(path, content)
    {
        let m = mounts.clone();
        let cb = file_activity_cb.clone();
        fs.set(
            "write",
            lua.create_function(move |_, args: MultiValue| {
                let validated = validate_args(&args, FS_DOC.params("write"), "fs.write")?;
                let path = match &validated[0] {
                    Value::String(s) => s.to_string_lossy().to_string(),
                    _ => unreachable!("validate_args ensures string"),
                };
                // Handle synthetic special files — discard writes to /dev/null etc,
                // reject writes to read-only synthetic files
                if path.starts_with("/dev/") {
                    return match path.as_str() {
                        "/dev/null" | "/dev/zero" | "/dev/urandom" => Ok(()),
                        "/dev/stdin" | "/dev/stdout" | "/dev/stderr" => Err(mlua::Error::external(
                            format!("{}: not a regular file in sandbox", path),
                        )),
                        _ => Err(mlua::Error::external(crate::MountError::ReadOnly(path))),
                    };
                }
                // Synthetic namespaces (/proc, /etc, most of /dev) are read-only
                // even when the sandbox has a writable root mount.
                if m.synthetic_dir_for(&path).is_some() {
                    return Err(mlua::Error::external(crate::MountError::ReadOnly(path)));
                }
                let content = match &validated[1] {
                    Value::String(s) => s.as_bytes().to_vec(),
                    _ => unreachable!("validate_args ensures string"),
                };
                let host_path = m.resolve_write(&path).map_err(mlua::Error::external)?;
                std::fs::write(&host_path, &content).map_err(mlua::Error::external)?;
                if let Some(ref cb) = cb {
                    cb(&path, "write");
                }
                Ok(())
            })?,
        )?;
    }

    // fs.list(path) -> table of strings
    {
        let m = mounts.clone();
        let cb = file_activity_cb.clone();
        fs.set(
            "list",
            lua.create_function(move |lua, args: MultiValue| {
                let validated = validate_args(&args, FS_DOC.params("list"), "fs.list")?;
                let path = match &validated[0] {
                    Value::String(s) => s.to_string_lossy().to_string(),
                    _ => unreachable!("validate_args ensures string"),
                };
                let entries = m.list_virtual_dir(&path).ok_or_else(|| {
                    mlua::Error::external(crate::MountError::NotFound(path.clone()))
                })?;
                let table = lua.create_table()?;
                for (i, name) in entries.iter().enumerate() {
                    table.set(i + 1, name.as_str())?;
                }
                if let Some(ref cb) = cb {
                    cb(&path, "read");
                }
                Ok(table)
            })?,
        )?;
    }

    // fs.exists(path) -> boolean
    {
        let m = mounts.clone();
        fs.set(
            "exists",
            lua.create_function(move |_, args: MultiValue| {
                let validated = validate_args(&args, FS_DOC.params("exists"), "fs.exists")?;
                let path = match &validated[0] {
                    Value::String(s) => s.to_string_lossy().to_string(),
                    _ => unreachable!("validate_args ensures string"),
                };
                Ok(m.exists(&path))
            })?,
        )?;
    }

    // fs.writable(path) — check if a path is under a writable mount
    {
        let m = mounts.clone();
        fs.set(
            "writable",
            lua.create_function(move |_, args: MultiValue| {
                let validated = validate_args(&args, FS_DOC.params("writable"), "fs.writable")?;
                let path = match &validated[0] {
                    Value::String(s) => s.to_string_lossy().to_string(),
                    _ => unreachable!("validate_args ensures string"),
                };
                if m.synthetic_dir_for(&path).is_some() {
                    return Ok(false);
                }
                // Check if resolve_write would succeed (ignoring whether the path exists)
                Ok(m.resolve_write(&path).is_ok() || m.resolve_write_deep(&path).is_ok())
            })?,
        )?;
    }

    // fs.mkdir(path)
    {
        let m = mounts.clone();
        let cb = file_activity_cb.clone();
        fs.set(
            "mkdir",
            lua.create_function(move |_, args: MultiValue| {
                let validated = validate_args(&args, FS_DOC.params("mkdir"), "fs.mkdir")?;
                let path = match &validated[0] {
                    Value::String(s) => s.to_string_lossy().to_string(),
                    _ => unreachable!("validate_args ensures string"),
                };
                if m.synthetic_dir_for(&path).is_some() {
                    return Err(mlua::Error::external(crate::MountError::ReadOnly(path)));
                }
                let host_path = m.resolve_write_deep(&path).map_err(mlua::Error::external)?;
                std::fs::create_dir_all(&host_path).map_err(mlua::Error::external)?;
                if let Some(ref cb) = cb {
                    cb(&path, "write");
                }
                Ok(())
            })?,
        )?;
    }

    // fs.rename(src, dst)
    {
        let m = mounts.clone();
        let cb = file_activity_cb.clone();
        fs.set(
            "rename",
            lua.create_function(move |_, args: MultiValue| {
                let validated = validate_args(&args, FS_DOC.params("rename"), "fs.rename")?;
                let src = match &validated[0] {
                    Value::String(s) => s.to_string_lossy().to_string(),
                    _ => unreachable!("validate_args ensures string"),
                };
                let dst = match &validated[1] {
                    Value::String(s) => s.to_string_lossy().to_string(),
                    _ => unreachable!("validate_args ensures string"),
                };
                if m.synthetic_dir_for(&src).is_some() {
                    return Err(mlua::Error::external(crate::MountError::ReadOnly(src)));
                }
                if m.synthetic_dir_for(&dst).is_some() {
                    return Err(mlua::Error::external(crate::MountError::ReadOnly(dst)));
                }
                if m.is_mount_root(&src) {
                    return Err(mlua::Error::external(crate::MountError::MountRoot(src)));
                }
                if m.is_mount_root(&dst) {
                    return Err(mlua::Error::external(crate::MountError::MountRoot(dst)));
                }
                let host_src = m.resolve_write(&src).map_err(mlua::Error::external)?;
                if !host_src.exists() {
                    return Err(mlua::Error::external(crate::MountError::NotFound(
                        src.clone(),
                    )));
                }
                let host_dst = m.resolve_write(&dst).map_err(mlua::Error::external)?;
                std::fs::rename(&host_src, &host_dst).map_err(mlua::Error::external)?;
                if let Some(ref cb) = cb {
                    cb(&dst, "write");
                }
                Ok(())
            })?,
        )?;
    }

    // fs.remove(path)
    {
        let m = mounts.clone();
        let cb = file_activity_cb.clone();
        fs.set(
            "remove",
            lua.create_function(move |_, args: MultiValue| {
                let validated = validate_args(&args, FS_DOC.params("remove"), "fs.remove")?;
                let path = match &validated[0] {
                    Value::String(s) => s.to_string_lossy().to_string(),
                    _ => unreachable!("validate_args ensures string"),
                };
                if m.synthetic_dir_for(&path).is_some() {
                    return Err(mlua::Error::external(crate::MountError::ReadOnly(path)));
                }
                if m.is_mount_root(&path) {
                    return Err(mlua::Error::external(crate::MountError::MountRoot(path)));
                }
                let host_path = m.resolve_write(&path).map_err(mlua::Error::external)?;
                if !host_path.exists() {
                    return Err(mlua::Error::external(crate::MountError::NotFound(
                        path.clone(),
                    )));
                }
                if host_path.is_dir() {
                    std::fs::remove_dir_all(&host_path).map_err(mlua::Error::external)?;
                } else {
                    std::fs::remove_file(&host_path).map_err(mlua::Error::external)?;
                }
                if let Some(ref cb) = cb {
                    cb(&path, "write");
                }
                Ok(())
            })?,
        )?;
    }

    // fs.isdir(path) -> boolean
    {
        let m = mounts.clone();
        fs.set(
            "isdir",
            lua.create_function(move |_, args: MultiValue| {
                let validated = validate_args(&args, FS_DOC.params("isdir"), "fs.isdir")?;
                let path = match &validated[0] {
                    Value::String(s) => s.to_string_lossy().to_string(),
                    _ => unreachable!("validate_args ensures string"),
                };
                // Synthetic entries (e.g. /dev/null) are files, not dirs
                if m.is_synthetic_entry(&path) {
                    return Ok(false);
                }
                if m.is_virtual_dir(&path) {
                    return Ok(true);
                }
                if m.is_mount_root(&path) {
                    if let Ok((host_path, _)) = m.resolve(&path) {
                        return Ok(host_path.is_dir());
                    }
                }
                match m.resolve(&path) {
                    Ok((host_path, _)) => Ok(host_path.is_dir()),
                    Err(_) => Ok(false),
                }
            })?,
        )?;
    }

    // fs.isfile(path) -> boolean
    {
        let m = mounts.clone();
        fs.set(
            "isfile",
            lua.create_function(move |_, args: MultiValue| {
                let validated = validate_args(&args, FS_DOC.params("isfile"), "fs.isfile")?;
                let path = match &validated[0] {
                    Value::String(s) => s.to_string_lossy().to_string(),
                    _ => unreachable!("validate_args ensures string"),
                };
                // Synthetic entries (e.g. /dev/null, /dev/zero) are file-like
                if m.is_synthetic_entry(&path) {
                    return Ok(true);
                }
                if m.is_virtual_dir(&path) {
                    return Ok(false);
                }
                match m.resolve(&path) {
                    Ok((host_path, _)) => Ok(host_path.is_file()),
                    Err(_) => Ok(false),
                }
            })?,
        )?;
    }

    // fs.size(path) -> number (bytes)
    {
        let m = mounts.clone();
        fs.set(
            "size",
            lua.create_function(move |_, args: MultiValue| {
                let validated = validate_args(&args, FS_DOC.params("size"), "fs.size")?;
                let path = match &validated[0] {
                    Value::String(s) => s.to_string_lossy().to_string(),
                    _ => unreachable!("validate_args ensures string"),
                };
                // Synthetic entries have size 0
                if m.is_synthetic_entry(&path) {
                    return Ok(0u64);
                }
                if m.is_virtual_dir(&path) {
                    return Ok(0u64);
                }
                let host_path = m.resolve_read(&path).map_err(mlua::Error::external)?;
                let meta = std::fs::metadata(&host_path).map_err(mlua::Error::external)?;
                Ok(meta.len())
            })?,
        )?;
    }

    // fs.copy(src, dst)
    {
        let m = mounts.clone();
        let cb = file_activity_cb.clone();
        fs.set(
            "copy",
            lua.create_function(move |_, args: MultiValue| {
                let validated = validate_args(&args, FS_DOC.params("copy"), "fs.copy")?;
                let src = match &validated[0] {
                    Value::String(s) => s.to_string_lossy().to_string(),
                    _ => unreachable!("validate_args ensures string"),
                };
                let dst = match &validated[1] {
                    Value::String(s) => s.to_string_lossy().to_string(),
                    _ => unreachable!("validate_args ensures string"),
                };
                if m.synthetic_dir_for(&dst).is_some() {
                    return Err(mlua::Error::external(crate::MountError::ReadOnly(dst)));
                }
                let host_src = m.resolve_read(&src).map_err(mlua::Error::external)?;
                if !host_src.exists() {
                    return Err(mlua::Error::external(crate::MountError::NotFound(src)));
                }
                if host_src.is_dir() {
                    return Err(mlua::Error::external(crate::MountError::IsDirectory(src)));
                }
                let host_dst = m.resolve_write(&dst).map_err(mlua::Error::external)?;
                std::fs::copy(&host_src, &host_dst).map_err(mlua::Error::external)?;
                if let Some(ref cb) = cb {
                    cb(&dst, "write");
                }
                Ok(())
            })?,
        )?;
    }

    Ok(())
}

#[cfg(feature = "mod-fs")]
fn register_fs_search(
    lua: &Lua,
    fs: &mlua::Table,
    mounts: Arc<MountTable>,
) -> Result<(), mlua::Error> {
    #[cfg(feature = "mod-ripgrep")]
    {
        crate::grep_api::register_fs_grep(
            lua,
            fs,
            crate::grep_api::RipgrepProvider::new(mounts.clone()),
        )?;
    }

    #[cfg(all(feature = "mod-fff", not(feature = "mod-ripgrep")))]
    {
        crate::grep_api::register_fs_grep(
            lua,
            fs,
            crate::grep_api::FffGrepProvider::fs_compatible(mounts.clone()),
        )?;
    }

    #[cfg(not(any(feature = "mod-ripgrep", feature = "mod-fff")))]
    {
        let _ = (lua, fs, mounts);
    }

    Ok(())
}

#[cfg(feature = "mod-fs")]
fn register_fs_tree(
    lua: &Lua,
    fs: &mlua::Table,
    mounts: Arc<MountTable>,
) -> Result<(), mlua::Error> {
    // fs.tree(opts) -> string (ASCII directory tree)
    // Walks through the virtual mount layer (not the host filesystem directly),
    // so it sees all mounts, virtual dirs, and synthetic entries.
    #[cfg(feature = "mod-ripgrep")]
    {
        let m = mounts.clone();
        fs.set(
            "tree",
            lua.create_function(move |_, args: MultiValue| {
                let validated = validate_args(&args, FS_DOC.params("tree"), "fs.tree")?;
                let opts = match &validated[0] {
                    Value::Table(t) => t.clone(),
                    _ => unreachable!("validate_args ensures table"),
                };

                // Extract required fields
                let sandbox_path: String = opts
                    .get::<mlua::String>("path")
                    .map_err(|_| {
                        mlua::Error::external("fs.tree: missing required field 'path' (string)")
                    })?
                    .to_string_lossy()
                    .to_string();

                // Extract optional fields
                let max_depth: usize = opts
                    .get::<Value>("depth")
                    .ok()
                    .and_then(|v| match v {
                        Value::Integer(n) => Some(n.max(0) as usize),
                        Value::Number(n) => Some((n as i64).max(0) as usize),
                        _ => None,
                    })
                    .unwrap_or(3);

                let dirs_only: bool = opts
                    .get::<Value>("dirs_only")
                    .ok()
                    .and_then(|v| match v {
                        Value::Boolean(b) => Some(b),
                        _ => None,
                    })
                    .unwrap_or(false);

                let glob_pattern: Option<String> =
                    opts.get::<Value>("glob").ok().and_then(|v| match v {
                        Value::String(s) => Some(s.to_string_lossy().to_string()),
                        _ => None,
                    });

                // Compile glob filter if specified
                let compiled_glob: Option<globset::GlobMatcher> = match glob_pattern {
                    Some(ref g) => {
                        let glob = globset::Glob::new(g).map_err(|e| {
                            mlua::Error::external(format!("fs.tree: invalid glob: {}", e))
                        })?;
                        Some(glob.compile_matcher())
                    }
                    None => None,
                };

                // Check that the path exists in the virtual filesystem
                let is_dir = m.is_virtual_dir(&sandbox_path);
                let is_file = if !is_dir {
                    // Check if it resolves to a real file
                    match m.resolve_read(&sandbox_path) {
                        Ok(hp) => hp.is_file(),
                        Err(_) => false,
                    }
                } else {
                    false
                };

                if !is_dir && !is_file {
                    // Check if it exists at all
                    if !m.exists(&sandbox_path) {
                        return Err(mlua::Error::external(crate::MountError::NotFound(
                            sandbox_path,
                        )));
                    }
                }

                // If path is a file, just return the filename
                if is_file {
                    let name = std::path::Path::new(&sandbox_path)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(&sandbox_path);
                    return Ok(format!("{}\n\n0 directories, 1 file", name));
                }

                // Walk the virtual directory tree using MountTable primitives
                struct TreeEntry {
                    rel_path: String,
                    is_dir: bool,
                    depth: usize,
                }

                let mut entries: Vec<TreeEntry> = Vec::new();

                // Recursive walk through the virtual filesystem
                fn walk_virtual(
                    m: &MountTable,
                    virtual_dir: &str,
                    root: &str,
                    depth: usize,
                    max_depth: usize,
                    entries: &mut Vec<TreeEntry>,
                ) {
                    if depth > max_depth {
                        return;
                    }
                    if let Some(children) = m.list_virtual_dir(virtual_dir) {
                        for child in &children {
                            let child_path = if virtual_dir == "/" {
                                format!("/{}", child)
                            } else {
                                format!("{}/{}", virtual_dir, child)
                            };
                            // Compute relative path from root
                            let rel = if root == "/" {
                                child_path[1..].to_string() // strip leading /
                            } else {
                                child_path[root.len() + 1..].to_string()
                            };
                            let child_is_dir = m.is_virtual_dir(&child_path)
                                || m.resolve(&child_path)
                                    .map(|(hp, _)| hp.is_dir())
                                    .unwrap_or(false);
                            entries.push(TreeEntry {
                                rel_path: rel,
                                is_dir: child_is_dir,
                                depth,
                            });
                            if child_is_dir {
                                walk_virtual(m, &child_path, root, depth + 1, max_depth, entries);
                            }
                        }
                    }
                }

                walk_virtual(&m, &sandbox_path, &sandbox_path, 1, max_depth, &mut entries);

                // Apply filters
                if dirs_only {
                    entries.retain(|e| e.is_dir);
                }

                if let Some(ref glob) = compiled_glob {
                    // Keep files matching glob and dirs that are ancestors of matches
                    entries.retain(|e| {
                        if !e.is_dir {
                            // Match against the file name component
                            let name = std::path::Path::new(&e.rel_path)
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or(&e.rel_path);
                            glob.is_match(name)
                        } else {
                            true // keep dirs initially, prune below
                        }
                    });
                    // Prune dirs with no matching file descendants
                    let ancestor_paths: std::collections::HashSet<String> = entries
                        .iter()
                        .filter(|e| !e.is_dir)
                        .flat_map(|e| {
                            let path = std::path::Path::new(&e.rel_path);
                            let mut ancestors = Vec::new();
                            let mut current = path.parent();
                            while let Some(p) = current {
                                if !p.as_os_str().is_empty() {
                                    ancestors.push(p.to_string_lossy().to_string());
                                }
                                current = p.parent();
                            }
                            ancestors
                        })
                        .collect();
                    entries.retain(|e| !e.is_dir || ancestor_paths.contains(&e.rel_path));
                }
                // Suppress unused variable warning when mod-ripgrep is disabled
                let _ = &compiled_glob;

                let dir_count = entries.iter().filter(|e| e.is_dir).count();
                let file_count = entries.iter().filter(|e| !e.is_dir).count();

                // Build ASCII tree
                let root_name = if sandbox_path == "/" {
                    "/"
                } else {
                    std::path::Path::new(&sandbox_path)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(&sandbox_path)
                };

                let mut output = format!(
                    "{}\n",
                    if sandbox_path == "/" {
                        "/".to_string()
                    } else {
                        format!("{}/", root_name)
                    }
                );

                for i in 0..entries.len() {
                    let entry = &entries[i];
                    let depth = entry.depth;

                    // Build prefix for ancestor levels
                    let mut prefix = String::new();
                    for level in 1..depth {
                        let ancestor: String = std::path::Path::new(&entry.rel_path)
                            .components()
                            .take(level)
                            .collect::<std::path::PathBuf>()
                            .to_string_lossy()
                            .to_string();
                        let ancestor_parent: String = std::path::Path::new(&ancestor)
                            .parent()
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_default();
                        // Check if a later entry is a sibling of this ancestor
                        let has_later_sibling = entries.iter().skip(i + 1).any(|e| {
                            if e.depth < level {
                                return false;
                            }
                            let e_at_level: String = std::path::Path::new(&e.rel_path)
                                .components()
                                .take(level)
                                .collect::<std::path::PathBuf>()
                                .to_string_lossy()
                                .to_string();
                            let e_parent: String = std::path::Path::new(&e_at_level)
                                .parent()
                                .map(|p| p.to_string_lossy().to_string())
                                .unwrap_or_default();
                            e_parent == ancestor_parent && e_at_level != ancestor
                        });
                        if has_later_sibling {
                            prefix.push_str("\u{2502}   ");
                        } else {
                            prefix.push_str("    ");
                        }
                    }

                    // Determine if this entry is the last sibling
                    let parent_path: String = std::path::Path::new(&entry.rel_path)
                        .parent()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_default();
                    let is_last = !entries.iter().skip(i + 1).any(|e| {
                        let e_parent = std::path::Path::new(&e.rel_path)
                            .parent()
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_default();
                        e_parent == parent_path
                    });

                    let connector = if is_last {
                        "\u{2514}\u{2500}\u{2500} "
                    } else {
                        "\u{251c}\u{2500}\u{2500} "
                    };
                    let name = std::path::Path::new(&entry.rel_path)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(&entry.rel_path);
                    let suffix = if entry.is_dir { "/" } else { "" };

                    output.push_str(&format!("{}{}{}{}\n", prefix, connector, name, suffix));
                }

                output.push_str(&format!(
                    "\n{} {}, {} {}",
                    dir_count,
                    if dir_count == 1 {
                        "directory"
                    } else {
                        "directories"
                    },
                    file_count,
                    if file_count == 1 { "file" } else { "files" },
                ));

                Ok(output)
            })?,
        )?;
    }

    #[cfg(not(feature = "mod-ripgrep"))]
    {
        let _ = (lua, fs, mounts);
    }

    Ok(())
}
