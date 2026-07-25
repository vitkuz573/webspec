use clap::Parser;
use colored::Colorize;
use webspec::error::SpecError;
use webspec::generators::python::PythonGenerator;
use webspec::generators::rust::RustGenerator;
use webspec::generators::typescript::TypeScriptGenerator;
use webspec::spec::ApiSpec;
use webspec::traits::LanguageGenerator;
use webspec::validation;
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
        spec: PathBuf,
        #[arg(long)]
        target: String,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
    Validate {
        #[arg(long)]
        spec: PathBuf,
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
}

fn main() {
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
        } => cmd_generate(&spec, &target, &output, dry_run, cli.verbose),
        Commands::Validate { spec } => cmd_validate(&spec),
        Commands::Watch {
            spec,
            target,
            output,
        } => cmd_watch(&spec, &target, &output),
        Commands::ListTargets => cmd_list_targets(),
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

fn cmd_generate(
    spec_path: &PathBuf,
    target: &str,
    output: &PathBuf,
    dry_run: bool,
    verbose: bool,
) -> anyhow::Result<()> {
    let spec = ApiSpec::load(spec_path.to_str().unwrap())?;

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

fn cmd_validate(spec_path: &PathBuf) -> anyhow::Result<()> {
    let spec = ApiSpec::load(spec_path.to_str().unwrap())?;
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

fn cmd_watch(
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
                            match ApiSpec::load(spec_path_clone.to_str().unwrap()) {
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

fn cmd_list_targets() -> anyhow::Result<()> {
    println!("{}", "Available targets:".green().bold());
    println!("  {:<12} {}", "rust", "Rust SDK (reqwest + scraper)".dimmed());
    println!("  {:<12} {}", "typescript", "TypeScript SDK (axios + cheerio)".dimmed());
    println!("  {:<12} {}  ", "python", "Python SDK (httpx + beautifulsoup4)".dimmed());
    Ok(())
}
