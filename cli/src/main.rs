//! Command-line interface and REPL entry point for CPSL.

mod config;

use clap::{Args, Parser, Subcommand};
use config::{SandboxConfig, MODULE_REGISTRY};
use cpsl_core::{sh_transpile, transpile};
use cpsl_core::{MountTable, Sandbox};
use rustyline::config::Configurer;
use rustyline::history::DefaultHistory;
use rustyline::{Cmd, Editor, EventHandler, KeyCode, KeyEvent, Modifiers};
use std::io::{BufRead, BufReader, IsTerminal, Write};
use std::path::PathBuf;
use std::time::Instant;

/// Resolved language mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Language {
    Bash,
    Python,
    Lua,
}

/// Determine the effective language from flags. Default is Bash.
fn resolve_language(_bash: bool, python: bool, lua: bool) -> Language {
    if python {
        Language::Python
    } else if lua {
        Language::Lua
    } else {
        // --bash or no flag → bash is the default
        Language::Bash
    }
}

#[cfg(feature = "mod-http")]
fn webview_pdf_rendering_allowed(allow_domains: &[String], deny_domains: &[String]) -> bool {
    deny_domains.is_empty() && allow_domains.iter().any(|domain| domain == "*")
}

#[derive(Parser)]
#[command(name = "cpsl", about = "Sandboxed runtime")]
#[command(args_conflicts_with_subcommands = true)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    #[command(flatten)]
    exec: ExecArgs,
}

#[derive(Args)]
struct ExecArgs {
    /// Start interactive REPL
    #[arg(short = 'i', long = "interactive")]
    interactive: bool,

    /// Bash mode (default): execute shell commands
    #[arg(long = "bash", group = "lang")]
    bash: bool,

    /// Python mode: execute Python code
    #[arg(long = "python", group = "lang")]
    python: bool,

    /// Lua mode: execute Lua directly
    #[arg(long = "lua", group = "lang")]
    lua: bool,

    /// Emit per-phase timing to stderr (key=value microseconds)
    #[arg(long = "bench")]
    bench: bool,

    /// Show generated code instead of executing (debug flag)
    #[arg(long = "emit-luau")]
    emit_luau: bool,

    /// Mount a host path into the sandbox: host:virtual[:ro]
    #[arg(short = 'v', long = "volume")]
    volumes: Vec<String>,

    /// Allow HTTP requests to this domain (repeatable)
    #[cfg(feature = "mod-http")]
    #[arg(long = "allow-domain")]
    allow_domain: Vec<String>,

    /// Deny HTTP requests to this domain (repeatable)
    #[cfg(feature = "mod-http")]
    #[arg(long = "deny-domain")]
    deny_domain: Vec<String>,

    /// Script file to execute
    script: Option<String>,

    /// Inline script (after --)
    #[arg(last = true)]
    inline: Vec<String>,
}

#[derive(Subcommand)]
enum Command {
    /// Build a sandbox from a cpsl.toml config
    Build(BuildArgs),
    /// Run a previously built sandbox
    Run(RunArgs),
    /// List available modules and their descriptions
    Modules,
    /// List all built sandboxes
    #[command(alias = "ls")]
    Sandboxes,
    /// Remove a built sandbox
    Rm(RmArgs),
}

#[derive(Args)]
struct RmArgs {
    /// Name of the sandbox to remove
    name: String,

    /// Skip confirmation prompt
    #[arg(short = 'f', long = "force")]
    force: bool,
}

#[derive(Args)]
struct BuildArgs {
    /// Path to config toml (default: ./cpsl.toml)
    #[arg(value_name = "CONFIG")]
    config: Option<String>,

    /// Path to config toml
    #[arg(short = 'f', long = "file", value_name = "CONFIG")]
    file: Option<String>,

    /// Sandbox image name (default: [sandbox].name from the config)
    #[arg(short = 't', long = "tag")]
    name: Option<String>,

    /// Build as library (.rlib/.a) instead of binary
    #[arg(long = "lib")]
    lib_mode: bool,

    /// Show raw cargo output for debugging
    #[arg(short = 'V', long = "verbose")]
    verbose: bool,
}

#[derive(Args)]
struct RunArgs {
    /// Name of the sandbox image to run
    name: String,

    /// Start interactive REPL
    #[arg(short = 'i', long = "interactive")]
    interactive: bool,

    /// Bash mode (default): execute shell commands
    #[arg(long = "bash", group = "lang")]
    bash: bool,

    /// Python mode: execute Python code
    #[arg(long = "python", group = "lang")]
    python: bool,

    /// Lua mode: execute Lua directly
    #[arg(long = "lua", group = "lang")]
    lua: bool,

    /// Mount a host path into the sandbox: host:virtual[:ro]
    #[arg(short = 'v', long = "volume")]
    volumes: Vec<String>,

    /// Allow HTTP requests to this domain (repeatable, merged with image config)
    #[cfg(feature = "mod-http")]
    #[arg(long = "allow-domain")]
    allow_domain: Vec<String>,

    /// Deny HTTP requests to this domain (repeatable, merged with image config)
    #[cfg(feature = "mod-http")]
    #[arg(long = "deny-domain")]
    deny_domain: Vec<String>,

    /// Script file to execute
    script: Option<String>,

    /// Inline code (after --)
    #[arg(last = true)]
    inline: Vec<String>,
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Some(Command::Build(args)) => cmd_build(args),
        Some(Command::Run(args)) => cmd_run(args),
        Some(Command::Modules) => cmd_modules(),
        Some(Command::Sandboxes) => cmd_sandboxes(),
        Some(Command::Rm(args)) => cmd_rm(args),
        None => cmd_exec(cli.exec),
    }
}

// ---------------------------------------------------------------------------
// Subcommand: build
// ---------------------------------------------------------------------------

