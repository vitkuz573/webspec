use std::fs;
use std::path::PathBuf;

use webspec::generators::ir::CodegenSpec;
use webspec::generators::{rust::RustGenerator, typescript::TypeScriptGenerator};
use webspec::spec::ApiSpec;
use webspec::traits::LanguageGenerator;

fn load_minimal_spec() -> ApiSpec {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../webspec-proto/examples/minimal.yaml");
    let text = fs::read_to_string(&path).expect("minimal.yaml fixture missing");
    serde_yaml::from_str(&text).expect("invalid minimal.yaml")
}

#[test]
fn rust_generator_minimal_snapshot() {
    let spec = load_minimal_spec();
    let ir = CodegenSpec::from_api_spec(&spec);
    let gen = RustGenerator;
    let out = gen.generate(&spec);

    for (path, content) in out.files {
        let suffix = path.replace('/', "__");
        insta::assert_snapshot!(format!("rust_minimal_{}", suffix), content);
    }
}

#[test]
fn typescript_generator_minimal_snapshot() {
    let spec = load_minimal_spec();
    let gen = TypeScriptGenerator;
    let out = gen.generate(&spec);

    for (path, content) in out.files {
        let suffix = path.replace('/', "__");
        insta::assert_snapshot!(format!("typescript_minimal_{}", suffix), content);
    }
}
