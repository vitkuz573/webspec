use super::DriftReport;
use serde::Serialize;

#[derive(Serialize)]
struct JsonReport {
    drifted: bool,
    checked_count: usize,
    checked: Vec<(String, String)>,
    missing: Vec<JsonFailure>,
    errors: Vec<JsonError>,
}

#[derive(Serialize)]
struct JsonFailure {
    url: String,
    selector: String,
    context: Option<String>,
}

#[derive(Serialize)]
struct JsonError {
    url: String,
    message: String,
}

pub fn format_json(report: &DriftReport) -> String {
    let json = JsonReport {
        drifted: report.drifted,
        checked_count: report.checked.len(),
        checked: report.checked.clone(),
        missing: report
            .missing
            .iter()
            .map(|m| JsonFailure {
                url: m.url.clone(),
                selector: m.selector.clone(),
                context: m.context.clone(),
            })
            .collect(),
        errors: report
            .errors
            .iter()
            .map(|(url, message)| JsonError {
                url: url.clone(),
                message: message.clone(),
            })
            .collect(),
    };
    serde_json::to_string_pretty(&json).unwrap_or_else(|_| "{\"drifted\":true}".to_string())
}

pub fn format_table(report: &DriftReport) -> String {
    let mut lines = Vec::new();
    lines.push(format!("Checked {} selector(s)", report.checked.len()));
    if !report.missing.is_empty() {
        lines.push("\nMissing selectors:".to_string());
        for m in &report.missing {
            let ctx = m.context.as_deref().unwrap_or("unknown");
            lines.push(format!("  - {} ({}) on {}", m.selector, ctx, m.url));
        }
    }
    if !report.errors.is_empty() {
        lines.push("\nErrors:".to_string());
        for (url, msg) in &report.errors {
            lines.push(format!("  - {}: {}", url, msg));
        }
    }
    lines.push(format!("\nDrifted: {}", report.drifted));
    lines.join("\n")
}
