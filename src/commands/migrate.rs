use crate::error::SpecError;
use crate::loader::SpecLoader;
use miette::Report;
use std::path::{Path, PathBuf};

pub async fn run(
    spec: &Path,
    to: &str,
    output: &Option<PathBuf>,
    loader: &SpecLoader,
) -> Result<(), Report> {
    let loaded = loader
        .load(spec)
        .await
        .map_err(|e| SpecError::from_load(e, spec))?;

    match to {
        "1.0.0" => {
            let yaml = serde_yaml::to_string(&loaded.value)
                .map_err(|e| SpecError::YamlParse(e.to_string()))?;
            if let Some(out) = output {
                std::fs::write(out, yaml).map_err(|e| SpecError::Io(e.to_string()))?;
                println!("Migrated spec written to {}", out.display());
            } else {
                print!("{}", yaml);
            }
            Ok(())
        }
        _ => Err(Report::new(SpecError::UnsupportedVersion {
            version: to.to_string(),
        })),
    }
}
