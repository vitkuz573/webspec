use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct LoadedSpec {
    pub value: serde_yaml::Value,
    pub source: Option<PathBuf>,
    pub raw: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct SpecLoader;

impl SpecLoader {
    pub fn new() -> Self {
        Self
    }

    pub async fn load(&self, path: impl AsRef<Path>) -> anyhow::Result<LoadedSpec> {
        let path = path.as_ref();
        let raw = if path.as_os_str().to_string_lossy().starts_with("http://")
            || path.as_os_str().to_string_lossy().starts_with("https://")
        {
            reqwest::get(path.as_os_str().to_string_lossy().to_string())
                .await?
                .text()
                .await?
        } else {
            std::fs::read_to_string(path)?
        };

        let value = serde_yaml::from_str(&raw)?;
        Ok(LoadedSpec {
            value,
            source: Some(path.to_path_buf()),
            raw: Some(raw),
        })
    }

    pub fn from_str(&self, raw: &str) -> anyhow::Result<LoadedSpec> {
        let value = serde_yaml::from_str(raw)?;
        Ok(LoadedSpec {
            value,
            source: None,
            raw: Some(raw.to_string()),
        })
    }
}

impl Default for &SpecLoader {
    fn default() -> Self {
        static SINGLETON: std::sync::OnceLock<SpecLoader> = std::sync::OnceLock::new();
        SINGLETON.get_or_init(SpecLoader::new)
    }
}
