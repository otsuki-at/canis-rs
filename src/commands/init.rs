use anyhow::Result;
use std::fs;
use std::path::PathBuf;
use dialoguer::{Input, Confirm};
use chrono::Utc;
use directories_next::ProjectDirs;

use crate::error::WatcherError;
use crate::cli::InitArgs;

pub fn init(args: InitArgs) -> Result<()>{
    if args.config.is_none() && args.start.is_none() && args.publish.is_none() {
        println!("Starting configuration file generation");
        init_config(&args)?;
        println!("Starting service file generation");
        init_start(&args)?;
        println!("Starting publish service file generation.");
        init_publish(&args)?;
    }

    if args.config.is_some() {
        println!("Starting configuration file generation");
        init_config(&args)?;
    }

    if args.start.is_some() {
        println!("Starting service file generation");
        init_start(&args)?;
    }

    if args.publish.is_some() {
        println!("Starting publish service file generation.");
        init_publish(&args)?;
    }

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


pub fn get_default_log_path(app_name: &str) -> Option<PathBuf> {
    directories_next::ProjectDirs::from("", "", app_name)
        .map(|proj_dirs| {
            let data_dir = proj_dirs.data_dir();
            data_dir.join("canis.log")
        })
}
    // ログファイルのパスを取得
    let dbfile = match &args.dbfile {
        Some(w) => w.clone(),
        None => {
            let proj_dirs = ProjectDirs::from("", "", "canis")
                .ok_or_else(|| WatcherError::ConfigError(
                    "Failed to retrieve XDG configuration directory".to_string()
                ))?;
            let default_dbfile = proj_dirs
                .data_dir()
                .join("canis.db");

            let default_dbfile_path_str = default_dbfile
                .to_string_lossy()
                .to_string()
                .replace('\\', "/");

            Input::new()
                .with_prompt("Enter databasefile path")
                .default(default_dbfile_path_str)
                .interact_text()?
        }
    };

    let dbfile_display = dbfile.replace('\\', "/");

    let token = match &args.token {
        Some(w) => w.clone(),
        None => Input::new()
            .with_prompt("Enter github token")
            .allow_empty(true)
            .interact_text()?
    };

    let hashdir = match &args.hashdir {
        Some(w) => w.clone(),
        None => {
            let proj_dirs = ProjectDirs::from("", "", "canis")
                .ok_or_else(|| WatcherError::ConfigError(
                    "Failed to retrieve XDG configuration directory".to_string()
                ))?;
            let dailyhashdir = proj_dirs
                .data_dir()
                .to_string_lossy()
                .to_string();

            let input: String = Input::new()
                .with_prompt("Enter path to store dailyhash")
                .default(dailyhashdir)
                .interact_text()?;
            PathBuf::from(input)
        }
    };
    let hashdir_display = hashdir.display().to_string().replace('\\', "/");

    let repo = match &args.repo {
        Some(w) => w.clone(),
        None => Input::new()
            .with_prompt("Enter github repository name")
            .default("dailyhash".to_string())
            .interact_text()?
    };

    let config_content = format!(
        r#"[basic_settings]
# Select the watcher implementation: notify or fuse
watcher = "{watcher}"

# Specify the paths of files or directories to monitor
# When using FUSE, only the first path in this list will be monitored
targets = [{targets}]

# Specify paths to files or directories whose operations should be ignored
# Any user actions on these paths will not be monitored or recorded
ignore = [{ignore}]

# Specify the path to the database file where digest will be stored
# Do not place the log file under any monitored path to avoid infinite loops
dbfile = "{dbfile}"

# Specify the GitHub access token used to publish hashes
# This token must have sufficient permissions to access the target repository
token = "{token}"

# Specify the directory where daily hash files will be stored
# The hash file for each day will be created under this directory
hashdir = "{hashdir}"

# Specify the name of the target GitHub repository
# The repository owner will be determined automatically from the access token
# The calculated daily hash will be published to this repository
repo = "{repo}"
"#,
        watcher = watcher,
        targets = targets,
        ignore=ignore,
        dbfile = dbfile_display,
        token = token,
        hashdir = hashdir_display,
        repo = repo,
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
        println!("To make changes, please edit the unit file\n");
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
        config_path = config_path.display(),
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
            let default_unitfile_path = home_dir.join("Library/LaunchAgents/com.canis.plist");

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

#[cfg(target_os = "linux")]
fn init_publish(args: &InitArgs)-> Result<()>{
    let home_dir = match std::env::var("HOME") {
        Ok(dir) => PathBuf::from(dir),
        Err(_) => {
            eprintln!("HOME environment variable is not set");
            std::process::exit(1);
        }
    };

    // timer ファイルのパス
    let timer_file_path = match &args.publish {
        Some(w) => w.clone(),
        None => {
            let default_timerfile_path = home_dir.join(".config/systemd/user/canis-publish.timer");

            PathBuf::from(
                Input::new()
                    .with_prompt("Enter timer file path")
                    .default(default_timerfile_path.display().to_string())
                    .interact_text()?
            )
        }
    };

    if timer_file_path.exists() {
        println!("{} already exists", timer_file_path.display());
        println!("To make changes, please edit the timer file\n");
        return Ok(());
    }

    // ユニットファイルのパス
    let unit_file_path = match &args.publish {
        Some(w) => {
            let unitfile_path = w.clone()
                .with_extension("service");

            unitfile_path
        }
        None => {
            let default_unitfile_path = home_dir.join(".config/systemd/user/canis-publish.service");

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
        println!("To make changes, please edit the unit file\n");
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

    // 日付を取得
    let date = match &args.date{
        Some(w) => w.clone(),
        None =>{
            let default_date = "yesterday".to_string();

            Input::new()
                .with_prompt("Enter date to create dailyhash")
                .default(default_date)
                .interact_text()?
        }
    };

    // 実行時間を取得
    let schedule = match &args.schedule{
        Some(w) => w.clone(),
        None =>{
            let default_schedule = "09:05".to_string();

            Input::new()
                .with_prompt("Enter schedule to create dailyhash")
                .default(default_schedule)
                .interact_text()?
        }
    };

    // timer ファイルの内容
    let timer_content = format!(
        r#"[Unit]
Description=Exec canis publish daily

[Timer]
OnCalendar=*-*-* {schedule}
Persistent=true
Unit={unit_file_path}

[Install]
WantedBy=timers.target
"#,
        schedule = schedule,
        unit_file_path = unit_file_path.display(),
    );

    // ファイルを作成して内容を書き込む
    fs::write(&timer_file_path, timer_content)?;
    println!("Timer file created at: {}", timer_file_path.display());

    // ユニットファイルの内容
    let unit_content = format!(
        r#"[Unit]
Description=canis publish

[Service]
Type=oneshot
ExecStart={binary_path} publish --date {date} --config {config_path}
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
fn init_publish(args: &InitArgs)-> Result<()>{
    // ユニットファイルのパス
    let unit_file_path = match &args.publish {
        Some(w) => w.clone(),
        None => {
            let proj_dirs = ProjectDirs::from("", "", "canis")
                .ok_or_else(|| WatcherError::ConfigError(
                    "Failed to retrieve XDG configuration directory".to_string()
                ))?;

            let default_path_str = proj_dirs
                .config_dir()
                .join("canis-publish.xml")
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

    // 日付を取得
    let date = match &args.date{
        Some(w) => w.clone(),
        None =>{
            let default_date = "yesterday".to_string();

            Input::new()
                .with_prompt("Enter date to create dailyhash")
                .default(default_date)
                .interact_text()?
        }
    };

    // 実行時間を取得
    let schedule = match &args.schedule{
        Some(w) => w.clone(),
        None =>{
            let default_schedule = "09:05".to_string();

            Input::new()
                .with_prompt("Enter schedule to create dailyhash")
                .default(default_schedule)
                .interact_text()?
        }
    };

    let today = Utc::now().date_naive();

    let unit_content = format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <Triggers>
    <CalendarTrigger>
      <StartBoundary>{today}T{schedule}:00</StartBoundary>
      <ScheduleByDay>
        <DaysInterval>1</DaysInterval>
      </ScheduleByDay>
    </CalendarTrigger>
  </Triggers>

  <Actions>
    <Exec>
      <Command>{binary_path}</Command>
      <Arguments>publish --date {date} --config {config_path}</Arguments>
    </Exec>
  </Actions>

  <Settings>
    <Enabled>true</Enabled>
    <ExecutionTimeLimit>PT1H</ExecutionTimeLimit>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
  </Settings>
</Task>
"#,
        today = today,
        schedule = schedule,
        binary_path = binary_path.display(),
        date = date,
        config_path = config_path.display(),
    );

    fs::write(&unit_file_path, unit_content)?;

    println!("WinSW service definition file created at: {}", unit_file_path.display());

    Ok(())
}

#[cfg(target_os = "macos")]
fn init_publish(args: &InitArgs)-> Result<()>{
    // launchd のユーザーエージェントディレクトリ
    let home_dir = match std::env::var("HOME") {
        Ok(dir) => PathBuf::from(dir),
        Err(_) => {
            eprintln!("HOME environment variable is not set");
            std::process::exit(1);
        }
    };

    // ユニットファイルのパス
    let unit_file_path = match &args.publish {
        Some(w) => w.clone(),
        None => {
            let default_unitfile_path = home_dir.join("Library/LaunchAgents/com.canis.publish.plist");

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

    // 日付を取得
    let date = match &args.date{
        Some(w) => w.clone(),
        None =>{
            let default_date = "yesterday".to_string();

            Input::new()
                .with_prompt("Enter date to create dailyhash")
                .default(default_date)
                .interact_text()?
        }
    };

    // 実行時間を取得
    let schedule = match &args.schedule{
        Some(w) => w.clone(),
        None =>{
            let default_schedule = "09:05".to_string();

            Input::new()
                .with_prompt("Enter schedule to create dailyhash")
                .default(default_schedule)
                .interact_text()?
        }
    };
    let (hour, minute) = {
        let parts: Vec<&str> = schedule.splitn(2, ':').collect();
        let hour: u32 = parts[0].parse();
        let minute: u32 = parts[1].parse();
        (hour, minute);
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

    let plist_content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>com.{username}.canis.publish</string>

  <!-- 実行コマンド -->
  <key>ProgramArguments</key>
  <array>
    <string>{bin_path}</string>
    <string>publish</string>
    <string>--date</string>
    <string>{date}</string>
    <string>--config</string>
    <string>{config_path}</string>
  </array>

  <key>StartCalendarInterval</key>
  <dict>
    <key>Hour</key>   <integer>{hour}</integer>
    <key>Minute</key> <integer>{minute}</integer>
  </dict>

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
}
