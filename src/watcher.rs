use std::sync::{Arc, RwLock};
use notify::{Watcher, RecursiveMode, Event, EventKind, RecommendedWatcher};
use notify::event::{ModifyKind, RenameMode};
use std::sync::mpsc::channel;
use std::sync::atomic::{AtomicBool, Ordering};
use std::path::Path;
use std::path::PathBuf;
use chrono::Utc;
use sysinfo::{System,Pid, ProcessesToUpdate, ProcessRefreshKind, UpdateKind};
use serde_json;
use url::Url;
use signal_hook::{consts::{SIGINT, SIGTERM}, iterator::Signals};
use std::sync::Mutex;

use crate::error::{Result, WatcherError};
use crate::event::{CanonicalEvent, FileEvent, ProcessInfo};
use crate::observer::{Observer, Subject};

fn path_to_uri(path: &Path) -> Result<Url> {
    // 絶対パスであるかどうか確認
    if !path.is_absolute() {
        return Err(WatcherError::UriFailed(
            format!("Expected absolute path: {}", path.display())
        ));
    }

    Url::from_file_path(path)
        .map_err(|_| WatcherError::UriFailed(path.display().to_string()))
}

/// 監視システムの種類
#[derive(Debug, Clone, PartialEq)]
enum WatcherSystemType {
    Notify,
    #[cfg(all(feature = "fuse", any(target_os = "linux", target_os = "macos")))]
    Fuse,
}

impl WatcherSystemType {
    fn from_str(s: &str) -> Result<Self> {
        match s {
            "notify" => Ok(Self::Notify),
            #[cfg(all(feature = "fuse", any(target_os = "linux", target_os = "macos")))]
            "fuse" => Ok(Self::Fuse),
            _ => Err(WatcherError::UnsupportedSystem(
                format!("Unsupported system: {}", s)
            )),
        }
    }

    fn supports_multiple_paths(&self) -> bool {
        match self {
            Self::Notify => true,
            #[cfg(all(feature = "fuse", any(target_os = "linux", target_os = "macos")))]
            Self::Fuse => false,
        }
    }
}

/// ファイル監視システムのメイン構造体
pub struct FileWatcher {
    system_type: WatcherSystemType,
    observers: Arc<RwLock<Vec<Box<dyn Observer>>>>,
}

impl FileWatcher {
    /// 新しいFileWatcherを生成
    pub fn new(system_name: &str) -> Result<Self> {
        let system_type = WatcherSystemType::from_str(system_name)?;

        Ok(Self {
            system_type,
            observers: Arc::new(RwLock::new(Vec::new())),
        })
    }

    /// 監視を開始
    pub fn start_watching(&mut self, paths: &[String], ignore_paths: &[String]) -> Result<()> {
        // 監視システムに応じて処理を分岐
        match self.system_type {
            WatcherSystemType::Notify => self.start_notify_watching(paths, ignore_paths),
            #[cfg(all(feature = "fuse", any(target_os = "linux", target_os = "macos")))]
            WatcherSystemType::Fuse => self.start_fuse_watching(paths, ignore_paths),
        }
    }

    /// notify を使った監視の開始
    fn start_notify_watching(&self, paths: &[String], ignore_paths: &[String]) -> Result<()> {
        let ignore_paths = Arc::new(ignore_paths.to_vec());

        let (tx, rx) = channel();
        let mut watcher = RecommendedWatcher::new(
            move |res| {
                let now = Utc::now();
                if let Err(e) = tx.send((res, now)) {
                    eprintln!("Event dispatch error: {:?}", e);
                }
            },
            notify::Config::default(),
        )?;

        let watcher = Arc::new(Mutex::new(Some(watcher)));
        let watcher_clone = Arc::clone(&watcher);

        // 各パスの監視を開始
        if let Ok(mut w) = watcher.lock() {
            if let Some(w) = w.as_mut() {
                for path in paths {
                    w.watch(path.as_ref(), RecursiveMode::Recursive)?;
                    println!("Started watching: {}", path);
                }
            }
        }

        // シグナルハンドラ
        let mut signals = Signals::new(&[SIGINT, SIGTERM])
            .expect("Failed to initialize signal handler");

        std::thread::spawn(move || {
            for sig in signals.forever() {
                eprintln!("\nReceived signal {}", sig);
                if let Ok(mut w) = watcher_clone.lock() {
                    w.take(); // drop → tx がdrop → rx のブロックが解ける
                }
                break;
            }
        });

        println!("Press Ctrl+C to exit\n");

        // イベント受信ループ
        for (res, time) in rx {
            match res {
                Ok(event) => {
                    if event.paths.iter().any(|p| self.should_ignore(p, &ignore_paths)) {
                        continue;
                    }
                    // notifyイベントをCanonicalEventに変換
                    let canonical_events = self.convert_notify_event(event, time);

                    // 各Observerに通知
                    for canonical_event in canonical_events {
                        let file_event = FileEvent{event: canonical_event, process_info: None};
                        self.notify(&file_event);
                    }
                }
                Err(e) => eprintln!("Watcher error: {:?}", e),
            }
        }

        Ok(())
    }

    /// notifyイベントをCanonicalEventに変換
    fn convert_notify_event(&self, event: Event, timestamp: chrono::DateTime<Utc>) -> Vec<CanonicalEvent> {
        let mut events = Vec::new();
        let time = timestamp.format("%Y-%m-%dT%H:%M:%S%.6f").to_string();

        match event.kind {
            EventKind::Create(_) => {
                for path in event.paths {
                    if let Some(uri) = path_to_uri(&path)
                        .map_err(|e| eprintln!("URI conversion failed: {:?}", e))
                        .ok()
                    {
                        events.push(CanonicalEvent::Create { uri, time: time.clone() });
                    }
                }
            }
            EventKind::Modify(ModifyKind::Data(_)) => {
                for path in event.paths {
                    if let Some(uri) = path_to_uri(&path)
                        .map_err(|e| eprintln!("URI conversion failed: {:?}", e))
                        .ok()
                    {
                        events.push(CanonicalEvent::Modify { uri, time: time.clone() });
                    }
                }
            }
            EventKind::Modify(ModifyKind::Name(RenameMode::Both)) => {
                // 両方成功した場合のみ登録
                if let [src, dst] = event.paths.as_slice() {
                    if let (Some(src_uri), Some(dst_uri)) = (
                        path_to_uri(&src).map_err(|e| eprintln!("URI conversion failed: {:?}", e)).ok(),
                        path_to_uri(&dst).map_err(|e| eprintln!("URI conversion failed: {:?}", e)).ok(),
                    ) {
                        events.push(CanonicalEvent::Move { src: src_uri, dst: dst_uri, time });
                    }
                }
            }
            _ => {}
        }

        events
    }

