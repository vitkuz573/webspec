use assert_cmd::Command;
use predicates::prelude::*;
use std::path::PathBuf;
use tempfile::tempdir;

/// Returns the directory containing the `webspec-mock` fixture binary.
fn fixture_bin_dir() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // Cargo places the dev binary next to the main binary in target/debug.
    manifest.join("target").join("debug")
}

fn with_mock_plugin(cmd: &mut Command) {
    let mut path = std::env::var_os("PATH").map(|p| std::ffi::OsString::from(p)).unwrap_or_default();
    path.push(":");
    path.push(fixture_bin_dir().as_os_str());
    cmd.env("PATH", path);
}

#[test]
fn list_plugins_shows_builtins() {
    let mut cmd = Command::cargo_bin("webspec").unwrap();
    cmd.arg("list-plugins");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("rust"))
        .stdout(predicate::str::contains("typescript"))
        .stdout(predicate::str::contains("python"));
}

#[test]
fn list_plugins_discovers_mock() {
    let mut cmd = Command::cargo_bin("webspec").unwrap();
    cmd.arg("list-plugins");
    with_mock_plugin(&mut cmd);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("mock"));
}

#[test]
fn generate_with_mock_plugin_emits_file() {
    let dir = tempdir().unwrap();
    let mut cmd = Command::cargo_bin("webspec").unwrap();
    cmd.args([
        "generate",
        "--spec",
        "/home/vitaly/projects/funpay/webspec-proto/examples/minimal.yaml",
        "--target",
        "mock",
        "--output",
        dir.path().to_str().unwrap(),
    ]);
    with_mock_plugin(&mut cmd);
    cmd.assert().success();

    let generated = dir.path().join("generated.txt");
    assert!(generated.exists());
    let content = std::fs::read_to_string(&generated).unwrap();
    assert!(content.contains("target=mock"));
    assert!(content.contains("spec=Minimal"));
}

#[test]
fn generate_with_explicit_plugin_path_emits_file() {
    let dir = tempdir().unwrap();
    let plugin_path = fixture_bin_dir().join("webspec-mock");
    let mut cmd = Command::cargo_bin("webspec").unwrap();
    cmd.args([
        "generate",
        "--spec",
        "/home/vitaly/projects/funpay/webspec-proto/examples/minimal.yaml",
        "--target",
        "mock",
        "--plugin",
        plugin_path.to_str().unwrap(),
        "--output",
        dir.path().to_str().unwrap(),
    ]);
    cmd.assert().success();

    let generated = dir.path().join("generated.txt");
    assert!(generated.exists());
}

#[test]
fn generate_with_mock_plugin_bad_protocol_fails() {
    let dir = tempdir().unwrap();
    let mut cmd = Command::cargo_bin("webspec").unwrap();
    cmd.args([
        "generate",
        "--spec",
        "/home/vitaly/projects/funpay/webspec-proto/examples/minimal.yaml",
        "--target",
        "mock",
        "--output",
        dir.path().to_str().unwrap(),
    ]);
    // Point PATH to a directory without webspec-mock so discovery finds nothing.
    cmd.env("PATH", "/usr/bin:/bin");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("not registered"));
}
