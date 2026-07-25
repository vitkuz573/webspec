use colored::Colorize;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SpecError {
    #[error("YAML parse error: {0}")]
    YamlParse(#[from] serde_yaml::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("invalid spec: {0}")]
    Validation(String),

    #[error("file not found: {path}")]
    FileNotFound { path: String },

    #[error("unsupported target: {target}")]
    UnsupportedTarget { target: String },
}

impl SpecError {
    pub fn print_colored(&self, file: Option<&Path>) {
        match self {
            SpecError::YamlParse(e) => {
                let loc = format_yaml_error(e);
                eprintln!(
                    "{} {}{}",
                    "error".red().bold(),
                    loc.dimmed(),
                    format!("{}", e).red()
                );
                eprintln!(
                    "  {} check YAML syntax and structure",
                    "hint:".yellow().bold()
                );
            }
            SpecError::Io(e) => {
                eprintln!(
                    "{} {}",
                    "error".red().bold(),
                    format!("{}", e).red()
                );
            }
            SpecError::Validation(msg) => {
                let loc = file
                    .map(|p| format!("{}:", p.display()))
                    .unwrap_or_default();
                eprintln!(
                    "{} {}{}",
                    "error".red().bold(),
                    loc.dimmed(),
                    msg.red()
                );
            }
            SpecError::FileNotFound { path } => {
                eprintln!(
                    "{} {} {}",
                    "error".red().bold(),
                    format!("file not found: {}", path).red(),
                    "".dimmed()
                );
                eprintln!(
                    "  {} check the path and try again",
                    "hint:".yellow().bold()
                );
            }
            SpecError::UnsupportedTarget { target } => {
                eprintln!(
                    "{} {}",
                    "error".red().bold(),
                    format!("unsupported target: `{}`", target).red()
                );
                eprintln!(
                    "  {} use `webspec list-targets` to see available targets",
                    "hint:".yellow().bold()
                );
            }
        }
    }
}

fn format_yaml_error(e: &serde_yaml::Error) -> String {
    if let Some(loc) = e.location() {
        format!("line {}:{}: ", loc.line(), loc.column())
    } else {
        String::new()
    }
}

pub fn print_error_chain(err: &anyhow::Error) {
    eprintln!("\n{}", "Error chain:".red().bold());
    for (i, cause) in err.chain().enumerate() {
        if i == 0 {
            eprintln!("  {} {}", "=>".red(), cause.to_string().red());
        } else {
            eprintln!("  {} {}", "  cause:".dimmed(), cause.to_string().dimmed());
        }
    }
}