    fn should_ignore(&self, path: &Path, ignore_paths: &[String]) -> bool {
        ignore_paths.iter().any(|p| path.starts_with(p.as_str()))
    }

    /// FUSE を使った監視の開始
    #[cfg(all(feature = "fuse", any(target_os = "linux", target_os = "macos")))]
    fn start_fuse_watching(&self, paths: &[String], ignore_paths: &[String]) -> Result<()> {
        if paths.is_empty() {
            return Err(WatcherError::ConfigError(
                "No paths specified for watching".to_string()
            ));
        }

        let original_path = PathBuf::from(&paths[0]);
        if !original_path.exists() {
            return Err(WatcherError::ConfigError(
                format!("Target path does not exist: {}", paths[0])
            ));
        }

        let original_name = original_path.file_name()
            .ok_or_else(|| WatcherError::ConfigError("Invalid path".to_string()))?;

        let parent_dir = original_path.parent()
            .ok_or_else(|| WatcherError::ConfigError("Parent directory does not exist".to_string()))?;

        let renamed_path = parent_dir.join(format!("{}.watch", original_name.to_string_lossy()));
        std::fs::rename(&original_path, &renamed_path)?;
        std::fs::create_dir_all(&original_path)?;

        println!("Starting FUSE-based watching:");
        println!("  Source (actual data): {}", renamed_path.display());
        println!("  Mount point: {}", original_path.display());
        println!("  (Please access files through the original path: {})\n",original_path.display());

        // シグナルハンドラの設定
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = Arc::clone(&running);

        // SIGINTとSIGTERMを監視
        let mut signals = Signals::new(&[SIGINT, SIGTERM])
            .expect("Failed to initialize signal handler");

        // シグナル監視スレッド
        std::thread::spawn(move || {
            for sig in signals.forever() {
                eprintln!("\nReceived signal {}", sig);
                running_clone.store(false, Ordering::SeqCst);
                break;
            }
        });

        let observers = Arc::clone(&self.observers);
        let fs = PassthroughFS::new(renamed_path.clone(), observers, ignore_paths);

        // FUSEをマウント
        let _session = fuser::spawn_mount2(
            fs,
            &original_path,
            &[],
        )?;

        println!("FUSE watcher is running\nPress Ctrl+C to exit\n");

        // メインループ - シグナルを受け取るまで待機
        while running.load(Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }

        // ========== クリーンアップ処理 ==========
        println!("\nStarting cleanup");

        // 1. マウント解除
        Self::unmount_with_retry(&original_path);

        // 2. マウントポイント(空のディレクトリ)を削除
        if let Err(e) = std::fs::remove_dir(&original_path) {
            eprintln!("Failed to remove mount point:{}", e);
        }

        // 3. リネームしたディレクトリを元の名前に戻す
        if let Err(e) = std::fs::rename(&renamed_path, &original_path) {
            eprintln!("Failed to restore the original directory name: {}", e);
            eprintln!("Please manually rename {} back to {}",
                    renamed_path.display(), original_path.display());
            std::process::exit(1);
        }

        println!("Cleanup completed");
        Ok(())
    }

    #[cfg(all(feature = "fuse", target_os = "linux"))]
    fn unmount_with_retry(mount_point: &Path) {
        use std::process::Command;
        use std::thread;
        use std::time::Duration;

        const MAX_RETRIES: u32 = 5;
        const RETRY_DELAY_MS: u64 = 500;

        for attempt in 1..=MAX_RETRIES {
            println!("Attempting to unmount (attempt {}/{})", attempt, MAX_RETRIES);

            // Linux: fusermount -u を実行
            match Command::new("fusermount")
                .arg("-u")
                .arg(mount_point)
                .output()
            {
                Ok(output) => {
                    if output.status.success() {
                        println!("Unmount successful");
                        return;
                    } else {
                        eprintln!("Unmount failed: {}",
                                 String::from_utf8_lossy(&output.stderr));
                    }
                }
                Err(e) => {
                    eprintln!("Failed to execute fusermount command: {}", e);
                }
            }

            if attempt < MAX_RETRIES {
                println!("Retrying after {} ms", RETRY_DELAY_MS);
                thread::sleep(Duration::from_millis(RETRY_DELAY_MS));
            }
        }

        // 最後の手段: umount -l (lazy unmount)
        eprintln!("Normal unmount failed. Attempting forced unmount (lazy unmount)");
        match Command::new("umount")
            .arg("-l")
            .arg(mount_point)
            .output()
        {
            Ok(output) => {
                if output.status.success() {
                    println!("Forced unmount successful");
                } else {
                    eprintln!("Forced unmount failed: {}",
                             String::from_utf8_lossy(&output.stderr));
                    eprintln!("The mount point may still be active");
                    eprintln!("Please run 'sudo umount -l {}' manually",
                             mount_point.display());
                }
            }
            Err(e) => {
                eprintln!("Failed to execute umount command: {}", e);
                eprintln!("Please run 'sudo umount -l {}' manually",
                         mount_point.display());
            }
        }
    }

    #[cfg(all(feature = "fuse", target_os = "macos"))]
    fn unmount_with_retry(mount_point: &Path) {
        use std::process::Command;
        use std::thread;
        use std::time::Duration;

        const MAX_RETRIES: u32 = 5;
        const RETRY_DELAY_MS: u64 = 500;

        for attempt in 1..=MAX_RETRIES {
            println!("Attempting to unmount (attempt {}/{})", attempt, MAX_RETRIES);

            // macOS: umount を実行
            match Command::new("umount")
                .arg(mount_point)
                .output()
            {
                Ok(output) => {
                    if output.status.success() {
                        println!("Unmount successful");
                        return;
                    } else {
                        eprintln!("Unmount failed: {}",
                                 String::from_utf8_lossy(&output.stderr));
                    }
                }
                Err(e) => {
                    eprintln!("Failed to execute umount command: {}", e);
                }
            }

            if attempt < MAX_RETRIES {
                println!("Retrying after {} ms", RETRY_DELAY_MS);
                thread::sleep(Duration::from_millis(RETRY_DELAY_MS));
            }
        }

        // 最後の手段: umount -f (force unmount)
        eprintln!("Normal unmount failed. Attempting forced unmount (force unmount)");
        match Command::new("umount")
            .arg("-f")
            .arg(mount_point)
            .output()
        {
            Ok(output) => {
                if output.status.success() {
                    println!("Forced unmount successful");
                } else {
                    eprintln!("Forced unmount failed: {}",
                             String::from_utf8_lossy(&output.stderr));
                    eprintln!("The mount point may still be active");
                    eprintln!("Please run 'sudo umount -f {}' manually",
                             mount_point.display());
                }
            }
            Err(e) => {
                eprintln!("Failed to execute umount command: {}", e);
                eprintln!("Please run 'sudo umount -f {}' manually",
                         mount_point.display());
            }
        }
    }