fn cmd_build(args: BuildArgs) {
    if args.config.is_some() && args.file.is_some() {
        eprintln!("error: pass the config path either positionally or with -f/--file, not both");
        std::process::exit(1);
    }

    let config_path = PathBuf::from(
        args.file
            .or(args.config)
            .unwrap_or_else(|| "cpsl.toml".to_string()),
    );
    let config = SandboxConfig::from_file(&config_path).unwrap_or_else(|e| {
        eprintln!("error: {}", e);
        std::process::exit(1);
    });

    let image_name = args.name.unwrap_or_else(|| config.sandbox.name.clone());

    let features = config.to_cargo_features();
    let module_names = config.module_display_names();
    let python_suffix = if config.python_enabled() {
        " + python"
    } else {
        ""
    };

    // Locate the cpsl workspace root (where core and cli live)
    let workspace_root = find_workspace_root().unwrap_or_else(|| {
        eprintln!("error: cannot find cpsl workspace root (looked for core/Cargo.toml)");
        std::process::exit(1);
    });

    let package = if args.lib_mode {
        "cpsl-core"
    } else {
        "cpsl-cli"
    };

    let features_arg = if features.is_empty() {
        String::new()
    } else {
        features.join(",")
    };

    let mut cmd = std::process::Command::new("cargo");
    cmd.arg("build")
        .arg("--release")
        .arg("-p")
        .arg(package)
        .arg("--no-default-features");

    if !features_arg.is_empty() {
        cmd.arg("--features").arg(&features_arg);
    }

    cmd.current_dir(&workspace_root);

    if args.verbose {
        // Verbose mode: inherit stdio directly (raw cargo output)
        let status = cmd.status().unwrap_or_else(|e| {
            eprintln!("error: failed to run cargo: {}", e);
            std::process::exit(1);
        });

        if !status.success() {
            eprintln!("error: cargo build failed");
            std::process::exit(1);
        }
    } else {
        // Curated mode: capture stderr, parse progress, filter noise
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let mut child = cmd.spawn().unwrap_or_else(|e| {
            eprintln!("error: failed to run cargo: {}", e);
            std::process::exit(1);
        });

        let stderr = child.stderr.take().expect("captured stderr");
        let reader = BufReader::new(stderr);

        let is_tty = std::io::stderr().is_terminal();
        let mut compiled_count: usize = 0;
        let mut total_crates: Option<usize> = None;
        let mut error_lines: Vec<String> = Vec::new();
        let mut in_error_block = false;

        for line in reader.lines() {
            let line = match line {
                Ok(l) => l,
                Err(_) => continue,
            };

            // Track compilation progress
            if line.contains("Compiling") {
                compiled_count += 1;
                // Extract crate name: "   Compiling serde v1.0.0"
                let crate_name = line
                    .trim()
                    .strip_prefix("Compiling ")
                    .and_then(|s| s.split_whitespace().next())
                    .unwrap_or("...");

                if is_tty {
                    let progress = match total_crates {
                        Some(total) => format!("[{}/{}]", compiled_count, total),
                        None => format!("[{}]", compiled_count),
                    };
                    eprint!("\r\x1b[2K  Compiling... {} {}", progress, crate_name);
                    std::io::stderr().flush().ok();
                }
            } else if line.contains("Downloading") && line.contains("crates") {
                // "Downloading 42 crates..." — use as total estimate
                if let Some(n) = line
                    .split_whitespace()
                    .find_map(|w| w.parse::<usize>().ok())
                {
                    // Downloading count is a rough proxy; compilation count may differ
                    // but it's the best we have before compilation starts
                    if total_crates.is_none() {
                        total_crates = Some(n);
                    }
                }
            } else if line.starts_with("error") || line.starts_with("Error") {
                in_error_block = true;
                error_lines.push(line);
            } else if in_error_block {
                // Collect continuation lines of error blocks
                if line.starts_with(' ') || line.starts_with("  -->") || line.contains(" | ") {
                    error_lines.push(line);
                } else if line.is_empty() {
                    error_lines.push(String::new());
                } else {
                    in_error_block = false;
                }
            }
            // Silently drop: warnings, notes, "Finished", "Downloading", etc.
        }

        // Clear the progress line
        if is_tty && compiled_count > 0 {
            eprint!("\r\x1b[2K");
            std::io::stderr().flush().ok();
        }

        let status = child.wait().unwrap_or_else(|e| {
            eprintln!("error: waiting for cargo: {}", e);
            std::process::exit(1);
        });

        if !status.success() {
            if error_lines.is_empty() {
                eprintln!("error: cargo build failed");
            } else {
                for line in &error_lines {
                    eprintln!("{}", line);
                }
            }
            std::process::exit(1);
        }
    }

    // --- Post-build: install artifact and print summary ---

    if args.lib_mode {
        eprintln!(
            "\x1b[32m✓\x1b[0m Library '{}' ready ({} modules{}: {})",
            image_name,
            module_names.len(),
            python_suffix,
            module_names.join(", ")
        );
    } else {
        // Copy binary to ~/.cpsl/bin/<name>
        let bin_dir = dirs::home_dir()
            .expect("cannot determine home directory")
            .join(".cpsl")
            .join("bin");
        std::fs::create_dir_all(&bin_dir).unwrap_or_else(|e| {
            eprintln!("error: cannot create {}: {}", bin_dir.display(), e);
            std::process::exit(1);
        });

        let src_binary = workspace_root
            .join("target")
            .join("release")
            .join("cpsl-cli");
        let dst_binary = bin_dir.join(&image_name);
        std::fs::copy(&src_binary, &dst_binary).unwrap_or_else(|e| {
            eprintln!(
                "error: cannot copy {} -> {}: {}",
                src_binary.display(),
                dst_binary.display(),
                e
            );
            std::process::exit(1);
        });

        // Re-sign on macOS: cargo's linker-signed adhoc signature is invalidated
        // by the copy and the kernel will SIGKILL the binary on first run.
        #[cfg(target_os = "macos")]
        {
            let _ = std::process::Command::new("codesign")
                .args(["--force", "--sign", "-"])
                .arg(&dst_binary)
                .output();
        }

        // Save config alongside for `cpsl run` to reference
        let images_dir = dirs::home_dir()
            .expect("cannot determine home directory")
            .join(".cpsl")
            .join("images");
        std::fs::create_dir_all(&images_dir).unwrap_or_else(|e| {
            eprintln!("error: cannot create {}: {}", images_dir.display(), e);
            std::process::exit(1);
        });
        let config_content = std::fs::read_to_string(&config_path).unwrap_or_else(|e| {
            eprintln!("error: cannot re-read config: {}", e);
            std::process::exit(1);
        });
        let image_config = images_dir.join(format!("{}.toml", image_name));
        std::fs::write(&image_config, config_content).unwrap_or_else(|e| {
            eprintln!("error: cannot write {}: {}", image_config.display(), e);
            std::process::exit(1);
        });

        eprintln!(
            "\x1b[32m✓\x1b[0m Sandbox '{}' ready ({} modules{}: {})",
            image_name,
            module_names.len(),
            python_suffix,
            module_names.join(", ")
        );
    }
}

