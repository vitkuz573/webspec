use assert_cmd::Command;
use std::io::Write;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const SAMPLE_HTML: &str = include_str!("fixtures/drift/sample.html");

fn spec_with_base_url(base_url: &str) -> String {
    format!(
        r#"version: "1.0.0"
protocol: "webspec"
name: "Sample"
base_url: "{base_url}"
rate_limits:
  requests_per_second: 100.0
  max_retries: 0
drift_detection:
  enabled: true
  pages:
    sample:
      url: "/sample"
      selectors:
        title: ".item-title"
        price: ".missing-price"
"#
    )
}

fn spec_with_dry_run() -> String {
    r#"version: "1.0.0"
protocol: "webspec"
name: "Sample"
base_url: "https://example.com"
rate_limits:
  requests_per_second: 100.0
  max_retries: 0
drift_detection:
  enabled: true
  pages:
    sample:
      url: "/sample"
      selectors:
        title: ".item-title"
"#
    .to_string()
}

#[tokio::test]
async fn cli_drift_fails_on_missing_selector() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/sample"))
        .respond_with(ResponseTemplate::new(200).set_body_string(SAMPLE_HTML))
        .mount(&server)
        .await;

    let spec = spec_with_base_url(&server.uri());
    let mut spec_file = tempfile::NamedTempFile::with_suffix(".webspec.yaml").unwrap();
    spec_file.write_all(spec.as_bytes()).unwrap();

    let mut cmd = Command::cargo_bin("webspec").unwrap();
    cmd.arg("drift")
        .arg("--spec")
        .arg(spec_file.path())
        .arg("--format")
        .arg("json");

    let output = cmd.output().unwrap();
    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"drifted\": true"));
    assert!(stdout.contains(".missing-price"));
}

#[tokio::test]
async fn dry_run_prints_urls_without_fetching() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/sample"))
        .respond_with(ResponseTemplate::new(200).set_body_string(SAMPLE_HTML))
        .expect(0)
        .mount(&server)
        .await;

    let spec = spec_with_dry_run();
    let mut spec_file = tempfile::NamedTempFile::with_suffix(".webspec.yaml").unwrap();
    spec_file.write_all(spec.as_bytes()).unwrap();

    let mut cmd = Command::cargo_bin("webspec").unwrap();
    cmd.arg("drift")
        .arg("--spec")
        .arg(spec_file.path())
        .arg("--dry-run");

    let output = cmd.output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("https://example.com/sample"));
}

#[tokio::test]
async fn drift_rate_limit_respects_spec() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/sample"))
        .respond_with(ResponseTemplate::new(200).set_body_string(SAMPLE_HTML))
        .mount(&server)
        .await;

    let spec = r#"version: "1.0.0"
protocol: "webspec"
name: "Sample"
base_url: "{base_url}"
rate_limits:
  requests_per_second: 2.0
  max_retries: 0
drift_detection:
  enabled: true
  pages:
    sample1:
      url: "/sample"
      selectors:
        title: ".item-title"
    sample2:
      url: "/sample"
      selectors:
        title: ".item-title"
"#
    .replace("{base_url}", &server.uri());

    let mut spec_file = tempfile::NamedTempFile::with_suffix(".webspec.yaml").unwrap();
    spec_file.write_all(spec.as_bytes()).unwrap();

    let start = std::time::Instant::now();
    let mut cmd = Command::cargo_bin("webspec").unwrap();
    cmd.arg("drift")
        .arg("--spec")
        .arg(spec_file.path())
        .arg("--format")
        .arg("json");
    let output = cmd.output().unwrap();
    assert!(output.status.success());
    let elapsed = start.elapsed();

    assert!(elapsed >= std::time::Duration::from_millis(400));
}

#[test]
fn drift_missing_selector_unit() {
    use webspec::drift::check::selector_exists;
    assert!(selector_exists(SAMPLE_HTML, ".item-title").unwrap());
    assert!(!selector_exists(SAMPLE_HTML, ".missing-price").unwrap());
}