    #[cfg(all(feature = "fuse", not(any(target_os = "linux", target_os = "macos"))))]
    fn start_fuse_watching(&self, _paths: &[String]) -> Result<()> {
        Err(WatcherError::UnsupportedSystem(
            "FUSE-based watching is supported only on Linux and macOS".to_string()
        ))
    }
}

impl Subject for FileWatcher {
    fn attach(&mut self, observer: Box<dyn Observer>) {
        let mut observers = self.observers.write().unwrap();
        observers.push(observer);
    }

    fn notify(&self, event: &FileEvent) {
        let observers = self.observers.read().unwrap();
        for observer in observers.iter() {
            observer.update(event);
        }
    }
}

// ============================================================================
// FUSE 実装（Unix限定）- 完全版 + イベント通知
// ============================================================================

#[cfg(all(feature = "fuse", any(target_os = "linux", target_os = "macos")))]
use {
    fuser::{
        Filesystem, KernelConfig, ReplyAttr, ReplyCreate, ReplyData, ReplyDirectory,
        ReplyDirectoryPlus, ReplyEmpty, ReplyEntry, ReplyLock, ReplyOpen, ReplyStatfs, ReplyWrite,
        ReplyXattr, Request, TimeOrNow, ReplyLseek, ReplyBmap, ReplyIoctl,
    },
    libc::{c_int, EACCES, EINVAL, EIO, ENOENT, ENOSYS, ENOTEMPTY},
    std::collections::HashMap,
    std::ffi::OsStr,
    std::fs::{self, File, OpenOptions},
    std::io::{Read, Seek, SeekFrom, Write},
    std::os::unix::ffi::OsStrExt,
    std::os::unix::fs::{MetadataExt, PermissionsExt},
    std::sync::Mutex,
    std::time::{Duration, SystemTime, UNIX_EPOCH},
};

#[cfg(all(feature = "fuse", any(target_os = "linux", target_os = "macos")))]
const TTL: Duration = Duration::from_secs(1);

/// Convert a system time to a tuple of (seconds, nanoseconds)
#[cfg(all(feature = "fuse", any(target_os = "linux", target_os = "macos")))]
fn system_time_to_tuple(time: SystemTime) -> (i64, u32) {
    match time.duration_since(UNIX_EPOCH) {
        Ok(duration) => (duration.as_secs() as i64, duration.subsec_nanos()),
        Err(_) => (0, 0),
    }
}

/// Convert file metadata to FileAttr
#[cfg(all(feature = "fuse", any(target_os = "linux", target_os = "macos")))]
fn metadata_to_attr(ino: u64, metadata: &fs::Metadata) -> fuser::FileAttr {
    let (atime_sec, atime_nsec) = system_time_to_tuple(
        metadata
            .accessed()
            .unwrap_or(UNIX_EPOCH),
    );
    let (mtime_sec, mtime_nsec) = system_time_to_tuple(
        metadata
            .modified()
            .unwrap_or(UNIX_EPOCH),
    );
    let (ctime_sec, ctime_nsec) = system_time_to_tuple(
        metadata
            .created()
            .unwrap_or(UNIX_EPOCH),
    );

    let kind = if metadata.is_dir() {
        fuser::FileType::Directory
    } else if metadata.is_symlink() {
        fuser::FileType::Symlink
    } else {
        fuser::FileType::RegularFile
    };

    fuser::FileAttr {
        ino,
        size: metadata.len(),
        blocks: metadata.blocks(),
        atime: UNIX_EPOCH + Duration::new(atime_sec as u64, atime_nsec),
        mtime: UNIX_EPOCH + Duration::new(mtime_sec as u64, mtime_nsec),
        ctime: UNIX_EPOCH + Duration::new(ctime_sec as u64, ctime_nsec),
        crtime: UNIX_EPOCH,
        kind,
        perm: (metadata.permissions().mode() & 0o7777) as u16,
        nlink: metadata.nlink() as u32,
        uid: metadata.uid(),
        gid: metadata.gid(),
        rdev: metadata.rdev() as u32,
        blksize: metadata.blksize() as u32,
        flags: 0,
    }
}

#[cfg(all(feature = "fuse", any(target_os = "linux", target_os = "macos")))]
struct InodeData {
    path: PathBuf,
    lookup_count: u64,
}

#[cfg(all(feature = "fuse", any(target_os = "linux", target_os = "macos")))]
pub struct PassthroughFS {
    root: PathBuf,
    inodes: Arc<Mutex<HashMap<u64, InodeData>>>,
    path_to_inode: Arc<Mutex<HashMap<PathBuf, u64>>>,
    next_inode: Arc<Mutex<u64>>,
    file_handles: Arc<Mutex<HashMap<u64, File>>>,
    next_fh: Arc<Mutex<u64>>,
    // イベント通知用のobservers（追加）
    observers: Arc<RwLock<Vec<Box<dyn Observer>>>>,
    // 無視するファイルパスのリスト
    ignore_paths: Vec<String>,
    sys: System,
}

#[cfg(all(feature = "fuse", any(target_os = "linux", target_os = "macos")))]
impl PassthroughFS {
    pub fn new(root: PathBuf, observers: Arc<RwLock<Vec<Box<dyn Observer>>>>, ignore_paths: &[String]) -> Self {
        let mut inodes = HashMap::new();
        let mut path_to_inode = HashMap::new();

        // Initialize root inode
        inodes.insert(
            1,
            InodeData {
                path: root.clone(),
                lookup_count: 1,
            },
        );
        path_to_inode.insert(root.clone(), 1);

        Self {
            root,
            inodes: Arc::new(Mutex::new(inodes)),
            path_to_inode: Arc::new(Mutex::new(path_to_inode)),
            next_inode: Arc::new(Mutex::new(2)),
            file_handles: Arc::new(Mutex::new(HashMap::new())),
            next_fh: Arc::new(Mutex::new(1)),
            observers,
            ignore_paths: ignore_paths.to_vec(),
            sys: System::new(),
        }
    }

