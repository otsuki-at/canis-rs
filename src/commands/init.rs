use std::sync::Arc;
use anyhow::Result;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

use crate::config::Config;
use crate::logger::{self, Logger, FileLogger};
use crate::error::WatcherError;
use crate::cli::InitArgs;

use directories_next::ProjectDirs;

pub fn init(args: InitArgs) -> Result<()>{
    let all = !args.config && !args.start && !args.publish;

    if args.config || all  {
        init_config()?;
    }
    if args.start || all {
        init_start();
    }
    if args.publish || all {
        init_publish();
    }
    Ok(())
}

fn init_config() -> Result<()>{
    use directories_next::ProjectDirs;

    let proj_dirs = ProjectDirs::from("", "", "canis")
        .ok_or_else(|| WatcherError::ConfigError(
            "XDG設定ディレクトリを取得できませんでした".to_string()
        ))?;

    let config_path = proj_dirs.config_dir().join("config.toml");

    fs::create_dir_all(proj_dirs.config_dir())?;

    let config_content = r#"
watcher_system = "notify"
processor_level = 2

# 監視するディレクトリのパス
watch_paths = [
    "/path/to/watch",
]

# ログファイルのパス(オプション)
log_file = "/path/to/canis.log"

# 日次ハッシュ公開用リポジトリのパス(オプション)
dailyhash_repository="/path/of/git/repository"
"#;

    fs::write(&config_path, config_content)?;

    println!("設定ファイルを作成しました: {}\n必要な設定は書き換えてください", config_path.display());

    Ok(())
}

#[cfg(target_os = "linux")]
fn init_start()-> Result<()>{
    let home_dir = match std::env::var("HOME") {
        Ok(dir) => PathBuf::from(dir),
        Err(_) => {
            eprintln!("エラー: HOME環境変数が設定されていません");
            std::process::exit(1);
        }
    };

    // systemd ユーザーディレクトリのパス
    let systemd_user_dir = home_dir.join(".config/systemd/user");

    // ディレクトリが存在しない場合は作成
    fs::create_dir_all(&systemd_user_dir)?;

    // ユニットファイルのパス
    let unit_file_path = systemd_user_dir.join("canis-start.service");

    // 設定ファイルのパス
    let proj_dirs = ProjectDirs::from("", "", "canis")
        .ok_or_else(|| WatcherError::ConfigError(
            "XDG設定ディレクトリを取得できませんでした".to_string()
        ))?;
    let config_path = proj_dirs.config_dir().join("config.toml");

    let username = whoami::username();

    // ユニットファイルの内容
    let unit_content = format!(
        r#"[Unit]
Description=canis-start

[Service]
Type=simple
User={username}
ExecStart=%h/.local/bin/canis start --config {config_path}
Restart=always

[Install]
WantedBy=default.target
"#,
        username = username,
        config_path = config_path.display()
    );

    // ファイルを作成して内容を書き込む
    fs::write(&unit_file_path, unit_content)?;

    println!("systemd ユニットファイルを作成しました: {}", unit_file_path.display());

    Ok(())
}

#[cfg(target_os = "windows")]
fn init_start()-> Result<()>{
    // サービス定義の出力先
    let localappdata_dir = std::env::var("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            eprintln!("エラー: LOCALAPPDATA環境変数が設定されていません");
            std::process::exit(1);
        });

    let service_dir = localappdata_dir.join("winsw");
    fs::create_dir_all(&service_dir)?;

    let unit_file_path = service_dir.join("canis-start.xml");

    // 設定ファイルのパス
    let proj_dirs = ProjectDirs::from("", "", "canis")
        .ok_or_else(|| WatcherError::ConfigError(
            "XDG設定ディレクトリを取得できませんでした".to_string()
        ))?;
    let config_path = proj_dirs.config_dir().join("config.toml");

    // ユーザー名・ドメイン名の取得 (whoami クレートを使用)
    let username = whoami::username();
    let domain = whoami::devicename();

    // 実行ファイルのパスを自動取得
    let localappdata_dir = std::env::var("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            eprintln!("エラー: LOCALAPPDATA環境変数が設定されていません");
            std::process::exit(1);
        });
    let service_dir = localappdata_dir.join("canis");
    let exe_path = service_dir.join("canis.exe");

    let unit_content = format!(
        r#"<service>
  <id>canis-run</id>
  <name>canis-run</name>
  <description>canis-run</description>
  <executable>{exe_path}</executable>
  <arguments>start --config {config_path}</arguments>
  <serviceaccount>
    <username>{domain}\{username}</username>
    <password></password>
    <allowservicelogon>true</allowservicelogon>
  </serviceaccount>
</service>
"#,
        exe_path = exe_path.display(),
        config_path = config_path.display(),
        domain = domain,
        username = username,
    );

    fs::write(&unit_file_path, unit_content)?;

    println!("WinSW サービス定義ファイルを作成しました: {}", unit_file_path.display());

    Ok(())
}

#[cfg(target_os = "macos")]
fn init_start()-> Result<()>{
    // launchd のユーザーエージェントディレクトリ
    let home_dir = match std::env::var("HOME") {
        Ok(dir) => PathBuf::from(dir),
        Err(_) => {
            eprintln!("エラー: HOME環境変数が設定されていません");
            std::process::exit(1);
        }
    };
    let launchd_dir = home_dir.join("Library/LaunchAgents");

    // ディレクトリが存在しない場合は作成
    fs::create_dir_all(&launchd_dir)?;

    // plist ファイルのパス
    let plist_path = launchd_dir.join("com.canis.start.plist");

    // 実行ファイルのパス（~/.local/bin/canis）
    let bin_path = home_dir.join(".local/bin/canis");

    // 設定ファイルのパス
    let proj_dirs = ProjectDirs::from("", "", "canis")
        .ok_or_else(|| WatcherError::ConfigError(
            "XDG設定ディレクトリを取得できませんでした".to_string()
        ))?;
    let config_path = proj_dirs.config_dir().join("config.toml");

    // ログディレクトリのパス
    let log_dir = home_dir.join("Library/Logs");
    fs::create_dir_all(&log_dir)?;

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
    <string>com.{username}.canis-start</string>

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
    <string>{log_dir}/canis-start.log</string>

    <key>StandardErrorPath</key>
    <string>{log_dir}/canis-start.err</string>
</dict>
</plist>
"#,
        username = username,
        bin_path = bin_path.display(),
        config_path = config_path.display(),
        log_dir = log_dir.display()
    );

    // ファイルを作成して内容を書き込む
    fs::write(&plist_path, plist_content)?;

    println!("launchd plist ファイルを作成しました: {}", plist_path.display());

    Ok(())
}

fn init_publish(){
    println!("not implemented");
}
