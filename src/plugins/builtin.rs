use crate::generators::rust::RustGenerator;
use crate::generators::typescript::TypeScriptGenerator;
use crate::generators::python::PythonGenerator;
use crate::plugins::protocol::{GenerateRequest, GenerateResponse, GeneratedFile};
use crate::spec::ApiSpec;
use crate::traits::{GeneratedOutput, LanguageGenerator, Plugin};

/// Adapter wrapping a legacy `LanguageGenerator` so it behaves as a `Plugin`.
pub struct BuiltinPlugin<G> {
    target: &'static str,
    name: &'static str,
    generator: G,
}

impl<G> BuiltinPlugin<G> {
    pub fn new(target: &'static str, name: &'static str, generator: G) -> Self {
        Self {
            target,
            name,
            generator,
        }
    }
}

impl<G: LanguageGenerator + Send + Sync> Plugin for BuiltinPlugin<G> {
    fn target(&self) -> &str {
        self.target
    }

    fn name(&self) -> &str {
        self.name
    }

    fn generate(&self, request: &GenerateRequest) -> Result<GenerateResponse, crate::error::PluginError> {
        let spec: ApiSpec = serde_json::from_value(request.spec.clone())
            .map_err(|e| crate::error::PluginError::process_failed(request.target.clone(), e.to_string()))?;

        let output: GeneratedOutput = self.generator.generate(&spec);
        let files = output
            .files
            .into_iter()
            .map(|(path, content)| GeneratedFile { path, content })
            .collect();

        Ok(GenerateResponse {
            files,
            diagnostics: Vec::new(),
            unsupported_protocol_version: None,
        })
    }
}

/// Convenience constructors for the built-in generators.
pub fn rust() -> BuiltinPlugin<RustGenerator> {
    BuiltinPlugin::new("rust", "Rust", RustGenerator)
}

pub fn typescript() -> BuiltinPlugin<TypeScriptGenerator> {
    BuiltinPlugin::new("typescript", "TypeScript", TypeScriptGenerator)
}

pub fn python() -> BuiltinPlugin<PythonGenerator> {
    BuiltinPlugin::new("python", "Python", PythonGenerator)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::protocol::PROTOCOL_VERSION;
    use serde_json::json;
    use std::path::PathBuf;

    fn minimal_spec_value() -> serde_json::Value {
        json!({
            "version": "1.0.0",
            "name": "Minimal",
            "base_url": "https://example.com"
        })
    }

    #[test]
    fn builtin_rust_target_and_name() {
        let plugin = rust();
        assert_eq!(plugin.target(), "rust");
        assert_eq!(plugin.name(), "Rust");
    }

    #[test]
    fn builtin_rust_generates_expected_files() {
        let plugin = rust();
        let request = GenerateRequest::new("rust", minimal_spec_value(), PathBuf::from("/tmp/out"));
        let response = plugin.generate(&request).unwrap();
        let paths: Vec<_> = response.files.iter().map(|f| f.path.as_str()).collect();
        assert!(paths.contains(&"Cargo.toml"));
        assert!(paths.contains(&"src/lib.rs"));
        assert!(paths.contains(&"src/models.rs"));
        assert!(paths.contains(&"src/parser.rs"));
        assert!(paths.contains(&"src/client.rs"));
        assert!(paths.contains(&"src/error.rs"));
    }

    #[test]
    fn builtin_typescript_generates_expected_files() {
        let plugin = typescript();
        let request = GenerateRequest::new("typescript", minimal_spec_value(), PathBuf::from("/tmp/out"));
        let response = plugin.generate(&request).unwrap();
        let paths: Vec<_> = response.files.iter().map(|f| f.path.as_str()).collect();
        assert!(paths.contains(&"package.json"));
        assert!(paths.contains(&"tsconfig.json"));
        assert!(paths.contains(&"src/types.ts"));
        assert!(paths.contains(&"src/models.ts"));
        assert!(paths.contains(&"src/parser.ts"));
        assert!(paths.contains(&"src/client.ts"));
    }

    #[test]
    fn builtin_python_generates_expected_files() {
        let plugin = python();
        let request = GenerateRequest::new("python", minimal_spec_value(), PathBuf::from("/tmp/out"));
        let response = plugin.generate(&request).unwrap();
        let paths: Vec<_> = response.files.iter().map(|f| f.path.as_str()).collect();
        assert!(paths.contains(&"pyproject.toml"));
        assert!(paths.contains(&"src/__init__.py"));
        assert!(paths.contains(&"src/types.py"));
        assert!(paths.contains(&"src/models.py"));
        assert!(paths.contains(&"src/parser.py"));
        assert!(paths.contains(&"src/client.py"));
    }

    #[test]
    fn request_protocol_version_is_expected() {
        let request = GenerateRequest::new("rust", minimal_spec_value(), PathBuf::from("/tmp/out"));
        assert_eq!(request.protocol_version, PROTOCOL_VERSION);
    }
}
