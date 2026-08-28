//! 应用组装与运行时编排：对应原 PauseApp.swift + DependencyContainer.swift。
//!
//! 数据流保持单向：状态机是唯一计时真相源 → 每秒心跳产出快照 →
//! 驱动托盘 / 提醒窗口 / 前端事件；用户动作（菜单/前端）反向注入服务。

pub mod debug_log;
pub mod l10n;
pub mod platform;
pub mod reminder;
pub mod settings;
pub mod tray;
pub mod wallpaper;
pub mod windows;

use l10n::Lang;
use platform::SystemActivityService;
use reminder::{ReminderDeps, ReminderService, SharedReminderService, Snapshot};
use settings::{SettingsFile, SetResult, SharedSettings};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::mpsc;
use tray::TrayView;
use wallpaper::WallpaperService;

/// 进程级共享容器。
pub struct AppState {
    pub settings_file: SettingsFile,
    pub settings: SharedSettings,
    pub service: SharedReminderService,
    pub wallpapers: Arc<WallpaperService>,
    pub activity: Arc<SystemActivityService>,
    /// 间隔修改后的 300ms 重启防抖句柄。
    pub restart_debounce: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
    /// 休息期壁纸轮换的上次换图时刻。
    pub last_break_advance_ms: AtomicU64,
    /// 上一次心跳的相位标签（用于检出相变边沿）。
    pub prev_phase_tag: Mutex<String>,
}

const BREAK_ROTATION_SECS: f64 = 25.0;

// =====================================================================
// 入口
// =====================================================================

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|_app, _args, _cwd| {
            // 二次启动：静默忽略（托盘应用常驻，无主窗口可聚焦）
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            // Windows 上隐藏 taskbar 出现的空窗口柄由窗口 visible:false + skipTaskbar 保证

            let state = build_state(app.handle())?;
            app.manage(state);

            // 设置窗点关闭 = 隐藏而非销毁（Tauri 默认销毁会导致无法再次打开）
            if let Some(settings_win) = app.get_webview_window("settings") {
                let w = settings_win.clone();
                settings_win.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = w.hide();
                    }
                });
            }

            {
                let state = app.state::<AppState>();
                // 按真实使用时间与原版一致：期望值 != 实际状态才切换；
                // dev/debug 构建跳过，防止裸二进制误写 LaunchAgent
                let is_dev_build = cfg!(debug_assertions);
                if !is_dev_build {
                    let desired =
                        state.settings.lock().unwrap().launch_at_login;
                    sync_launch_at_login(app.handle(), desired);
                }
                drop(state);
            }
            start_tray(app.handle())?;
            start_tick_loop(app.handle());
            start_session_watch(app.handle());
            run_demo_mode(app.handle());
            spawn_startup_update_check(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            get_strings,
            get_settings,
            set_setting,
            set_language,
            switch_wallpaper,
            get_current_wallpaper,
            act_start_break,
            act_snooze,
            act_skip_break,
            act_pause,
            act_resume,
            open_settings_window,
            quit_app,
            app_version,
            check_update,
            github_latest_release,
            skip_update_version
        ])
        .run(tauri::generate_context!())
        .expect("error while running pause");
}

fn now_epoch_secs() -> f64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}


/// 将具体探针擦型为 trait 对象（unsized coercion）。
fn make_probe(a: &Arc<SystemActivityService>) -> Arc<dyn reminder::SystemActivityProviding> {
    let owned: Arc<SystemActivityService> = Arc::clone(a);
    let trait_obj: Arc<dyn reminder::SystemActivityProviding> = owned;
    trait_obj
}

