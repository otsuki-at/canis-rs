use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

#[cfg(unix)]
use nix::sys::signal::{self, Signal};
#[cfg(unix)]
use nix::unistd::Pid;

pub fn stop() -> Result<()> {
    println!("=== Canis デーモン停止 ===\n");

    let pid_file = get_pid_file_path()?;

    if !pid_file.exists() {
        anyhow::bail!("PIDファイルが見つかりません: {}\nデーモンは実行されていないようです", pid_file.display());
    }

    // PIDファイルから読み込み
    let pid_str = fs::read_to_string(&pid_file)
        .context("PIDファイルの読み込みに失敗しました")?;

    let pid: i32 = pid_str.trim().parse()
        .context("PIDファイルの内容が不正です")?;

    println!("プロセス {} に停止シグナルを送信しています...", pid);

    // SIGTERMを送信
    #[cfg(target_os = "linux")]
    {
        let pid = Pid::from_raw(pid);

        // プロセスが存在するか確認
        if let Err(e) = signal::kill(pid, None) {
            if e == nix::errno::Errno::ESRCH {
                println!("プロセスは既に終了しています");
                fs::remove_file(&pid_file)?;
                return Ok(());
            }
            return Err(e.into());
        }

        // SIGTERMを送信
        signal::kill(pid, Signal::SIGTERM)
            .context("停止シグナルの送信に失敗しました")?;

        // プロセスの終了を待機（最大10秒）
        for i in 0..10 {
            std::thread::sleep(Duration::from_millis(500));

            // プロセスがまだ存在するか確認
            if signal::kill(pid, None).is_err() {
                println!("デーモンを停止しました");
                fs::remove_file(&pid_file)?;
                return Ok(());
            }

            if i == 0 {
                print!("停止を待機中");
            } else {
                print!(".");
            }
            use std::io::Write;
            std::io::stdout().flush().ok();
        }

        println!("\n警告: プロセスが応答しません。強制終了を試みます...");

        // SIGKILL で強制終了
        signal::kill(pid, Signal::SIGKILL)
            .context("強制終了に失敗しました")?;

        std::thread::sleep(Duration::from_millis(500));
        println!("デーモンを強制終了しました");
    }

    // PIDファイルを削除
    if pid_file.exists() {
        fs::remove_file(&pid_file)?;
    }

    Ok(())
}

fn get_pid_file_path() -> Result<PathBuf> {
    use directories_next::ProjectDirs;

    let proj_dirs = ProjectDirs::from("", "", "canis")
        .ok_or_else(|| anyhow::anyhow!("XDG ディレクトリを取得できませんでした"))?;

    Ok(proj_dirs.data_dir().join("canis.pid"))
}
