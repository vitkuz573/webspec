use clap::Parser;
use webspec::spec::ApiSpec;
use webspec::generators::rust::RustGenerator;
use webspec::traits::LanguageGenerator;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "webspec")]
#[command(about = "Universal spec-to-code generator for web scraping SDKs")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::Subcommand)]
enum Commands {
    Generate {
        #[arg(long)]
        spec: PathBuf,
        #[arg(long)]
        target: String,
        #[arg(long)]
        output: PathBuf,
    },
    Validate {
        #[arg(long)]
        spec: PathBuf,
    },
    ListTargets,
}

fn main() -> anyhow::Result<()> {
    env_logger::init();
    let cli = Cli::parse();

    match cli.command {
        Commands::Generate { spec, target, output } => {
            let spec = ApiSpec::load(spec.to_str().unwrap())?;
            println!("Loaded spec '{}' v{}", spec.name, spec.version);

            let gen: Box<dyn LanguageGenerator> = match target.as_str() {
                "rust" => Box::new(RustGenerator),
                _ => anyhow::bail!("Unknown target: {}", target),
            };

            let out = gen.generate(&spec);
            for (path, content) in &out.files {
                let full_path = output.join(path);
                webspec::emitter::write_file(&full_path, content)?;
            }
            println!("Generated {} files in {}", out.files.len(), output.display());
        }
        Commands::Validate { spec } => {
            let spec = ApiSpec::load(spec.to_str().unwrap())?;
            println!("Spec '{}' v{} is valid", spec.name, spec.version);
            println!("  types: {}", spec.types.len());
            println!("  enums: {}", spec.enums.len());
            println!("  entities: {}", spec.entities.len());
            println!("  pages: {}", spec.pages.len());
        }
        Commands::ListTargets => {
            println!("Available targets:");
            println!("  rust     - Rust SDK (reqwest + scraper)");
        }
    }

    Ok(())
}