/// DependencyContainer 组装序列的 Rust 版。
fn build_state(app: &AppHandle) -> Result<AppState, Box<dyn std::error::Error>> {
    let config_dir = app.path().app_config_dir()?;
    let cache_dir = app.path().app_cache_dir()?;
    let settings_file = SettingsFile::new(&config_dir);
    let loaded = settings_file.load();
    let shared: SharedSettings = Arc::new(Mutex::new(loaded));

    let activity = Arc::new(SystemActivityService::new());
    let wallpapers = Arc::new(WallpaperService::new(shared.clone(), &cache_dir));
    let current_wallpaper = wallpapers.bootstrap();
    crate::debug_log::debug_log(&format!("bootstrap wallpaper {}", current_wallpaper.display()));

    // 资源协议放行缓存目录与临时目录（前端 convertFileSrc 加载壁纸）
    let _ = app.asset_protocol_scope().allow_directory(&cache_dir, true);
    let _ = app
        .asset_protocol_scope()
        .allow_directory(std::env::temp_dir(), true);

    // 提示音由前端播放内置 chime.wav
    let sound_handle = app.clone();
    let on_sound: reminder::SoundHook = Arc::new(move || {
        let _ = sound_handle.emit("play-sound", ());
    });

    // Move + 未定型强制：Arc<SystemActivityService> → Arc<dyn SystemActivityProviding>
    let system_probe: Arc<dyn reminder::SystemActivityProviding> = make_probe(&activity);
    let service = Arc::new(Mutex::new(ReminderService::new(ReminderDeps {
        settings: shared.clone(),
        clock: Arc::new(now_epoch_secs),
        system: system_probe,
        on_sound: Some(on_sound),
    })));
    service.lock().unwrap().start();

    Ok(AppState {
        settings_file,
        settings: shared,
        service,
        wallpapers,
        activity,
        restart_debounce: Mutex::new(None),
        last_break_advance_ms: AtomicU64::new(0),
        prev_phase_tag: Mutex::new(String::new()),
    })
}

// =====================================================================
// 心跳循环 / 会话事件 → 编排 + 托盘 + 推送
// =====================================================================

fn start_tick_loop(app: &AppHandle) {
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
        let mut first_beat_logged = false;
        loop {
            interval.tick().await;
            if !first_beat_logged {
                first_beat_logged = true;
                debug_log::debug_log("tick loop alive");
            }
            {
                let state = handle.state::<AppState>();
                state.activity.on_tick_poll();
            }
            // 状态机推进与一切副作用串行化到主线程（对齐原版 @MainActor 模型）
            dispatch_step(&handle);
        }
    });
}

fn start_session_watch(app: &AppHandle) {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let handle = app.clone();
    app.state::<AppState>().activity.start_watching(tx);
    tauri::async_runtime::spawn(async move {
        while let Some(ev) = rx.recv().await {
            debug_log::debug_log(&format!("session event {ev:?}"));
            // 唤醒后立即补一拍：过期 deadline 只转换为一次提醒（防风暴）
            dispatch_step(&handle);
        }
    });
}

/// 把一次心跳投递到主线程串行执行。
fn dispatch_step(handle: &AppHandle) {
    let h = handle.clone();
    let _ = handle.run_on_main_thread(move || step_forward(&h));
}

/// 一次推进：tick（或动作）→ 编排窗口 → 刷新托盘 → 推送快照。（主线程）
fn step_forward(handle: &AppHandle) {
    let snapshot = {
        let state = handle.state::<AppState>();
        let mut svc = state.service.lock().unwrap();
        svc.handle_tick(None);
        svc.snapshot()
    };
    orchestrate(handle, &snapshot);
    refresh_tray(handle, &snapshot);
    push_state(handle, &snapshot);
}