    fn get_path(&self, ino: u64) -> std::result::Result<PathBuf, c_int> {
        let inodes = self.inodes.lock().unwrap();
        inodes
            .get(&ino)
            .map(|data| data.path.clone())
            .ok_or(ENOENT)
    }

    fn allocate_inode(&self, path: PathBuf) -> u64 {
        let mut path_to_inode = self.path_to_inode.lock().unwrap();

        // Check if path already has an inode
        if let Some(&existing_ino) = path_to_inode.get(&path) {
            return existing_ino;
        }

        let mut next_inode = self.next_inode.lock().unwrap();
        let ino = *next_inode;
        *next_inode += 1;

        let mut inodes = self.inodes.lock().unwrap();
        inodes.insert(
            ino,
            InodeData {
                path: path.clone(),
                lookup_count: 1,
            },
        );
        path_to_inode.insert(path, ino);

        ino
    }

    fn allocate_fh(&self, file: File) -> u64 {
        let mut next_fh = self.next_fh.lock().unwrap();
        let fh = *next_fh;
        *next_fh += 1;

        let mut file_handles = self.file_handles.lock().unwrap();
        file_handles.insert(fh, file);

        fh
    }

    fn get_file_handle(&self, fh: u64) -> std::result::Result<File, c_int> {
        let file_handles = self.file_handles.lock().unwrap();
        file_handles
            .get(&fh)
            .and_then(|f| f.try_clone().ok())
            .ok_or(EIO)
    }

    fn release_fh(&self, fh: u64) {
        let mut file_handles = self.file_handles.lock().unwrap();
        file_handles.remove(&fh);
    }

    /// イベントを通知する（追加）
    fn notify_event(&self, event: &FileEvent) {
        let observers = self.observers.read().unwrap();
        for observer in observers.iter() {
            observer.update(&event);
        }
    }

    /// ファイルパスを無視リストに含まれているかチェック
    fn should_ignore(&self, path: &Path) -> bool {
        self.ignore_paths.iter().any(|p| path.starts_with(p.as_str()))
    }

    pub fn get_process_info(&mut self, pid: u32) -> Option<ProcessInfo> {
        self.sys.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing()
                .with_cmd(UpdateKind::Always)
                .with_exe(UpdateKind::Always),); // 最新情報に更新

        let sysinfo_pid = Pid::from(pid as usize);
        self.sys.process(sysinfo_pid).map(|p| ProcessInfo {
            start_time: p.start_time(),
            pid:       pid as i32,
            ppid:      p.parent().map(|p| p.as_u32() as i32).unwrap_or(-1),
            exe:       p.exe().map(|e| e.display().to_string()).unwrap_or_default(),
            cmd:       serde_json::to_string(
                            &p.cmd().iter()
                                .map(|s| s.to_string_lossy().into_owned())
                                .collect::<Vec<_>>()
                        ).unwrap_or_default(),
        })
    }
}

#[cfg(all(feature = "fuse", any(target_os = "linux", target_os = "macos")))]
impl Filesystem for PassthroughFS {
    fn init(
        &mut self,
        _req: &Request<'_>,
        _config: &mut KernelConfig,
    ) -> std::result::Result<(), c_int> {
        Ok(())
    }

    fn destroy(&mut self) {}

    fn lookup(
        &mut self,
        _req: &Request<'_>,
        parent: u64,
        name: &OsStr,
        reply: ReplyEntry,
    ) {
        let parent_path = match self.get_path(parent) {
            Ok(path) => path,
            Err(e) => {
                reply.error(e);
                return;
            }
        };

        let path = parent_path.join(name);

        match fs::metadata(&path) {
            Ok(metadata) => {
                let ino = self.allocate_inode(path);
                let attr = metadata_to_attr(ino, &metadata);
                reply.entry(&TTL, &attr, 0);
            }
            Err(e) => {
                reply.error(e.raw_os_error().unwrap_or(EIO));
            }
        }
    }

    fn forget(&mut self, _req: &Request<'_>, ino: u64, nlookup: u64) {
        let mut inodes = self.inodes.lock().unwrap();
        if let Some(data) = inodes.get_mut(&ino) {
            data.lookup_count = data.lookup_count.saturating_sub(nlookup);
            if data.lookup_count == 0 && ino != 1 {
                let path = data.path.clone();
                inodes.remove(&ino);

                let mut path_to_inode = self.path_to_inode.lock().unwrap();
                path_to_inode.remove(&path);
            }
        }
    }

    fn getattr(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        reply: ReplyAttr,
    ) {
        let path = match self.get_path(ino) {
            Ok(path) => path,
            Err(e) => {
                reply.error(e);
                return;
            }
        };

        match fs::metadata(&path) {
            Ok(metadata) => {
                let attr = metadata_to_attr(ino, &metadata);
                reply.attr(&TTL, &attr);
            }
            Err(e) => {
                reply.error(e.raw_os_error().unwrap_or(EIO));
            }
        }
    }

    fn setattr(
        &mut self,
        req: &Request<'_>,
        ino: u64,
        mode: Option<u32>,
        uid: Option<u32>,
        gid: Option<u32>,
        size: Option<u64>,
        _atime: Option<TimeOrNow>,
        _mtime: Option<TimeOrNow>,
        _ctime: Option<SystemTime>,
        fh: Option<u64>,
        _crtime: Option<SystemTime>,
        _chgtime: Option<SystemTime>,
        _bkuptime: Option<SystemTime>,
        _flags: Option<u32>,
        reply: ReplyAttr,
    ) {
        let path = match self.get_path(ino) {
            Ok(path) => path,
            Err(e) => {
                reply.error(e);
                return;
            }
        };

        // Handle size (truncate) - Modifyイベントを通知
        if let Some(size) = size {
            let result = if let Some(fh) = fh {
                if let Ok(file) = self.get_file_handle(fh) {
                    file.set_len(size)
                } else {
                    File::open(&path).and_then(|f| f.set_len(size))
                }
            } else {
                File::open(&path).and_then(|f| f.set_len(size))
            };

            if let Err(e) = result {
                reply.error(e.raw_os_error().unwrap_or(EIO));
                return;
            }

            // Modify イベント通知
            if !self.should_ignore(&path) {
                if let Some(uri) = path_to_uri(&path)
                    .map_err(|e| eprintln!("URI conversion failed: {:?}", e))
                    .ok()
                {
                    let now = Utc::now();
                    let time = now.format("%Y-%m-%dT%H:%M:%S%.6f").to_string();
                    let event = CanonicalEvent::Modify {
                        uri,
                        time,
                    };

                    let pid = req.pid();
                    let process_info = self.get_process_info(pid);

                    let file_event = FileEvent { event, process_info};
                    self.notify_event(&file_event);
                }
            }
        }

        // Handle mode
        if let Some(mode) = mode {
            if let Err(e) = fs::set_permissions(&path, fs::Permissions::from_mode(mode)) {
                reply.error(e.raw_os_error().unwrap_or(EIO));
                return;
            }
        }

        // Handle uid/gid (requires root privileges, so we skip for now)
        if uid.is_some() || gid.is_some() {
            // Skip changing ownership for simplicity
        }

        match fs::metadata(&path) {
            Ok(metadata) => {
                let attr = metadata_to_attr(ino, &metadata);
                reply.attr(&TTL, &attr);
            }
            Err(e) => {
                reply.error(e.raw_os_error().unwrap_or(EIO));
            }
        }
    }

