use miette::{Diagnostic, Report};
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error, Diagnostic, Clone)]
#[diagnostic()]
pub enum SpecError {
    #[error("YAML parse error: {0}")]
    #[diagnostic(code(webspec::parse))]
    YamlParse(String),

    #[error("IO error: {0}")]
    #[diagnostic(code(webspec::io))]
    Io(String),

    #[error("invalid spec: {0}")]
    #[diagnostic(code(webspec::validation))]
    Validation(String),

    #[error("file not found: {path}")]
    #[diagnostic(code(webspec::io::not_found), help("Check the path and try again."))]
    FileNotFound { path: String },

    #[error("unsupported target: {target}")]
    #[diagnostic(code(webspec::target), help("Use `webspec generate --help` to see available targets."))]
    UnsupportedTarget { target: String },

    #[error("unsupported version: {version}")]
    #[diagnostic(
        code(webspec::migrate::unsupported_version),
        help("Only 1.0.0 is supported in this release. See VERSIONING.md for migration policy.")
    )]
    UnsupportedVersion { version: String },
}

impl SpecError {
    pub fn from_load(err: anyhow::Error, path: &Path) -> Self {
        if let Some(io_err) = err.downcast_ref::<std::io::Error>() {
            if io_err.kind() == std::io::ErrorKind::NotFound {
                return SpecError::FileNotFound {
                    path: path.display().to_string(),
                };
            }
            return SpecError::Io(io_err.to_string());
        }
        if let Some(yaml_err) = err.downcast_ref::<serde_yaml::Error>() {
            return SpecError::YamlParse(yaml_err.to_string());
        }
        SpecError::Validation(err.to_string())
    }

    pub fn as_report(self) -> Report {
        Report::new(self)
    }
}

impl From<std::io::Error> for SpecError {
    fn from(e: std::io::Error) -> Self {
        SpecError::Io(e.to_string())
    }
}

impl From<serde_yaml::Error> for SpecError {
    fn from(e: serde_yaml::Error) -> Self {
        SpecError::YamlParse(e.to_string())
    }
}