/// phase → UI 编排（DependencyContainer.start 第 4 步）。
/// 仅在**相变边沿**执行窗口/壁纸动作，稳定期内不做重复调度；
/// 休息期内部的 25 秒壁纸轮换除外。
fn orchestrate(handle: &AppHandle, snap: &Snapshot) {
    let tag = snap.phase.tag();
    let changed = {
        let state = handle.state::<AppState>();
        let mut prev = state.prev_phase_tag.lock().unwrap();
        let changed = *prev != tag;
        *prev = tag.to_string();
        changed
    };

    if !changed {
        if tag == "breaking" {
            maybe_rotate_break_wallpaper(handle);
        }
        return;
    }

    match tag {
        "reminding" => {
            // 到点提醒：切图 + 弹窗（永不等待网络，预取就绪图优先）
            let path = { handle.state::<AppState>().wallpapers.advance() };
            push_wallpaper(handle, &path);
            handle
                .state::<AppState>()
                .last_break_advance_ms
                .store(0, Ordering::Relaxed); // 重置轮换计时基准
            windows::show_reminder(handle);
        }
        "breaking" => {
            handle
                .state::<AppState>()
                .last_break_advance_ms
                .store((now_epoch_secs() * 1000.0) as u64, Ordering::Relaxed);
            windows::ensure_visible(handle);
        }
        _ => windows::hide_reminder(handle),
    }
}

/// 休息期间每 25 秒换一张壁纸。
fn maybe_rotate_break_wallpaper(handle: &AppHandle) {
    let state = handle.state::<AppState>();
    let now_ms = (now_epoch_secs() * 1000.0) as u64;
    let last = state.last_break_advance_ms.load(Ordering::Relaxed);
    if last == 0 {
        state.last_break_advance_ms.store(now_ms, Ordering::Relaxed);
        return;
    }
    if (now_ms - last) as f64 / 1000.0 >= BREAK_ROTATION_SECS {
        state.last_break_advance_ms.store(now_ms, Ordering::Relaxed);
        let path = state.wallpapers.advance();
        push_wallpaper(handle, &path);
    }
}

// =====================================================================
// 展示素材计算（MenuBarViewModel 逻辑）
// =====================================================================

fn current_lang(app: &AppHandle) -> Lang {
    let lang_str = {
        let state = app.state::<AppState>();
        let s = state.settings.lock().unwrap();
        s.app_language.clone()
    };
    Lang::parse(&lang_str)
}

fn current_settings(app: &AppHandle) -> settings::Settings {
    app.state::<AppState>().settings.lock().unwrap().clone()
}

/// 计算菜单栏标签、状态行、暂停项文案。
pub fn compute_tray_view(lang: Lang, snap: &Snapshot) -> TrayView {
    let bar_label = bar_label(snap);
    let status_title = status_title(lang, snap);
    let pause_label = if matches!(snap.phase.tag(), "paused") {
        l10n::tr(lang, "menuResume")
    } else {
        l10n::tr(lang, "menuPause")
    };
    TrayView { bar_label, status_title, pause_label }
}

fn minutes_from_deadline(deadline: f64) -> Option<i64> {
    Some(((deadline - now_epoch_secs()) / 60.0).ceil().max(0.0) as i64)
}

fn bar_label(snap: &Snapshot) -> String {
    match &snap.phase {
        reminder::Phase::Paused => "⏸".into(),
        reminder::Phase::Reminding { .. } => "!".into(),
        reminder::Phase::Working { deadline } => {
            if snap.is_waiting_for_presentation {
                "!".into()
            } else {
                deadline_minutes_or_placeholder(*deadline)
            }
        }
        reminder::Phase::Snoozing { deadline, .. } => deadline_minutes_or_placeholder(*deadline),
        reminder::Phase::Breaking { started_at, duration_secs } => {
            let remaining =
                reminder::BreakSession { started_at: *started_at, duration_secs: *duration_secs }
                    .remaining(now_epoch_secs());
            let m = ((remaining / 60.0).ceil() as i64).max(0);
            format!("{m}m")
        }
    }
}

fn deadline_minutes_or_placeholder(deadline: f64) -> String {
    match minutes_from_deadline(deadline) {
        Some(m) => format!("{m}m"),
        None => "--m".into(),
    }
}

