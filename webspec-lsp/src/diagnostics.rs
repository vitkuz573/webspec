use tower_lsp::lsp_types::{CompletionItem, CompletionItemKind, Diagnostic, DiagnosticSeverity, Position, Range};
use webspec::validation::{validate_spec_by_json_value, ValidationDiagnostic};

pub fn validate(text: &str) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    match serde_yaml::from_str::<serde_yaml::Value>(text) {
        Ok(value) => {
            let json_value = match serde_json::to_value(&value) {
                Ok(v) => v,
                Err(e) => {
                    diagnostics.push(Diagnostic {
                        range: Range::default(),
                        severity: Some(DiagnosticSeverity::ERROR),
                        source: Some("webspec".to_string()),
                        message: format!("JSON conversion failed: {e}"),
                        ..Default::default()
                    });
                    return diagnostics;
                }
            };
            for err in validate_spec_by_json_value(&json_value) {
                diagnostics.push(diagnostic_to_lsp(&err));
            }
        }
        Err(e) => {
            let (line, column) = e
                .location()
                .map(|loc| (loc.line() as u32, loc.column() as u32))
                .unwrap_or((0, 0));
            diagnostics.push(Diagnostic {
                range: Range {
                    start: Position {
                        line: line.saturating_sub(1),
                        character: column,
                    },
                    end: Position {
                        line: line.saturating_sub(1),
                        character: column + 1,
                    },
                },
                severity: Some(DiagnosticSeverity::ERROR),
                source: Some("webspec".to_string()),
                message: format!("YAML parse error: {e}"),
                ..Default::default()
            });
        }
    }

    diagnostics
}

fn diagnostic_to_lsp(err: &ValidationDiagnostic) -> Diagnostic {
    Diagnostic {
        range: Range {
            start: Position {
                line: err.line.unwrap_or(0) as u32,
                character: err.column.unwrap_or(0) as u32,
            },
            end: Position {
                line: err.line.unwrap_or(0) as u32,
                character: (err.column.unwrap_or(0) + 1) as u32,
            },
        },
        severity: Some(DiagnosticSeverity::ERROR),
        source: Some("webspec".to_string()),
        message: err.message.clone(),
        ..Default::default()
    }
}

pub fn completions(text: &str, position: Position) -> Vec<CompletionItem> {
    let mut items = Vec::new();

    for key in [
        "version", "protocol", "info", "base_url", "types", "enums", "entities", "pages", "auth",
        "rate_limits", "drift_detection",
    ] {
        items.push(CompletionItem {
            label: key.to_string(),
            kind: Some(CompletionItemKind::PROPERTY),
            detail: Some(format!("Top-level webspec `{key}` key")),
            ..Default::default()
        });
    }

    for value in ["cookie", "header", "bearer", "query", "path", "next_link", "cursor"] {
        items.push(CompletionItem {
            label: value.to_string(),
            kind: Some(CompletionItemKind::VALUE),
            detail: Some(format!("webspec enum value `{value}`")),
            ..Default::default()
        });
    }

    let line = text.lines().nth(position.line as usize).unwrap_or("");
    let prefix = &line[..(position.character as usize).min(line.len())];
    let filter = prefix.split_whitespace().last().unwrap_or(prefix);
    items
        .into_iter()
        .filter(|i| i.label.starts_with(filter) || filter.is_empty())
        .collect()
}
