use tower_lsp::lsp_types::Position;
use webspec_lsp::diagnostics;

#[test]
fn diagnostics_report_missing_version() {
    let text = r#"
protocol: "webspec"
name: test
"#;
    let diags = diagnostics::validate(text);
    assert!(!diags.is_empty(), "expected validation diagnostics for missing version");
}

#[test]
fn diagnostics_pass_for_valid_minimal_spec() {
    let text = r#"
version: "1.0.0"
protocol: "webspec"
name: Example
base_url: "https://api.example.com"
entities:
  Product:
    fields:
      id:
        type: string
        selector: ".id"
pages:
  products:
    url: "/products"
    entity: Product
"#;
    let diags = diagnostics::validate(text);
    assert!(diags.is_empty(), "expected no diagnostics, got: {:#?}", diags);
}

#[test]
fn completions_include_top_level_keys() {
    let text = "";
    let items = diagnostics::completions(text, Position { line: 0, character: 0 });
    let labels: Vec<_> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"version"));
    assert!(labels.contains(&"entities"));
    assert!(labels.contains(&"pages"));
}
