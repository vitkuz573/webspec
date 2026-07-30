use crate::validation::diagnostics::{DiagnosticInput, ValidationDiagnostic};
use serde_json::Value;
use std::path::PathBuf;

const SCHEMA_BYTES: &[u8] = include_bytes!("/home/vitaly/projects/funpay/webspec-proto/schema/v1.schema.json");

pub fn schema_value() -> anyhow::Result<Value> {
    Ok(serde_json::from_slice(SCHEMA_BYTES)?)
}

pub fn validate_against_schema(
    value: &Value,
    source_path: Option<PathBuf>,
    source: Option<String>,
) -> anyhow::Result<Vec<ValidationDiagnostic>> {
    let schema = schema_value()?;
    let validator = jsonschema::draft202012::options()
        .should_validate_formats(true)
        .build(&schema)
        .map_err(|e| anyhow::anyhow!("failed to compile schema: {e}"))?;

    let errors: Vec<ValidationDiagnostic> = validator
        .iter_errors(value)
        .map(|e| jsonschema_error_to_diagnostic(e, &source_path, source.clone()))
        .collect();

    Ok(errors)
}

fn jsonschema_error_to_diagnostic(
    error: jsonschema::ValidationError<'_>,
    source_path: &Option<PathBuf>,
    source: Option<String>,
) -> ValidationDiagnostic {
    let pointer = error.instance_path().to_string();
    let yaml_path = pointer_to_yaml_path(&pointer);
    let message = error.to_string();
    let code = format!("webspec::validation::{}", sanitize_code(&message));

    ValidationDiagnostic::from_input(DiagnosticInput {
        code,
        message,
        help: Some("Fix the field to match the webspec v1.0.0 protocol.".into()),
        path: yaml_path.clone(),
        instance_path: pointer,
        source_path: source_path.clone(),
        source,
        line: None,
        column: None,
    })
}

fn pointer_to_yaml_path(pointer: &str) -> String {
    if pointer.is_empty() || pointer == "/" {
        return ".".to_string();
    }
    pointer
        .trim_start_matches('/')
        .split('/')
        .map(|s| s.replace("~1", "/").replace("~0", "~"))
        .collect::<Vec<_>>()
        .join(".")
}

fn sanitize_code(message: &str) -> String {
    message
        .to_lowercase()
        .replace(|c: char| !c.is_alphanumeric() && c != ' ', " ")
        .split_whitespace()
        .take(6)
        .collect::<Vec<_>>()
        .join("_")
}
