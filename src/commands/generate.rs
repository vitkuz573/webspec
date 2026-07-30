use crate::error::SpecError;
use crate::loader::SpecLoader;
use crate::spec::ApiSpec;
use crate::traits::LanguageGenerator;
use crate::generators::rust::RustGenerator;
use crate::generators::typescript::TypeScriptGenerator;
use crate::generators::python::PythonGenerator;
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
    let loaded = loader
        .load(spec)
        .await
        .map_err(|e| SpecError::from_load(e, spec))?;

    let spec: ApiSpec = serde_yaml::from_value(loaded.value.clone())
        .map_err(|e| SpecError::YamlParse(e.to_string()))?;

    let gen = resolve_generator(target)?;

    if verbose {
        println!("Loaded spec: {} v{}", spec.name, spec.version);
        println!("  target: {}", target);
        println!("  output: {}", output.display());
    }

    let out = gen.generate(&spec);

    if dry_run {
        println!("[dry-run] Would generate {} files:", out.files.len());
        for (path, _) in &out.files {
            println!("  -> {}/{}", output.display(), path);
        }
        return Ok(());
    }

    for (path, content) in &out.files {
        let full_path = output.join(path);
        crate::emitter::write_file(&full_path, content)?;
    }
    println!(
        "Generated {} files in {}",
        out.files.len(),
        output.display()
    );

    Ok(())
}

fn resolve_generator(target: &str) -> Result<Box<dyn LanguageGenerator>, Report> {
    match target {
        "rust" => Ok(Box::new(RustGenerator)),
        "typescript" | "ts" => Ok(Box::new(TypeScriptGenerator)),
        "python" | "py" => Ok(Box::new(PythonGenerator)),
        _ => Err(Report::new(SpecError::UnsupportedTarget {
            target: target.to_string(),
        })),
    }
}
