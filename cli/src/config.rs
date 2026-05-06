//! Configuration parsing and module feature selection for the CPSL CLI.

use serde::Deserialize;
use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

/// A self-describing module definition. Single source of truth for module name,
/// description, and Cargo feature flag mapping.
#[derive(Debug, Clone)]
pub struct ModuleManifest {
    /// Short name used in `cpsl.toml` and CLI output (e.g., `"json"`).
    pub name: &'static str,
    /// Human-readable description shown in `cpsl modules`.
    pub description: &'static str,
    /// Cargo feature flag in `cpsl-core` (e.g., `"mod-json"`).
    pub cargo_feature: &'static str,
}

/// The canonical list of all built-in modules. Every other part of the system
/// (config validation, `cpsl modules`, feature flag translation) reads from here.
pub static MODULE_REGISTRY: &[ModuleManifest] = &[
    ModuleManifest {
        name: "fs",
        description: "Filesystem operations (read, write, list, mkdir, etc.)",
        cargo_feature: "mod-fs",
    },
    ModuleManifest {
        name: "json",
        description: "JSON encoding and decoding",
        cargo_feature: "mod-json",
    },
    ModuleManifest {
        name: "csv",
        description: "CSV parsing and generation",
        cargo_feature: "mod-csv",
    },
    ModuleManifest {
        name: "yaml",
        description: "YAML parsing and generation",
        cargo_feature: "mod-yaml",
    },
    ModuleManifest {
        name: "xml",
        description: "XML parsing and generation",
        cargo_feature: "mod-xml",
    },
    ModuleManifest {
        name: "http",
        description: "HTTP client (GET, POST, etc.) via sandboxed gateway",
        cargo_feature: "mod-http",
    },
    ModuleManifest {
        name: "compress",
        description: "Archive and compression (zip, tar, gzip, bzip2, xz, 7z)",
        cargo_feature: "mod-compress",
    },
    ModuleManifest {
        name: "doc",
        description: "Document parsing (PDF, Excel, RTF, Markdown)",
        cargo_feature: "mod-doc",
    },
    ModuleManifest {
        name: "plot",
        description: "Chart and plot generation (line, bar, scatter, histogram)",
        cargo_feature: "mod-plot",
    },
    ModuleManifest {
        name: "numx",
        description: "Numerical computing (matrices, linear algebra, statistics)",
        cargo_feature: "mod-numpy",
    },
];

/// Returns the list of valid module names, derived from `MODULE_REGISTRY`.
pub fn valid_module_names() -> Vec<&'static str> {
    MODULE_REGISTRY.iter().map(|m| m.name).collect()
}

/// Look up a module manifest by name.
pub fn find_module(name: &str) -> Option<&'static ModuleManifest> {
    MODULE_REGISTRY.iter().find(|m| m.name == name)
}

/// A module entry in `cpsl.toml`. Supports two forms:
///
/// - `json = true` / `json = false` — built-in module, enabled or disabled
/// - `json = { source = "github.com/cpsl/mod-json" }` — external module (future)
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum ModuleEntry {
    /// Simple boolean: `json = true`
    Enabled(bool),
    /// Extended form with source: `json = { source = "..." }`
    External { source: String },
}

impl ModuleEntry {
    /// Whether this module is enabled. External modules are always considered enabled.
    pub fn is_enabled(&self) -> bool {
        match self {
            ModuleEntry::Enabled(b) => *b,
            ModuleEntry::External { .. } => true,
        }
    }

    /// Whether this module references an external source.
    pub fn is_external(&self) -> bool {
        matches!(self, ModuleEntry::External { .. })
    }
}

impl fmt::Display for ModuleEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ModuleEntry::Enabled(b) => write!(f, "{}", b),
            ModuleEntry::External { source } => write!(f, "{{ source = \"{}\" }}", source),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct SandboxConfig {
    pub sandbox: SandboxMeta,
    #[serde(default)]
    pub modules: BTreeMap<String, ModuleEntry>,
    #[serde(default)]
    pub python: PythonConfig,
    #[serde(default)]
    pub mounts: MountsConfig,
    #[serde(default)]
    pub http: HttpConfig,
}