/// Walk up from the executable to find the workspace root containing core.
fn find_workspace_root() -> Option<PathBuf> {
    // Try relative to executable
    if let Ok(exe) = std::env::current_exe() {
        let mut dir = exe.parent().map(|p| p.to_path_buf());
        for _ in 0..10 {
            if let Some(ref d) = dir {
                if d.join("core").join("Cargo.toml").exists() {
                    return Some(d.clone());
                }
                dir = d.parent().map(|p| p.to_path_buf());
            }
        }
    }

    // Try relative to CWD
    for candidate in &[".", "stash/cpsl", "../cpsl", "../.."] {
        let p = PathBuf::from(candidate);
        if p.join("core").join("Cargo.toml").exists() {
            return Some(std::fs::canonicalize(&p).unwrap_or(p));
        }
    }

    None
}

// ---------------------------------------------------------------------------
// Subcommand: run
// ---------------------------------------------------------------------------

fn cmd_run(args: RunArgs) {
    let bin_path = dirs::home_dir()
        .expect("cannot determine home directory")
        .join(".cpsl")
        .join("bin")
        .join(&args.name);

    if !bin_path.exists() {
        eprintln!("error: sandbox '{}' not found", args.name);

        // List available images
        let bin_dir = dirs::home_dir()
            .expect("cannot determine home directory")
            .join(".cpsl")
            .join("bin");
        if bin_dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&bin_dir) {
                let names: Vec<String> = entries
                    .flatten()
                    .filter_map(|e| e.file_name().into_string().ok())
                    .collect();
                if !names.is_empty() {
                    eprintln!("Available sandboxes: {}", names.join(", "));
                }
            }
        }
        eprintln!(
            "Hint: build a sandbox first with `cpsl build -t {}`",
            args.name
        );
        std::process::exit(1);
    }

    let image_config = dirs::home_dir()
        .expect("cannot determine home directory")
        .join(".cpsl")
        .join("images")
        .join(format!("{}.toml", args.name));
    let image_config = if image_config.exists() {
        Some(SandboxConfig::from_file(&image_config).unwrap_or_else(|e| {
            eprintln!(
                "error: invalid saved config for sandbox '{}': {}",
                args.name, e
            );
            std::process::exit(1);
        }))
    } else {
        None
    };

    let mut cmd = std::process::Command::new(&bin_path);

    if args.interactive {
        cmd.arg("-i");
    }
    if args.python {
        cmd.arg("--python");
    } else if args.lua {
        cmd.arg("--lua");
    } else {
        cmd.arg("--bash");
    }
    if let Some(config) = &image_config {
        for vol in config.mount_volumes() {
            cmd.arg("-v").arg(vol);
        }
    }
    for vol in &args.volumes {
        cmd.arg("-v").arg(vol);
    }
    #[cfg(feature = "mod-http")]
    {
        let (cfg_allow, cfg_deny) = image_config
            .as_ref()
            .map(|cfg| cfg.http_gateway_config())
            .unwrap_or_default();
        for d in cfg_allow.iter().chain(args.allow_domain.iter()) {
            cmd.arg("--allow-domain").arg(d);
        }
        for d in cfg_deny.iter().chain(args.deny_domain.iter()) {
            cmd.arg("--deny-domain").arg(d);
        }
    }
    if let Some(ref script) = args.script {
        cmd.arg(script);
    }
    if !args.inline.is_empty() {
        cmd.arg("--");
        for code in &args.inline {
            cmd.arg(code);
        }
    }

    let status = cmd.status().unwrap_or_else(|e| {
        eprintln!("error: failed to run {}: {}", bin_path.display(), e);
        std::process::exit(1);
    });

    std::process::exit(status.code().unwrap_or(1));
}

// ---------------------------------------------------------------------------
// Subcommand: modules
// ---------------------------------------------------------------------------

fn cmd_modules() {
    println!("Available modules:\n");
    for m in MODULE_REGISTRY {
        println!("  {:<12} {}", m.name, m.description);
    }
    println!(
        "  {:<12} Content search via fs.grep; providers: ripgrep, fff; modes: regex (default), plain; requires fs",
        "grep"
    );
    println!("\nUse these names in [modules] section of cpsl.toml:");
    println!("  [modules]");
    println!("  fs = true");
    println!("  grep = {{ provider = \"ripgrep\" }}");
    println!("  json = true");
    println!("  csv = true");
}

// ---------------------------------------------------------------------------
// Subcommand: sandboxes (ls)
// ---------------------------------------------------------------------------

