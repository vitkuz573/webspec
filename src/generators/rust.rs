use crate::generators::ir::CodegenSpec;
use crate::generators::rust::render::{extraction, RustContext};
use crate::spec::ApiSpec;
use crate::traits::{GeneratedOutput, LanguageGenerator};
use askama::Template;

mod render;

pub struct RustGenerator;

pub mod filters {
    use askama::Values;

    #[askama::filter_fn]
    pub fn pascal(_input: &dyn std::fmt::Display, _env: &dyn Values) -> askama::Result<String> {
        let s = _input.to_string();
        Ok(s.split(['_', '-'])
            .map(|part| {
                let mut chars = part.chars();
                match chars.next() {
                    None => String::new(),
                    Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
                }
            })
            .collect())
    }

    #[askama::filter_fn]
    pub fn snake(_input: &dyn std::fmt::Display, _env: &dyn Values) -> askama::Result<String> {
        let s = _input.to_string();
        let mut out = String::new();
        let mut prev_lower = false;
        for (i, c) in s.chars().enumerate() {
            if c.is_uppercase() && i > 0 && prev_lower {
                out.push('_');
            }
            out.push(c.to_lowercase().next().unwrap_or(c));
            prev_lower = c.is_lowercase();
        }
        Ok(out)
    }

    #[askama::filter_fn]
    pub fn join(_input: &dyn std::fmt::Display, _env: &dyn Values, #[optional(" ")] sep: &str) -> askama::Result<String> {
        let s = _input.to_string();
        Ok(s.split(',').collect::<Vec<_>>().join(sep))
    }
}

#[derive(Template)]
#[template(path = "rust/templates/Cargo.toml.j2", escape = "none")]
struct CargoTomlTemplate<'a> {
    spec: &'a CodegenSpec,
    crate_name: &'a str,
}

#[derive(Template)]
#[template(path = "rust/templates/lib.rs.j2", escape = "none")]
struct LibRsTemplate;

#[derive(Template)]
#[template(path = "rust/templates/models.rs.j2", escape = "none")]
struct ModelsRsTemplate<'a> {
    spec: &'a CodegenSpec,
    ctx: RustContext,
}

#[derive(Template)]
#[template(path = "rust/templates/parser.rs.j2", escape = "none")]
struct ParserRsTemplate<'a> {
    spec: &'a CodegenSpec,
}

#[derive(Template)]
#[template(path = "rust/templates/client.rs.j2", escape = "none")]
struct ClientRsTemplate<'a> {
    spec: &'a CodegenSpec,
    ctx: RustContext,
    error_name: String,
}

#[derive(Template)]
#[template(path = "rust/templates/error.rs.j2", escape = "none")]
struct ErrorRsTemplate {
    ctx: RustContext,
}

impl LanguageGenerator for RustGenerator {
    fn target(&self) -> &str {
        "rust"
    }

    fn file_extension(&self) -> &str {
        "rs"
    }

    fn generate(&self, spec: &ApiSpec) -> GeneratedOutput {
        let ir = CodegenSpec::from_api_spec(spec);
        let ctx = RustContext::new(&ir);
        let crate_name = ctx.crate_name.clone();

        let files = vec![
            ("Cargo.toml".into(), CargoTomlTemplate { spec: &ir, crate_name: &crate_name }.render().unwrap()),
            ("src/lib.rs".into(), LibRsTemplate.render().unwrap()),
            ("src/models.rs".into(), ModelsRsTemplate { spec: &ir, ctx: RustContext::new(&ir) }.render().unwrap()),
            ("src/parser.rs".into(), ParserRsTemplate { spec: &ir }.render().unwrap()),
            ("src/client.rs".into(), ClientRsTemplate { spec: &ir, ctx: RustContext::new(&ir), error_name: ctx.error_name.clone() }.render().unwrap()),
            ("src/error.rs".into(), ErrorRsTemplate { ctx: RustContext::new(&ir) }.render().unwrap()),
        ];

        GeneratedOutput { files }
    }
}

impl<'a> ParserRsTemplate<'a> {
    fn extraction(&self, field: &crate::generators::ir::Field) -> String {
        extraction(field)
    }
}

