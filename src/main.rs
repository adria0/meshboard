//! This example connects via Bluetooth LE to the radio and prints out all received packets.
#[allow(unused)]
use std::collections::{BTreeMap, HashMap, VecDeque};

use anyhow::Result;
use clap::{Parser, Subcommand};

mod bbs;
mod display;
mod mesh;
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
    TestDisplay,
    Start,
    /// Run REPL utility
    MeshTool,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_secs()
        .init();

    let cli = Cli::parse();
    match cli.command {
        Commands::TestDisplay => display::test_display()?,
        Commands::Start => bbs::run_bbs().await?,
        Commands::MeshTool => tool::run_tool().await?,
    }

    Ok(())
}