fn status_title(lang: Lang, snap: &Snapshot) -> String {
    match &snap.phase {
        reminder::Phase::Paused => l10n::tr(lang, "statusPaused"),
        reminder::Phase::Reminding { .. } => l10n::tr(lang, "statusReminding"),
        reminder::Phase::Working { deadline } | reminder::Phase::Snoozing { deadline, .. } => {
            if snap.is_user_idle {
                return l10n::tr(lang, "statusIdle");
            }
            if snap.is_waiting_for_presentation {
                return l10n::tr(lang, "statusWaiting");
            }
            match minutes_from_deadline(*deadline) {
                Some(m) if !matches!(snap.phase.tag(), "snoozing") => {
                    l10n::tf(lang, "statusNextBreak", &[("m", m.to_string())])
                }
                Some(_) if matches!(snap.phase.tag(), "working") => String::new(),
                Some(m) => l10n::tf(lang, "statusSnoozed", &[("m", m.to_string())]),
                None => String::new(),
            }
        }
        reminder::Phase::Breaking { started_at, duration_secs } => {
            let remaining =
                reminder::BreakSession { started_at: *started_at, duration_secs: *duration_secs }
                    .remaining(now_epoch_secs());
            let m = ((remaining / 60.0).ceil() as i64).max(0);
            l10n::tf(lang, "statusBreaking", &[("m", m.to_string())])
        }
    }
}

// =====================================================================
// 推送与托盘刷新
// =====================================================================

fn push_state(handle: &AppHandle, snap: &Snapshot) {
    let _ = handle.emit("state-changed", snap);
}

fn push_strings(handle: &AppHandle) {
    let lang = current_lang(handle);
    let payload = serde_json::json!({
        "lang": lang.storage_key(),
        "strings": l10n::strings_map(lang),
    });
    let _ = handle.emit("strings-changed", payload);
    // 设置窗标题同步（仅「设置」/「Settings」）
    let title = l10n::tr(lang, "settingsTitle");
    if let Some(win) = handle.get_webview_window("settings") {
        let _ = win.set_title(title.as_str());
    }
}

fn push_wallpaper(handle: &AppHandle, path: &std::path::Path) {
    let _ = handle.emit(
        "wallpaper-changed",
        serde_json::json!({ "path": path.to_string_lossy() }),
    );
}

pub fn refresh_tray(handle: &AppHandle, snap: &Snapshot) {
    if let Some(tray) = handle.tray_by_id("pause-tray") {
        let view = compute_tray_view(current_lang(handle), snap);
        if let Err(err) = tray::refresh(handle, &tray, &view) {
            debug_log::debug_log(&format!("tray refresh failed: {err}"));
        }
    }
}

fn initial_tray_refresh(handle: &AppHandle) {
    let snap = { handle.state::<AppState>().service.lock().unwrap().snapshot() };
    orchestrate(handle, &snap);
    refresh_tray(handle, &snap);
    push_state(handle, &snap);
}

fn start_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let _icon = tray::build(app)?;
    initial_tray_refresh(app);
    Ok(())
}

// =====================================================================
// 用户动作统一入口（菜单 + 前端命令共用）
// =====================================================================

/// 用户动作入口：迁移至主线程执行（原版 @MainActor 语义），
/// 与心跳、会话事件天然互斥，无需额外锁序设计。
fn perform_action<F: FnOnce(&mut ReminderService) + Send + 'static>(
    handle: &AppHandle,
    action: F,
) {
    let h = handle.clone();
    let _ = handle.run_on_main_thread(move || {
        let snapshot = {
            let state = h.state::<AppState>();
            let mut svc = state.service.lock().unwrap();
            action(&mut svc);
            svc.snapshot_after_action()
        };
        orchestrate(&h, &snapshot);
        refresh_tray(&h, &snapshot);
        push_state(&h, &snapshot);
    });
}

/// tray.rs 回调的菜单分发。
pub fn lib_handle_menu(app: &AppHandle, id: &str) {
    match id {
        "break-now" => perform_action(app, |svc| svc.start_break()),
        "pause-toggle" => {
            let is_paused = matches!(
                app.state::<AppState>().service.lock().unwrap().phase(),
                reminder::Phase::Paused
            );
            if is_paused {
                perform_action(app, |svc| svc.resume());
            } else {
                perform_action(app, |svc| svc.pause());
            }
        }
        "open-settings" => windows::open_settings(app),
        "quit" => app.exit(0),
        _ => {}
    }
}

