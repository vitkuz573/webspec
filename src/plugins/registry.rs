use crate::error::PluginError;
use crate::plugins::builtin;
use crate::plugins::external::ExternalPlugin;
use crate::plugins::protocol::{GenerateRequest, GenerateResponse};
use crate::traits::Plugin;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Registry of built-in and discovered external generator plugins.
pub struct PluginRegistry {
    plugins: HashMap<String, Box<dyn Plugin>>,
}

impl Default for PluginRegistry {
    fn default() -> Self {
        let mut registry = Self::empty();
        registry.register(builtin::rust());
        registry.register(builtin::typescript());
        registry.register(builtin::python());
        registry
    }
}

impl PluginRegistry {
    pub fn empty() -> Self {
        Self {
            plugins: HashMap::new(),
        }
    }

    pub fn register<P: Plugin + 'static>(&mut self, plugin: P) {
        self.plugins.insert(plugin.target().to_string(), Box::new(plugin));
    }

    /// Register an external plugin by explicit path.
    pub fn register_external(&mut self, target: impl Into<String>, path: impl AsRef<Path>) {
        let target = target.into();
        self.plugins.insert(
            target.clone(),
            Box::new(ExternalPlugin::new(target, path.as_ref().to_path_buf())),
        );
    }

    pub fn resolve(&self, target: &str) -> Result<&dyn Plugin, PluginError> {
        self.plugins
            .get(target)
            .map(|p| p.as_ref())
            .ok_or_else(|| PluginError::not_found(target))
    }

    pub fn all(&self) -> Vec<&dyn Plugin> {
        self.plugins.values().map(|p| p.as_ref()).collect()
    }

    /// Discover `webspec-<target>` executables on PATH and in `WEBSPEC_PLUGIN_DIR`.
    pub fn discover(&mut self) -> Result<(), PluginError> {
        let mut search_dirs: Vec<PathBuf> = Vec::new();

        if let Ok(plugin_dir) = std::env::var("WEBSPEC_PLUGIN_DIR") {
            for dir in std::env::split_paths(&plugin_dir) {
                search_dirs.push(dir);
            }
        }

        if let Ok(path) = std::env::var("PATH") {
            for dir in std::env::split_paths(&path) {
                search_dirs.push(dir);
            }
        }

        let current_exe = std::env::current_exe().ok();

        for dir in search_dirs {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };

            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();

                // Skip the CLI binary itself to avoid self-invocation.
                if current_exe.as_ref().map(|e| e == &path).unwrap_or(false) {
                    continue;
                }

                let Some(name) = path.file_stem().and_then(|s| s.to_str()) else {
                    continue;
                };

                if let Some(target) = parse_plugin_name(name) {
                    if self.plugins.contains_key(target) {
                        // Built-ins take precedence.
                        continue;
                    }

                    if is_executable(&path) {
                        self.register_external(target, &path);
                    }
                }
            }
        }

        Ok(())
    }

    pub fn generate(&self, target: &str, request: &GenerateRequest) -> Result<GenerateResponse, PluginError> {
        self.resolve(target)?.generate(request)
    }
}

/// Parse `webspec-<target>` or `webspec-<target>.exe` into the target name.
fn parse_plugin_name(name: &str) -> Option<&str> {
    let name = name.strip_suffix(".exe").unwrap_or(name);
    name.strip_prefix("webspec-")
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(windows)]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

#[cfg(not(any(unix, windows)))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::PathBuf;

    #[test]
    fn default_registry_has_builtins() {
        let registry = PluginRegistry::default();
        assert!(registry.resolve("rust").is_ok());
        assert!(registry.resolve("typescript").is_ok());
        assert!(registry.resolve("python").is_ok());
    }

    #[test]
    fn resolves_builtin_by_target() {
        let registry = PluginRegistry::default();
        let plugin = registry.resolve("rust").unwrap();
        assert_eq!(plugin.target(), "rust");
    }

    #[test]
    fn builtin_plugin_generates_files() {
        let registry = PluginRegistry::default();
        let request = GenerateRequest::new(
            "rust",
            json!({"version": "1.0.0", "name": "Minimal"}),
            PathBuf::from("/tmp/out"),
        );
        let response = registry.generate("rust", &request).unwrap();
        assert!(!response.files.is_empty());
    }

    #[test]
    fn parse_plugin_name_matches() {
        assert_eq!(parse_plugin_name("webspec-mock"), Some("mock"));
        assert_eq!(parse_plugin_name("webspec-mock.exe"), Some("mock"));
        assert_eq!(parse_plugin_name("not-a-plugin"), None);
        assert_eq!(parse_plugin_name("webspec"), None);
    }
}
