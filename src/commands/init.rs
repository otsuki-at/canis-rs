use anyhow::Result;
use std::fs;
use std::path::PathBuf;
use dialoguer::{Input, Confirm};

use crate::logger::{self};
use crate::error::WatcherError;
use crate::cli::InitArgs;

use directories_next::ProjectDirs;

pub fn init(args: InitArgs) -> Result<()>{
    if args.config.is_none() && args.start.is_none() && args.publish.is_none() {
        println!("Starting configuration file generation");
        init_config(&args)?;
        println!("Starting service file generation");
        init_start(&args)?;
        // println!("Starting publish service file generation.");
        // init_publish(&args)?;
    }

    if args.config.is_some() {
        println!("Starting configuration file generation");
        init_config(&args)?;
    }

    if args.start.is_some() {
        println!("Starting service file generation");
        init_start(&args)?;
    }

    // if args.publish.is_some() {
    //     println!("Starting publish service file generation.");
    //     init_publish(&args)?;
    // }

    Ok(())
}

fn init_config(args: &InitArgs) -> Result<()>{
    // 設定ファイルのパスを取得
    let config_path = match &args.config {
        Some(w) => w.clone(),
        None => {
            let proj_dirs = ProjectDirs::from("", "", "canis")
                .ok_or_else(|| WatcherError::ConfigError(
                    "Failed to retrieve XDG configuration directory".to_string()
                ))?;

            let default_path_str = proj_dirs
                .config_dir()
                .join("config.toml")
                .to_string_lossy()
                .to_string();

            PathBuf::from(
                Input::new()
                    .with_prompt("Enter config file path")
                    .default(default_path_str)
                    .interact_text()?
            )
        }
    };

    if config_path.exists() {
        println!("{} already exists", config_path.display());
        println!("To make changes, please edit the existing configuration file\n");
        return Ok(())
    }

    let watcher = match &args.watcher {
        Some(w) => w.clone(),
        None => Input::new()
            .with_prompt("Enter watcher implementation")
            .default("notify".to_string())
            .interact_text()?
    };

    // 監視対象パスを取得
    let watch_paths: Vec<PathBuf> = match &args.targets {
        Some(targets) if !targets.is_empty() => targets.clone(),
        _ => {
            let mut paths = Vec::new();

            loop {
                let path: String = Input::new()
                    .with_prompt(format!("Enter target path #{}", paths.len() + 1))
                    .allow_empty(true)
                    .interact_text()?;

                if path.is_empty() {
                    if paths.is_empty() {
                        println!("Please enter at least one target path.");
                        continue;
                    }
                    break;
                }

                paths.push(PathBuf::from(path));
            }

            paths
        }
    };

    // 設定ファイル用の文字列に変換
    let targets = watch_paths
        .iter()
        .map(|path| {
            let path_str = path.display().to_string().replace('\\', "/");
            format!(r#""{}""#, path_str)
        })
        .collect::<Vec<_>>()
        .join(", ");

    // 無視するパスを取得
    let ignore_paths: Vec<String> = match &args.ignore {
        Some(ignore) if !ignore.is_empty() => ignore.clone(),
        _ => {
            let mut paths: Vec<String> = Vec::new();

            loop {
                let path: String = Input::new()
                    .with_prompt(format!("Enter ignore path #{}", paths.len() + 1))
                    .allow_empty(true)
                    .interact_text()?;

                if path.is_empty() {
                    break;
                }

                paths.push(path);
            }

            paths
        }
    };

    // 設定ファイル用の文字列に変換
    let ignore = ignore_paths
        .iter()
        .map(|path| {
            let path_str = path.replace('\\', "/");
            format!(r#""{}""#, path_str)
        })
        .collect::<Vec<_>>()
        .join(", ");

    // ログファイルのパスを取得
    let logfile = match &args.logfile {
        Some(w) => w.clone(),
        None => {
            let default_logfile_path = logger::get_default_log_path("canis")
                .ok_or_else(|| {
                    eprintln!("Failed to retrieve XDG configuration directory");
                    std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        "XDG data directory not available"
                    )
                })?;

            let default_logfile_path_str = default_logfile_path
                .to_string_lossy()
                .to_string()
                .replace('\\', "/");

            PathBuf::from(
                Input::new()
                    .with_prompt("Enter logfile path")
                    .default(default_logfile_path_str)
                    .interact_text()?
            )
        }
    };

    let logfile_display = logfile.display().to_string().replace('\\', "/");

    let config_content = format!(
        r#"
[basic_settings]
# Select the watcher implementation: notify or fuse
watcher = "{watcher}"

# Specify the paths of files or directories to monitor
# When using FUSE, only the first path in this list will be monitored
targets = [{targets}]

# Specify paths to files or directories whose operations should be ignored
# Any user actions on these paths will not be monitored or recorded
ignore = [{ignore}]

# Specify the path to the log file where digest will be stored
# Do not place the log file under any monitored path to avoid infinite loops
logfile = "{logfile}"

# Specify the path to a local clone of the Git repository
# This repository is used to publish daily hash values
hashdir = "/path/to/local/gitrepository/"
"#,
        watcher = watcher,
        targets = targets,
        ignore=ignore,
        logfile = logfile_display,
    );

    fs::write(&config_path, config_content)?;

    println!("Configuration file created at: {}\n", config_path.display());

    Ok(())
}

#[cfg(target_os = "linux")]
fn init_start(args: &InitArgs)-> Result<()>{

    let home_dir = match std::env::var("HOME") {
        Ok(dir) => PathBuf::from(dir),
        Err(_) => {
            eprintln!("HOME environment variable is not set");
            std::process::exit(1);
        }
    };

    // ユニットファイルのパス
    let unit_file_path = match &args.start {
        Some(w) => w.clone(),
        None => {
            let default_unitfile_path = home_dir.join(".config/systemd/user/canis.service");

            PathBuf::from(
                Input::new()
                    .with_prompt("Enter unit file path")
                    .default(default_unitfile_path.display().to_string())
                    .interact_text()?
            )
        }
    };

    if unit_file_path.exists() {
        println!("{} already exists", unit_file_path.display());
        println!("To make changes, please edit the service configuration file\n");
        return Ok(());
    }

    // 実行ファイルのパス
    let binary_path = match &args.binary {
        Some(w) => w.clone(),
        None => {
            let default_binary_path = home_dir.join("bin/canis");

            PathBuf::from(
                Input::new()
                    .with_prompt("Enter binary file path")
                    .default(default_binary_path.display().to_string())
                    .interact_text()?
            )
        }
    };

    // 設定ファイルのパスを取得
    let config_path = match &args.config{
        Some(w) => w.clone(),
        None => {
            let proj_dirs = ProjectDirs::from("", "", "canis")
                .ok_or_else(|| WatcherError::ConfigError(
                    "Failed to retrieve XDG configuration directory".to_string()
                ))?;

            let default_path_str = proj_dirs
                .config_dir()
                .join("config.toml")
                .to_string_lossy()
                .to_string();

            PathBuf::from(
                Input::new()
                    .with_prompt("Enter config file path")
                    .default(default_path_str)
                    .interact_text()?
            )
        }
    };


    // ユニットファイルの内容
    let unit_content = format!(
        r#"[Unit]
Description=canis-start

[Service]
Type=simple
ExecStart={binary_path} start --config {config_path}
Restart=always

[Install]
WantedBy=default.target
"#,
        binary_path = binary_path.display(),
        config_path = config_path.display()
    );

    // ファイルを作成して内容を書き込む
    fs::write(&unit_file_path, unit_content)?;

    println!("Unit file created at: {}", unit_file_path.display());

    Ok(())
}

#[cfg(target_os = "windows")]
fn init_start(args: &InitArgs)-> Result<()>{
    // ユニットファイルのパス
    let unit_file_path = match &args.start {
        Some(w) => w.clone(),
        None => {
            let proj_dirs = ProjectDirs::from("", "", "canis")
                .ok_or_else(|| WatcherError::ConfigError(
                    "Failed to retrieve XDG configuration directory".to_string()
                ))?;
            let default_unitfile_path = proj_dirs.config_dir().join("canis.xml");

            PathBuf::from(
                Input::new()
                    .with_prompt("Enter unit file path")
                    .default(default_unitfile_path.display().to_string())
                    .interact_text()?
            )
        }
    };

    if unit_file_path.exists() {
        println!("{} already exists", unit_file_path.display());
        println!("To make changes, please edit the service configuration file\n");
        return Ok(());
    }

    // ユーザー名・ドメイン名の取得
    let username = whoami::username();
    let domain = whoami::devicename();

    // 実行ファイルのパス
    let binary_path = match &args.binary {
        Some(w) => w.clone(),
        None => {
            let default_binary_path = format!(r"C:\Users\{}\bin\canis.exe", username);

            PathBuf::from(
                Input::new()
                    .with_prompt("Enter binary file path")
                    .default(default_binary_path)
                    .interact_text()?
            )
        }
    };

    let unit_content = format!(
        r#"<service>
  <id>canis</id>
  <name>canis</name>
  <description>canis</description>
  <executable>{binary_path}</executable>
  <arguments>start --config {config_path}</arguments>
  <serviceaccount>
    <username>{domain}\{username}</username>
    <password></password>
    <allowservicelogon>true</allowservicelogon>
  </serviceaccount>
</service>
"#,
        binary_path = binary_path.display(),
        config_path = config.config_path.display(),
        domain = domain,
        username = username,
    );

    fs::write(&unit_file_path, unit_content)?;

    println!("WinSW service definition file created at: {}", unit_file_path.display());

    Ok(())
}

#[cfg(target_os = "macos")]
fn init_start(args: &InitArgs)-> Result<()>{
    // launchd のユーザーエージェントディレクトリ
    let home_dir = match std::env::var("HOME") {
        Ok(dir) => PathBuf::from(dir),
        Err(_) => {
            eprintln!("HOME environment variable is not set");
            std::process::exit(1);
        }
    };

    // ユニットファイルのパス
    let unit_file_path = match &args.start {
        Some(w) => w.clone(),
        None => {
            let default_unitfile_path = home_dir.join("Library/LaunchAgents/com.canis.start.plist");

            PathBuf::from(
                Input::new()
                    .with_prompt("Enter unit file path")
                    .default(default_unitfile_path.display().to_string())
                    .interact_text()?
            )
        }
    };

    if unit_file_path.exists() {
        println!("{} already exists", unit_file_path.display());
        println!("To make changes, please edit the service configuration file\n");
        return Ok(());
    }

    // 実行ファイルのパス
    let binary_path = match &args.binary {
        Some(w) => w.clone(),
        None => {
            let default_binary_path = home_dir.join("bin/canis");

            PathBuf::from(
                Input::new()
                    .with_prompt("Enter binary file path")
                    .default(default_binary_path.display().to_string())
                    .interact_text()?
            )
        }
    };

    // ログファイル(標準出力)のパス
    let stdout_path = match &args.daemon_out {
        Some(w) => w.clone(),
        None => {
            let default_stdout_path = home_dir.join("Library/Logs/canis.log");

            PathBuf::from(
                Input::new()
                    .with_prompt("Enter daemon stdout logfile path")
                    .default(default_stdout_path.display().to_string())
                    .interact_text()?
            )
        }
    };

    // ログファイル(標準エラー)のパス
    let stderr_path = match &args.daemon_err {
        Some(w) => w.clone(),
        None => {
            let default_stderr_path = home_dir.join("Library/Logs/canis.err");

            PathBuf::from(
                Input::new()
                    .with_prompt("Enter daemon stdout logfile path")
                    .default(default_stderr_path.display().to_string())
                    .interact_text()?
            )
        }
    };

    // 現在のユーザー名を取得
    let username = whoami::username();

    // plist ファイルの内容
    let plist_content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.{username}.canis</string>

    <key>ProgramArguments</key>
    <array>
        <string>{bin_path}</string>
        <string>start</string>
        <string>--config</string>
        <string>{config_path}</string>
    </array>

    <key>KeepAlive</key>
    <true/>

    <key>RunAtLoad</key>
    <true/>

    <key>StandardOutPath</key>
    <string>{stdout_path}</string>

    <key>StandardErrorPath</key>
    <string>{stderr_path}</string>
</dict>
</plist>
"#,
        username = username,
        bin_path = binary_path.display(),
        config_path = config.config_path.display(),
        stdout_path = stdout_path.display(),
        stderr_path = stderr_path.display()
    );

    // ファイルを作成して内容を書き込む
    fs::write(&unit_file_path, plist_content)?;

    println!("launchd plist file created at: {}", unit_file_path.display());

    Ok(())
}

// fn init_publish(){
//     println!("not implemented");
// }
