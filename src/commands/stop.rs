use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

#[cfg(unix)]
use nix::sys::signal::{self, Signal};
#[cfg(unix)]
use nix::unistd::Pid;

pub fn stop() -> Result<()> {
    println!("Stopping Canis daemon\n");

    let pid_file = get_pid_file_path()?;

    if !pid_file.exists() {
        anyhow::bail!("PID file not found: {}\nThe daemon does not appear to be running", pid_file.display());
    }

    // PIDファイルから読み込み
    let pid_str = fs::read_to_string(&pid_file)
        .context("Failed to read PID file")?;

    let pid: i32 = pid_str.trim().parse()
        .context("Invalid PID file contents")?;

    println!("Sending stop signal to process {}", pid);

    // SIGTERMを送信
    #[cfg(target_os = "linux")]
    {
        let pid = Pid::from_raw(pid);

        // プロセスが存在するか確認
        if let Err(e) = signal::kill(pid, None) {
            if e == nix::errno::Errno::ESRCH {
                println!("The process has already terminated");
                fs::remove_file(&pid_file)?;
                return Ok(());
            }
            return Err(e.into());
        }

        // SIGTERMを送信
        signal::kill(pid, Signal::SIGTERM)
            .context("Failed to send stop signal")?;

        // プロセスの終了を待機（最大10秒）
        for i in 0..10 {
            std::thread::sleep(Duration::from_millis(500));

            // プロセスがまだ存在するか確認
            if signal::kill(pid, None).is_err() {
                println!("Daemon stopped successfully");
                fs::remove_file(&pid_file)?;
                return Ok(());
            }

            if i == 0 {
                print!("Waiting for shutdown");
            } else {
                print!(".");
            }
            use std::io::Write;
            std::io::stdout().flush().ok();
        }

        println!("The process is not responding. Attempting force termination");

        // SIGKILL で強制終了
        signal::kill(pid, Signal::SIGKILL)
            .context("Failed to forcefully terminate the process")?;

        std::thread::sleep(Duration::from_millis(500));
        println!("Daemon forcefully terminated");
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
        .ok_or_else(|| anyhow::anyhow!("Failed to retrieve XDG configuration directory"))?;

    Ok(proj_dirs.data_dir().join("canis.pid"))
}
