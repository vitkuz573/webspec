use crate::error::SpecError;
use crate::loader::SpecLoader;
use miette::Report;
use std::path::Path;

pub async fn run(
    spec: &Path,
    check: bool,
    in_place: bool,
    loader: &SpecLoader,
) -> Result<(), Report> {
    let loaded = loader
        .load(spec)
        .await
        .map_err(|e| SpecError::from_load(e, spec))?;

    let formatted = serde_yaml::to_string(&loaded.value)
        .map_err(|e| SpecError::YamlParse(e.to_string()))?;

    let current = match &loaded.raw {
        Some(raw) => raw.clone(),
        None => serde_yaml::to_string(&loaded.value)
            .map_err(|e| SpecError::YamlParse(e.to_string()))?,
    };

    let normalized_current = current.trim_end_matches('\n').to_string() + "\n";
    let normalized_formatted = formatted.trim_end_matches('\n').to_string() + "\n";

    if check {
        if normalized_current == normalized_formatted {
            println!("Spec is already formatted");
            Ok(())
        } else {
            Err(Report::new(SpecError::Validation(
                "Spec is not formatted (run without --check to format)".into(),
            )))
        }
    } else if in_place {
        std::fs::write(spec, &normalized_formatted)
            .map_err(|e| SpecError::Io(e.to_string()))?;
        println!("Formatted {}", spec.display());
        Ok(())
    } else {
        print!("{}", normalized_formatted);
        Ok(())
    }
}
