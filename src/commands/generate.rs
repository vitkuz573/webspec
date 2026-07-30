use crate::emitter;
use crate::error::SpecError;
use crate::loader::SpecLoader;
use crate::plugins::{GenerateRequest, PluginRegistry};
use miette::Report;
use std::path::Path;

pub async fn run(
    spec: &Path,
    target: &str,
    output: &Path,
    dry_run: bool,
    verbose: bool,
    loader: &SpecLoader,
) -> Result<(), Report> {
    run_with_registry(spec, target, output, dry_run, verbose, loader, None).await
}

pub async fn run_with_registry(
    spec: &Path,
    target: &str,
    output: &Path,
    dry_run: bool,
    verbose: bool,
    loader: &SpecLoader,
    explicit_plugin: Option<&Path>,
) -> Result<(), Report> {
    let loaded = loader
        .load(spec)
        .await
        .map_err(|e| SpecError::from_load(e, spec))?;

    let spec_value: serde_json::Value = serde_json::to_value(&loaded.value)
        .map_err(|e| SpecError::YamlParse(e.to_string()))?;

    let mut registry = PluginRegistry::default();
    registry.discover().map_err(SpecError::from)?;

    if let Some(path) = explicit_plugin {
        registry.register_external(target, path);
    }

    let request = GenerateRequest::new(target, spec_value, output.to_path_buf());

    if verbose {
        println!("Loaded spec from {}", spec.display());
        println!("  target: {}", target);
        println!("  output: {}", output.display());
    }

    let response = registry
        .generate(target, &request)
        .map_err(|e| Report::new(SpecError::from(e)))?;

    if dry_run {
        println!("[dry-run] Would generate {} files:", response.files.len());
        for file in &response.files {
            println!("  -> {}/{}", output.display(), file.path);
        }
        return Ok(());
    }

    for file in &response.files {
        let full_path = output.join(&file.path);
        emitter::write_file(&full_path, &file.content)?;
    }

    println!(
        "Generated {} files in {}",
        response.files.len(),
        output.display()
    );

    Ok(())
}