// =====================================================================
// Tauri commands（前端调用）
// =====================================================================

#[tauri::command]
fn get_snapshot(app: AppHandle) -> Snapshot {
    // 该命令由前端挂载时调用一次 —— 作为「webview 加载成功」探针
    debug_log::debug_log("frontend probe: reminder window mounted");
    app.state::<AppState>().service.lock().unwrap().snapshot()
}

#[tauri::command]
fn get_strings(app: AppHandle) -> serde_json::Value {
    let lang = current_lang(&app);
    debug_log::debug_log(&format!(
        "strings fetched: window-locale={}",
        lang.storage_key()
    ));
    serde_json::json!({
        "lang": lang.storage_key(),
        "strings": l10n::strings_map(lang),
    })
}

#[tauri::command]
fn get_settings(app: AppHandle) -> settings::Settings {
    debug_log::debug_log("frontend probe: settings window mounted");
    current_settings(&app)
}

#[tauri::command]
fn set_language(app: AppHandle, lang: String) -> bool {
    let key = Lang::parse(&lang).storage_key().to_string();
    let changed = {
        let state = app.state::<AppState>();
        let mut s = state.settings.lock().unwrap();
        let differs = s.app_language != key;
        if differs {
            s.app_language = key.clone();
        }
        drop(s);
        if let Err(err) = state.settings_file.save(&state.settings.lock().unwrap().clone()) {
            debug_log::debug_log(&format!("save language failed: {err}"));
        }
        differs
    };
    if changed {
        push_strings(&app);
        let snapshot = app.state::<AppState>().service.lock().unwrap().snapshot();
        refresh_tray(&app, &snapshot);
    }
    true
}

/// 写设置统一路径：加锁改值 → **释放锁** → 落盘 → 副作用。
/// 原实现在持锁语句块内二次 lock 同一把 Mutex（std 互斥锁不可重入），
/// 同线程自死锁并冻结主线程——设置窗改任何项都会卡死，此处为根因修复。
fn persist_and_effect(app: &AppHandle, key: &str, value: serde_json::Value) -> SetResult {
    let (result, snapshot) = {
        let state = app.state::<AppState>();
        let mut s = state.settings.lock().unwrap();
        (settings::apply_setting(&mut s, key, &value), s.clone())
    };
    let state = app.state::<AppState>();
    if let Err(err) = state.settings_file.save(&snapshot) {
        debug_log::debug_log(&format!("save setting failed: {err}"));
    }
    drop(state);
    apply_setting_side_effects(app, key, &value);
    result
}

#[tauri::command]
fn set_setting(app: AppHandle, key: String, value: serde_json::Value) -> SetResult {
    persist_and_effect(&app, &key, value)
}

fn apply_setting_side_effects(app: &AppHandle, key: &str, value: &serde_json::Value) {
    let state = app.state::<AppState>();
    let settings_now = current_settings(app);
    match key {
        // 从修改时刻重新开始本工作周期（300ms debounce 防连点抖动）
        "reminderIntervalMinutes" => spawn_restart_debounce(app),
        "reminderWindowOpacity" => {
            windows::apply_opacity_live(app, settings_now.reminder_window_opacity)
        }
        "overlayOtherWindows" => {
            if let Some(win) = app.get_webview_window("reminder") {
                let _ = win.set_always_on_top(settings_now.overlay_other_windows);
            }
        }
        "launchAtLogin" => sync_launch_at_login(app, settings_now.launch_at_login),
        "wallpaperImageURLString" => {
            // 地址变更：作废旧预取并按新地址重新预取
            state.wallpapers.advance();
        }
        "soundEnabled" | "autoStartBreak" | "autoStartBreakDelaySeconds" | _ => {}
    }
    let _ = value;
}