#[derive(Debug, Deserialize)]
pub struct SandboxMeta {
    pub name: String,
}

#[derive(Debug, Default, Deserialize)]
pub struct PythonConfig {
    #[serde(default)]
    pub enabled: bool,
}

#[derive(Debug, Default, Deserialize)]
pub struct MountsConfig {
    #[serde(default)]
    pub volumes: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct HttpConfig {
    #[serde(default)]
    pub allowed_domains: Vec<String>,
    #[serde(default)]
    pub denied_domains: Vec<String>,
}

impl SandboxConfig {
    /// Parse a `cpsl.toml` file and validate its contents.
    pub fn from_file(path: &Path) -> Result<Self, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read {}: {}", path.display(), e))?;
        Self::from_str(&content, path.display().to_string())
    }

    /// Parse from a TOML string with a label for error messages.
    pub fn from_str(content: &str, label: String) -> Result<Self, String> {
        let config: SandboxConfig =
            toml::from_str(content).map_err(|e| format!("invalid config {}: {}", label, e))?;
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<(), String> {
        let valid_names = valid_module_names();
        for (name, entry) in &self.modules {
            if entry.is_external() {
                return Err(format!(
                    "external modules not yet supported — use built-in modules \
                     (module '{}' has source = \"...\")",
                    name
                ));
            }
            if !valid_names.contains(&name.as_str()) {
                return Err(format!(
                    "unknown module '{}' — valid modules: {}",
                    name,
                    valid_names.join(", ")
                ));
            }
        }
        Ok(())
    }

    /// Translate the modules map into Cargo feature flag names.
    ///
    /// Looks up each enabled built-in module in the manifest to get its `cargo_feature`.
    /// External modules are skipped (they don't have Cargo features).
    pub fn to_cargo_features(&self) -> Vec<String> {
        self.modules
            .iter()
            .filter(|(_, entry)| entry.is_enabled() && !entry.is_external())
            .filter_map(|(name, _)| find_module(name).map(|m| m.cargo_feature.to_string()))
            .collect()
    }

    pub fn python_enabled(&self) -> bool {
        self.python.enabled
    }

    pub fn mount_volumes(&self) -> &[String] {
        &self.mounts.volumes
    }

    /// Return (allowed_domains, denied_domains) for constructing an HttpGateway.
    ///
    /// Credentials are never stored in config — the host injects them at runtime.
    pub fn http_gateway_config(&self) -> (Vec<String>, Vec<String>) {
        (
            self.http.allowed_domains.clone(),
            self.http.denied_domains.clone(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_config() {
        let toml = r#"
[sandbox]
name = "data-processor"

[modules]
fs = true
json = true
csv = true
yaml = false

[python]
enabled = true

[mounts]
volumes = ["./data:/data:ro"]

[http]
allowed_domains = ["api.example.com"]
denied_domains = ["evil.com"]
"#;
        let config = SandboxConfig::from_str(toml, "test".into()).unwrap();
        assert_eq!(config.sandbox.name, "data-processor");
        assert_eq!(config.modules.len(), 4);
        assert_eq!(config.modules["fs"], ModuleEntry::Enabled(true));
        assert_eq!(config.modules["yaml"], ModuleEntry::Enabled(false));
        assert!(config.python.enabled);
        assert_eq!(config.mounts.volumes, vec!["./data:/data:ro"]);
        assert_eq!(config.http.allowed_domains, vec!["api.example.com"]);
        assert_eq!(config.http.denied_domains, vec!["evil.com"]);
    }

    #[test]
    fn reject_unknown_module() {
        let toml = r#"
[sandbox]
name = "bad"

[modules]
json = true
foobar = true
"#;
        let err = SandboxConfig::from_str(toml, "test".into()).unwrap_err();
        assert!(err.contains("unknown module 'foobar'"), "got: {}", err);
    }

    #[test]
    fn to_cargo_features_only_enabled() {
        let toml = r#"
[sandbox]
name = "test"

[modules]
json = true
csv = false
yaml = true
fs = true
"#;
        let config = SandboxConfig::from_str(toml, "test".into()).unwrap();
        let features = config.to_cargo_features();
        // BTreeMap is sorted, so features come out in alphabetical order
        assert_eq!(features, vec!["mod-fs", "mod-json", "mod-yaml"]);
    }

    #[test]
    fn to_cargo_features_empty_modules() {
        let toml = r#"
[sandbox]
name = "empty"
"#;
        let config = SandboxConfig::from_str(toml, "test".into()).unwrap();
        assert!(config.to_cargo_features().is_empty());
    }

    #[test]
    fn http_gateway_config_extraction() {
        let toml = r#"
[sandbox]
name = "net"

[http]
allowed_domains = ["a.com", "b.com"]
denied_domains = ["c.com"]
"#;
        let config = SandboxConfig::from_str(toml, "test".into()).unwrap();
        let (allowed, denied) = config.http_gateway_config();
        assert_eq!(allowed, vec!["a.com", "b.com"]);
        assert_eq!(denied, vec!["c.com"]);
    }

    #[test]
    fn http_gateway_config_defaults_empty() {
        let toml = r#"
[sandbox]
name = "minimal"
"#;
        let config = SandboxConfig::from_str(toml, "test".into()).unwrap();
        let (allowed, denied) = config.http_gateway_config();
        assert!(allowed.is_empty());
        assert!(denied.is_empty());
    }

    #[test]
    fn all_valid_modules_accepted() {
        let modules_toml: String = valid_module_names()
            .iter()
            .map(|m| format!("{} = true", m))
            .collect::<Vec<_>>()
            .join("\n");
        let toml = format!("[sandbox]\nname = \"all\"\n\n[modules]\n{}", modules_toml);
        let config = SandboxConfig::from_str(&toml, "test".into()).unwrap();
        assert_eq!(config.to_cargo_features().len(), valid_module_names().len());
    }

    #[test]
    fn from_file_nonexistent() {
        let err = SandboxConfig::from_file(Path::new("/nonexistent/cpsl.toml")).unwrap_err();
        assert!(err.contains("cannot read"), "got: {}", err);
    }

    #[test]
    fn external_module_source_rejected() {
        let toml = r#"
[sandbox]
name = "ext"

[modules]
json = true
[modules.csv]
source = "github.com/cpsl/mod-csv"
"#;
        let err = SandboxConfig::from_str(toml, "test".into()).unwrap_err();
        assert!(
            err.contains("external modules not yet supported"),
            "got: {}",
            err
        );
        assert!(
            err.contains("csv"),
            "error should mention module name, got: {}",
            err
        );
    }

    #[test]
    fn external_module_entry_parses() {
        // Verify the deserialization works (even though validation rejects it).
        // We test the raw parse without validation.
        let toml = r#"
[sandbox]
name = "ext"

[modules]
json = true
[modules.csv]
source = "github.com/cpsl/mod-csv"
"#;
        let config: SandboxConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.modules["json"], ModuleEntry::Enabled(true));
        assert_eq!(
            config.modules["csv"],
            ModuleEntry::External {
                source: "github.com/cpsl/mod-csv".into()
            }
        );
        assert!(config.modules["csv"].is_external());
        assert!(config.modules["csv"].is_enabled());
    }

    #[test]
    fn module_entry_display() {
        assert_eq!(format!("{}", ModuleEntry::Enabled(true)), "true");
        assert_eq!(format!("{}", ModuleEntry::Enabled(false)), "false");
        assert_eq!(
            format!(
                "{}",
                ModuleEntry::External {
                    source: "github.com/foo".into()
                }
            ),
            "{ source = \"github.com/foo\" }"
        );
    }
}
