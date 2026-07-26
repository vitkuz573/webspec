use clap::Parser;
use colored::Colorize;
use webspec::error::SpecError;
use webspec::generators::python::PythonGenerator;
use webspec::generators::rust::RustGenerator;
use webspec::generators::typescript::TypeScriptGenerator;
use webspec::llm::client::LlmClient;
use webspec::llm::ChatMessage;
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
        #[arg(long)]
        api_url: String,
        #[arg(long)]
        api_key: String,
        #[arg(long)]
        model: String,
        #[arg(long, default_value_t = 1)]
        depth: u32,
        #[arg(long, default_value_t = 15)]
        pages: usize,
        #[arg(long)]
        no_cache: bool,
    },
    TestLlm,
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
        Commands::Discover { url, output, api_url, api_key, model, depth, pages, no_cache } => cmd_discover(&url, output, &api_url, &api_key, &model, depth, pages, no_cache).await,
        Commands::TestLlm => cmd_test_llm().await,
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

async fn cmd_discover(url: &str, output: Option<PathBuf>, api_url: &str, api_key: &str, model: &str, depth: u32, max_pages: usize, no_cache: bool) -> anyhow::Result<()> {
    println!(
        "{} Analyzing {} (depth={}, max_pages={})...",
        "Discovering".cyan().bold(),
        url, depth, max_pages
    );

    let config = webspec::discover::DiscoverConfig {
        url: url.to_string(),
        api_url: api_url.to_string(),
        api_key: api_key.to_string(),
        model: model.to_string(),
        output: output.clone(),
        depth,
        max_pages,
        no_cache,
    };

    let result = webspec::discover::discover(config).await?;

    let raw_data = &result.raw_data;
    println!(
        "\n{} {}",
        "Titles:".green().bold(),
        raw_data.titles.join(", ")
    );
    println!(
        "  {} selectors, {} data-* attributes, {} URL patterns",
        raw_data.selectors.len(),
        raw_data.data_attributes.len(),
        raw_data.url_patterns.len()
    );

    if !raw_data.pages_crawled.is_empty() {
        println!(
            "\n{} {} pages crawled:",
            "Pages:".green().bold(),
            raw_data.pages_crawled.len()
        );
        for page in &raw_data.pages_crawled {
            println!(
                "  {} ({})",
                page.url.cyan(),
                page.title.dimmed(),
            );
        }
    }

    if !result.spec.entities.is_empty() {
        println!(
            "\n{} {} entities in spec:",
            "Entities:".green().bold(),
            result.spec.entities.len()
        );
        for (name, entity) in &result.spec.entities {
            let field_count = entity.fields.as_ref().map_or(0, |f| f.len());
            println!("  {} ({} fields)", name.cyan(), field_count);
        }
    }

    if !raw_data.url_patterns.is_empty() {
        println!(
            "\n{} {} patterns found:",
            "URL Patterns:".green().bold(),
            raw_data.url_patterns.len()
        );
        for pattern in &raw_data.url_patterns {
            println!(
                "  {} ({} samples, params: {:?})",
                pattern.pattern.cyan(),
                pattern.samples.len(),
                pattern.parameters
            );
        }
    }

    if let Some(path) = &output {
        std::fs::write(path, &result.yaml)?;
        println!(
            "\n{} Saved to {}",
            "Done!".green().bold(),
            path.display()
        );
    } else {
        println!("\n{}", "=== YAML Output ===".green().bold());
        println!("{}", result.yaml);
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

async fn cmd_test_llm() -> anyhow::Result<()> {
    let base_url = "http://127.0.0.1:5200/v1";
    let api_key = "test-key";
    let model = "opencode/mimo-v2.5-free";

    println!("{}", "Testing LLM client...".cyan().bold());
    println!("  base_url: {base_url}");
    println!("  model: {model}");

    let client = LlmClient::new(base_url, api_key, model);

    println!("\n{}", "Listing models:".green().bold());
    match client.list_models().await {
        Ok(models) => {
            if models.is_empty() {
                println!("  No models found");
            } else {
                for m in &models {
                    println!("  - {m}");
                }
            }
        }
        Err(e) => {
            eprintln!("  {} Failed to list models: {e}", "error".red().bold());
        }
    }

    println!("\n{}", "Sending chat message:".green().bold());
    let messages = vec![
        ChatMessage {
            role: "system".to_string(),
            content: "You are a helpful assistant.".to_string(),
        },
        ChatMessage {
            role: "user".to_string(),
            content: "Say hello in exactly 5 words.".to_string(),
        },
    ];

    match client.chat(messages).await {
        Ok(response) => {
            println!("  Response: {response}");
        }
        Err(e) => {
            eprintln!("  {} Chat failed: {e}", "error".red().bold());
        }
    }

    Ok(())
}