fn spawn_restart_debounce(app: &AppHandle) {
    let state = app.state::<AppState>();
    let mut guard = state.restart_debounce.lock().unwrap();
    if let Some(old) = guard.take() {
        old.abort();
    }
    let new_handle = {
        let h = app.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            // 相位修改必须与其他状态机工作串行 → 主线程执行
            let h2 = h.clone();
            let _ = h.run_on_main_thread(move || {
                h2.state::<AppState>().service.lock().unwrap().on_interval_changed();
            });
        })
    };
    *guard = Some(new_handle);
}

/// 开机启动同步：期望值与实际状态不一致才切换；失败时回滚内存设置供 UI 反映。
fn sync_launch_at_login(app: &AppHandle, desired: bool) {
    use tauri_plugin_autostart::ManagerExt as _;
    // 插件 2.5.x 的方法名为 autolaunch（v3 计划更名为 autostart）
    let manager = app.autolaunch();
    let enabled = manager.is_enabled().unwrap_or(false);
    if enabled == desired {
        return;
    }
    let result = if desired {
        manager.enable()
    } else {
        manager.disable()
    };

    if result.is_err() {
        debug_log::debug_log("launch-at-login toggle failed; rolling back");
        let state = app.state::<AppState>();
        let mut s = state.settings.lock().unwrap();
        s.launch_at_login = !desired;
        drop(s);
        let _ = state.settings_file.save(&state.settings.lock().unwrap().clone());
    }
}

/// 「切换图片」：每次强制从网络获取一张新随机图；网络失败才降级本地 advance。
/// 返回最终图片路径，供设置窗即时刷新预览（reminder 窗走事件）。
#[tauri::command]
async fn switch_wallpaper(app: AppHandle) -> String {
    let wallpapers = app.state::<AppState>().wallpapers.clone();
    let path = match wallpapers.fetch_fresh().await {
        Some(p) => p,
        None => wallpapers.advance(),
    };
    push_wallpaper(&app, &path);
    path.to_string_lossy().into_owned()
}

#[tauri::command]
fn get_current_wallpaper(app: AppHandle) -> String {
    app.state::<AppState>()
        .wallpapers
        .current_path()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[tauri::command]
fn act_start_break(app: AppHandle) {
    perform_action(&app, |svc| svc.start_break());
}
#[tauri::command]
fn act_snooze(app: AppHandle) {
    perform_action(&app, |svc| svc.snooze());
}
#[tauri::command]
fn act_skip_break(app: AppHandle) {
    perform_action(&app, |svc| svc.skip_break());
}
#[tauri::command]
fn act_pause(app: AppHandle) {
    perform_action(&app, |svc| svc.pause());
}
#[tauri::command]
fn act_resume(app: AppHandle) {
    perform_action(&app, |svc| svc.resume());
}

#[tauri::command]
fn open_settings_window(app: AppHandle) {
    windows::open_settings(&app);
}

#[tauri::command]
fn quit_app(app: AppHandle) {
    app.exit(0);
}

#[tauri::command]
fn app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

// =====================================================================
// 检查更新（双通道：应用内 updater 下载 / 跳转 GitHub Release 页）
// =====================================================================

const GITHUB_LATEST_API: &str = "https://api.github.com/repos/Vindac/pause/releases/latest";
const GITHUB_RELEASE_PAGE: &str = "https://github.com/Vindac/pause/releases/latest";

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct GithubReleaseInfo {
    /// Release tag（如 "v2.0.1"）。
    tag: String,
    /// Release 页面地址。
    url: String,
    /// 更新说明正文。
    notes: String,
}

/// 「v2.0.1」/「2.0.1」→ (2, 0, 1)；解析失败返回 None。
fn parse_semver(s: &str) -> Option<(u64, u64, u64)> {
    let t = s.trim().trim_start_matches(['v', 'V']);
    let mut it = t.split('.');
    let a = it.next()?.parse().ok()?;
    let b = it.next()?.parse().ok()?;
    let c = it.next().unwrap_or("0").parse().ok()?;
    Some((a, b, c))
}

/// 严格更新判定：a > b 才算有更新；任一版本非法视为无更新。
fn semver_newer(a: &str, b: &str) -> bool {
    match (parse_semver(a), parse_semver(b)) {
        (Some(x), Some(y)) => x > y,
        _ => false,
    }
}

async fn fetch_github_latest() -> Option<GithubReleaseInfo> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .ok()?;
    let resp = client
        .get(GITHUB_LATEST_API)
        .header("User-Agent", "pause-updater") // GitHub API 强制要求 UA
        .send()
        .await
        .ok()?;
    if resp.status().as_u16() != 200 {
        return None; // 无 Release / 限流 / 网络失败
    }
    let body = resp.text().await.ok()?;
    let v: serde_json::Value = serde_json::from_str(&body).ok()?;
    let tag = v.get("tag_name")?.as_str()?.to_string();
    Some(GithubReleaseInfo {
        url: v
            .get("html_url")
            .and_then(|x| x.as_str())
            .unwrap_or(GITHUB_RELEASE_PAGE)
            .to_string(),
        notes: v
            .get("body")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        tag,
    })
}

