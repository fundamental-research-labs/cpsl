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

const GREP_MODULE: &str = "grep";
const GREP_PROVIDERS: &[&str] = &["fff", "ripgrep"];

/// The canonical list of built-in boolean modules. Provider-backed capabilities
/// such as `grep` are validated and translated separately.
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

/// Returns valid public module/capability names accepted in `cpsl.toml`.
pub fn valid_module_names() -> Vec<&'static str> {
    let mut names: Vec<&'static str> = MODULE_REGISTRY.iter().map(|m| m.name).collect();
    names.push(GREP_MODULE);
    names
}

/// Look up a module manifest by name.
pub fn find_module(name: &str) -> Option<&'static ModuleManifest> {
    MODULE_REGISTRY.iter().find(|m| m.name == name)
}

fn grep_provider_feature(provider: &str) -> Option<&'static str> {
    match provider {
        "fff" => Some("mod-fff"),
        "ripgrep" => Some("mod-ripgrep"),
        _ => None,
    }
}

fn is_grep_provider_module(name: &str) -> bool {
    matches!(name, "fff" | "ripgrep")
}

/// Extra configuration for module entries that use table syntax.
#[derive(Debug, Clone, Deserialize, PartialEq, Default)]
pub struct ModuleConfig {
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, toml::Value>,
}

/// A module entry in `cpsl.toml`. Supports these forms:
///
/// - `json = true` / `json = false` — built-in module, enabled or disabled
/// - `grep = { provider = "ripgrep" }` — grep capability provider selection
/// - `json = { source = "github.com/cpsl/mod-json" }` — external module (future)
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum ModuleEntry {
    /// Simple boolean: `json = true`
    Enabled(bool),
    /// Extended table form for grep providers and future external modules.
    Config(ModuleConfig),
}

impl ModuleEntry {
    /// Whether this module is enabled. Table configs are always considered enabled.
    pub fn is_enabled(&self) -> bool {
        match self {
            ModuleEntry::Enabled(b) => *b,
            ModuleEntry::Config(_) => true,
        }
    }

    /// Whether this module references an external source.
    pub fn is_external(&self) -> bool {
        matches!(self, ModuleEntry::Config(config) if config.source.is_some())
    }

    fn grep_provider(&self) -> Option<&str> {
        match self {
            ModuleEntry::Config(config) => config.provider.as_deref(),
            ModuleEntry::Enabled(_) => None,
        }
    }
}