fn cmd_sandboxes() {
    let home = dirs::home_dir().expect("cannot determine home directory");
    let bin_dir = home.join(".cpsl").join("bin");
    let images_dir = home.join(".cpsl").join("images");

    let mut entries: Vec<(String, Vec<String>, u64, Option<std::time::SystemTime>)> = Vec::new();

    if bin_dir.is_dir() {
        if let Ok(dir) = std::fs::read_dir(&bin_dir) {
            for entry in dir.flatten() {
                let name = match entry.file_name().into_string() {
                    Ok(n) if !n.starts_with('.') => n,
                    _ => continue,
                };

                let metadata = entry.metadata().ok();
                let size = metadata.as_ref().map(|m| m.len()).unwrap_or(0);
                let modified = metadata.and_then(|m| m.modified().ok());

                let modules: Vec<String> = {
                    let config_path = images_dir.join(format!("{}.toml", name));
                    if config_path.exists() {
                        match SandboxConfig::from_file(&config_path) {
                            Ok(config) => config.module_display_names(),
                            Err(error) => vec![format!("invalid config: {error}")],
                        }
                    } else {
                        vec![]
                    }
                };

                entries.push((name, modules, size, modified));
            }
        }
    }

    if entries.is_empty() {
        println!("No sandboxes built yet.");
        println!("Hint: build one with `cpsl build -t <name>`");
        return;
    }

    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let name_width = entries.iter().map(|e| e.0.len()).max().unwrap_or(4).max(4);

    println!(
        "{:<nw$}  {:<40}  {:>10}  {}",
        "NAME",
        "MODULES",
        "SIZE",
        "BUILT",
        nw = name_width
    );

    for (name, modules, size, modified) in &entries {
        let modules_str = if modules.is_empty() {
            "(no config)".to_string()
        } else {
            let joined = modules.join(", ");
            if joined.len() > 38 {
                format!("{}...", &joined[..35])
            } else {
                joined
            }
        };

        let size_str = format_size(*size);
        let date_str = modified
            .map(|t| format_date(t))
            .unwrap_or_else(|| "unknown".to_string());

        println!(
            "{:<nw$}  {:<40}  {:>10}  {}",
            name,
            modules_str,
            size_str,
            date_str,
            nw = name_width
        );
    }
}

// ---------------------------------------------------------------------------
// Subcommand: rm
// ---------------------------------------------------------------------------

fn cmd_rm(args: RmArgs) {
    let home = dirs::home_dir().expect("cannot determine home directory");
    let bin_path = home.join(".cpsl").join("bin").join(&args.name);
    let config_path = home
        .join(".cpsl")
        .join("images")
        .join(format!("{}.toml", args.name));

    if !bin_path.exists() && !config_path.exists() {
        eprintln!("error: sandbox '{}' does not exist", args.name);

        // List available sandboxes
        let bin_dir = home.join(".cpsl").join("bin");
        if bin_dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&bin_dir) {
                let names: Vec<String> = entries
                    .flatten()
                    .filter_map(|e| e.file_name().into_string().ok())
                    .filter(|n| !n.starts_with('.'))
                    .collect();
                if !names.is_empty() {
                    eprintln!("Available sandboxes: {}", names.join(", "));
                }
            }
        }
        std::process::exit(1);
    }

    if !args.force {
        eprint!("Remove sandbox '{}'? [y/N] ", args.name);
        std::io::stderr().flush().ok();
        let mut answer = String::new();
        if std::io::stdin().read_line(&mut answer).is_err()
            || !answer.trim().eq_ignore_ascii_case("y")
        {
            eprintln!("Aborted.");
            std::process::exit(1);
        }
    }

    let mut removed = false;
    if bin_path.exists() {
        std::fs::remove_file(&bin_path).unwrap_or_else(|e| {
            eprintln!("error: cannot remove {}: {}", bin_path.display(), e);
            std::process::exit(1);
        });
        removed = true;
    }
    if config_path.exists() {
        std::fs::remove_file(&config_path).unwrap_or_else(|e| {
            eprintln!("error: cannot remove {}: {}", config_path.display(), e);
            std::process::exit(1);
        });
        removed = true;
    }

    if removed {
        eprintln!("Removed sandbox '{}'.", args.name);
    }
}

fn format_size(bytes: u64) -> String {
    if bytes >= 1_000_000_000 {
        format!("{:.1} GB", bytes as f64 / 1_000_000_000.0)
    } else if bytes >= 1_000_000 {
        format!("{:.1} MB", bytes as f64 / 1_000_000.0)
    } else if bytes >= 1_000 {
        format!("{:.0} KB", bytes as f64 / 1_000.0)
    } else {
        format!("{} B", bytes)
    }
}

/// Convert SystemTime to YYYY-MM-DD string (Howard Hinnant's civil_from_days algorithm).
fn format_date(time: std::time::SystemTime) -> String {
    let secs = time
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let days = secs.div_euclid(86400);
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{:04}-{:02}-{:02}", y, m, d)
}

// ---------------------------------------------------------------------------
// Default command: exec (original CLI behavior)
// ---------------------------------------------------------------------------

