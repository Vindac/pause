//! TrayIcon 与下拉菜单：对应原 MenuBarExtra + MenuBarView。
//!
//! 双平台标签策略：
//! - macOS：不设置图标，`TrayIcon::set_title` 只显示 `43m` / `!` / `⏸` 纯文本
//!   （系统自适应深浅，与原 Swift MenuBarExtra 形态一致）；
//! - Windows：托盘不支持原生文本标题，显示应用图标 + tooltip 携带完整状态行，
//!   剩余分钟信息随 tooltip 分钟级刷新。
//!
//! 菜单五段：状态行(disabled) / 立即休息 / 暂停-继续(⌘P) / 设置…(⌘,) / 退出(⌘Q)。
//! 全部 item 恒可点击（与原版一致，无 disabled 条件）。语言或状态变化时整体重建。

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder};
use crate::l10n;
use tauri::{AppHandle, Wry};

/// 由 lib.rs 依据 Snapshot + 当前语言预算好的展示素材。
#[derive(Debug, Clone, PartialEq)]
pub struct TrayView {
    pub bar_label: String,
    pub status_title: String,
    pub pause_label: String,
}

pub fn build(app: &AppHandle) -> tauri::Result<TrayIcon> {
    let empty_menu = Menu::new(app)?;
    #[cfg(not(target_os = "macos"))]
    let default_icon = app.default_window_icon().cloned();
    let mut builder = TrayIconBuilder::with_id("pause-tray")
        .tooltip("")
        .menu(&empty_menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| {
            crate::lib_handle_menu(app, event.id().as_ref());
        })
        .on_tray_icon_event(|_tray, event| {
            // 左键即弹菜单由 show_menu_on_left_click 承担；此处仅记录
            if matches!(event, tauri::tray::TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. }) {
                crate::debug_log::debug_log("tray clicked");
            }
        });
    // macOS 纯文本形态：不设置图标，标签即全部内容
    #[cfg(not(target_os = "macos"))]
    if let Some(icon) = default_icon {
        builder = builder.icon(icon);
    }
    let tray = builder.build(app)?;

    Ok(tray)
}

/// 整体重建菜单并刷新标签（状态或语言变化时调用）。
/// 内容未变化时跳过重建 —— 心跳每秒调用一次，避免无谓打扰展开中的菜单。
pub fn refresh(
    app: &AppHandle,
    tray: &TrayIcon,
    view: &TrayView,
) -> tauri::Result<()> {
    let mut last = LAST_VIEW.lock().unwrap();
    if last.as_ref() == Some(view) {
        return Ok(());
    }
    *last = Some(view.clone());
    drop(last);

    build_and_set_menu(app, tray, view)?;
    apply_bar_label(tray, view);
    let _ = tray.set_tooltip(Some(view.status_title.clone()));
    Ok(())
}

static LAST_VIEW: std::sync::Mutex<Option<TrayView>> = std::sync::Mutex::new(None);

fn build_and_set_menu(app: &AppHandle, tray: &TrayIcon, view: &TrayView) -> tauri::Result<()> {
    let lang = crate::current_lang(app);
    let status =
        MenuItem::with_id(app, "status", view.status_title.clone(), false, None::<&str>)?;
    let sep1 = PredefinedMenuItem::separator(app)?;
    let break_now = MenuItem::with_id(
        app,
        "break-now",
        l10n::tr(lang, "menuBreakNow"),
        true,
        None::<&str>,
    )?;
    let toggle = MenuItem::with_id(
        app,
        "pause-toggle",
        view.pause_label.clone(),
        true,
        Some("CmdOrCtrl+P"),
    )?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let settings = MenuItem::with_id(
        app,
        "open-settings",
        l10n::tr(lang, "menuSettings"),
        true,
        Some("CmdOrCtrl+,"),
    )?;
    let sep3 = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(
        app,
        "quit",
        l10n::tr(lang, "menuQuit"),
        true,
        Some("CmdOrCtrl+Q"),
    )?;

    let menu = Menu::with_items(
        app,
        &[&status, &sep1, &break_now, &toggle, &sep2, &settings, &sep3, &quit],
    )?;
    let _ = tray.set_menu(Some(menu));
    Ok(())
}

fn apply_bar_label(tray: &TrayIcon, view: &TrayView) {
    crate::debug_log::debug_log(&format!("bar label -> {:?}", view.bar_label));
    #[cfg(target_os = "macos")]
    {
        if view.bar_label.is_empty() {
            let _ = tray.set_title(None::<String>);
        } else {
            let _ = tray.set_title(Some(view.bar_label.clone()));
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = tray; // Windows 用 tooltip 表达，无需额外处理
    }
}

/// 供测试与复用：暂停项文案切换。
pub fn pause_label(lang: crate::l10n::Lang, is_paused: bool) -> String {
    if is_paused {
        crate::l10n::tr(lang, "menuResume")
    } else {
        crate::l10n::tr(lang, "menuPause")
    }
}

pub type TrayRef = TrayIcon<Wry>;