    fn readlink(&mut self, _req: &Request<'_>, ino: u64, reply: ReplyData) {
        let path = match self.get_path(ino) {
            Ok(path) => path,
            Err(e) => {
                reply.error(e);
                return;
            }
        };

        match fs::read_link(&path) {
            Ok(target) => {
                reply.data(target.as_os_str().as_bytes());
            }
            Err(e) => {
                reply.error(e.raw_os_error().unwrap_or(EIO));
            }
        }
    }

    fn mknod(
        &mut self,
        req: &Request<'_>,
        parent: u64,
        name: &OsStr,
        mode: u32,
        _umask: u32,
        _rdev: u32,
        reply: ReplyEntry,
    ) {
        let parent_path = match self.get_path(parent) {
            Ok(path) => path,
            Err(e) => {
                reply.error(e);
                return;
            }
        };

        let path = parent_path.join(name);

        // Create an empty file
        match File::create(&path) {
            Ok(_) => {
                if let Err(e) = fs::set_permissions(&path, fs::Permissions::from_mode(mode)) {
                    reply.error(e.raw_os_error().unwrap_or(EIO));
                    return;
                }

                match fs::metadata(&path) {
                    Ok(metadata) => {
                        let ino = self.allocate_inode(path.clone());
                        let attr = metadata_to_attr(ino, &metadata);

                        // Create イベント通知
                        if !self.should_ignore(&path) {
                            if let Some(uri) = path_to_uri(&path)
                                .map_err(|e| eprintln!("URI conversion failed: {:?}", e))
                                .ok()
                            {
                                let now = Utc::now();
                                let time = now.format("%Y-%m-%dT%H:%M:%S%.6f").to_string();
                                let event = CanonicalEvent::Create {
                                    uri,
                                    time,
                                };
                                let pid = req.pid();
                                let process_info = self.get_process_info(pid);

                                let file_event = FileEvent { event, process_info};
                                self.notify_event(&file_event);
                            }
                        }

                        reply.entry(&TTL, &attr, 0);
                    }
                    Err(e) => {
                        reply.error(e.raw_os_error().unwrap_or(EIO));
                    }
                }
            }
            Err(e) => {
                reply.error(e.raw_os_error().unwrap_or(EIO));
            }
        }
    }

    fn mkdir(
        &mut self,
        req: &Request<'_>,
        parent: u64,
        name: &OsStr,
        mode: u32,
        _umask: u32,
        reply: ReplyEntry,
    ) {
        let parent_path = match self.get_path(parent) {
            Ok(path) => path,
            Err(e) => {
                reply.error(e);
                return;
            }
        };

        let path = parent_path.join(name);

        match fs::create_dir(&path) {
            Ok(_) => {
                if let Err(e) = fs::set_permissions(&path, fs::Permissions::from_mode(mode)) {
                    reply.error(e.raw_os_error().unwrap_or(EIO));
                    return;
                }

                match fs::metadata(&path) {
                    Ok(metadata) => {
                        let ino = self.allocate_inode(path.clone());
                        let attr = metadata_to_attr(ino, &metadata);

                        // Create イベント通知
                        if !self.should_ignore(&path) {
                            if let Some(uri) = path_to_uri(&path)
                                .map_err(|e| eprintln!("URI conversion failed: {:?}", e))
                                .ok()
                            {
                                let now = Utc::now();
                                let time = now.format("%Y-%m-%dT%H:%M:%S%.6f").to_string();
                                let event = CanonicalEvent::Create {
                                    uri,
                                    time,
                                };
                                let pid = req.pid();
                                let process_info = self.get_process_info(pid);

                                let file_event = FileEvent { event, process_info};
                                self.notify_event(&file_event);
                            }
                        }

                        reply.entry(&TTL, &attr, 0);
                    }
                    Err(e) => {
                        reply.error(e.raw_os_error().unwrap_or(EIO));
                    }
                }
            }
            Err(e) => {
                reply.error(e.raw_os_error().unwrap_or(EIO));
            }
        }
    }

    fn unlink(&mut self, _req: &Request<'_>, parent: u64, name: &OsStr, reply: ReplyEmpty) {
        let parent_path = match self.get_path(parent) {
            Ok(path) => path,
            Err(e) => {
                reply.error(e);
                return;
            }
        };

        let path = parent_path.join(name);

        match fs::remove_file(&path) {
            Ok(_) => reply.ok(),
            Err(e) => {
                reply.error(e.raw_os_error().unwrap_or(EIO));
            }
        }
    }

    fn rmdir(&mut self, _req: &Request<'_>, parent: u64, name: &OsStr, reply: ReplyEmpty) {
        let parent_path = match self.get_path(parent) {
            Ok(path) => path,
            Err(e) => {
                reply.error(e);
                return;
            }
        };

        let path = parent_path.join(name);

        match fs::remove_dir(&path) {
            Ok(_) => reply.ok(),
            Err(e) => {
                let err = if e.kind() == std::io::ErrorKind::Other {
                    ENOTEMPTY
                } else {
                    e.raw_os_error().unwrap_or(EIO)
                };
                reply.error(err);
            }
        }
    }

    fn symlink(
        &mut self,
        _req: &Request<'_>,
        parent: u64,
        name: &OsStr,
        link: &Path,
        reply: ReplyEntry,
    ) {
        let parent_path = match self.get_path(parent) {
            Ok(path) => path,
            Err(e) => {
                reply.error(e);
                return;
            }
        };

        let path = parent_path.join(name);

        #[cfg(all(feature = "fuse", any(target_os = "linux", target_os = "macos")))]
        match std::os::unix::fs::symlink(link, &path) {
            Ok(_) => match fs::symlink_metadata(&path) {
                Ok(metadata) => {
                    let ino = self.allocate_inode(path);
                    let attr = metadata_to_attr(ino, &metadata);
                    reply.entry(&TTL, &attr, 0);
                }
                Err(e) => {
                    reply.error(e.raw_os_error().unwrap_or(EIO));
                }
            },
            Err(e) => {
                reply.error(e.raw_os_error().unwrap_or(EIO));
            }
        }
    }