fn cmd_exec(cli: ExecArgs) {
    let t_start = Instant::now();
    let bench = cli.bench;
    let emit_luau = cli.emit_luau;
    let lang = resolve_language(cli.bash, cli.python, cli.lua);

    let mut mounts = MountTable::new();
    for spec in &cli.volumes {
        if let Err(e) = mounts.parse_and_add(spec) {
            eprintln!("error: invalid mount: {}", e);
            std::process::exit(1);
        }
    }

    // If no user volume covers "/", create an ephemeral directory as the root mount.
    // The EphemeralDir guard removes the directory when dropped (session exit).
    let _ephemeral_guard = if mounts.mount_key_for("/").is_none() {
        let dir = create_ephemeral_dir();
        let spec = format!("{}:/", dir.display());
        if let Err(e) = mounts.parse_and_add(&spec) {
            eprintln!("error: cannot mount ephemeral root: {}", e);
            std::process::exit(1);
        }
        Some(EphemeralDir(dir))
    } else {
        None
    };

    let builder = Sandbox::builder().mounts(mounts);
    #[cfg(feature = "mod-http")]
    let builder = {
        let mut builder = builder.allow_webview_pdf_rendering(webview_pdf_rendering_allowed(
            &cli.allow_domain,
            &cli.deny_domain,
        ));
        if !cli.allow_domain.is_empty() || !cli.deny_domain.is_empty() {
            let mut gw = cpsl_core::HttpGateway::builder();
            for d in &cli.allow_domain {
                gw = gw.allow_domain(d);
            }
            for d in &cli.deny_domain {
                gw = gw.deny_domain(d);
            }
            builder = builder.http_gateway(std::sync::Arc::new(gw.build()));
        }
        builder
    };
    let sandbox = builder.build().unwrap_or_else(|e| {
        eprintln!("error: failed to create sandbox: {}", e);
        std::process::exit(1);
    });

    let t_sandbox = Instant::now();

    // Set up language-specific runtimes
    match lang {
        Language::Python => {
            let pyrt_source = find_pyrt().unwrap_or_else(|| {
                eprintln!("error: cannot find runtime/pyrt.luau — run from the cpsl directory");
                std::process::exit(1);
            });
            sandbox
                .setup_python_runtime(&pyrt_source)
                .unwrap_or_else(|e| {
                    eprintln!("error: failed to load Python runtime: {}", e);
                    std::process::exit(1);
                });
            for (name, source) in find_stdlib() {
                sandbox.register_module(&name, &source).unwrap_or_else(|e| {
                    eprintln!("warning: failed to load stdlib module '{}': {}", name, e);
                });
            }
        }
        Language::Bash => {
            let shrt_source = find_shrt().unwrap_or_else(|| {
                eprintln!("error: cannot find runtime/shrt.luau — run from the cpsl directory");
                std::process::exit(1);
            });
            sandbox
                .setup_shell_runtime(&shrt_source)
                .unwrap_or_else(|e| {
                    eprintln!("error: failed to load shell runtime: {}", e);
                    std::process::exit(1);
                });
        }
        Language::Lua => {} // Raw Luau — no runtime needed
    }

    let t_ready = Instant::now();

    if bench {
        let startup_us = t_ready.duration_since(t_start).as_micros();
        let sandbox_us = t_sandbox.duration_since(t_start).as_micros();
        let rt_us = t_ready.duration_since(t_sandbox).as_micros();
        eprintln!("bench:startup_us={}", startup_us);
        eprintln!("bench:sandbox_us={}", sandbox_us);
        eprintln!("bench:runtime_us={}", rt_us);
    }

    // Emit transpiled Luau and exit (works for both python and bash)
    if emit_luau && lang != Language::Lua {
        let source = if !cli.inline.is_empty() {
            cli.inline.join(" ")
        } else if let Some(ref path) = cli.script {
            std::fs::read_to_string(path).unwrap_or_else(|e| {
                eprintln!("error: cannot read {}: {}", path, e);
                std::process::exit(1);
            })
        } else {
            eprintln!("error: --emit-luau requires a script or inline code");
            std::process::exit(1);
        };
        let result = match lang {
            Language::Python => transpile::transpile(&source),
            Language::Bash => sh_transpile::transpile_sh(&source),
            Language::Lua => unreachable!(),
        };
        match result {
            Ok(result) => {
                for w in &result.warnings {
                    eprintln!("\x1b[33mwarning: {}\x1b[0m", w);
                }
                println!("{}", result.luau_source);
            }
            Err(e) => {
                eprintln!("\x1b[31m{}\x1b[0m", e);
                std::process::exit(1);
            }
        }
        return;
    }

    // Handle inline script (arguments after --)
    if !cli.inline.is_empty() {
        let code = cli.inline.join(" ");
        match lang {
            Language::Python => run_python(&sandbox, &code, bench),
            Language::Bash => run_bash(&sandbox, &code, bench),
            Language::Lua => run_code(&sandbox, &code, bench),
        }
        return;
    }

    if let Some(script_path) = &cli.script {
        let code = std::fs::read_to_string(script_path).unwrap_or_else(|e| {
            eprintln!("error: cannot read {}: {}", script_path, e);
            std::process::exit(1);
        });
        match lang {
            Language::Python => run_python(&sandbox, &code, bench),
            Language::Bash => run_bash(&sandbox, &code, bench),
            Language::Lua => run_code(&sandbox, &code, bench),
        }
        if cli.interactive {
            match lang {
                Language::Python => python_repl(&sandbox),
                Language::Bash => bash_repl(&sandbox),
                Language::Lua => repl(&sandbox),
            }
        }
    } else if cli.interactive {
        match lang {
            Language::Python => python_repl(&sandbox),
            Language::Bash => bash_repl(&sandbox),
            Language::Lua => repl(&sandbox),
        }
    } else {
        eprintln!(
            "error: provide a script file, use -i for interactive mode, or pass code after --"
        );
        std::process::exit(1);
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// An ephemeral directory that is removed when dropped.
struct EphemeralDir(PathBuf);

impl Drop for EphemeralDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Create an ephemeral scratch directory under `~/.cpsl/ephemeral/`.
fn create_ephemeral_dir() -> PathBuf {
    let base = dirs::home_dir()
        .expect("cannot determine home directory")
        .join(".cpsl")
        .join("ephemeral");
    std::fs::create_dir_all(&base).unwrap_or_else(|e| {
        eprintln!("error: cannot create {}: {}", base.display(), e);
        std::process::exit(1);
    });
    // Use PID + timestamp for uniqueness
    let name = format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    );
    let dir = base.join(name);
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| {
        eprintln!("error: cannot create {}: {}", dir.display(), e);
        std::process::exit(1);
    });
    dir
}

fn history_path(lang: Language) -> Option<PathBuf> {
    let name = match lang {
        Language::Python => ".cpsl_py_history",
        Language::Bash => ".cpsl_sh_history",
        Language::Lua => ".cpsl_history",
    };
    dirs::home_dir().map(|h| h.join(name))
}

