//! This example connects via Bluetooth LE to the radio and prints out all received packets.
#[allow(unused)]
use std::collections::{BTreeMap, HashMap, VecDeque};

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::screen::NoScreen;

mod bbs;
mod mesh;
mod screen;
mod tool;

include!(concat!(env!("OUT_DIR"), "/build_info.rs"));

#[derive(Parser)]
#[command(name = "MeshBoard")]
#[command(version = VERSION)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Display test
    Start,
    /// Display test
    StartNoDisplay,
    /// Run REPL utility
    MeshTool,
}

#[cfg(target_os = "linux")]
async fn run_bbs_display() -> Result<()> {
    let display = crate::screen::epd::EpdScreen::new()?;
    bbs::run_bbs(display).await?;
    Ok(())
}

#[cfg(not(target_os = "linux"))]
async fn run_bbs_display() -> Result<()> {
    use crate::screen::NoScreen;

    bbs::run_bbs(NoScreen {}).await?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_secs()
        .init();

    let cli = Cli::parse();
    match cli.command {
        Commands::Start => run_bbs_display().await?,
        Commands::StartNoDisplay => bbs::run_bbs(NoScreen {}).await?,
        Commands::MeshTool => tool::run_tool().await?,
    }

    Ok(())
}
