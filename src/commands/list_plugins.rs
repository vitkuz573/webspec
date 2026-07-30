use crate::error::SpecError;
use crate::plugins::PluginRegistry;
use miette::Report;

pub async fn run() -> Result<(), Report> {
    let mut registry = PluginRegistry::default();
    registry.discover().map_err(SpecError::from)?;

    let mut plugins: Vec<_> = registry.all();
    plugins.sort_by_key(|p| p.target());

    if plugins.is_empty() {
        println!("No plugins registered.");
        return Ok(());
    }

    println!("{:<20} {}", "TARGET", "NAME");
    for plugin in plugins {
        println!("{:<20} {}", plugin.target(), plugin.name());
    }

    Ok(())
}
