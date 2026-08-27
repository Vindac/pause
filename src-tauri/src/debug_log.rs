//! 调试日志：对齐原 DebugLog.swift，追加写 pause_debug.log。
//! 默认静默，PAUSE_DEBUG=1 时启用，便于现场排查。

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

static LOG_LOCK: Mutex<()> = Mutex::new(());

fn log_path() -> PathBuf {
    if cfg!(target_os = "windows") {
        std::env::temp_dir().join("pause_debug.log")
    } else {
        PathBuf::from("/tmp/pause_debug.log")
    }
}

/// append 一行时间戳日志；失败静默（日志绝不能影响主流程）。
pub fn debug_log(message: &str) {
    if std::env::var("PAUSE_DEBUG").ok().as_deref() != Some("1") {
        return;
    }
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let line = format!(
        "{ms}ms [{:?}] {message}\n",
        std::thread::current().name().unwrap_or("main")
    );
    let _guard = LOG_LOCK.lock();
    if let Ok(mut f) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path())
    {
        let _ = f.write_all(line.as_bytes());
    }
}
