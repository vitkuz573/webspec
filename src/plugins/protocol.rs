use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

pub const PROTOCOL_VERSION: &str = "1.0.0";

/// Request sent from the CLI to an external plugin via stdin as JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateRequest {
    pub protocol_version: String,
    pub target: String,
    /// The parsed webspec as JSON value.
    pub spec: serde_json::Value,
    /// Directory where the plugin should write generated files.
    pub output_dir: PathBuf,
    /// Target-specific options supplied by the user.
    #[serde(default)]
    pub options: HashMap<String, serde_json::Value>,
}

impl GenerateRequest {
    pub fn new(target: impl Into<String>, spec: serde_json::Value, output_dir: PathBuf) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION.to_string(),
            target: target.into(),
            spec,
            output_dir,
            options: HashMap::new(),
        }
    }
}

/// A single file returned by a plugin.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GeneratedFile {
    /// Relative path inside `output_dir`.
    pub path: String,
    /// File content as UTF-8 string.
    pub content: String,
}

/// Diagnostic message returned by a plugin.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PluginDiagnostic {
    pub severity: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

/// Response read from an external plugin via stdout as JSON.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GenerateResponse {
    #[serde(default)]
    pub files: Vec<GeneratedFile>,
    #[serde(default)]
    pub diagnostics: Vec<PluginDiagnostic>,
    /// When set by the plugin, the CLI will surface a protocol-mismatch error.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unsupported_protocol_version: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_serializes_with_protocol_version() {
        let req = GenerateRequest::new(
            "rust",
            serde_json::json!({"name": "Minimal", "version": "1.0.0"}),
            PathBuf::from("/tmp/out"),
        );
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["protocol_version"], "1.0.0");
        assert_eq!(json["target"], "rust");
        assert!(json["options"].is_object());
    }

    #[test]
    fn response_round_trips_with_diagnostics() {
        let resp = GenerateResponse {
            files: vec![GeneratedFile {
                path: "src/lib.rs".into(),
                content: "fn main() {}".into(),
            }],
            diagnostics: vec![PluginDiagnostic {
                severity: "warning".into(),
                message: "unused variable".into(),
                path: Some("pages.home".into()),
            }],
            unsupported_protocol_version: None,
        };
        let json = serde_json::to_string(&resp).unwrap();
        let back: GenerateResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(resp.files, back.files);
        assert_eq!(resp.diagnostics, back.diagnostics);
    }
}