/// Find runtime/pyrt.luau relative to the executable or CWD.
fn find_pyrt() -> Option<String> {
    // Try relative to the executable
    if let Ok(exe) = std::env::current_exe() {
        let mut dir = exe.parent().map(|p| p.to_path_buf());
        for _ in 0..5 {
            if let Some(ref d) = dir {
                let candidate = d.join("runtime").join("pyrt.luau");
                if candidate.exists() {
                    return std::fs::read_to_string(&candidate).ok();
                }
                dir = d.parent().map(|p| p.to_path_buf());
            }
        }
    }

    // Try relative to CWD
    for candidate in &["runtime/pyrt.luau", "stash/cpsl/runtime/pyrt.luau"] {
        if let Ok(content) = std::fs::read_to_string(candidate) {
            return Some(content);
        }
    }

    None
}

/// Find runtime/shrt.luau relative to the executable or CWD.
fn find_shrt() -> Option<String> {
    // Try relative to the executable
    if let Ok(exe) = std::env::current_exe() {
        let mut dir = exe.parent().map(|p| p.to_path_buf());
        for _ in 0..5 {
            if let Some(ref d) = dir {
                let candidate = d.join("runtime").join("shrt.luau");
                if candidate.exists() {
                    return std::fs::read_to_string(&candidate).ok();
                }
                dir = d.parent().map(|p| p.to_path_buf());
            }
        }
    }

    // Try relative to CWD
    for candidate in &["runtime/shrt.luau", "stash/cpsl/runtime/shrt.luau"] {
        if let Ok(content) = std::fs::read_to_string(candidate) {
            return Some(content);
        }
    }

    None
}

/// Find stdlib/*.luau module shims relative to the executable or CWD.
fn find_stdlib() -> Vec<(String, String)> {
    let mut modules = Vec::new();

    let search_dirs: Vec<PathBuf> = {
        let mut dirs = Vec::new();
        // Try relative to the executable
        if let Ok(exe) = std::env::current_exe() {
            let mut dir = exe.parent().map(|p| p.to_path_buf());
            for _ in 0..5 {
                if let Some(ref d) = dir {
                    let stdlib = d.join("stdlib");
                    if stdlib.is_dir() {
                        dirs.push(stdlib);
                        break;
                    }
                    dir = d.parent().map(|p| p.to_path_buf());
                }
            }
        }
        // Try relative to CWD
        for candidate in &["stdlib", "stash/cpsl/stdlib"] {
            let p = PathBuf::from(candidate);
            if p.is_dir() {
                dirs.push(p);
                break;
            }
        }
        dirs
    };

    for dir in search_dirs {
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "luau") {
                    if let Some(stem) = path.file_stem() {
                        if let Ok(source) = std::fs::read_to_string(&path) {
                            modules.push((stem.to_string_lossy().to_string(), source));
                        }
                    }
                }
            }
        }
        break; // Use the first stdlib dir found
    }

    modules
}

fn run_code(sandbox: &Sandbox, code: &str, bench: bool) {
    let t0 = Instant::now();
    match sandbox.exec(code) {
        Ok(output) => {
            let exec_us = t0.elapsed().as_micros();
            if !output.is_empty() {
                println!("{}", output);
            }
            if bench {
                eprintln!("bench:exec_us={}", exec_us);
            }
        }
        Err(e) => {
            eprintln!("\x1b[31m{}\x1b[0m", e);
            std::process::exit(1);
        }
    }
}

fn run_python(sandbox: &Sandbox, python_source: &str, bench: bool) {
    let t0 = Instant::now();
    match transpile::transpile(python_source) {
        Ok(result) => {
            let transpile_us = t0.elapsed().as_micros();

            for w in &result.warnings {
                eprintln!("\x1b[33mwarning: {}\x1b[0m", w);
            }

            let t1 = Instant::now();
            match sandbox.exec(&result.luau_source) {
                Ok(output) => {
                    let exec_us = t1.elapsed().as_micros();
                    if !output.is_empty() {
                        println!("{}", output);
                    }
                    if bench {
                        eprintln!("bench:transpile_us={}", transpile_us);
                        eprintln!("bench:exec_us={}", exec_us);
                    }
                }
                Err(e) => {
                    let translated =
                        transpile::translate_error(&e, &result.source_map, python_source);
                    eprintln!("\x1b[31m{}\x1b[0m", translated);
                    std::process::exit(1);
                }
            }
        }
        Err(e) => {
            eprintln!("\x1b[31m{}\x1b[0m", e);
            std::process::exit(1);
        }
    }
}

fn run_python_repl_line(sandbox: &Sandbox, python_source: &str) -> Result<(), String> {
    let result = transpile::transpile(python_source)?;

    for w in &result.warnings {
        eprintln!("\x1b[33mwarning: {}\x1b[0m", w);
    }

    match sandbox.exec(&result.luau_source) {
        Ok(output) => {
            if !output.is_empty() {
                println!("{}", output);
            }
            Ok(())
        }
        Err(e) => {
            let translated = transpile::translate_error(&e, &result.source_map, python_source);
            Err(translated)
        }
    }
}

fn run_bash(sandbox: &Sandbox, shell_source: &str, bench: bool) {
    let t0 = Instant::now();
    match sh_transpile::transpile_sh(shell_source) {
        Ok(result) => {
            let transpile_us = t0.elapsed().as_micros();

            for w in &result.warnings {
                eprintln!("\x1b[33mwarning: {}\x1b[0m", w);
            }

            let t1 = Instant::now();
            match sandbox.exec(&result.luau_source) {
                Ok(output) => {
                    let exec_us = t1.elapsed().as_micros();
                    if !output.is_empty() {
                        println!("{}", output);
                    }
                    if bench {
                        eprintln!("bench:transpile_us={}", transpile_us);
                        eprintln!("bench:exec_us={}", exec_us);
                    }
                }
                Err(e) => {
                    eprintln!("\x1b[31m{}\x1b[0m", e);
                    std::process::exit(1);
                }
            }
        }
        Err(e) => {
            eprintln!("\x1b[31m{}\x1b[0m", e);
            std::process::exit(1);
        }
    }
}