    fn rename(
        &mut self,
        req: &Request<'_>,
        parent: u64,
        name: &OsStr,
        newparent: u64,
        newname: &OsStr,
        _flags: u32,
        reply: ReplyEmpty,
    ) {
        let parent_path = match self.get_path(parent) {
            Ok(path) => path,
            Err(e) => {
                reply.error(e);
                return;
            }
        };

        let newparent_path = match self.get_path(newparent) {
            Ok(path) => path,
            Err(e) => {
                reply.error(e);
                return;
            }
        };

        let src = parent_path.join(name);
        let dst = newparent_path.join(newname);

        match fs::rename(&src, &dst) {
            Ok(_) => {
                // Move イベント通知
                if !self.should_ignore(&src) && !self.should_ignore(&dst) {
                    if let (Some(src_uri), Some(dst_uri)) = (
                        path_to_uri(&src).map_err(|e| eprintln!("URI conversion failed: {:?}", e)).ok(),
                        path_to_uri(&dst).map_err(|e| eprintln!("URI conversion failed: {:?}", e)).ok(),
                    ) {
                        let now = Utc::now();
                        let time = now.format("%Y-%m-%dT%H:%M:%S%.6f").to_string();
                        let event = CanonicalEvent::Move {
                            src: src_uri,
                            dst: dst_uri,
                            time,
                        };
                        let pid = req.pid();
                        let process_info = self.get_process_info(pid);

                        let file_event = FileEvent { event, process_info};
                        self.notify_event(&file_event);
                    }
                }

                reply.ok();
            }
            Err(e) => {
                reply.error(e.raw_os_error().unwrap_or(EIO));
            }
        }
    }

    fn link(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        newparent: u64,
        newname: &OsStr,
        reply: ReplyEntry,
    ) {
        let path = match self.get_path(ino) {
            Ok(path) => path,
            Err(e) => {
                reply.error(e);
                return;
            }
        };

        let newparent_path = match self.get_path(newparent) {
            Ok(path) => path,
            Err(e) => {
                reply.error(e);
                return;
            }
        };

        let newpath = newparent_path.join(newname);

        match fs::hard_link(&path, &newpath) {
            Ok(_) => match fs::metadata(&newpath) {
                Ok(metadata) => {
                    let ino = self.allocate_inode(newpath);
                    let attr = metadata_to_attr(ino, &metadata);
                    reply.entry(&TTL, &attr, 0);
                }
                Err(e) => {
                    reply.error(e.raw_os_error().unwrap_or(EIO));
                }
            },
            Err(e) => {
                reply.error(e.raw_os_error().unwrap_or(EIO));
            }
        }
    }

    fn open(&mut self, req: &Request<'_>, ino: u64, flags: i32, reply: ReplyOpen) {
        let path = match self.get_path(ino) {
            Ok(path) => path,
            Err(e) => {
                reply.error(e);
                return;
            }
        };

        let mut options = OpenOptions::new();
        let access_mask = flags & libc::O_ACCMODE;

        if access_mask == libc::O_RDONLY {
            options.read(true);
        } else if access_mask == libc::O_WRONLY {
            options.write(true);
        } else if access_mask == libc::O_RDWR {
            options.read(true).write(true);
        }

        if flags & libc::O_APPEND != 0 {
            options.append(true);
        }
        if flags & libc::O_TRUNC != 0 {
            options.truncate(true);
        }

        match options.open(&path) {
            Ok(file) => {
                let fh = self.allocate_fh(file);

                // Open イベント通知
                if !self.should_ignore(&path) {
                    if let Some(uri) = path_to_uri(&path)
                        .map_err(|e| eprintln!("URI conversion failed: {:?}", e))
                        .ok()
                    {
                        let now = Utc::now();
                        let time = now.format("%Y-%m-%dT%H:%M:%S%.6f").to_string();
                        let event = CanonicalEvent::Open {
                            uri,
                            time,
                        };
                        let pid = req.pid();
                        let process_info = self.get_process_info(pid);

                        let file_event = FileEvent { event, process_info};
                        self.notify_event(&file_event);
                    }
                }

                let mut open_flags = 0;
                #[cfg(target_os = "linux")]
                {
                    if flags & libc::O_DIRECT != 0 {
                        open_flags |= fuser::consts::FOPEN_DIRECT_IO;
                    }
                }

                reply.opened(fh, open_flags);
            }
            Err(e) => {
                reply.error(e.raw_os_error().unwrap_or(EIO));
            }
        }
    }

    fn read(
        &mut self,
        _req: &Request<'_>,
        _ino: u64,
        fh: u64,
        offset: i64,
        size: u32,
        _flags: i32,
        _lock_owner: Option<u64>,
        reply: ReplyData,
    ) {
        let mut file = match self.get_file_handle(fh) {
            Ok(f) => f,
            Err(e) => {
                reply.error(e);
                return;
            }
        };

        if let Err(e) = file.seek(SeekFrom::Start(offset as u64)) {
            reply.error(e.raw_os_error().unwrap_or(EIO));
            return;
        }

        let mut buffer = vec![0u8; size as usize];
        match file.read(&mut buffer) {
            Ok(n) => {
                buffer.truncate(n);
                reply.data(&buffer);
            }
            Err(e) => {
                reply.error(e.raw_os_error().unwrap_or(EIO));
            }
        }
    }

    fn write(
        &mut self,
        req: &Request<'_>,
        ino: u64,
        fh: u64,
        offset: i64,
        data: &[u8],
        _write_flags: u32,
        flags: i32,
        _lock_owner: Option<u64>,
        reply: ReplyWrite,
    ) {
        let path = match self.get_path(ino) {
            Ok(path) => path,
            Err(e) => {
                reply.error(e);
                return;
            }
        };

        let mut file = match self.get_file_handle(fh) {
            Ok(f) => f,
            Err(e) => {
                reply.error(e);
                return;
            }
        };

        if let Err(e) = file.seek(SeekFrom::Start(offset as u64)) {
            reply.error(e.raw_os_error().unwrap_or(EIO));
            return;
        }

        match file.write(data) {
            Ok(n) => {
                // Write イベント通知
                if !self.should_ignore(&path) {
                    if let Some(uri) = path_to_uri(&path)
                        .map_err(|e| eprintln!("URI conversion failed: {:?}", e))
                        .ok()
                    {
                        let now = Utc::now();
                        let time = now.format("%Y-%m-%dT%H:%M:%S%.6f").to_string();

                            // O_APPEND フラグで Append/Write を切り替え
                        let event = if flags & libc::O_APPEND != 0 {
                            CanonicalEvent::Append {
                                uri,
                                time,
                            }
                        } else {
                            CanonicalEvent::Write {
                                uri,
                                content: data.to_vec(),
                                time,
                            }
                        };

                        let pid = req.pid();
                        let process_info = self.get_process_info(pid);

                        let file_event = FileEvent { event, process_info};
                        self.notify_event(&file_event);
                    }
                }

                reply.written(n as u32);
            }
            Err(e) => {
                reply.error(e.raw_os_error().unwrap_or(EIO));
            }
        }
    }

