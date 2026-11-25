use anyhow::Result;
use clap::{Parser, Subcommand};
use fpk_core::{fetch_manifest, game_dir, install_client, install_runtime_files, list_versions};

const DEFAULT_GAME_VERSION: &str = "26.3";

#[derive(Parser)]
#[command(name = "fpk", version, about = "FPK Launcher - Minecraft Java 26.3 bootstrap")]
struct Cli { #[command(subcommand)] command: Command }
#[derive(Subcommand)]
enum Command {
    /// Show launcher data directory
    Home,
    /// List Mojang versions (release, snapshot, or all)
    List { #[arg(default_value = "release")] kind: String },
    /// Download version metadata and vanilla client JAR (defaults to Minecraft Java 26.3)
    Install { #[arg(default_value = DEFAULT_GAME_VERSION)] version: String },
}

#[tokio::main]
async fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Home => println!("{}", game_dir()?.display()),
        Command::List { kind } => {
            let manifest = fetch_manifest().await?;
            println!("Latest release: {} | snapshot: {}", manifest.latest.release, manifest.latest.snapshot);
            for item in list_versions(&manifest, &kind) {
                println!("{}\t{}", item.kind, item.id);
            }
        }
        Command::Install { version } => { install_client(&version).await?; install_runtime_files(&version).await?; }
    }
    Ok(())
}
