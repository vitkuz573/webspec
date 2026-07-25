use clap::Parser;
use colored::Colorize;
use webspec::error::SpecError;
use webspec::generators::python::PythonGenerator;
use webspec::generators::rust::RustGenerator;
use webspec::generators::typescript::TypeScriptGenerator;
use webspec::spec::ApiSpec;
use webspec::traits::LanguageGenerator;
use webspec::validation;
use webspec::analyzer;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "webspec")]
#[command(about = "Universal spec-to-code generator for web scraping SDKs")]
#[command(version)]
struct Cli {
    #[arg(long, short = 'v', global = true)]
    verbose: bool,

    #[arg(long, short = 'q', global = true)]
    quiet: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::Subcommand)]
enum Commands {
    Generate {
        #[arg(long)]
        spec: String,
        #[arg(long)]
        target: String,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
    Validate {
        #[arg(long)]
        spec: String,
    },
    Watch {
        #[arg(long)]
        spec: PathBuf,
        #[arg(long)]
        target: String,
        #[arg(long)]
        output: PathBuf,
    },
    ListTargets,
    Discover {
        #[arg(long)]
        url: String,
        #[arg(long, short = 'o')]
        output: Option<PathBuf>,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let log_level = if cli.verbose {
        "debug"
    } else if cli.quiet {
        "error"
    } else {
        "info"
    };
    std::env::set_var("RUST_LOG", log_level);
    env_logger::init();

    let result = match cli.command {
        Commands::Generate {
            spec,
            target,
            output,
            dry_run,
        } => cmd_generate(&spec, &target, &output, dry_run, cli.verbose).await,
        Commands::Validate { spec } => cmd_validate(&spec).await,
        Commands::Watch {
            spec,
            target,
            output,
        } => cmd_watch(&spec, &target, &output).await,
        Commands::ListTargets => cmd_list_targets(),
        Commands::Discover { url, output } => cmd_discover(&url, output).await,
    };

    if let Err(e) = result {
        if let Some(spec_err) = e.downcast_ref::<SpecError>() {
            spec_err.print_colored(None);
        } else {
            eprintln!("{} {}", "error".red().bold(), e);
        }
        std::process::exit(1);
    }
}

fn resolve_generator(target: &str) -> anyhow::Result<Box<dyn LanguageGenerator>> {
    match target {
        "rust" => Ok(Box::new(RustGenerator)),
        "typescript" | "ts" => Ok(Box::new(TypeScriptGenerator)),
        "python" | "py" => Ok(Box::new(PythonGenerator)),
        _ => Err(SpecError::UnsupportedTarget {
            target: target.to_string(),
        }
        .into()),
    }
}

async fn cmd_generate(
    spec_path: &str,
    target: &str,
    output: &PathBuf,
    dry_run: bool,
    verbose: bool,
) -> anyhow::Result<()> {
    let spec = ApiSpec::load(spec_path).await?;

    let gen = resolve_generator(target)?;

    if verbose {
        println!(
            "{} {} v{}",
            "Loaded spec:".green().bold(),
            spec.name,
            spec.version
        );
        println!("  target: {}", target);
        println!("  output: {}", output.display());
        println!("  types: {}, enums: {}, entities: {}, pages: {}",
            spec.types.len(),
            spec.enums.len(),
            spec.entities.len(),
            spec.pages.len()
        );
    }

    let out = gen.generate(&spec);

    if dry_run {
        println!(
            "\n{} Would generate {} files:",
            "[dry-run]".cyan().bold(),
            out.files.len().to_string().cyan()
        );
        for (path, _) in &out.files {
            println!("  -> {}/{}", output.display(), path);
        }
        if verbose {
            println!("\n{}", "File contents:".dimmed());
            for (path, content) in &out.files {
                println!(
                    "\n{} {}",
                    "---".dimmed(),
                    format!("{}/{}", output.display(), path).dimmed()
                );
                for line in content.lines() {
                    println!("  {}", line.dimmed());
                }
                println!("{}", "---".dimmed());
            }
        }
        return Ok(());
    }

    for (path, content) in &out.files {
        let full_path = output.join(path);
        webspec::emitter::write_file(&full_path, content)?;
    }
    println!(
        "{} Generated {} files in {}",
        "Done!".green().bold(),
        out.files.len().to_string().green(),
        output.display()
    );

    Ok(())
}

async fn cmd_validate(spec_path: &str) -> anyhow::Result<()> {
    let spec = ApiSpec::load(spec_path).await?;
    let result = validation::validate(&spec);

    println!(
        "Validating {} v{}...",
        spec.name,
        spec.version
    );
    result.print_report();

    if !result.is_valid() {
        std::process::exit(1);
    }
    Ok(())
}

async fn cmd_watch(
    spec_path: &PathBuf,
    target: &str,
    output: &PathBuf,
) -> anyhow::Result<()> {
    use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
    use std::sync::mpsc;

    let spec_path_clone = spec_path.clone();
    let target_clone = target.to_string();
    let output_clone = output.clone();

    let gen = resolve_generator(&target_clone)?;
    let handle = tokio::runtime::Handle::current();

    println!(
        "{} Watching {} for changes (target: {}, output: {})",
        "👀".cyan(),
        spec_path_clone.display(),
        target_clone,
        output_clone.display()
    );
    println!("Press Ctrl+C to stop\n");

    let (tx, rx) = mpsc::channel::<notify::Result<Event>>();

    let mut watcher = RecommendedWatcher::new(
        tx,
        Config::default().with_poll_interval(std::time::Duration::from_secs(2)),
    )?;

    if let Some(parent) = spec_path_clone.parent() {
        watcher.watch(parent.as_ref(), RecursiveMode::NonRecursive)?;
    }

    loop {
        match rx.recv() {
            Ok(Ok(event)) => {
                if let notify::EventKind::Modify(_) = event.kind {
                    for path in &event.paths {
                        if path == &spec_path_clone {
                            println!(
                                "{} Spec changed, regenerating...",
                                "[watch]".yellow().bold()
                            );
                            match handle.block_on(ApiSpec::load(spec_path_clone.to_str().unwrap())) {
                                Ok(spec) => {
                                    let out = gen.generate(&spec);
                                    for (path, content) in &out.files {
                                        let full_path = output_clone.join(path);
                                        let _ = webspec::emitter::write_file(&full_path, content);
                                    }
                                    println!(
                                        "{} Regenerated {} files",
                                        "Done!".green().bold(),
                                        out.files.len()
                                    );
                                }
                                Err(e) => {
                                    if let Some(spec_err) = e.downcast_ref::<SpecError>() {
                                        spec_err.print_colored(Some(&spec_path_clone));
                                    } else {
                                        eprintln!("{} {}", "error".red().bold(), e);
                                    }
                                }
                            }
                            break;
                        }
                    }
                }
            }
            Ok(Err(e)) => {
                eprintln!("{} {}", "Watch error:".red().bold(), e);
            }
            Err(e) => {
                eprintln!("{} {}", "Channel error:".red().bold(), e);
                break;
            }
        }
    }

    Ok(())
}

async fn cmd_discover(url: &str, output: Option<PathBuf>) -> anyhow::Result<()> {
    println!(
        "{} Analyzing {}...",
        "Discovering".cyan().bold(),
        url
    );

    let result = analyzer::analyze_url(url).await?;

    println!(
        "\n{} Title: {}",
        "Page:".green().bold(),
        result.title
    );
    println!(
        "  HTML: {} -> {} bytes ({:.0}% reduction)",
        result.raw_html_size,
        result.reduced_html_size,
        100.0 * (1.0 - result.reduced_html_size as f64 / result.raw_html_size.max(1) as f64)
    );

    if !result.entities.is_empty() {
        println!(
            "\n{} {} entities found:",
            "Entities:".green().bold(),
            result.entities.len()
        );
        for entity in &result.entities {
            println!(
                "  {} ({} items, confidence: {:.0}%)",
                entity.name.cyan(),
                entity.item_count,
                entity.confidence * 100.0
            );
            for field in &entity.fields {
                println!(
                    "    {} -> {} ({:?}, confidence: {:.0}%)",
                    field.name.yellow(),
                    field.css_selector.dimmed(),
                    field.field_type,
                    field.confidence * 100.0
                );
                if !field.sample_values.is_empty() {
                    println!(
                        "      samples: {:?}",
                        &field.sample_values[..field.sample_values.len().min(3)]
                    );
                }
            }
        }
    }

    if !result.url_patterns.is_empty() {
        println!(
            "\n{} {} patterns found:",
            "URL Patterns:".green().bold(),
            result.url_patterns.len()
        );
        for pattern in &result.url_patterns {
            println!(
                "  {} ({} samples, params: {:?})",
                pattern.pattern.cyan(),
                pattern.samples.len(),
                pattern.parameters
            );
        }
    }

    let yaml = result.to_yaml();

    if let Some(path) = output {
        std::fs::write(&path, &yaml)?;
        println!(
            "\n{} Saved to {}",
            "Done!".green().bold(),
            path.display()
        );
    } else {
        println!("\n{}", "=== YAML Output ===".green().bold());
        println!("{}", yaml);
    }

    Ok(())
}

fn cmd_list_targets() -> anyhow::Result<()> {
    println!("{}", "Available targets:".green().bold());
    println!("  {:<12} {}", "rust", "Rust SDK (reqwest + scraper)".dimmed());
    println!("  {:<12} {}", "typescript", "TypeScript SDK (axios + cheerio)".dimmed());
    println!("  {:<12} {}  ", "python", "Python SDK (httpx + beautifulsoup4)".dimmed());
    Ok(())
}
