//! This example connects via Bluetooth LE to the radio and prints out all received packets.
#[allow(unused)]
use std::collections::{BTreeMap, HashMap, VecDeque};

use anyhow::Result;
use clap::{Parser, Subcommand};

mod bbs;
mod mesh;
mod repl;

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
    /// Start BBS Service
    Start,
    /// Run REPL utility
    Repl,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_secs()
        .init();

    let cli = Cli::parse();
    match cli.command {
        Commands::Start => bbs::run_bbs().await?,
        Commands::Repl => repl::run_repl().await?,
    }

    Ok(())
}