fn run_bash_repl_line(sandbox: &Sandbox, shell_source: &str) -> Result<(), String> {
    let result = sh_transpile::transpile_sh(shell_source)?;

    for w in &result.warnings {
        eprintln!("\x1b[33mwarning: {}\x1b[0m", w);
    }

    match sandbox.exec(&result.luau_source) {
        Ok(output) => {
            if !output.is_empty() {
                println!("{}", output);
            }
            Ok(())
        }
        Err(e) => Err(e.to_string()),
    }
}

fn bash_repl(sandbox: &Sandbox) {
    let mut rl: Editor<(), DefaultHistory> = Editor::new().expect("failed to create editor");

    rl.bind_sequence(
        KeyEvent(KeyCode::Char('k'), Modifiers::CTRL),
        EventHandler::Simple(Cmd::ClearScreen),
    );

    if let Some(path) = history_path(Language::Bash) {
        let _ = rl.load_history(&path);
    }

    rl.set_max_history_size(10_000).ok();

    println!("\x1b[32m~ bash mode ~\x1b[0m");

    loop {
        let prompt = "\x1b[32m$\x1b[0m ";

        match rl.readline(prompt) {
            Ok(line) => {
                if line.trim().is_empty() {
                    continue;
                }

                match run_bash_repl_line(sandbox, &line) {
                    Ok(()) => {}
                    Err(e) => eprintln!("\x1b[31m{}\x1b[0m", e),
                }
                rl.add_history_entry(&line).ok();
            }
            Err(rustyline::error::ReadlineError::Interrupted) => {}
            Err(rustyline::error::ReadlineError::Eof) => break,
            Err(e) => {
                eprintln!("error: {}", e);
                break;
            }
        }
    }

    if let Some(path) = history_path(Language::Bash) {
        let _ = rl.save_history(&path);
    }

    println!();
}

fn repl(sandbox: &Sandbox) {
    let mut rl: Editor<(), DefaultHistory> = Editor::new().expect("failed to create editor");

    // Cmd-K clears the screen
    rl.bind_sequence(
        KeyEvent(KeyCode::Char('k'), Modifiers::CTRL),
        EventHandler::Simple(Cmd::ClearScreen),
    );

    // Load persistent history
    if let Some(path) = history_path(Language::Lua) {
        let _ = rl.load_history(&path);
    }

    rl.set_max_history_size(10_000).ok();

    println!("\x1b[33m~ lua mode ~\x1b[0m");

    let mut buffer = String::new();
    let mut in_multiline = false;

    loop {
        let prompt = if in_multiline {
            "\x1b[33m>>\x1b[0m "
        } else {
            "\x1b[32m>\x1b[0m "
        };

        match rl.readline(prompt) {
            Ok(line) => {
                if line.trim().is_empty() && !in_multiline {
                    continue;
                }

                if in_multiline {
                    buffer.push('\n');
                    buffer.push_str(&line);
                } else {
                    buffer = line;
                }

                // Try to execute — if it fails with an incomplete statement error, enter multi-line mode
                match sandbox.exec(&buffer) {
                    Ok(output) => {
                        if !output.is_empty() {
                            println!("{}", output);
                        }
                        rl.add_history_entry(&buffer).ok();
                        in_multiline = false;
                        buffer.clear();
                    }
                    Err(e) => {
                        let err_str = e.to_string();
                        if is_incomplete_error(&err_str) {
                            in_multiline = true;
                            continue;
                        }
                        eprintln!("\x1b[31m{}\x1b[0m", err_str);
                        rl.add_history_entry(&buffer).ok();
                        in_multiline = false;
                        buffer.clear();
                    }
                }
            }
            Err(rustyline::error::ReadlineError::Interrupted) => {
                // Ctrl-C: cancel current input
                if in_multiline {
                    in_multiline = false;
                    buffer.clear();
                    continue;
                }
            }
            Err(rustyline::error::ReadlineError::Eof) => {
                // Ctrl-D: exit
                break;
            }
            Err(e) => {
                eprintln!("error: {}", e);
                break;
            }
        }
    }

    // Save history on exit
    if let Some(path) = history_path(Language::Lua) {
        let _ = rl.save_history(&path);
    }

    println!();
}

fn python_repl(sandbox: &Sandbox) {
    let mut rl: Editor<(), DefaultHistory> = Editor::new().expect("failed to create editor");

    rl.bind_sequence(
        KeyEvent(KeyCode::Char('k'), Modifiers::CTRL),
        EventHandler::Simple(Cmd::ClearScreen),
    );

    if let Some(path) = history_path(Language::Python) {
        let _ = rl.load_history(&path);
    }

    rl.set_max_history_size(10_000).ok();

    let mut buffer = String::new();
    let mut in_multiline = false;

    println!("\x1b[36m~ python mode ~\x1b[0m");

    loop {
        let prompt = if in_multiline {
            "\x1b[33m...\x1b[0m "
        } else {
            "\x1b[36m>>>\x1b[0m "
        };

        match rl.readline(prompt) {
            Ok(line) => {
                if line.trim().is_empty() && in_multiline {
                    // Empty line in multiline = execute the block
                    match run_python_repl_line(sandbox, &buffer) {
                        Ok(()) => {}
                        Err(e) => eprintln!("\x1b[31m{}\x1b[0m", e),
                    }
                    rl.add_history_entry(&buffer).ok();
                    in_multiline = false;
                    buffer.clear();
                    continue;
                }

                if line.trim().is_empty() && !in_multiline {
                    continue;
                }

                if in_multiline {
                    buffer.push('\n');
                    buffer.push_str(&line);
                    continue;
                }

                buffer = line.clone();

                // Check if this line starts a block (ends with :)
                let trimmed = line.trim();
                if trimmed.ends_with(':')
                    || trimmed.starts_with("def ")
                    || trimmed.starts_with("if ")
                    || trimmed.starts_with("for ")
                    || trimmed.starts_with("while ")
                    || trimmed.starts_with("try:")
                    || trimmed.starts_with("elif ")
                    || trimmed.starts_with("else:")
                    || trimmed.starts_with("except")
                    || trimmed.starts_with("finally:")
                {
                    in_multiline = true;
                    continue;
                }

                // Single line — try to execute
                match run_python_repl_line(sandbox, &buffer) {
                    Ok(()) => {}
                    Err(e) => eprintln!("\x1b[31m{}\x1b[0m", e),
                }
                rl.add_history_entry(&buffer).ok();
                buffer.clear();
            }
            Err(rustyline::error::ReadlineError::Interrupted) => {
                if in_multiline {
                    in_multiline = false;
                    buffer.clear();
                    continue;
                }
            }
            Err(rustyline::error::ReadlineError::Eof) => break,
            Err(e) => {
                eprintln!("error: {}", e);
                break;
            }
        }
    }

    if let Some(path) = history_path(Language::Python) {
        let _ = rl.save_history(&path);
    }

    println!();
}

