use clap::{Parser, Subcommand, Args, ArgGroup};
use std::path::PathBuf;


#[derive(Parser)]
#[command(about = "Timestamping System for Research Data Management", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Generate template files for configuration
    Init(InitArgs),
    /// Start canis system
    Start(StartArgs),
    #[cfg(target_os = "linux")]
    /// Stop canis system
    Stop,
    /// Display digest about file
    Info(InfoArgs),
    /// Publish daily hash
    Publish(PublishArgs),
}

#[derive(Args)]
pub struct InitArgs {
    /// Generate only config file
    #[arg(short = 'c', long = "config")]
    pub config: bool,

    /// Generate only service file for monitoring (canis start)
    #[arg(short = 's', long = "start")]
    pub start: bool,

    /// Generate only service file for daily publishing (canis publish)
    #[arg(short = 'p', long = "publish")]
    pub publish: bool,
}

#[derive(Args)]
pub struct StartArgs {
    /// Specify config file
    #[arg(short = 'c', long = "config")]
    pub config: Option<PathBuf>,

    /// Start background
    #[cfg_attr(not(unix), arg(hide = true))]
    #[arg(short = 'd', long = "daemon")]
    pub daemon: bool,
}

#[derive(Args)]
pub struct InfoArgs {
    /// Path to the file to search
    #[arg(value_name = "FILEPATH")]
    pub filepath: String,

    /// Generate log about file
    #[arg(short = 'l', long = "log")]
    pub log: bool,
}

#[derive(Args)]
pub struct PublishArgs {
    /// Specify config file
    #[arg(short = 'c', long = "config")]
    pub config: Option<PathBuf>,
}
