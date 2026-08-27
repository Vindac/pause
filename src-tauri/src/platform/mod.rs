//! SystemActivityService.swift 的 Rust 移植：锁屏/屏保/屏幕睡眠状态 +
//! 键鼠空闲检测 + 唤醒通知。平台实现在 `macos.rs` / `windows.rs`，
//! 本文件提供跨平台共享的标志集合与探针类型。

#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "windows")]
pub mod windows;

use crate::reminder::SystemActivityProviding;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;

/// 平台会话事件：仅"环境恢复展示"类需要触发主循环立即补一拍（防风暴语义）。
#[derive(Debug, Clone, Copy)]
pub enum SessionEvent {
    /// 系统从睡眠唤醒。
    SystemWake,
    /// 显示器唤醒（屏幕重新点亮）。
    ScreensAwake,
    /// 用户解除锁屏（恢复观感，但不一定伴随系统唤醒）。
    Unlocked,
}

/// 三态演示受阻标志：与原版 isScreenLocked/isScreensaverActive/isScreenAsleep 一致。
#[derive(Default)]
pub struct ActivityFlags {
    pub screen_locked: AtomicBool,
    pub screensaver_active: AtomicBool,
    pub screen_asleep: AtomicBool,
}

impl ActivityFlags {
    pub fn presentation_blocked(&self) -> bool {
        self.screen_locked.load(Ordering::Relaxed)
            || self.screensaver_active.load(Ordering::Relaxed)
            || self.screen_asleep.load(Ordering::Relaxed)
    }

    fn set_locked(&self, v: bool) {
        self.screen_locked.store(v, Ordering::Relaxed);
    }
    fn set_saver(&self, v: bool) {
        self.screensaver_active.store(v, Ordering::Relaxed);
    }
    fn set_asleep(&self, v: bool) {
        self.screen_asleep.store(v, Ordering::Relaxed);
    }
}

type SharedFlags = Arc<ActivityFlags>;

/// 组合探针：flag 由监听线程维护；空闲值由专用低频轮询线程写入缓存，
/// 主线程只读原子快照 —— 直接在主线程查 CGEventSource 会触发
/// SkyLight 内部锁与 NSApplication 事件循环互等而钳死。
pub struct SystemActivityService {
    #[allow(dead_code)]
    flags: SharedFlags,
    idle_bits: Arc<AtomicU64>,
}

impl SystemActivityProviding for SystemActivityService {
    fn is_presentation_blocked(&self) -> bool {
        self.flags.presentation_blocked()
    }
    fn user_idle_seconds(&self) -> f64 {
        f64::from_bits(self.idle_bits.load(Ordering::Relaxed))
    }
}

impl SystemActivityService {
    pub fn new() -> Self {
        let svc = Self {
            flags: Arc::new(ActivityFlags::default()),
            idle_bits: Arc::new(AtomicU64::new(0)),
        };
        svc.start_idle_poller();
        svc
    }

    /// 200ms 轮询原生空闲值入缓存（后台线程，代价可忽略）。
    fn start_idle_poller(&self) {
        let idle_bits = Arc::clone(&self.idle_bits);
        std::thread::Builder::new()
            .name("pause-idle-poll".into())
            .spawn(move || loop {
                let v = platform_idle_seconds();
                idle_bits.store(v.to_bits(), Ordering::Relaxed);
                std::thread::sleep(std::time::Duration::from_millis(200));
            })
            .expect("spawn idle poller");
    }

    /// 启动平台监听线程（锁屏/屏保/睡眠），恢复类事件发往 sink。
    pub fn start_watching(&self, sink: UnboundedSender<SessionEvent>) {
        platform_start_session_watch(self.flags.clone(), sink);
    }

    /// 每秒心跳调用的平台轮询钩子。
    /// Windows 屏保无进程内事件，走定时查询；macOS 已有分布式通知，无需处理。
    pub fn on_tick_poll(&self) {
        platform_on_tick_poll(&self.flags);
    }
}

impl Default for SystemActivityService {
    fn default() -> Self {
        Self::new()
    }
}

// ------------------------- 平台分派 -------------------------

/// 距上次键鼠输入的秒数（只读时间戳，无需辅助功能权限）。
#[cfg(target_os = "macos")]
fn platform_idle_seconds() -> f64 {
    macos::user_idle_seconds()
}
#[cfg(target_os = "windows")]
fn platform_idle_seconds() -> f64 {
    windows::user_idle_seconds()
}

#[cfg(target_os = "macos")]
fn platform_start_session_watch(
    flags: SharedFlags,
    sink: UnboundedSender<SessionEvent>,
) {
    macos::start_session_watch(flags, sink)
}
#[cfg(target_os = "windows")]
fn platform_start_session_watch(
    flags: SharedFlags,
    sink: UnboundedSender<SessionEvent>,
) {
    windows::start_session_watch(flags, sink)
}

#[cfg(target_os = "macos")]
fn platform_on_tick_poll(_flags: &ActivityFlags) {}
#[cfg(target_os = "windows")]
fn platform_on_tick_poll(flags: &ActivityFlags) {
    windows::poll_screensaver(flags)
}

// 非 mac/win 平台的兜底（保持全栈可在其它桌面环境静态编译）
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn platform_idle_seconds() -> f64 {
    0.0
}
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn platform_start_session_watch(
    _flags: SharedFlags,
    _sink: UnboundedSender<SessionEvent>,
) {
}
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn platform_on_tick_poll(_flags: &ActivityFlags) {}

// ------------------------- 多屏几何统一导出 -------------------------

/// 全局「左上原点逻辑点」几何类型与显示器枚举 —— 与 Tauri LogicalPosition 一致。
#[cfg(target_os = "macos")]
pub use macos::{RectL, ScreenInfoL, cursor_position, list_screens};
#[cfg(target_os = "windows")]
pub use windows::{RectL, ScreenInfoL, cursor_position, list_screens};

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
#[derive(Debug, Clone, Copy)]
pub struct RectL {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
impl RectL {
    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
}
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
#[derive(Debug, Clone, Copy)]
pub struct ScreenInfoL {
    pub frame: RectL,
    pub work_area: RectL,
}
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn list_screens() -> Vec<ScreenInfoL> {
    vec![ScreenInfoL {
        frame: RectL { x: 0.0, y: 0.0, w: 1920.0, h: 1080.0 },
        work_area: RectL { x: 0.0, y: 0.0, w: 1920.0, h: 1055.0 },
    }]
}
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn cursor_position() -> Option<(f64, f64)> {
    Some((960.0, 540.0))
}
