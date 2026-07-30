use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn all_official_examples_are_valid() {
    let examples = [
        "/home/vitaly/projects/funpay/webspec-proto/examples/minimal.yaml",
        "/home/vitaly/projects/funpay/webspec-proto/examples/ecommerce.yaml",
        "/home/vitaly/projects/funpay/webspec-proto/examples/multi-language.yaml",
        "/home/vitaly/projects/funpay/webspec-proto/examples/complex.yaml",
    ];

    for example in &examples {
        let mut cmd = Command::cargo_bin("webspec").unwrap();
        cmd.args(["validate", "--spec", example]);
        cmd.assert().success().stdout(predicate::str::contains("Spec is valid"));
    }
}

#[test]
fn missing_name_rejected() {
    let mut cmd = Command::cargo_bin("webspec").unwrap();
    cmd.args(["validate", "--spec", "tests/fixtures/invalid/missing_name.yaml"]);
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("name"));
}

#[test]
fn unknown_version_rejected() {
    let mut cmd = Command::cargo_bin("webspec").unwrap();
    cmd.args(["validate", "--spec", "tests/fixtures/invalid/unknown_version.yaml"]);
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("version"));
}

#[test]
fn both_url_and_pattern_rejected() {
    let mut cmd = Command::cargo_bin("webspec").unwrap();
    cmd.args([
        "validate",
        "--spec",
        "tests/fixtures/invalid/both_url_and_pattern.yaml",
    ]);
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("pages.home"));
}
