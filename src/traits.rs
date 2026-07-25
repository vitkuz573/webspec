use crate::spec::ApiSpec;

pub struct GeneratedOutput {
    pub files: Vec<(String, String)>,
}

pub trait LanguageGenerator {
    fn target(&self) -> &str;
    fn file_extension(&self) -> &str;
    fn generate(&self, spec: &ApiSpec) -> GeneratedOutput;
}
