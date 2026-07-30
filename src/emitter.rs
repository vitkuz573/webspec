use std::path::Path;

pub fn write_file(path: &Path, content: &str) -> Result<(), crate::error::SpecError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| crate::error::SpecError::Io(e.to_string()))?;
    }
    std::fs::write(path, content).map_err(|e| crate::error::SpecError::Io(e.to_string()))?;
    log::info!("Wrote {}", path.display());
    Ok(())
}