/// 双通道探测结果：前端弹窗据此渲染两个按钮的可用性。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    /// 更新版本号（无 v 前缀）。
    pub version: String,
    /// GitHub Release tag。
    pub tag: String,
    /// Release 页面地址（「前往 GitHub 下载」按钮目标）。
    pub url: String,
    /// 更新说明。
    pub notes: String,
    /// 应用内 updater 通道是否可用（latest.json 存在且版本更新）。
    pub in_app_available: bool,
}

/// 双通道探测：GitHub API 判断是否有新版本；updater 插件判断能否应用内安装。
/// 任一通道发现新版本即返回 Some。
async fn collect_update_info(handle: &AppHandle) -> Option<UpdateInfo> {
    use tauri_plugin_updater::UpdaterExt;

    let current = env!("CARGO_PKG_VERSION");
    let gh = fetch_github_latest().await;
    let gh_newer = gh
        .as_ref()
        .map(|g| semver_newer(&g.tag, current))
        .unwrap_or(false);

    // updater 通道：latest.json 只会由带签名的 Release 生成，
    // 旧版本 Release 无此文件 → check 失败，仅剩 GitHub 跳转通道。
    let mut updater_version: Option<String> = None;
    if let Ok(updater) = handle.updater_builder().build() {
        if let Ok(Some(update)) = updater.check().await {
            updater_version = Some(update.version.clone());
        }
    }
    let in_app_available = updater_version
        .as_deref()
        .map(|v| semver_newer(v, current))
        .unwrap_or(false);

    if !gh_newer && !in_app_available {
        return None;
    }
    let (tag, url, notes) = match gh {
        Some(g) => (g.tag, g.url, g.notes),
        None => (
            format!("v{}", updater_version.unwrap_or_default()),
            GITHUB_RELEASE_PAGE.to_string(),
            String::new(),
        ),
    };
    Some(UpdateInfo {
        version: tag.trim_start_matches(['v', 'V']).to_string(),
        tag,
        url,
        notes,
        in_app_available,
    })
}

/// 手动「检查更新」：无更新返回 null，有更新返回双通道信息。
#[tauri::command]
async fn check_update(app: AppHandle) -> Option<UpdateInfo> {
    collect_update_info(&app).await
}

/// 方案 B 独立通道：GitHub 最新 Release 原始信息（tag / 页面 / 说明）。
#[tauri::command]
async fn github_latest_release() -> Option<GithubReleaseInfo> {
    fetch_github_latest().await
}

/// 「暂不提醒（本版本）」：仅抑制启动自检弹窗，手动检查不受影响。
#[tauri::command]
fn skip_update_version(app: AppHandle, version: String) {
    let state = app.state::<AppState>();
    {
        let mut s = state.settings.lock().unwrap();
        s.skipped_update_version = version;
    }
    let _ = state
        .settings_file
        .save(&state.settings.lock().unwrap().clone());
}

