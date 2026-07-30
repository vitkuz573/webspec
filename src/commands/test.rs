use crate::commands::generate;
use crate::error::SpecError;
use crate::loader::SpecLoader;
use miette::Report;
use std::path::Path;

pub async fn run(
    spec: &Path,
    target: &str,
    loader: &SpecLoader,
) -> Result<(), Report> {
    let temp_dir = tempfile::tempdir()
        .map_err(|e| SpecError::Io(e.to_string()))?;

    generate::run(spec, target, temp_dir.path(), false, false, loader).await?;

    let entries: Vec<_> = std::fs::read_dir(temp_dir.path())
        .map_err(|e| SpecError::Io(e.to_string()))?
        .filter_map(|e| e.ok())
        .collect();

    if entries.is_empty() {
        return Err(Report::new(SpecError::Validation(
            "No files were generated".into(),
        )));
    }

    if target == "rust" {
        if which::which("cargo").is_ok() {
            let status = std::process::Command::new("cargo")
                .arg("check")
                .current_dir(temp_dir.path())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();

            match status {
                Ok(s) if s.success() => {
                    println!("Rust smoke-check passed");
                }
                _ => {
                    println!("Rust smoke-check skipped (cargo check failed or unavailable)");
                }
            }
        } else {
            println!("Rust smoke-check skipped (cargo not found on PATH)");
        }
    } else {
        println!("Smoke-check: {} files emitted", entries.len());
    }

    Ok(())
}