impl fmt::Display for ModuleEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ModuleEntry::Enabled(b) => write!(f, "{}", b),
            ModuleEntry::Config(config) => {
                let mut fields = Vec::new();
                if let Some(source) = &config.source {
                    fields.push(format!("source = \"{}\"", source));
                }
                if let Some(provider) = &config.provider {
                    fields.push(format!("provider = \"{}\"", provider));
                }
                fields.extend(config.extra.keys().map(|key| format!("{} = ...", key)));
                write!(f, "{{ {} }}", fields.join(", "))
            }
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
            if name == GREP_MODULE {
                self.validate_grep_entry(entry)?;
                continue;
            }
            if is_grep_provider_module(name) {
                return Err(format!(
                    "standalone grep provider module '{}' is not supported — use \
                     grep = {{ provider = \"{}\" }} with fs = true",
                    name, name
                ));
            }
            if entry.is_external() {
                return Err(format!(
                    "external modules not yet supported — use built-in modules \
                     (module '{}' has source = \"...\")",
                    name
                ));
            }
            if find_module(name).is_none() {
                return Err(format!(
                    "unknown module '{}' — valid modules: {}",
                    name,
                    valid_names.join(", ")
                ));
            }
            if matches!(entry, ModuleEntry::Config(_)) {
                return Err(format!(
                    "module '{}' must be a boolean entry; only grep supports provider config",
                    name
                ));
            }
        }
        Ok(())
    }

    fn validate_grep_entry(&self, entry: &ModuleEntry) -> Result<(), String> {
        let config = match entry {
            ModuleEntry::Enabled(_) => {
                return Err(
                    "module 'grep' must be configured as grep = { provider = \"fff\" } \
                     or grep = { provider = \"ripgrep\" }; boolean grep entries are not supported"
                        .to_string(),
                );
            }
            ModuleEntry::Config(config) => config,
        };

        let provider = config.provider.as_deref().ok_or_else(|| {
            "module 'grep' requires provider = \"fff\" or provider = \"ripgrep\"".to_string()
        })?;

        if config.source.is_some() || !config.extra.is_empty() {
            return Err(
                "module 'grep' only supports a provider field; supported providers: fff, ripgrep"
                    .to_string(),
            );
        }

        if grep_provider_feature(provider).is_none() {
            return Err(format!(
                "unknown grep provider '{}' — supported providers: {}",
                provider,
                GREP_PROVIDERS.join(", ")
            ));
        }

        if !self.module_enabled("fs") {
            return Err(
                "module 'grep' requires fs = true because the public API is fs.grep".to_string(),
            );
        }

        Ok(())
    }

    fn module_enabled(&self, name: &str) -> bool {
        matches!(self.modules.get(name), Some(ModuleEntry::Enabled(true)))
    }

    /// Translate the modules map into Cargo feature flag names.
    ///
    /// Looks up each enabled built-in module in the manifest to get its `cargo_feature`.
    /// External modules are skipped (they don't have Cargo features).
    pub fn to_cargo_features(&self) -> Vec<String> {
        self.modules
            .iter()
            .filter(|(_, entry)| entry.is_enabled() && !entry.is_external())
            .filter_map(|(name, entry)| {
                if name == GREP_MODULE {
                    return entry
                        .grep_provider()
                        .and_then(grep_provider_feature)
                        .map(str::to_string);
                }
                find_module(name).map(|m| m.cargo_feature.to_string())
            })
            .collect()
    }

    /// Return user-facing module/capability labels for CLI summaries.
    pub fn module_display_names(&self) -> Vec<String> {
        self.modules
            .iter()
            .filter_map(|(name, entry)| match entry {
                ModuleEntry::Enabled(true) => Some(name.clone()),
                ModuleEntry::Config(config) if name == GREP_MODULE => config
                    .provider
                    .as_ref()
                    .map(|provider| format!("grep({provider})")),
                _ => None,
            })
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
    fn reject_old_grep_module_name() {
        let toml = r#"
[sandbox]
name = "bad"

[modules]
fs = true
grep = true
"#;
        let err = SandboxConfig::from_str(toml, "test".into()).unwrap_err();
        assert!(
            err.contains("boolean grep entries are not supported"),
            "got: {}",
            err
        );
    }

    #[test]
    fn reject_grep_false() {
        let toml = r#"
[sandbox]
name = "bad"

[modules]
fs = true
grep = false
"#;
        let err = SandboxConfig::from_str(toml, "test".into()).unwrap_err();
        assert!(
            err.contains("boolean grep entries are not supported"),
            "got: {}",
            err
        );
    }

    #[test]
    fn reject_grep_missing_provider() {
        let toml = r#"
[sandbox]
name = "bad"

[modules]
fs = true
grep = {}
"#;
        let err = SandboxConfig::from_str(toml, "test".into()).unwrap_err();
        assert!(err.contains("requires provider"), "got: {}", err);
    }

    #[test]
    fn reject_grep_unknown_provider() {
        let toml = r#"
[sandbox]
name = "bad"

[modules]
fs = true
grep = { provider = "ag" }
"#;
        let err = SandboxConfig::from_str(toml, "test".into()).unwrap_err();
        assert!(err.contains("unknown grep provider 'ag'"), "got: {}", err);
        assert!(err.contains("fff, ripgrep"), "got: {}", err);
    }

    #[test]
    fn reject_grep_without_fs() {
        for toml in [
            r#"
[sandbox]
name = "bad"

[modules]
grep = { provider = "ripgrep" }
"#,
            r#"
[sandbox]
name = "bad"

[modules]
fs = false
grep = { provider = "ripgrep" }
"#,
        ] {
            let err = SandboxConfig::from_str(toml, "test".into()).unwrap_err();
            assert!(err.contains("requires fs = true"), "got: {}", err);
        }
    }

    #[test]
    fn reject_standalone_grep_provider_modules() {
        for provider in ["fff", "ripgrep"] {
            let toml = format!(
                r#"
[sandbox]
name = "bad"

[modules]
fs = true
{} = true
"#,
                provider
            );
            let err = SandboxConfig::from_str(&toml, "test".into()).unwrap_err();
            assert!(
                err.contains("standalone grep provider module"),
                "{} got: {}",
                provider,
                err
            );
        }
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
grep = { provider = "ripgrep" }
"#;
        let config = SandboxConfig::from_str(toml, "test".into()).unwrap();
        let features = config.to_cargo_features();
        // BTreeMap sorts config keys, so grep's provider feature follows fs.
        assert_eq!(
            features,
            vec!["mod-fs", "mod-ripgrep", "mod-json", "mod-yaml"]
        );
    }

    #[test]
    fn to_cargo_features_selects_only_fff_provider() {
        let toml = r#"
[sandbox]
name = "test"

[modules]
fs = true
grep = { provider = "fff" }
"#;
        let config = SandboxConfig::from_str(toml, "test".into()).unwrap();
        let features = config.to_cargo_features();
        assert_eq!(features, vec!["mod-fs", "mod-fff"]);
        assert!(
            features.iter().all(|feature| feature != "mod-ripgrep"),
            "unexpected ripgrep provider in {:?}",
            features
        );
    }

    #[test]
    fn module_display_names_use_public_grep_capability() {
        let toml = r#"
[sandbox]
name = "test"

[modules]
json = true
fs = true
grep = { provider = "ripgrep" }
"#;
        let config = SandboxConfig::from_str(toml, "test".into()).unwrap();
        assert_eq!(
            config.module_display_names(),
            vec![
                "fs".to_string(),
                "grep(ripgrep)".to_string(),
                "json".to_string()
            ]
        );
    }

    #[test]
    fn sample_manifests_use_grep_provider_feature() {
        let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("cli crate should be inside workspace");

        for manifest in ["minimal.toml", "all.toml", "full.toml"] {
            let path = workspace_root.join("manifests").join(manifest);
            let config = SandboxConfig::from_file(&path).unwrap();
            let features = config.to_cargo_features();

            assert!(
                features.iter().any(|feature| feature == "mod-ripgrep"),
                "{} mapped to {:?}",
                manifest,
                features
            );
            assert!(
                features.iter().all(|feature| feature != "mod-fff"),
                "{} mapped to unselected provider {:?}",
                manifest,
                features
            );
        }
    }

    #[test]
    fn grep_provider_fixtures_select_expected_features() {
        let fixtures_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures");

        let ripgrep = SandboxConfig::from_file(&fixtures_root.join("grep-ripgrep.toml")).unwrap();
        let ripgrep_features = ripgrep.to_cargo_features();
        assert_eq!(ripgrep_features, vec!["mod-fs", "mod-ripgrep"]);
        assert!(
            ripgrep_features.iter().all(|feature| feature != "mod-fff"),
            "unexpected fff provider in {:?}",
            ripgrep_features
        );

        let fff = SandboxConfig::from_file(&fixtures_root.join("grep-fff.toml")).unwrap();
        let fff_features = fff.to_cargo_features();
        assert_eq!(fff_features, vec!["mod-fs", "mod-fff"]);
        assert!(
            fff_features.iter().all(|feature| feature != "mod-ripgrep"),
            "unexpected ripgrep provider in {:?}",
            fff_features
        );
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
    fn all_boolean_modules_accepted() {
        let modules_toml: String = MODULE_REGISTRY
            .iter()
            .map(|m| format!("{} = true", m.name))
            .collect::<Vec<_>>()
            .join("\n");
        let toml = format!("[sandbox]\nname = \"all\"\n\n[modules]\n{}", modules_toml);
        let config = SandboxConfig::from_str(&toml, "test".into()).unwrap();
        assert_eq!(config.to_cargo_features().len(), MODULE_REGISTRY.len());
    }

    #[test]
    fn valid_module_names_include_grep_capability_not_provider_modules() {
        let names = valid_module_names();
        assert!(names.contains(&"grep"), "got: {:?}", names);
        assert!(!names.contains(&"fff"), "got: {:?}", names);
        assert!(!names.contains(&"ripgrep"), "got: {:?}", names);
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
            ModuleEntry::Config(ModuleConfig {
                source: Some("github.com/cpsl/mod-csv".into()),
                provider: None,
                extra: BTreeMap::new(),
            })
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
                ModuleEntry::Config(ModuleConfig {
                    source: Some("github.com/foo".into()),
                    provider: None,
                    extra: BTreeMap::new(),
                })
            ),
            "{ source = \"github.com/foo\" }"
        );
        assert_eq!(
            format!(
                "{}",
                ModuleEntry::Config(ModuleConfig {
                    source: None,
                    provider: Some("fff".into()),
                    extra: BTreeMap::new(),
                })
            ),
            "{ provider = \"fff\" }"
        );
    }
}
