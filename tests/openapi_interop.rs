use assert_cmd::Command;
use insta::assert_yaml_snapshot;
use std::path::PathBuf;
use webspec::openapi::{convert_openapi_to_webspec, convert_webspec_to_openapi};
use webspec::spec::ApiSpec;

fn fixture(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(path)
}

fn read_fixture(path: &str) -> String {
    std::fs::read_to_string(fixture(path)).expect("fixture exists")
}

#[test]
fn openapi_to_webspec_petstore() {
    let raw = read_fixture("openapi/petstore.openapi.yaml");
    let oas: oas3::Spec = serde_yaml::from_str(&raw).expect("parse openapi");
    let spec = convert_openapi_to_webspec(&oas).expect("convert");

    assert_eq!(spec.name, "SwaggerPetstore");
    assert_eq!(spec.base_url.as_deref(), Some("https://petstore.example.com/v1"));
    assert!(spec.pages.contains_key("listpets"));
    assert!(spec.pages.contains_key("showpetbyid"));
    assert!(spec.entities.contains_key("Pet"));

    let value = serde_json::to_value(&spec).expect("serialize");
    let diagnostics = webspec::validation::validate_spec_by_json(&value, None).expect("validate");
    assert!(diagnostics.is_empty(), "validation errors: {diagnostics:?}");

    let yaml_value: serde_yaml::Value = serde_json::from_value(value).expect("json to yaml");
    assert_yaml_snapshot!(&yaml_value);
}

#[test]
fn openapi_to_webspec_minimal() {
    let raw = read_fixture("openapi/minimal.openapi.yaml");
    let oas: oas3::Spec = serde_yaml::from_str(&raw).expect("parse openapi");
    let spec = convert_openapi_to_webspec(&oas).expect("convert");

    assert_eq!(spec.pages.len(), 1);
    assert_eq!(spec.entities.len(), 1);
    assert!(spec.pages.contains_key("listitems"));
    assert!(spec.entities.contains_key("Item"));

    let value = serde_json::to_value(&spec).expect("serialize");
    let diagnostics = webspec::validation::validate_spec_by_json(&value, None).expect("validate");
    assert!(diagnostics.is_empty(), "validation errors: {diagnostics:?}");
}

#[test]
fn webspec_to_openapi_ecommerce() {
    let raw = read_fixture("convert/ecommerce.webspec.yaml");
    let spec: ApiSpec = serde_yaml::from_str(&raw).expect("parse webspec");
    let oas = convert_webspec_to_openapi(&spec).expect("convert");

    assert!(oas.paths.as_ref().map(|p| !p.is_empty()).unwrap_or(false));
    let schemas = oas
        .components
        .as_ref()
        .map(|c| &c.schemas)
        .expect("schemas exist");
    assert!(schemas.contains_key("Product"));
    assert!(schemas.contains_key("ProductDetail"));
    assert!(schemas.contains_key("StockStatus"));

    let yaml = serde_yaml::to_string(&oas).expect("serialize openapi");
    let reparsed: oas3::Spec = serde_yaml::from_str(&yaml).expect("reparse openapi");
    assert_eq!(reparsed.info.title, "ExampleShop");

    let value: serde_yaml::Value = serde_yaml::from_str(&yaml).expect("yaml value");
    assert_yaml_snapshot!(&value);
}

#[test]
fn cli_convert_dry_run() {
    let mut cmd = Command::cargo_bin("webspec").unwrap();
    cmd.args([
        "convert",
        "--from",
        fixture("openapi/minimal.openapi.yaml").to_str().unwrap(),
        "--to",
        "/tmp/should-not-write.yaml",
        "--target",
        "webspec",
        "--dry-run",
    ]);
    cmd.assert().success();
    let stdout = String::from_utf8(cmd.assert().success().get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("[dry-run]"), "stdout should contain [dry-run]: {stdout}");
    assert!(!std::path::Path::new("/tmp/should-not-write.yaml").exists());
}