/// Heuristic: detect if a Luau parse error indicates incomplete input.
fn is_incomplete_error(err: &str) -> bool {
    // Luau typically says "Expected <something> (to close <something> at ...)" for incomplete code
    err.contains("Expected") && (err.contains("to close") || err.contains("Expected 'end'"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, UNIX_EPOCH};

    #[test]
    fn format_size_bytes() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(999), "999 B");
    }

    #[test]
    fn format_size_kilobytes() {
        assert_eq!(format_size(1_000), "1 KB");
        assert_eq!(format_size(1_500), "2 KB");
        assert_eq!(format_size(999_999), "1000 KB");
    }

    #[test]
    fn format_size_megabytes() {
        assert_eq!(format_size(1_000_000), "1.0 MB");
        assert_eq!(format_size(12_400_000), "12.4 MB");
        assert_eq!(format_size(999_999_999), "1000.0 MB");
    }

    #[test]
    fn format_size_gigabytes() {
        assert_eq!(format_size(1_000_000_000), "1.0 GB");
        assert_eq!(format_size(2_500_000_000), "2.5 GB");
    }

    #[test]
    fn format_date_epoch() {
        let time = UNIX_EPOCH;
        assert_eq!(format_date(time), "1970-01-01");
    }

    #[test]
    fn format_date_known_dates() {
        // 2026-02-27 = day 20,511 since epoch (2026-02-27T00:00:00Z)
        // 20511 * 86400 = 1772150400
        let time = UNIX_EPOCH + Duration::from_secs(1772150400);
        assert_eq!(format_date(time), "2026-02-27");

        // 2000-01-01 = 946684800
        let time = UNIX_EPOCH + Duration::from_secs(946684800);
        assert_eq!(format_date(time), "2000-01-01");

        // 2024-02-29 (leap day) = 1709164800
        let time = UNIX_EPOCH + Duration::from_secs(1709164800);
        assert_eq!(format_date(time), "2024-02-29");
    }

    // --- Language resolution tests ---

    #[test]
    fn resolve_language_default_is_bash() {
        assert_eq!(resolve_language(false, false, false), Language::Bash);
    }

    #[test]
    fn resolve_language_explicit_bash() {
        assert_eq!(resolve_language(true, false, false), Language::Bash);
    }

    #[test]
    fn resolve_language_python() {
        assert_eq!(resolve_language(false, true, false), Language::Python);
    }

    #[test]
    fn resolve_language_lua() {
        assert_eq!(resolve_language(false, false, true), Language::Lua);
    }

    #[cfg(feature = "mod-http")]
    #[test]
    fn webview_pdf_rendering_requires_fully_unrestricted_network_policy() {
        assert!(!webview_pdf_rendering_allowed(&[], &[]));
        assert!(!webview_pdf_rendering_allowed(
            &["example.com".to_string()],
            &[]
        ));
        assert!(webview_pdf_rendering_allowed(&["*".to_string()], &[]));
        assert!(!webview_pdf_rendering_allowed(
            &["*".to_string()],
            &["private.example".to_string()]
        ));
    }

    // --- Clap flag parsing tests ---

    #[test]
    fn clap_bash_python_mutually_exclusive() {
        let result = Cli::try_parse_from(["cpsl", "--bash", "--python", "-i"]);
        assert!(result.is_err(), "should reject --bash --python");
    }

    #[test]
    fn clap_bash_lua_mutually_exclusive() {
        let result = Cli::try_parse_from(["cpsl", "--bash", "--lua", "-i"]);
        assert!(result.is_err(), "should reject --bash --lua");
    }

    #[test]
    fn clap_python_lua_mutually_exclusive() {
        let result = Cli::try_parse_from(["cpsl", "--python", "--lua", "-i"]);
        assert!(result.is_err(), "should reject --python --lua");
    }

    #[test]
    fn clap_run_bash_python_mutually_exclusive() {
        let result = Cli::try_parse_from(["cpsl", "run", "test", "--bash", "--python"]);
        assert!(result.is_err(), "should reject run --bash --python");
    }

    #[test]
    fn clap_accepts_single_language_flag() {
        let cli = Cli::try_parse_from(["cpsl", "--python", "-i"]).unwrap();
        assert!(cli.exec.python);
        assert!(!cli.exec.bash);
        assert!(!cli.exec.lua);
    }

    #[test]
    fn clap_no_language_flag_defaults() {
        let cli = Cli::try_parse_from(["cpsl", "-i"]).unwrap();
        assert!(!cli.exec.python);
        assert!(!cli.exec.bash);
        assert!(!cli.exec.lua);
        // resolve_language will default to Bash
        let lang = resolve_language(cli.exec.bash, cli.exec.python, cli.exec.lua);
        assert_eq!(lang, Language::Bash);
    }
}
