use clap::{Parser, Subcommand};
use std::path::PathBuf;
use webspec::loader::SpecLoader;

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

#[derive(Subcommand)]
enum Commands {
    /// List built-in and discovered generator plugins.
    ListPlugins,
    Validate {
        #[arg(long)]
        spec: PathBuf,
    },
    Generate {
        #[arg(long)]
        spec: PathBuf,
        #[arg(long)]
        target: String,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        plugin: Option<PathBuf>,
        #[arg(long)]
        dry_run: bool,
    },
    Fmt {
        #[arg(long)]
        spec: PathBuf,
        #[arg(long)]
        check: bool,
        #[arg(long, alias = "in-place")]
        in_place: bool,
    },
    Migrate {
        #[arg(long)]
        spec: PathBuf,
        #[arg(long, default_value = "1.0.0")]
        to: String,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    Test {
        #[arg(long)]
        spec: PathBuf,
        #[arg(long)]
        target: String,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let log_level = if cli.quiet {
        "error"
    } else if cli.verbose {
        "debug"
    } else {
        "info"
    };
    std::env::set_var("RUST_LOG", log_level);
    let _ = env_logger::try_init();

    let loader = SpecLoader::new();

    let result = match cli.command {
        Commands::ListPlugins => webspec::commands::list_plugins::run().await,
        Commands::Validate { spec } => {
            webspec::commands::validate::run(&spec, &loader, cli.verbose).await
        }
        Commands::Generate {
            spec,
            target,
            output,
            plugin,
            dry_run,
        } => {
            webspec::commands::generate::run_with_registry(
                &spec, &target, &output, dry_run, cli.verbose, &loader, plugin.as_deref(),
            )
            .await
        }
        Commands::Fmt {
            spec,
            check,
            in_place,
        } => webspec::commands::fmt::run(&spec, check, in_place, &loader).await,
        Commands::Migrate { spec, to, output } => {
            webspec::commands::migrate::run(&spec, &to, &output, &loader).await
        }
        Commands::Test { spec, target } => {
            webspec::commands::test::run(&spec, &target, &loader).await
        }
    };

    if let Err(e) = result {
        eprintln!("{:?}", e);
        std::process::exit(1);
    }
}
