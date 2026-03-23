use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Manifest {
    pub package: PackageSection,
    #[serde(default)]
    pub capabilities: CapabilitySection,
    #[serde(default)]
    pub dependencies: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PackageSection {
    pub name: String,
    pub version: String,
    pub entry: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CapabilitySection {
    #[serde(default)]
    pub default: Vec<String>,
}

impl Manifest {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            package: PackageSection {
                name: name.into(),
                version: "0.1.0".to_string(),
                entry: "src/main.ae".to_string(),
            },
            capabilities: CapabilitySection {
                default: vec!["Console".to_string()],
            },
            dependencies: BTreeMap::new(),
        }
    }
}

pub fn load_manifest(path: &Path) -> Result<Manifest> {
    let source = fs::read_to_string(path)
        .with_context(|| format!("failed to read manifest {}", path.display()))?;
    let manifest = toml::from_str::<Manifest>(&source)
        .with_context(|| format!("failed to parse manifest {}", path.display()))?;
    Ok(manifest)
}

pub fn save_manifest(path: &Path, manifest: &Manifest) -> Result<()> {
    let source = toml::to_string_pretty(manifest)?;
    fs::write(path, source).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

pub fn init_project(dir: &Path, name: &str) -> Result<()> {
    fs::create_dir_all(dir.join("src"))
        .with_context(|| format!("failed to create {}", dir.join("src").display()))?;
    save_manifest(&dir.join("Aeris.toml"), &Manifest::new(name))?;
    fs::write(dir.join("src/main.ae"), starter_source())
        .with_context(|| format!("failed to write {}", dir.join("src/main.ae").display()))?;
    Ok(())
}

pub fn starter_source() -> String {
    [
        "module app.main;",
        "",
        "effect io;",
        "cap Console;",
        "",
        "fn main(console: Console) -> Int !io {",
        "    print(console, \"Hello from AERIS\");",
        "    0",
        "}",
        "",
    ]
    .join("\n")
}
