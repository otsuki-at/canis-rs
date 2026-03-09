mod commands;
mod watcher;
mod processor;
mod adapter;
mod logger;
mod config;
mod observer;
mod error;
mod event;
mod cli;

use error::Result;
use cli::{Cli, Commands};
use clap::{Parser};

fn main() -> Result<()> {

    let cli = Cli::parse();

    match cli.command {
        Commands::Init (args) => {
            commands::init::init(args)?
        },
        Commands::Start (args) => {
            commands::start::start(args)?;
        },
        #[cfg(target_os = "linux")]
        Commands::Stop => {
            commands::stop::stop()?;
        },
        Commands::Info(args) => {
            if args.log {
                println!("InfoCommand is not implemented");
            } else {
                println!("InfoCommand is not implemented");
            }
        },
        Commands::Publish(args) =>{
            println!("PublishCommand is not implemented");
        },
    }

    Ok(())
}
