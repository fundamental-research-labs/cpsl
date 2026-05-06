use std::fs;
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set by Cargo"),
    );
    let sandbox_manifest = std::env::var_os("CPSL_WEB_MANIFEST")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest_dir.join("../public/cpsl-web.toml"));

    println!("cargo:rerun-if-changed={}", sandbox_manifest.display());

    let source = fs::read_to_string(&sandbox_manifest).unwrap_or_else(|error| {
        panic!(
            "failed to read web sandbox manifest {}: {error}",
            sandbox_manifest.display()
        )
    });
    let parsed: toml::Value = toml::from_str(&source).unwrap_or_else(|error| {
        panic!(
            "failed to parse web sandbox manifest {}: {error}",
            sandbox_manifest.display()
        )
    });

    let allowed_domains = parsed
        .get("http")
        .and_then(|http| http.get("allowed_domains"))
        .and_then(toml::Value::as_array)
        .map(|domains| {
            domains
                .iter()
                .map(|domain| {
                    domain
                        .as_str()
                        .unwrap_or_else(|| {
                            panic!(
                                "http.allowed_domains entries in {} must be strings",
                                sandbox_manifest.display()
                            )
                        })
                        .to_string()
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    println!(
        "cargo:rustc-env=CPSL_WEB_HTTP_ALLOWED_DOMAINS={}",
        allowed_domains.join(",")
    );
}
