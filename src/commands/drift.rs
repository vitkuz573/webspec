use crate::drift;
use crate::spec::ApiSpec;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, Default, clap::ValueEnum)]
pub enum OutputFormat {
    #[default]
    Table,
    Json,
}

#[derive(clap::Args)]
pub struct DriftArgs {
    #[arg(long)]
    pub spec: PathBuf,

    #[arg(long, value_enum, default_value = "table")]
    pub format: OutputFormat,

    #[arg(long)]
    pub dry_run: bool,

    #[arg(long, value_delimiter = ',')]
    pub pages: Option<Vec<String>>,
}

pub async fn run(args: DriftArgs) -> Result<i32, miette::Report> {
    let spec = ApiSpec::load(args.spec.to_string_lossy().as_ref())
        .await
        .map_err(|e| miette::Report::msg(format!("failed to load spec: {e}")))?;

    let client = reqwest::Client::new();
    match drift::run_drift(&spec, &client, args.dry_run).await {
        Ok(report) => {
            let output = match args.format {
                OutputFormat::Json => drift::report::format_json(&report),
                OutputFormat::Table => drift::report::format_table(&report),
            };
            println!("{output}");
            if report.drifted {
                Ok(1)
            } else {
                Ok(0)
            }
        }
        Err(e) => {
            eprintln!("Drift runner failed: {e}");
            Ok(2)
        }
    }
}
