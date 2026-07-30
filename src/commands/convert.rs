use crate::emitter;
use crate::error::SpecError;
use crate::loader::SpecLoader;
use crate::openapi::{convert_openapi_to_webspec, convert_webspec_to_openapi, OpenapiError};
use crate::spec::ApiSpec;
use miette::Report;
use std::path::Path;

const MAX_INPUT_SIZE: u64 = 10 * 1024 * 1024;

fn parse_oas(raw: &str) -> Result<oas3::Spec, OpenapiError> {
    serde_yaml::from_str::<oas3::Spec>(raw)
        .or_else(|_| serde_json::from_str::<oas3::Spec>(raw))
        .map_err(OpenapiError::from)
}

fn oas_to_yaml(oas: &oas3::Spec) -> Result<String, OpenapiError> {
    serde_yaml::to_string(oas).map_err(|e| OpenapiError::Parse(e.to_string()))
}

pub async fn run(
    from: &Path,
    to: &Path,
    target: &str,
    dry_run: bool,
    verbose: bool,
    loader: &SpecLoader,
) -> Result<(), Report> {
    let metadata = std::fs::metadata(from).map_err(|e| SpecError::from_load(e.into(), from))?;
    if metadata.len() > MAX_INPUT_SIZE {
        return Err(Report::new(OpenapiError::Validation(format!(
            "input file exceeds 10 MiB limit ({} bytes)",
            metadata.len()
        ))));
    }

    let raw = std::fs::read_to_string(from).map_err(|e| SpecError::from_load(e.into(), from))?;

    if dry_run {
        let summary = match target {
            "webspec" => {
                let oas = parse_oas(&raw)?;
                let spec = convert_openapi_to_webspec(&oas).map_err(|e| Report::new(e))?;
                let yaml = spec.to_yaml().map_err(|e| Report::new(SpecError::YamlParse(e.to_string())))?;
                format!("[dry-run] would write {} lines to {}", yaml.lines().count(), to.display())
            }
            "openapi" => {
                let loaded = loader.load(from).await.map_err(|e| SpecError::from_load(e, from))?;
                let spec: ApiSpec = serde_yaml::from_value(loaded.value.clone()).map_err(|e| SpecError::YamlParse(e.to_string()))?;
                let oas = convert_webspec_to_openapi(&spec).map_err(|e| Report::new(e))?;
                let yaml = oas_to_yaml(&oas)?;
                format!("[dry-run] would write {} lines to {}", yaml.lines().count(), to.display())
            }
            _ => return Err(Report::new(SpecError::UnsupportedTarget { target: target.to_string() })),
        };
        println!("{summary}");
        return Ok(());
    }

    match target {
        "webspec" => {
            let oas = parse_oas(&raw)?;
            let spec = convert_openapi_to_webspec(&oas).map_err(|e| Report::new(e))?;

            let json_value = serde_json::to_value(&spec).map_err(|e| SpecError::Validation(e.to_string()))?;
            let diagnostics = crate::validation::validate_spec_by_json(&json_value, Some(from))?;
            if !diagnostics.is_empty() {
                for d in &diagnostics {
                    eprintln!("{:?}", Report::new(d.clone()));
                }
                return Err(Report::new(SpecError::Validation(format!(
                    "{} validation error(s) found",
                    diagnostics.len()
                ))));
            }

            let yaml = spec.to_yaml().map_err(|e| SpecError::YamlParse(e.to_string()))?;
            emitter::write_file(to, &yaml)?;
        }
        "openapi" => {
            let loaded = loader.load(from).await.map_err(|e| SpecError::from_load(e, from))?;
            let spec: ApiSpec = serde_yaml::from_value(loaded.value.clone()).map_err(|e| SpecError::YamlParse(e.to_string()))?;
            let oas = convert_webspec_to_openapi(&spec).map_err(|e| Report::new(e))?;
            let yaml = oas_to_yaml(&oas)?;
            emitter::write_file(to, &yaml)?;
        }
        _ => return Err(Report::new(SpecError::UnsupportedTarget { target: target.to_string() })),
    }

    if verbose {
        println!("Converted {} -> {} (target: {})", from.display(), to.display(), target);
    }

    Ok(())
}
