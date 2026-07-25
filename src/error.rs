use thiserror::Error;

#[derive(Debug, Error)]
pub enum SpecError {
    #[error("YAML parse error: {0}")]
    YamlParse(#[from] serde_yaml::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid spec: {0}")]
    Validation(String),
}