    fn flush(
        &mut self,
        _req: &Request<'_>,
        _ino: u64,
        fh: u64,
        _lock_owner: u64,
        reply: ReplyEmpty,
    ) {
        if let Ok(file) = self.get_file_handle(fh) {
            match file.sync_all() {
                Ok(_) => reply.ok(),
                Err(e) => reply.error(e.raw_os_error().unwrap_or(EIO)),
            }
        } else {
            reply.error(EIO);
        }
    }

    fn release(
        &mut self,
        _req: &Request<'_>,
        _ino: u64,
        fh: u64,
        _flags: i32,
        _lock_owner: Option<u64>,
        _flush: bool,
        reply: ReplyEmpty,
    ) {
        self.release_fh(fh);
        reply.ok();
    }

    fn fsync(
        &mut self,
        _req: &Request<'_>,
        _ino: u64,
        fh: u64,
        _datasync: bool,
        reply: ReplyEmpty,
    ) {
        if let Ok(file) = self.get_file_handle(fh) {
            match file.sync_all() {
                Ok(_) => reply.ok(),
                Err(e) => reply.error(e.raw_os_error().unwrap_or(EIO)),
            }
        } else {
            reply.error(EIO);
        }
    }

    fn opendir(&mut self, _req: &Request<'_>, ino: u64, _flags: i32, reply: ReplyOpen) {
        let path = match self.get_path(ino) {
            Ok(path) => path,
            Err(e) => {
                reply.error(e);
                return;
            }
        };

        match fs::read_dir(&path) {
            Ok(_) => {
                reply.opened(0, 0);
            }
            Err(e) => {
                reply.error(e.raw_os_error().unwrap_or(EIO));
            }
        }
    }

    fn readdir(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        _fh: u64,
        offset: i64,
        mut reply: ReplyDirectory,
    ) {
        let path = match self.get_path(ino) {
            Ok(path) => path,
            Err(e) => {
                reply.error(e);
                return;
            }
        };

        let entries = match fs::read_dir(&path) {
            Ok(entries) => entries,
            Err(e) => {
                reply.error(e.raw_os_error().unwrap_or(EIO));
                return;
            }
        };

        let mut index = 0;
        for entry in entries.skip(offset as usize) {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };

            let name = entry.file_name();
            let metadata = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };

            let kind = if metadata.is_dir() {
                fuser::FileType::Directory
            } else if metadata.is_symlink() {
                fuser::FileType::Symlink
            } else {
                fuser::FileType::RegularFile
            };

            let child_path = path.join(&name);
            let child_ino = self.allocate_inode(child_path);

            let is_full = reply.add(child_ino, offset + index + 1, kind, &name);
            if is_full {
                break;
            }

