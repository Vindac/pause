//! 提醒窗口编排：ReminderWindowController.swift 的移植。
//!
//! 职责划分（对应原 NSPanel 动画链条，按跨端一致性重构）：
//! - Rust：多屏定位与等比缩放、层级升降、show / 延迟 hide 时序；
//! - 前端（reminder.html）：16px 圆角裁剪、Ken Burns、两页交叉淡化、
//!   淡入 0.35s / 淡出 0.5s / 透明度调整 0.2s（CSS transition，
//!   由 `reminder-window-changed` 事件驱动）。
//!
//! - 900×600 基准按目标显示器 work area 内缩 40pt 等比缩放（保持 3:2）；
//! - 在鼠标所在显示器**整屏中心**居中；
//! - 「覆盖其他窗口」以 always-on-top 近似 .floating/.screenSaver 升降；
//! - ensureVisible：休息中复用已显示窗口，不重复淡入。

use crate::platform;
use std::sync::atomic::{AtomicU64, Ordering};
use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager};

/// 基准尺寸（3:2）与缩放内边距（与原版一致）
pub const PREFERRED_W: f64 = 900.0;
pub const PREFERRED_H: f64 = 600.0;
const INSET: f64 = 40.0;
/// 淡出完成后 orderOut 的等待时间（略长于 0.5s 动画）。
const HIDE_AFTER_MS: u64 = 520;

static HIDE_TOKEN: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, serde::Serialize)]
#[serde(tag = "command", rename_all = "kebab-case")]
pub enum ReminderWindowCommand {
    /// 从透明淡入至目标不透明度（0.35s）。
    ShowFade { opacity: f64 },
    /// 平滑过渡到新不透明度（0.2s）。
    SetOpacity { opacity: f64 },
    /// 开始淡出至 0（0.5s），随后由 Rust 调 hide。
    HideFade,
}

fn push_command(app: &AppHandle, cmd: ReminderWindowCommand) {
    let _ = app.emit("reminder-window-changed", &cmd);
}

/// 900×600 等比缩放：可用区域为 work_area 内缩 40pt，向下取整保比例。
pub fn scaled_size(work: (f64, f64)) -> (u32, u32) {
    let avail_w = (work.0 - 2.0 * INSET).max(120.0);
    let avail_h = (work.1 - 2.0 * INSET).max(80.0);
    let scale = 1.0f64.min(avail_w / PREFERRED_W).min(avail_h / PREFERRED_H);
    (
        (PREFERRED_W * scale).floor().max(200.0) as u32,
        (PREFERRED_H * scale).floor().max(134.0) as u32,
    )
}

fn screen_for_mouse() -> Option<platform::ScreenInfoL> {
    let screens = platform::list_screens();
    if screens.is_empty() {
        return None;
    }
    platform::cursor_position()
        .and_then(|(cx, cy)| screens.iter().find(|s| s.frame.contains(cx, cy)).copied())
        .or_else(|| screens.first().copied())
}

/// 展示提醒窗（前端承接淡入）。定位-尺寸-层级全部就绪后才 show。
pub fn show_reminder(app: &AppHandle) {
    let Some(win) = app.get_webview_window("reminder") else {
        return;
    };
    let Some(screen) = screen_for_mouse() else { return };
    let settings = crate::current_settings(app);

    let (w, h) = scaled_size((screen.work_area.w, screen.work_area.h));
    let _ = win.set_size(LogicalSize::new(w as f64, h as f64));
    // 相对整屏 frame 居中（非可见区），与原版 centeredFrame 一致
    let x = screen.frame.x + (screen.frame.w - w as f64) / 2.0;
    let y = screen.frame.y + (screen.frame.h - h as f64) / 2.0;
    let _ = win.set_position(LogicalPosition::new(x, y));

    let _ = win.set_always_on_top(settings.overlay_other_windows);
    let _ = win.show(); // 窗口已可见但内容 CSS 为 opacity 0

    crate::debug_log::debug_log(&format!(
        "show reminder at ({x:.0},{y:.0}) size {w}x{h}"
    ));
    // 使尚未执行的隐藏任务失效
    HIDE_TOKEN.fetch_add(1, Ordering::Relaxed);
    push_command(
        app,
        ReminderWindowCommand::ShowFade { opacity: settings.reminder_window_opacity },
    );
}

/// 休息中复用可见窗口；仅当确实不可见时补齐定位与淡入。
pub fn ensure_visible(app: &AppHandle) {
    let Some(win) = app.get_webview_window("reminder") else { return };
    match win.is_visible() {
        Ok(true) => {}
        _ => show_reminder(app),
    }
}

/// 透明度设置实时生效（前端 0.2s 过渡）。
pub fn apply_opacity_live(app: &AppHandle, target: f64) {
    push_command(app, ReminderWindowCommand::SetOpacity { opacity: target });
}

/// 隐藏提醒窗：先通知前端淡出（0.5s），超时后 orderOut。
/// 若期间发生新的 show，令牌失配即自动取消。
pub fn hide_reminder(app: &AppHandle) {
    let Some(win) = app.get_webview_window("reminder") else { return };
    match win.is_visible() {
        Ok(false) => return,
        Ok(_) => {}
        Err(_) => return,
    }
    push_command(app, ReminderWindowCommand::HideFade);

    let token = HIDE_TOKEN.fetch_add(1, Ordering::Relaxed) + 1;
    let wl = win.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(HIDE_AFTER_MS)).await;
        if HIDE_TOKEN.load(Ordering::Relaxed) == token {
            let _ = wl.hide();
            crate::debug_log::debug_log("reminder hidden");
        }
    });
}

/// 打开设置窗口（titled 常规窗；标题随语言更新）。
pub fn open_settings(app: &AppHandle) {
    let Some(win) = app.get_webview_window("settings") else { return };
    let lang = crate::current_lang(app);
    let title = crate::l10n::tr(lang, "settingsTitle");
    let _ = win.set_title(title.as_str());
    let _ = win.center();
    let _ = win.show();
    let _ = win.set_focus();
    crate::debug_log::debug_log("settings window opened");
}
