use crate::error::SpecError;
use crate::loader::SpecLoader;
use miette::Report;
use std::path::Path;

pub async fn run(
    spec: &Path,
    loader: &SpecLoader,
    _verbose: bool,
) -> Result<(), Report> {
    let loaded = loader
        .load(spec)
        .await
        .map_err(|e| SpecError::from_load(e, spec))?;

    let diagnostics = crate::validation::validate_spec(&loaded.value, loaded.source.as_deref())?;

    if diagnostics.is_empty() {
        println!("Spec is valid");
        Ok(())
    } else {
        for d in &diagnostics {
            eprintln!("{:?}", Report::new(d.clone()));
        }
        Err(Report::new(SpecError::Validation(format!(
            "{} validation error(s) found",
            diagnostics.len()
        ))))
    }
}
