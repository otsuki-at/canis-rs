use clap::{Parser, Subcommand, Args};
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
    /// Path to configuration file
    #[arg(short = 'c', long = "config")]
    pub config: Option<PathBuf>,

    /// File system watcher backend to use
    #[arg(long)]
    pub watcher: Option<String>,

    /// Paths to watch for operation
    #[arg(long, value_delimiter = ',')]
    pub targets: Option<Vec<PathBuf>>,

    /// Path to log file for digest
    #[arg(long)]
    pub logfile: Option<PathBuf>,

    /// Paths to ignore by the file system watcher
    #[arg(long, value_delimiter = ',')]
    pub ignore: Option<Vec<String>>,

    /// Local git repository path for hash storage and publication
    #[arg(long)]
    pub hashdir: Option<PathBuf>,

    /// Path to daemon stdout log file
    #[cfg(target_os = "macos")]
    #[arg(long)]
    pub daemon_out: Option<PathBuf>,

    /// Path to daemon stderr log file
    #[cfg(target_os = "macos")]
    #[arg(long)]
    pub daemon_err: Option<PathBuf>,

    /// Generate service file for monitoring (canis start)
    #[arg(short = 's', long = "start")]
    pub start: Option<PathBuf>,

    /// Path to canis binary file
    #[arg(short = 'b', long = "binary")]
    pub binary: Option<PathBuf>,

    /// Generate only service file for daily publishing (canis publish)
    #[arg(short = 'p', long = "publish")]
    pub publish: Option<PathBuf>,
}

#[derive(Args)]
pub struct StartArgs {
    /// Specify config file
    #[arg(short = 'c', long = "config")]
    pub config: Option<PathBuf>,

    /// File system watcher backend to use
    #[arg(long)]
    pub watcher: Option<String>,

    /// Paths to watch for operation
    #[arg(long, value_delimiter = ',')]
    pub targets: Option<Vec<String>>,

    /// Path to log file for digest
    #[arg(long)]
    pub logfile: Option<String>,

    /// Paths to ignore by the file system watcher
    #[arg(long, value_delimiter = ',')]
    pub ignore: Option<Vec<String>>,

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

impl StartArgs {
    pub fn is_complete(&self) -> bool {
        self.watcher.as_ref().map_or(false, |w| !w.is_empty())
            && self.targets.as_ref().map_or(false, |t| {
                !t.is_empty() && t.iter().all(|s| !s.is_empty())
            })
            && self.logfile.as_ref().map_or(false, |l| !l.is_empty())
    }
}
