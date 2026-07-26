use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiSpec {
    pub version: String,
    pub name: String,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub types: BTreeMap<String, TypeMapping>,
    #[serde(default)]
    pub enums: BTreeMap<String, EnumDef>,
    #[serde(default)]
    pub entities: BTreeMap<String, EntityDef>,
    #[serde(default)]
    pub pages: BTreeMap<String, PageDef>,
    #[serde(default)]
    pub auth: Option<AuthDef>,
    #[serde(default)]
    pub rate_limits: Option<RateLimitsDef>,
    #[serde(default)]
    pub drift_detection: Option<DriftDetectionDef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeMapping {
    #[serde(default)]
    pub rust: Option<String>,
    #[serde(default)]
    pub typescript: Option<String>,
    #[serde(default)]
    pub python: Option<String>,
    #[serde(default)]
    pub go: Option<String>,
    #[serde(default)]
    pub java: Option<String>,
    #[serde(default)]
    pub newtype: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnumDef {
    #[serde(default)]
    pub description: Option<String>,
    pub values: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityDef {
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub list_selector: Option<String>,
    #[serde(default)]
    pub fields: Option<BTreeMap<String, FieldDef>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldDef {
    pub r#type: String,
    #[serde(default)]
    pub nullable: Option<bool>,
    #[serde(default)]
    pub selector: Option<String>,
    #[serde(default)]
    pub attribute: Option<String>,
    #[serde(default)]
    pub transform: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageDef {
    #[serde(default)]
    pub description: Option<String>,
    pub entity: String,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub url_pattern: Option<String>,
    #[serde(default)]
    pub list_selector: Option<String>,
    #[serde(default)]
    pub method: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthDef {
    #[serde(default)]
    pub r#type: Option<String>,
    #[serde(default)]
    pub cookie_name: Option<String>,
    #[serde(default)]
    pub required_for: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitsDef {
    #[serde(default)]
    pub requests_per_second: Option<f64>,
    #[serde(default)]
    pub max_retries: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftDetectionDef {
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub pages: Option<BTreeMap<String, DriftPage>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftPage {
    pub url: String,
    pub selectors: BTreeMap<String, String>,
}

impl ApiSpec {
    pub async fn load(path: &str) -> anyhow::Result<Self> {
        if path.starts_with("http://") || path.starts_with("https://") {
            let content = reqwest::get(path).await?.text().await?;
            Self::from_str(&content)
        } else {
            let content = std::fs::read_to_string(path)?;
            Self::from_str(&content)
        }
    }

    pub fn from_str(yaml: &str) -> anyhow::Result<Self> {
        let spec: Self = serde_yaml::from_str(yaml)?;
        Ok(spec)
    }

    pub fn to_yaml(&self) -> anyhow::Result<String> {
        Ok(serde_yaml::to_string(self)?)
    }
}
