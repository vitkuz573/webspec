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
    #[diagnostic(code(webspec::target), help("Use `webspec list-plugins` to see available targets."))]
    UnsupportedTarget { target: String },

    #[error("unsupported version: {version}")]
    #[diagnostic(
        code(webspec::migrate::unsupported_version),
        help("Only 1.0.0 is supported in this release. See VERSIONING.md for migration policy.")
    )]
    UnsupportedVersion { version: String },
}

#[derive(Debug, Error, Diagnostic, Clone)]
pub enum PluginError {
    #[error("plugin for target `{target}` is not registered")]
    #[diagnostic(code(webspec::plugin::not_found), help("Run `webspec list-plugins` to see available targets."))]
    NotFound { target: String },

    #[error("plugin process failed for `{target}`: {message}")]
    #[diagnostic(code(webspec::plugin::process))]
    ProcessFailed { target: String, message: String },

    #[error("plugin for `{target}` returned an unsupported protocol version `{version}`")]
    #[diagnostic(
        code(webspec::plugin::unsupported_protocol),
        help("Update the plugin to match the current CLI protocol version.")
    )]
    UnsupportedProtocol { target: String, version: String },

    #[error("plugin for `{target}` returned an error: {message}")]
    #[diagnostic(code(webspec::plugin::diagnostic))]
    DiagnosticError { target: String, message: String },

    #[error("plugin for `{target}` returned an invalid path: {path}")]
    #[diagnostic(code(webspec::plugin::path))]
    InvalidPath { target: String, path: String },

    #[error("plugin for `{target}` timed out")]
    #[diagnostic(code(webspec::plugin::timeout))]
    Timeout { target: String },

    #[error("plugin for `{target}` produced output exceeding the maximum size")]
    #[diagnostic(code(webspec::plugin::too_large))]
    OutputTooLarge { target: String },
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

impl From<PluginError> for SpecError {
    fn from(e: PluginError) -> Self {
        SpecError::Validation(e.to_string())
    }
}

impl PluginError {
    pub fn not_found(target: impl Into<String>) -> Self {
        PluginError::NotFound {
            target: target.into(),
        }
    }

    pub fn process_failed(target: impl Into<String>, message: impl Into<String>) -> Self {
        PluginError::ProcessFailed {
            target: target.into(),
            message: message.into(),
        }
    }

    pub fn unsupported_protocol(target: impl Into<String>, version: impl Into<String>) -> Self {
        PluginError::UnsupportedProtocol {
            target: target.into(),
            version: version.into(),
        }
    }

    pub fn diagnostic_error(target: impl Into<String>, message: impl Into<String>) -> Self {
        PluginError::DiagnosticError {
            target: target.into(),
            message: message.into(),
        }
    }

    pub fn invalid_path(target: impl Into<String>, path: impl Into<String>) -> Self {
        PluginError::InvalidPath {
            target: target.into(),
            path: path.into(),
        }
    }
}
