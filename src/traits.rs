use crate::plugins::protocol::{GenerateRequest, GenerateResponse};

/// Legacy output format used by the built-in string-concat generators.
pub struct GeneratedOutput {
    pub files: Vec<(String, String)>,
}

/// Unified plugin trait exposed by both built-in and external generators.
pub trait Plugin: Send + Sync {
    /// Short target name (e.g. `rust`, `typescript`, `python`).
    fn target(&self) -> &str;

    /// Human-readable name for display (defaults to target).
    fn name(&self) -> &str {
        self.target()
    }

    /// Execute generation for the given request.
    fn generate(&self, request: &GenerateRequest) -> Result<GenerateResponse, crate::error::PluginError>;
}

/// Legacy trait kept for backwards compatibility with the existing generators.
pub trait LanguageGenerator {
    fn target(&self) -> &str;
    fn file_extension(&self) -> &str;
    fn generate(&self, spec: &crate::spec::ApiSpec) -> GeneratedOutput;
}
