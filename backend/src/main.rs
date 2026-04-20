mod agent;
mod api;
mod common;
mod config;
mod db;
mod logger;
mod master;

use clap::{Parser, Subcommand};
use std::error::Error;

#[derive(Parser, Debug)]
#[clap(version = "0.2.0", author = "djkcyl")]
struct Cli {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// 运行为主节点服务器模式
    Master(config::MasterConfig),

    /// 运行为从节点客户端模式
    Agent(config::AgentConfig),
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();

    match cli.command {
        Command::Master(config) => {
            if let Err(e) = logger::init_tracing(&config.log_dir) {
                eprintln!("{}", e);
                return Err(e.into());
            }
            master::run(config).await
        }
        Command::Agent(config) => {
            if let Err(e) = logger::init_tracing(&config.log_dir) {
                eprintln!("{}", e);
                return Err(e.into());
            }
            agent::run(config).await
        }
    }
}
