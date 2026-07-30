use crate::spec::ApiSpec;
use miette::Diagnostic;
use thiserror::Error;

pub mod export;
pub mod import;
pub mod info;
pub mod lossy;
pub mod naming;
pub mod schema;

#[derive(Debug, Error, Diagnostic, Clone)]
#[diagnostic()]
pub enum OpenapiError {
    #[error("OpenAPI parse error: {0}")]
    #[diagnostic(code(openapi::parse))]
    Parse(String),

    #[error("OpenAPI validation error: {0}")]
    #[diagnostic(code(openapi::validation))]
    Validation(String),

    #[error("unsupported OpenAPI feature: {feature}")]
    #[diagnostic(code(openapi::unsupported), help("{help}"))]
    Unsupported { feature: String, help: String },

    #[error("reference resolution failed: {0}")]
    #[diagnostic(code(openapi::unresolved_ref), help("Make sure the $ref target exists in the spec."))]
    UnresolvedRef(String),

    #[error("IO error: {0}")]
    #[diagnostic(code(openapi::io))]
    Io(String),
}

impl From<std::io::Error> for OpenapiError {
    fn from(e: std::io::Error) -> Self {
        OpenapiError::Io(e.to_string())
    }
}

impl From<serde_yaml::Error> for OpenapiError {
    fn from(e: serde_yaml::Error) -> Self {
        OpenapiError::Parse(e.to_string())
    }
}

impl From<serde_json::Error> for OpenapiError {
    fn from(e: serde_json::Error) -> Self {
        OpenapiError::Parse(e.to_string())
    }
}

impl From<oas3::spec::Error> for OpenapiError {
    fn from(e: oas3::spec::Error) -> Self {
        OpenapiError::Parse(e.to_string())
    }
}

pub fn convert_openapi_to_webspec(oas: &oas3::Spec) -> Result<ApiSpec, OpenapiError> {
    let mut report = lossy::LossReport::new();
    import::openapi_to_webspec(oas, &mut report)
}

pub fn convert_webspec_to_openapi(spec: &ApiSpec) -> Result<oas3::Spec, OpenapiError> {
    export::webspec_to_openapi(spec)
}

#[cfg(test)]
mod tests {
    #[test]
    fn module_compiles() {
        assert!(std::mem::size_of::<super::OpenapiError>() > 0);
    }
}
