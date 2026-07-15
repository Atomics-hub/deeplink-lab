use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::model::LabSpec;

#[derive(Debug, Clone)]
pub struct LoadedSpec {
    pub spec: LabSpec,
    pub source_path: PathBuf,
    pub root: PathBuf,
}

impl LoadedSpec {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let source_path = path.as_ref().to_path_buf();
        let raw = fs::read_to_string(&source_path)
            .with_context(|| format!("failed to read {}", source_path.display()))?;
        let spec: LabSpec = serde_yaml::from_str(&raw)
            .with_context(|| format!("failed to parse {}", source_path.display()))?;
        let root = source_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        Ok(Self {
            spec,
            source_path,
            root,
        })
    }

    pub fn resolve(&self, path: &Path) -> PathBuf {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.root.join(path)
        }
    }

    pub fn portable_spec_name(&self) -> String {
        self.source_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("deeplinklab.yml")
            .to_owned()
    }
}
