use std::fs;
use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};
use conflux_core::parse::parse_clash_yaml;
use conflux_core::{
    fetch_and_normalize, normalize, parse_and_normalize, parse_body, ConfluxError,
    ConfluxSubscription, ParseResult, SubscriptionExtras, SubscriptionFormat,
};
use conflux_daemon::{load_config, run_daemon};
use thiserror::Error;

#[derive(Debug, Parser)]
#[command(name = "conflux", version, about = "Conflux subscription engine CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Download and normalize a subscription URL.
    Fetch {
        url: String,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Convert a local subscription file to normalized JSON.
    Convert {
        file: PathBuf,
        #[arg(short, long, value_enum, default_value_t = ConvertFormat::Auto)]
        format: ConvertFormat,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Validate a normalized profile JSON file.
    Validate { file: PathBuf },
    /// Run the IPC daemon in the foreground.
    Daemon,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ConvertFormat {
    Auto,
    Clash,
    #[value(name = "uri-list")]
    UriList,
}

#[derive(Debug, Error)]
enum CliError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Core(#[from] ConfluxError),

    #[error("validation failed: {0}")]
    Validation(String),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("daemon error: {0}")]
    Daemon(#[from] conflux_daemon::DaemonError),

    #[error("config error: {0}")]
    Config(#[from] conflux_daemon::ConfigError),
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    if let Err(err) = run(cli).await {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> Result<(), CliError> {
    match cli.command {
        Commands::Fetch { url, output } => fetch_command(&url, &output).await,
        Commands::Convert {
            file,
            format,
            output,
        } => convert_command(&file, format, &output),
        Commands::Validate { file } => validate_command(&file),
        Commands::Daemon => daemon_command().await,
    }
}

async fn fetch_command(url: &str, output: &PathBuf) -> Result<(), CliError> {
    let profile = fetch_and_normalize(url).await?;
    write_profile(output, &profile)?;
    println!(
        "wrote {} nodes to {}",
        profile.nodes.len(),
        output.display()
    );
    Ok(())
}

fn convert_command(
    file: &PathBuf,
    format: ConvertFormat,
    output: &PathBuf,
) -> Result<(), CliError> {
    let body = fs::read_to_string(file)?;
    let profile = match format {
        ConvertFormat::Auto => parse_and_normalize(&body, None, None)?,
        ConvertFormat::Clash => {
            let parsed = parse_clash_forced(&body)?;
            normalize(parsed, None, None)?
        }
        ConvertFormat::UriList => {
            let parsed = parse_body(&body, None)?;
            if parsed.format == SubscriptionFormat::ClashYaml {
                return Err(CliError::Validation(
                    "input looks like Clash YAML; use --format clash or auto".into(),
                ));
            }
            normalize(parsed, None, None)?
        }
    };
    write_profile(output, &profile)?;
    println!(
        "wrote {} nodes to {}",
        profile.nodes.len(),
        output.display()
    );
    Ok(())
}

fn parse_clash_forced(body: &str) -> Result<ParseResult, ConfluxError> {
    let parsed = parse_clash_yaml(body)?;
    Ok(ParseResult {
        format: SubscriptionFormat::ClashYaml,
        nodes: parsed.nodes,
        body_metadata: Default::default(),
        extras: SubscriptionExtras {
            clash_proxy_groups: parsed.proxy_groups,
            clash_rules: parsed.rules,
        },
        expanded_body: body.trim().to_string(),
    })
}

fn validate_command(file: &PathBuf) -> Result<(), CliError> {
    let text = fs::read_to_string(file)?;
    let profile: ConfluxSubscription = serde_json::from_str(&text)
        .map_err(|err| CliError::Validation(format!("invalid JSON profile: {err}")))?;

    if profile.nodes.is_empty() {
        return Err(CliError::Validation("profile contains no nodes".into()));
    }

    for node in &profile.nodes {
        if node.server.trim().is_empty() {
            return Err(CliError::Validation(format!(
                "node {} has empty server",
                node.id
            )));
        }
        if node.port == 0 {
            return Err(CliError::Validation(format!(
                "node {} has invalid port",
                node.id
            )));
        }
    }

    println!(
        "valid profile: title={:?}, nodes={}",
        profile.title,
        profile.nodes.len()
    );
    Ok(())
}

async fn daemon_command() -> Result<(), CliError> {
    let loaded = load_config()?;
    run_daemon(loaded).await?;
    Ok(())
}

fn write_profile(path: &PathBuf, profile: &ConfluxSubscription) -> Result<(), CliError> {
    let json = serde_json::to_string_pretty(profile)?;
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    fs::write(path, json)?;
    Ok(())
}
