pub mod diagnostics;
pub mod json_schema;

pub use diagnostics::ValidationDiagnostic;

use crate::error::SpecError;
use std::path::Path;

pub fn validate_spec(
    value: &serde_yaml::Value,
    source: Option<&Path>,
) -> Result<Vec<ValidationDiagnostic>, miette::Report> {
    let json_value = serde_json::to_value(value).map_err(|e| {
        miette::Report::new(SpecError::Validation(format!("JSON conversion failed: {e}")))
    })?;
    validate_spec_by_json(&json_value, source)
}

pub fn validate_spec_by_json(
    value: &serde_json::Value,
    source: Option<&Path>,
) -> Result<Vec<ValidationDiagnostic>, miette::Report> {
    let raw = source.and_then(|p| std::fs::read_to_string(p).ok());
    json_schema::validate_against_schema(value, source.map(Path::to_path_buf), raw)
        .map_err(|e| miette::Report::new(SpecError::Validation(e.to_string())))
}

pub fn validate_spec_by_json_value(
    value: &serde_json::Value,
) -> Vec<ValidationDiagnostic> {
    json_schema::validate_against_schema(value, None, None).unwrap_or_default()
}