            index += 1;
        }

        reply.ok();
    }

    fn readdirplus(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        _fh: u64,
        offset: i64,
        mut reply: ReplyDirectoryPlus,
    ) {
        let path = match self.get_path(ino) {
            Ok(path) => path,
            Err(e) => {
                reply.error(e);
                return;
            }
        };

        let entries = match fs::read_dir(&path) {
            Ok(entries) => entries,
            Err(e) => {
                reply.error(e.raw_os_error().unwrap_or(EIO));
                return;
            }
        };

        let mut index = 0;
        for entry in entries.skip(offset as usize) {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };

            let name = entry.file_name();
            let metadata = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };

            let child_path = path.join(&name);
            let child_ino = self.allocate_inode(child_path);
            let attr = metadata_to_attr(child_ino, &metadata);

            let is_full = reply.add(child_ino, offset + index + 1, &name, &TTL, &attr, 0);
            if is_full {
                break;
            }

            index += 1;
        }

        reply.ok();
    }

    fn releasedir(
        &mut self,
        _req: &Request<'_>,
        _ino: u64,
        _fh: u64,
        _flags: i32,
        reply: ReplyEmpty,
    ) {
        reply.ok();
    }

    fn fsyncdir(
        &mut self,
        _req: &Request<'_>,
        _ino: u64,
        _fh: u64,
        _datasync: bool,
        reply: ReplyEmpty,
    ) {
        reply.ok();
    }

    fn statfs(&mut self, _req: &Request<'_>, ino: u64, reply: ReplyStatfs) {
        let path = match self.get_path(ino) {
            Ok(path) => path,
            Err(e) => {
                reply.error(e);
                return;
            }
        };

        match fs::metadata(&path) {
            Ok(_) => {
                // Return some default values
                reply.statfs(
                    1000000, // blocks
                    500000,  // bfree
                    500000,  // bavail
                    100000,  // files
                    50000,   // ffree
                    4096,    // bsize
                    255,     // namelen
                    4096,    // frsize
                );
            }
            Err(e) => {
                reply.error(e.raw_os_error().unwrap_or(EIO));
            }
        }
    }

    fn setxattr(
        &mut self,
        _req: &Request<'_>,
        _ino: u64,
        _name: &OsStr,
        _value: &[u8],
        _flags: i32,
        _position: u32,
        reply: ReplyEmpty,
    ) {
        reply.error(ENOSYS);
    }

    fn getxattr(
        &mut self,
        _req: &Request<'_>,
        _ino: u64,
        _name: &OsStr,
        _size: u32,
        reply: ReplyXattr,
    ) {
        reply.error(ENOSYS);
    }

    fn listxattr(
        &mut self,
        _req: &Request<'_>,
        _ino: u64,
        _size: u32,
        reply: ReplyXattr,
    ) {
        reply.error(ENOSYS);
    }

    fn removexattr(
        &mut self,
        _req: &Request<'_>,
        _ino: u64,
        _name: &OsStr,
        reply: ReplyEmpty,
    ) {
        reply.error(ENOSYS);
    }

    fn access(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        _mask: i32,
        reply: ReplyEmpty,
    ) {
        let path = match self.get_path(ino) {
            Ok(path) => path,
            Err(e) => {
                reply.error(e);
                return;
            }
        };

        match fs::metadata(&path) {
            Ok(_) => reply.ok(),
            Err(e) => {
                reply.error(e.raw_os_error().unwrap_or(EACCES));
            }
        }
    }

    fn create(
        &mut self,
        req: &Request<'_>,
        parent: u64,
        name: &OsStr,
        mode: u32,
        _umask: u32,
        flags: i32,
        reply: ReplyCreate,
    ) {
        let parent_path = match self.get_path(parent) {
            Ok(path) => path,
            Err(e) => {
                reply.error(e);
                return;
            }
        };

        let path = parent_path.join(name);

        let mut options = OpenOptions::new();
        options.write(true).create(true);

        if flags & libc::O_EXCL != 0 {
            options.create_new(true);
        }
        if flags & libc::O_TRUNC != 0 {
            options.truncate(true);
        }

        match options.open(&path) {
            Ok(file) => {
                if let Err(e) = fs::set_permissions(&path, fs::Permissions::from_mode(mode)) {
                    reply.error(e.raw_os_error().unwrap_or(EIO));
                    return;
                }

                match fs::metadata(&path) {
                    Ok(metadata) => {
                        let ino = self.allocate_inode(path.clone());
                        let attr = metadata_to_attr(ino, &metadata);
                        let fh = self.allocate_fh(file);

                        // Create イベント通知
                        if !self.should_ignore(&path) {
                            if let Some(uri) = path_to_uri(&path)
                                .map_err(|e| eprintln!("URI conversion failed: {:?}", e))
                                .ok()
                            {
                                let now = Utc::now();
                                let time = now.format("%Y-%m-%dT%H:%M:%S%.6f").to_string();
                                let event = CanonicalEvent::Create {
                                    uri,
                                    time,
                                };

                                let pid = req.pid();
                                let process_info = self.get_process_info(pid);

                                let file_event = FileEvent { event, process_info};
                                self.notify_event(&file_event);
                            }
                        }

                        let mut open_flags = 0;
                        #[cfg(target_os = "linux")]
                        {
                            if flags & libc::O_DIRECT != 0 {
                                open_flags |= fuser::consts::FOPEN_DIRECT_IO;
                            }
                        }

                        reply.created(&TTL, &attr, 0, fh, open_flags);
                    }
                    Err(e) => {
                        reply.error(e.raw_os_error().unwrap_or(EIO));
                    }
                }
            }
            Err(e) => {
                reply.error(e.raw_os_error().unwrap_or(EIO));
            }
        }
    }

    fn getlk(
        &mut self,
        _req: &Request<'_>,
        _ino: u64,
        _fh: u64,
        _lock_owner: u64,
        _start: u64,
        _end: u64,
        _typ: i32,
        _pid: u32,
        reply: ReplyLock,
    ) {
        reply.error(ENOSYS);
    }

    fn setlk(
        &mut self,
        _req: &Request<'_>,
        _ino: u64,
        _fh: u64,
        _lock_owner: u64,
        _start: u64,
        _end: u64,
        _typ: i32,
        _pid: u32,
        _sleep: bool,
        reply: ReplyEmpty,
    ) {
        reply.error(ENOSYS);
    }

    fn bmap(
        &mut self,
        _req: &Request<'_>,
        _ino: u64,
        _blocksize: u32,
        _idx: u64,
        reply: ReplyBmap,
    ) {
        reply.error(ENOSYS);
    }

    fn ioctl(
        &mut self,
        _req: &Request<'_>,
        _ino: u64,
        _fh: u64,
        _flags: u32,
        _cmd: u32,
        _in_data: &[u8],
        _out_size: u32,
        reply: ReplyIoctl,
    ) {
        reply.error(ENOSYS);
    }

    fn fallocate(
        &mut self,
        _req: &Request<'_>,
        _ino: u64,
        fh: u64,
        offset: i64,
        length: i64,
        _mode: i32,
        reply: ReplyEmpty,
    ) {
        if let Ok(file) = self.get_file_handle(fh) {
            let new_size = offset as u64 + length as u64;
            match file.set_len(new_size) {
                Ok(_) => reply.ok(),
                Err(e) => reply.error(e.raw_os_error().unwrap_or(EIO)),
            }
        } else {
            reply.error(EIO);
        }
    }

    fn lseek(
        &mut self,
        _req: &Request<'_>,
        _ino: u64,
        fh: u64,
        offset: i64,
        whence: i32,
        reply: ReplyLseek,
    ) {
        if let Ok(mut file) = self.get_file_handle(fh) {
            let seek_from = match whence {
                libc::SEEK_SET => SeekFrom::Start(offset as u64),
                libc::SEEK_CUR => SeekFrom::Current(offset),
                libc::SEEK_END => SeekFrom::End(offset),
                _ => {
                    reply.error(EINVAL);
                    return;
                }
            };

            match file.seek(seek_from) {
                Ok(pos) => reply.offset(pos as i64),
                Err(e) => reply.error(e.raw_os_error().unwrap_or(EIO)),
            }
        } else {
            reply.error(EIO);
        }
    }

    fn copy_file_range(
        &mut self,
        _req: &Request<'_>,
        _ino_in: u64,
        fh_in: u64,
        offset_in: i64,
        _ino_out: u64,
        fh_out: u64,
        offset_out: i64,
        len: u64,
        _flags: u32,
        reply: ReplyWrite,
    ) {
        let mut file_in = match self.get_file_handle(fh_in) {
            Ok(f) => f,
            Err(e) => {
                reply.error(e);
                return;
            }
        };

        let mut file_out = match self.get_file_handle(fh_out) {
            Ok(f) => f,
            Err(e) => {
                reply.error(e);
                return;
            }
        };

        if let Err(e) = file_in.seek(SeekFrom::Start(offset_in as u64)) {
            reply.error(e.raw_os_error().unwrap_or(EIO));
            return;
        }

        if let Err(e) = file_out.seek(SeekFrom::Start(offset_out as u64)) {
            reply.error(e.raw_os_error().unwrap_or(EIO));
            return;
        }

        let mut buffer = vec![0u8; len as usize];
        match file_in.read(&mut buffer) {
            Ok(n) => {
                buffer.truncate(n);
                match file_out.write(&buffer) {
                    Ok(written) => reply.written(written as u32),
                    Err(e) => reply.error(e.raw_os_error().unwrap_or(EIO)),
                }
            }
            Err(e) => {
                reply.error(e.raw_os_error().unwrap_or(EIO));
            }
        }
    }
}
