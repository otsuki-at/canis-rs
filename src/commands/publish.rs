use std::sync::Arc;
use std::fs;
use sha2::{Digest, Sha256};
use chrono::{Utc, Duration, NaiveDate};
use base64::{engine::general_purpose, Engine as _};
use serde_json::json;
use directories_next::ProjectDirs;

use crate::config::Config;
use crate::error::{Result, WatcherError};
use crate::cli::PublishArgs;
use crate::db::EventRepository;

pub fn publish(args: PublishArgs) -> Result<()>{
    // 設定ファイルを取得
    let config = if args.is_complete() {
        None
    } else if let Some(config_path) = args.config {
        Some(Config::from_file(&config_path)?)
    } else {
        Some(Config::from_xdg()?)
    };

    let settings = config.as_ref().map(|c| &c.basic_settings);

    let database_path = args.dbfile
        .filter(|l| !l.is_empty())
        .or_else(|| settings.and_then(|s| s.dbfile.clone()))
        .or_else(|| {
            let proj_dirs = ProjectDirs::from("", "", "canis")?;
            Some(proj_dirs.data_dir().join("canis.db").display().to_string())
        })
        .ok_or_else(|| WatcherError::ConfigError(
            "Failed to determine log file path".to_string()
        ))?;

    let db = EventRepository::new(&database_path)?;

    // 日次ハッシュを作成する日付を取得
    let today = Utc::now().date_naive();
    let date = match args.date.as_str() {
        "today"     => today,
        "yesterday" => today - Duration::days(1),
        s           => NaiveDate::parse_from_str(s, "%Y-%m-%d")
                        .map_err(|_| WatcherError::Other(
                            format!("Invalid date format: {s} (expected: YYYY-MM-DD, e.g. 2026-03-24)")
                        ))?,
    };
    let date_str = date.format("%Y-%m-%d").to_string();

    // 証跡ログから指定した日付の証跡一覧を取得
    let entries = db.get_entries(&date_str)?;

    if entries.is_empty() {
        return Err(WatcherError::HashError(
            "No entries found for hash calculation".to_string()
        ));
    }

    let combined = entries.join("\n") + "\n";

    // 日次ハッシュを作成
    let digest = Sha256::digest(combined.as_bytes());
    let hash = hex::encode(digest);

    let hashdir = args.hashdir
        .or_else(|| settings.and_then(|s| s.hashdir.clone()))
        .ok_or(WatcherError::ConfigError("token is not specified".to_string()))?;

    let filename = format!("dailyhash-{date}.txt");
    let filepath = hashdir.join(&filename);

    let token = args.token
        .or_else(|| settings.and_then(|s| s.token.clone()))
        .ok_or(WatcherError::ConfigError("token is not specified".to_string()))?;

    let repo = args.repo
        .or_else(|| settings.and_then(|s| s.repo.clone()))
        .ok_or(WatcherError::ConfigError("repo is not specified".to_string()))?;

    fs::write(&filepath, &hash)
        .map_err(WatcherError::IoError)?;

    let _ = push_to_github(&token, &date, &hash, &repo);

    println!("Publish completed");

    Ok(())
}

pub fn push_to_github(token: &str, date: &NaiveDate, hash: &str, name: &str) -> Result<()> {
    let agent = ureq::agent();
    let auth  = format!("Bearer {token}");

    // トークンからリポジトリを自動取得
    let user: serde_json::Value = agent
        .get("https://api.github.com/user")
        .set("Authorization", &auth)
        .set("User-Agent", "canis-publisher")
        .call()
        .map_err(|e| WatcherError::Other(e.to_string()))?
        .into_string()
        .map_err(|e| WatcherError::IoError(e))?
        .parse::<serde_json::Value>()
        .map_err(|e| WatcherError::Other(e.to_string()))?;

    let owner = user["login"]
        .as_str()
        .ok_or(WatcherError::ConfigError("Failed to retrieve user login".to_string()))?;

    // ファイル名・パス
    let filename    = format!("dailyhash-{date}.txt");
    let url         = format!("https://api.github.com/repos/{owner}/{name}/contents/{filename}");

    // 既存ファイルの SHA を取得（更新の場合に必要）
    let sha: Option<String> = agent
        .get(&url)
        .set("Authorization", &auth)
        .set("User-Agent", "canis-publisher")
        .call()
        .ok()
        .and_then(|r| r.into_string().ok())
        .and_then(|s| s.parse::<serde_json::Value>().ok())
        .and_then(|j| j["sha"].as_str().map(|s: &str| s.to_string()));

    // ファイル内容を base64 エンコード
    let encoded = general_purpose::STANDARD.encode(hash.as_bytes());

    // リクエストボディを組み立て
    let mut body = json!({
        "message": format!("publish: dailyhash-{date}"),
        "content": encoded,
        "branch":  "main",
    });

    if let Some(s) = sha {
        body["sha"] = json!(s);
    }

    let body_str = serde_json::to_string(&body)
    .map_err(|e| WatcherError::Other(e.to_string()))?;

    // ファイルの作成または更新
    agent
        .put(&url)
        .set("Authorization", &auth)
        .set("User-Agent", "canis-publisher")
        .set("Content-Type", "application/json")
        .send_bytes(body_str.as_bytes())
        .map_err(|e| WatcherError::Other(e.to_string()))?;

    Ok(())
}