/// 启动 8 秒后静默检查一次：发现新版本且未被跳过 → 弹开设置窗并广播事件，
/// 前端渲染双通道更新弹窗（立即更新 / 前往 GitHub 下载）。
fn spawn_startup_update_check(handle: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(8)).await;
        let Some(info) = collect_update_info(&handle).await else {
            return;
        };
        let skipped = {
            let state = handle.state::<AppState>();
            let v = state.settings.lock().unwrap().skipped_update_version.clone();
            v
        };
        if skipped == info.version || skipped == info.tag {
            return;
        }
        debug_log::debug_log(&format!(
            "update available: {} (in-app: {})",
            info.tag, info.in_app_available
        ));
        let _ = handle.emit("update-available", &info);
        windows::open_settings(&handle);
    });
}

#[cfg(test)]
mod update_tests {
    use super::*;

    #[test]
    fn test_semver_compare() {
        assert!(semver_newer("v2.0.1", "2.0.0"));
        assert!(semver_newer("2.1.0", "2.0.9"));
        assert!(semver_newer("v3.0.0", "2.99.99"));
        assert!(!semver_newer("v2.0.0", "2.0.0"));
        assert!(!semver_newer("v1.9.9", "2.0.0"));
        // 非法输入一律视为无更新
        assert!(!semver_newer("", "2.0.0"));
        assert!(!semver_newer("latest", "2.0.0"));
        assert_eq!(parse_semver("v2.1"), Some((2, 1, 0)));
        assert_eq!(parse_semver("abc"), None);
    }
}

// =====================================================================
// 演示模式（PAUSE_DEMO=1 / PAUSE_DEMO_SETTINGS=1）
// =====================================================================

fn run_demo_mode(app: &AppHandle) {
    let demo = std::env::var("PAUSE_DEMO").ok().as_deref() == Some("1");
    let demo_settings =
        std::env::var("PAUSE_DEMO_SETTINGS").ok().as_deref() == Some("1");
    if !demo && !demo_settings {
        return;
    }
    // 演示序列用专用线程驱动（不依赖 tokio 时间驱动的启动时序）
    let handle = app.clone();
    debug_log::debug_log(&format!(
        "demo mode active: demo={demo} demo_settings={demo_settings}"
    ));
    std::thread::Builder::new()
        .name("pause-demo".into())
        .spawn(move || {
            use std::time::Duration;
            // 给主事件循环留出完成启动（托盘/双窗/资产放行）的时间
            std::thread::sleep(Duration::from_millis(1800));
            if demo {
                debug_log::debug_log("demo: triggering reminder");
                perform_action(&handle, |svc| svc.demo_trigger());
                std::thread::sleep(Duration::from_millis(4500));
                debug_log::debug_log("demo: starting break");
                perform_action(&handle, |svc| svc.start_break());
                std::thread::sleep(Duration::from_millis(3500));
                debug_log::debug_log("demo: exiting");
                handle.exit(0);
            } else {
                std::thread::sleep(Duration::from_millis(800));
                windows::open_settings(&handle);
                // 回归验证：连续修改设置项（曾因同线程双锁死锁）
                debug_log::debug_log("demo settings: set delay=60");
                let _ = persist_and_effect(
                    &handle,
                    "autoStartBreakDelaySeconds",
                    serde_json::json!(60),
                );
                debug_log::debug_log("demo settings: set delay=30 (no deadlock)");
                let _ = persist_and_effect(
                    &handle,
                    "autoStartBreakDelaySeconds",
                    serde_json::json!(30),
                );
                // 回归验证：关闭 → 再次打开（曾因关闭即销毁导致重开失败）
                std::thread::sleep(Duration::from_millis(600));
                if let Some(w) = handle.get_webview_window("settings") {
                    let _ = w.close();
                }
                debug_log::debug_log("demo settings: close requested");
                std::thread::sleep(Duration::from_millis(600));
                windows::open_settings(&handle);
                debug_log::debug_log("demo settings: reopened (window survived)");
                std::thread::sleep(Duration::from_millis(2000));
                debug_log::debug_log("demo settings: exiting");
                handle.exit(0);
            }
        })
        .expect("spawn demo thread");
}

// =====================================================================
