use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiSpec {
    pub version: String,
    pub protocol: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub info: Option<serde_json::Value>,
    #[serde(default)]
    pub types: BTreeMap<String, TypeMapping>,
    #[serde(default)]
    pub enums: BTreeMap<String, EnumDef>,
    #[serde(default)]
    pub entities: BTreeMap<String, EntityDef>,
    #[serde(default)]
    pub pages: BTreeMap<String, PageDef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth: Option<AuthDef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate_limits: Option<RateLimitsDef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drift_detection: Option<DriftDetectionDef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeMapping {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rust: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub typescript: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub python: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub go: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub java: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub newtype: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnumDef {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub values: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityDef {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub list_selector: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fields: Option<BTreeMap<String, FieldDef>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldDef {
    pub r#type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nullable: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selector: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attribute: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transform: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageDef {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub entity: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url_pattern: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub list_selector: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthDef {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cookie_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub header_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required_for: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitsDef {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requests_per_second: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_retries: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftDetectionDef {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
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

impl ApiSpec {
    pub fn into_yaml_value(&self) -> anyhow::Result<serde_yaml::Value> {
        let json = serde_json::to_string(self)?;
        Ok(serde_yaml::from_str(&json)?)
    }
}
