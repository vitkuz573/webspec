use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

#[test]
fn help_lists_all_subcommands() {
    let mut cmd = Command::cargo_bin("webspec").unwrap();
    cmd.arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("validate"))
        .stdout(predicate::str::contains("generate"))
        .stdout(predicate::str::contains("fmt"))
        .stdout(predicate::str::contains("migrate"))
        .stdout(predicate::str::contains("test"));
}

#[test]
fn validate_missing_spec_returns_error() {
    let mut cmd = Command::cargo_bin("webspec").unwrap();
    cmd.args(["validate", "--spec", "/nonexistent/file.yaml"]);
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("file not found"));
}

#[test]
fn validate_official_example_succeeds() {
    let mut cmd = Command::cargo_bin("webspec").unwrap();
    cmd.args([
        "validate",
        "--spec",
        "/home/vitaly/projects/funpay/webspec-proto/examples/minimal.yaml",
    ]);
    cmd.assert().success().stdout(predicate::str::contains("Spec is valid"));
}

#[test]
fn generate_creates_files() {
    let dir = tempdir().unwrap();
    let mut cmd = Command::cargo_bin("webspec").unwrap();
    cmd.args([
        "generate",
        "--spec",
        "/home/vitaly/projects/funpay/webspec-proto/examples/minimal.yaml",
        "--target",
        "rust",
        "--output",
        dir.path().to_str().unwrap(),
    ]);
    cmd.assert().success();
    assert!(dir.path().join("Cargo.toml").exists());
    assert!(dir.path().join("src/lib.rs").exists());
}

#[test]
fn fmt_check_succeeds_for_formatted_spec() {
    let dir = tempdir().unwrap();
    let spec_path = dir.path().join("spec.yaml");
    fs::write(&spec_path, "version: 1.0.0\nprotocol: webspec\nname: Minimal\n").unwrap();

    let mut cmd = Command::cargo_bin("webspec").unwrap();
    cmd.args(["fmt", "--spec", spec_path.to_str().unwrap(), "--check"]);
    cmd.assert().success();
}

#[test]
fn migrate_identity_transform() {
    let mut cmd = Command::cargo_bin("webspec").unwrap();
    cmd.args([
        "migrate",
        "--spec",
        "/home/vitaly/projects/funpay/webspec-proto/examples/minimal.yaml",
        "--to",
        "1.0.0",
    ]);
    cmd.assert().success();
}

#[test]
fn migrate_rejects_unsupported_version() {
    let mut cmd = Command::cargo_bin("webspec").unwrap();
    cmd.args([
        "migrate",
        "--spec",
        "/home/vitaly/projects/funpay/webspec-proto/examples/minimal.yaml",
        "--to",
        "2.0.0",
    ]);
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("unsupported version"));
}

#[test]
fn test_emits_files() {
    let mut cmd = Command::cargo_bin("webspec").unwrap();
    cmd.args([
        "test",
        "--spec",
        "/home/vitaly/projects/funpay/webspec-proto/examples/minimal.yaml",
        "--target",
        "rust",
    ]);
    cmd.assert().success();
}
